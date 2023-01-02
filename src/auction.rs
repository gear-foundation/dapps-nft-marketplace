
use crate::{
    nft_messages::{get_owner, nft_transfer},
    payment::transfer_tokens,
    sale::*,
    Market, MarketEvent, BASE_PERCENT,
    payouts,
};
use gstd::{exec, msg, prelude::*, ActorId};
use market_io::*;
const MIN_BID_PERIOD: u64 = 60_000;

impl Market {
    pub async fn create_auction(
        &mut self,
        nft_contract_id: &ContractId,
        ft_contract_id: Option<ContractId>,
        token_id: TokenId,
        min_price: Price,
        bid_period: u64,
        duration: u64,
    ) {
        self.check_approved_nft_contract(nft_contract_id);
        self.check_approved_ft_contract(ft_contract_id);
        let contract_and_token_id = (*nft_contract_id, token_id);
        let item = self
            .items
            .get_mut(&contract_and_token_id)
            .expect("Item does not exist");
        assert_eq!(
            item.owner,
            msg::source(),
            "Only owner has a right to add NFT to the marketplace and start the auction"
        );
        assert!(item.auction.is_none(), "There is already an auction");
        assert!(
            item.price.is_none(),
            "Remove the item from the sale before starting the auction"
        );
        assert!(
            bid_period >= MIN_BID_PERIOD && duration >= MIN_BID_PERIOD,
            "bid period and auction duration can't be less than 1 minute"
        );
        assert!(min_price > 0, "minimum price can't be equal to zero");

        let transaction_id = match &item.transaction_id {
            Some(transaction_id) => *transaction_id,
            None => {
                let transaction_id = self.transaction_id;
                self.transaction_id = self.transaction_id.wrapping_add(1);
                item.transaction_id = Some(transaction_id);
                transaction_id
            }
        };

        if nft_transfer(
            transaction_id,
            &nft_contract_id,
            &exec::program_id(),
            token_id,
        )
        .await
        .is_err()
        {
            item.transaction_id = None;
            reply_transaction_failed();
            return;
        }

        item.transaction_id = None;
        item.ft_contract_id = ft_contract_id;
        item.auction = Some(Auction {
            bid_period,
            started_at: exec::block_timestamp(),
            ended_at: exec::block_timestamp() + duration,
            current_price: min_price,
            current_winner: ActorId::zero(),
            transaction: None,
        });
        msg::reply(
            MarketEvent::AuctionCreated {
                nft_contract_id: *nft_contract_id,
                token_id,
                price: min_price,
            },
            0,
        )
        .expect("Error in reply [MarketEvent::AuctionCreated]");
    }

    /// Settles the auction.
    ///
    /// Requirements:
    /// * The auction must be over.
    ///
    /// Arguments:
    /// * `nft_contract_id`: the NFT contract address
    /// * `token_id`: the NFT id
    ///
    /// On success auction replies [`MarketEvent::AuctionSettled`].
    /// If no bids were made replies [`MarketEvent::AuctionCancelled`].

    pub async fn settle_auction(&mut self, nft_contract_id: &ContractId, token_id: TokenId) {
        let contract_and_token_id = (*nft_contract_id, token_id);

        let item = self
            .items
            .get_mut(&contract_and_token_id)
            .expect("Item does not exist");

        let auction: &mut Auction = item.auction.as_mut().expect("Auction doesn not exist");

        if auction.ended_at > exec::block_timestamp() {
            panic!("Auction is not over");
        }

        let (winner, price) = if let Some((account, price, transaction_id)) = auction.transaction {
            // if the transaction of the last bid was not completed
            if transfer_tokens(
                transaction_id,
                &item.ft_contract_id.expect("Ft contract id can't be None"),
                &account,
                &exec::program_id(),
                price,
            )
            .await
            .is_ok()
            {
                auction.current_price = price;
                auction.current_winner = account;
            }
            auction.transaction = None;
            (auction.current_winner, auction.current_price)
        } else {
            (auction.current_winner, auction.current_price)
        };

        if winner == ActorId::zero() {
            msg::reply(
                MarketEvent::AuctionCancelled {
                    nft_contract_id: *nft_contract_id,
                    token_id,
                },
                0,
            )
            .expect("Error in reply [MarketEvent::AuctionCancelled]");

            return;
        }

        // calculate fee for treasury
        let treasury_fee = price * (self.treasury_fee * BASE_PERCENT) as u128 / 10_000u128;

        // payouts for NFT sale (includes royalty accounts and seller)
        let mut payouts = payouts(nft_contract_id, &item.owner, price - treasury_fee).await;
        payouts.insert(self.treasury_id, treasury_fee);

        if let Some(ft_contract_id) = item.ft_contract_id {
            sale_with_tokens(
                &mut self.transaction_id,
                nft_contract_id,
                &ft_contract_id,
                &winner,
                &exec::program_id(),
                token_id,
                item,
                price,
                &payouts,
            )
            .await;
        } else {
            sale_with_value(
                &mut self.transaction_id,
                nft_contract_id,
                &winner,
                token_id,
                item,
                price,
                &payouts,
            )
            .await
        }

        item.auction = None;
        // msg::reply(
        //     MarketEvent::AuctionSettled {
        //         nft_contract_id: *nft_contract_id,
        //         token_id,
        //         price,
        //     },
        //     0,
        // )
        // .expect("Error in reply [MarketEvent::AuctionSettled]");
    }

