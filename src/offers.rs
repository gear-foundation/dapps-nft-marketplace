use crate::{
    nft_messages::*, payment::*, ContractId, Item, Market, MarketErr, MarketEvent,
    MarketTx, Price, TokenId, TransactionId, BASE_PERCENT, MINIMUM_VALUE,
};
use gstd::{exec, msg, prelude::*, ActorId};

impl Market {
    pub async fn add_offer(
        &mut self,
        nft_contract_id: &ContractId,
        ft_contract_id: Option<ContractId>,
        token_id: TokenId,
        price: Price,
    ) -> Result<MarketEvent, MarketErr> {
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

        if item.offers.contains_key(&(ft_contract_id, price)) {
            panic!("the offer with these params already exists");
        };

        let ft_id = if let Some(ft_id) = ft_contract_id {
            ft_id
        } else {
            return Ok(MarketEvent::OfferAdded {
                nft_contract_id: *nft_contract_id,
                ft_contract_id,
                token_id,
                price,
            });
        };

        if let Some((tx_id, tx)) = item.tx.clone() {
            match tx {
                MarketTx::Offer {
                    ft_id,
                    price,
                    account,
                } => {
                    let new_price = price;
                    let new_ft_id = ft_id;
                    let result =
                        add_offer_tx(tx_id, item, nft_contract_id, &ft_id, token_id, price).await;
                    if account == msg::source() && new_price == price && new_ft_id == ft_id {
                        return result;
                    }
                }
                _ => {
                    return Err(MarketErr::WrongTransaction);
                }
            }
        }

        let tx_id = self.tx_id;
        self.tx_id = self.tx_id.wrapping_add(1);
        item.tx = Some((
            tx_id,
            MarketTx::Offer {
                ft_id,
                price,
                account: msg::source(),
            },
        ));

        add_offer_tx(tx_id, item, nft_contract_id, &ft_id, token_id, price).await
    }

    pub async fn accept_offer(
        &mut self,
        nft_contract_id: &ContractId,
        token_id: TokenId,
        ft_contract_id: Option<ContractId>,
        price: Price,
    ) -> Result<MarketEvent, MarketErr> {
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

        if let Some((tx_id, tx)) = item.tx.clone() {
            match tx {
                MarketTx::AcceptOffer => {
                    return accept_offer_tx(
                        tx_id,
                        item,
                        nft_contract_id,
                        ft_contract_id,
                        account,
                        token_id,
                        price,
                        &payouts,
                    )
                    .await;
                }
                _ => {
                    return Err(MarketErr::WrongTransaction);
                }
            }
        }

        let tx_id = self.tx_id;
        self.tx_id = self.tx_id.wrapping_add(1);
        item.tx = Some((tx_id, MarketTx::AcceptOffer));

        accept_offer_tx(
            tx_id,
            item,
            nft_contract_id,
            ft_contract_id,
            account,
            token_id,
            price,
            &payouts,
        )
        .await
    }

    pub async fn withdraw(
        &mut self,
        nft_contract_id: &ContractId,
        token_id: TokenId,
        ft_contract_id: Option<ContractId>,
        price: Price,
    ) -> Result<MarketEvent, MarketErr> {
        let contract_and_token_id = (*nft_contract_id, token_id);

        let item = self
            .items
            .get_mut(&contract_and_token_id)
            .expect("Item does not exist");

        let account = if let Some(account) = item.offers.get(&(ft_contract_id, price)) {
            *account
        } else {
            panic!("Offer does not exsit");
        };

        if account != msg::source() {
            panic!("Can not withdraw others user's value");
        }

        let ft_id = if let Some(ft_id) = ft_contract_id {
            ft_id
        } else {
            msg::send(account, MarketEvent::TransferValue, price).expect("Error in sending value");
            return Ok(MarketEvent::Withdraw {
                nft_contract_id: *nft_contract_id,
                token_id,
                price,
            });
        };

        if let Some((tx_id, tx)) = item.tx.clone() {
            match tx {
                MarketTx::Withdraw {
                    ft_id,
                    price,
                    account,
                } => {
                    let new_price = price;
                    let new_ft_id = ft_id;
                    let result = withdraw_tx(
                        tx_id,
                        item,
                        nft_contract_id,
                        &ft_id,
                        token_id,
                        &account,
                        price,
                    )
                    .await;
                    if account == msg::source() && new_price == price && new_ft_id == ft_id {
                        return result;
                    }
                }
                _ => {
                    return Err(MarketErr::WrongTransaction);
                }
            }
        }

        let tx_id = self.tx_id;
        self.tx_id = self.tx_id.wrapping_add(1);
        item.tx = Some((
            tx_id,
            MarketTx::Withdraw {
                ft_id,
                price,
                account: msg::source(),
            },
        ));
        withdraw_tx(
            tx_id,
            item,
            nft_contract_id,
            &ft_id,
            token_id,
            &account,
            price,
        )
        .await
    }
}

