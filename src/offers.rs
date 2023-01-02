use crate::{nft_messages::*, payment::*, sale::*, Market, BASE_PERCENT, MINIMUM_VALUE};
use gstd::{msg, prelude::*, ActorId};
use market_io::*;
impl Market {
    pub fn add_offer(
        &mut self,
        nft_contract_id: &ContractId,
        ft_contract_id: Option<ContractId>,
        token_id: TokenId,
        price: Price,
    ) {
        let contract_and_token_id = (*nft_contract_id, token_id);
        self.check_approved_ft_contract(ft_contract_id);
        assert!(
            ft_contract_id.is_some() && price > 0
                || ft_contract_id.is_none() && price > MINIMUM_VALUE.into(),
            "Invalid price"
        );
        assert!(
            ft_contract_id.is_some() || ft_contract_id.is_none() && msg::value() == price,
            "Not enough attached value"
        );
        let item = self
            .items
            .get_mut(&contract_and_token_id)
            .expect("Item does not exist");
        if item.auction.is_some() {
            panic!("There is an opened auction");
        }

        if item
            .offers
            .insert((ft_contract_id, price), msg::source())
            .is_some()
        {
            panic!("the offer with these params already exists");
        };

        msg::reply(
            MarketEvent::OfferAdded {
                nft_contract_id: *nft_contract_id,
                ft_contract_id,
                token_id,
                price,
            },
            0,
        )
        .expect("Error in reply [MarketEvent::OfferAdded]");
    }

    pub async fn accept_offer(
        &mut self,
        nft_contract_id: &ContractId,
        token_id: TokenId,
        ft_contract_id: Option<ContractId>,
        price: Price,
    ) {
        let contract_and_token_id = (*nft_contract_id, token_id);

        let item = self
            .items
            .get_mut(&contract_and_token_id)
            .expect("Item does not exist");

        assert!(item.auction.is_none(), "There is an opened auction");
        assert!(item.owner == msg::source(), "Only owner can accept offer");
        assert!(
            item.price.is_none(),
            "Remove the item from the sale when accepting the offer"
        );
        let offers = item.offers.clone();

        let account = offers
            .get(&(ft_contract_id, price))
            .expect("Offer does not exist");
        // calculate fee for treasury
        let treasury_fee = price * (self.treasury_fee * BASE_PERCENT) as u128 / 10_000u128;

        // payouts for NFT sale (includes royalty accounts and seller)
        let mut payouts = payouts(nft_contract_id, &item.owner, price - treasury_fee).await;
        payouts.insert(self.treasury_id, treasury_fee);

        if let Some(ft_contract_id) = ft_contract_id {
            sale_with_tokens(
                &mut self.transaction_id,
                nft_contract_id,
                &ft_contract_id,
                account,
                account,
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
                account,
                token_id,
                item,
                price,
                &payouts,
            )
            .await;
        };
        item.offers.remove(&(ft_contract_id, price));
    }

    pub fn withdraw(&mut self, nft_contract_id: &ContractId, token_id: TokenId, price: Price) {
        let contract_and_token_id = (*nft_contract_id, token_id);

        let item = self
            .items
            .get_mut(&contract_and_token_id)
            .expect("Item does not exist");

        if let Some(&account) = item.offers.get(&(None, price)) {
            assert!(
                account == msg::source(),
                "Can not withdraw others user's value"
            );
            if msg::send(account, MarketEvent::TransferValue, price).is_err() {
                reply_rerun_transaction();
                return;
            } else {
                item.offers.remove(&(None, price));
                msg::reply(
                    MarketEvent::Withdraw {
                        nft_contract_id: *nft_contract_id,
                        token_id,
                        price,
                    },
                    0,
                )
                .expect("Error in reply [MarketEvent::Withdraw]");
            }
        } else {
            panic!("Offer does not exist");
        }
    }
}