    pub async fn add_bid(&mut self, nft_contract_id: &ContractId, token_id: TokenId, price: Price) {
        let contract_and_token_id = (*nft_contract_id, token_id);

        let item = self
            .items
            .get_mut(&contract_and_token_id)
            .expect("Item does not exist");

        let auction: &mut Auction = item.auction.as_mut().expect("Auction doesn not exist");
        if auction.ended_at < exec::block_timestamp() {
            panic!("Auction has already ended");
        }

        if let Some(ft_contract_id) = item.ft_contract_id {
            bid_with_token(&mut self.transaction_id, auction, &ft_contract_id, price).await;
        } else {
            assert!(
                price > auction.current_price,
                "Cant offer less or equal to the current bid price"
            );
            assert!(msg::value() == price, "Not enough attached value");
        }

        auction.current_price = price;
        auction.current_winner = msg::source();

        msg::reply(
            MarketEvent::BidAdded {
                nft_contract_id: *nft_contract_id,
                token_id,
                price,
            },
            0,
        )
        .expect("Error in reply [MarketEvent::BidAdded]");
    }
}
async fn bid_with_token(
    transaction_id: &mut TransactionId,
    auction: &mut Auction,
    ft_contract_id: &ActorId,
    price: Price,
) {
    // if the previous transaction was not completed
    if let Some((account, price, transaction_id)) = auction.transaction {
        if transfer_tokens(
            transaction_id,
            &ft_contract_id,
            &account,
            &exec::program_id(),
            price,
        )
        .await
        .is_ok()
        {
            auction.current_price = price;
            auction.current_winner = account;
        }
        auction.transaction = None;
    }

    assert!(
        price > auction.current_price,
        "Cant offer less or equal to the current bid price"
    );

    if auction.ended_at <= exec::block_timestamp() + auction.bid_period {
        auction.ended_at = exec::block_timestamp() + auction.bid_period;
    }

    let current_transaction_id = *transaction_id;
    *transaction_id = transaction_id.wrapping_add(1);
    auction.transaction = Some((msg::source(), price, current_transaction_id));

    if transfer_tokens(
        current_transaction_id,
        &ft_contract_id,
        &msg::source(),
        &exec::program_id(),
        price,
    )
    .await
    .is_err()
    {
        auction.transaction = None;
        panic!("Error during transferring fungible tokens");
    }
}

async fn settle_with_token(
    transaction_id: &mut TransactionId,
    auction: &mut Auction,
    ft_contract_id: &ActorId,
    price: Price,
) {
    // if the transaction of the last bid was not completed
    if let Some((account, price, transaction_id)) = auction.transaction {
        if transfer_tokens(
            transaction_id,
            &ft_contract_id,
            &account,
            &exec::program_id(),
            price,
        )
        .await
        .is_ok()
        {
            auction.current_price = price;
            auction.current_winner = account;
        }
    }

    let current_transaction_id = *transaction_id;
    *transaction_id = transaction_id.wrapping_add(1);
    auction.transaction = Some((msg::source(), price, current_transaction_id));

    if transfer_tokens(
        current_transaction_id,
        &ft_contract_id,
        &msg::source(),
        &exec::program_id(),
        price,
    )
    .await
    .is_err()
    {
        auction.transaction = None;
        panic!("Error during transferring fungible tokens");
    }
}