async fn add_offer_tx(
    tx_id: TransactionId,
    item: &mut Item,
    nft_contract_id: &ContractId,
    ft_contract_id: &ContractId,
    token_id: TokenId,
    price: Price,
) -> Result<MarketEvent, MarketErr> {
    let ft_id = Some(*ft_contract_id);
    if transfer_tokens(
        tx_id,
        ft_contract_id,
        &msg::source(),
        &exec::program_id(),
        price,
    )
    .await
    .is_err()
    {
        item.tx = None;
        return Err(MarketErr::TokenTransferFailed);
    }
    item.tx = None;
    item.offers.insert((ft_id, price), msg::source());
    Ok(MarketEvent::OfferAdded {
        nft_contract_id: *nft_contract_id,
        ft_contract_id: ft_id,
        token_id,
        price,
    })
}

#[allow(clippy::too_many_arguments)] 
async fn accept_offer_tx(
    mut tx_id: TransactionId,
    item: &mut Item,
    nft_contract_id: &ContractId,
    ft_contract_id: Option<ContractId>,
    new_owner: &ActorId,
    token_id: TokenId,
    price: Price,
    payouts: &Payout,
) -> Result<MarketEvent, MarketErr> {
    let ft_id = if let Some(ft_contract_id) = ft_contract_id {
        ft_contract_id
    } else {
        return accept_offer_tx_with_value(
            tx_id,
            item,
            nft_contract_id,
            new_owner,
            token_id,
            price,
            payouts,
        )
        .await;
    };

    // transfer NFT to the marketplace account
    if nft_transfer(tx_id, nft_contract_id, &exec::program_id(), token_id)
        .await
        .is_err()
    {
        item.tx = None;
        return Err(MarketErr::NFTTransferFailed);
    }

    // send tokens to the seller, royalties and tresuary account
    // since tokens are on the marketplace account, the error can be only due the lack of gas
    for (account, amount) in payouts.iter() {
        tx_id = tx_id.wrapping_add(1);
        if transfer_tokens(tx_id, &ft_id, &exec::program_id(), account, *amount)
            .await
            .is_err()
        {
            return Err(MarketErr::RerunTransaction);
        };
    }

    // transfer NFT to the buyer
    if nft_transfer(tx_id, nft_contract_id, new_owner, token_id)
        .await
        .is_err()
    {
        return Err(MarketErr::RerunTransaction);
    }

    item.owner = *new_owner;
    item.price = None;
    item.tx = None;
    item.offers.remove(&(ft_contract_id, price));

    Ok(MarketEvent::OfferAccepted {
        nft_contract_id: *nft_contract_id,
        token_id,
        new_owner: *new_owner,
        price,
    })
}

pub async fn accept_offer_tx_with_value(
    tx_id: TransactionId,
    item: &mut Item,
    nft_contract_id: &ContractId,
    new_owner: &ActorId,
    token_id: TokenId,
    price: Price,
    payouts: &Payout,
) -> Result<MarketEvent, MarketErr> {
    // transfer NFT to the
    if nft_transfer(tx_id, nft_contract_id, new_owner, token_id)
        .await
        .is_err()
    {
        item.tx = None;
        return Err(MarketErr::NFTTransferFailed);
    }

    // send tokens to the seller, royalties and tresuary account
    // since tokens are on the marketplace account, the error can be only due the lack of gas
    for (account, amount) in payouts.iter() {
        if account != &exec::program_id() && price > MINIMUM_VALUE.into() {
            msg::send(*account, "", *amount).expect("Error in sending value");
        }
    }

    item.owner = *new_owner;
    item.price = None;
    item.tx = None;

    item.offers.remove(&(None, price));

    Ok(MarketEvent::OfferAccepted {
        nft_contract_id: *nft_contract_id,
        token_id,
        new_owner: *new_owner,
        price,
    })
}

async fn withdraw_tx(
    tx_id: TransactionId,
    item: &mut Item,
    nft_contract_id: &ContractId,
    ft_contract_id: &ContractId,
    token_id: TokenId,
    account: &ActorId,
    price: Price,
) -> Result<MarketEvent, MarketErr> {
    if transfer_tokens(tx_id, ft_contract_id, &exec::program_id(), account, price)
        .await
        .is_err()
    {
        item.tx = None;
        return Err(MarketErr::TokenTransferFailed);
    }
    item.tx = None;
    item.offers.remove(&(Some(*ft_contract_id), price));
    Ok(MarketEvent::Withdraw {
        nft_contract_id: *nft_contract_id,
        token_id,
        price,
    })
}
