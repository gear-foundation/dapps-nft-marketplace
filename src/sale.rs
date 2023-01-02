use crate::{
    nft_messages::*, payment::*, ContractId, Item, Market, MarketEvent, TokenId, TransactionId,
    BASE_PERCENT, MINIMUM_VALUE,
};
use gstd::{exec, msg, prelude::*, ActorId};

impl Market {
    pub async fn buy_item(&mut self, nft_contract_id: &ContractId, token_id: TokenId) {
        let contract_and_token_id = (*nft_contract_id, token_id);

        let item = self
            .items
            .get_mut(&contract_and_token_id)
            .expect("Item does not exist");

        assert!(item.auction.is_none(), "There is an opened auction");

        let price = item.price.expect("The item is not on sale");

        // calculate fee for treasury
        let treasury_fee = price * (self.treasury_fee * BASE_PERCENT) as u128 / 10_000u128;

        let treasury_fee = 0;
        // payouts for NFT sale (includes royalty accounts and seller)
        let mut payouts = payouts(nft_contract_id, &item.owner, price - treasury_fee).await;
        payouts.insert(self.treasury_id, treasury_fee);

        if let Some(ft_contract_id) = item.ft_contract_id {
            sale_with_tokens(
                &mut self.transaction_id,
                nft_contract_id,
                &ft_contract_id,
                &msg::source(),
                &msg::source(),
                token_id,
                item,
                price,
                &payouts,
            )
            .await
        } else {
            sale_with_value(
                &mut self.transaction_id,
                nft_contract_id,
                &msg::source(),
                token_id,
                item,
                price,
                &payouts,
            )
            .await
        }
    }
}

pub async fn sale_with_tokens(
    current_transaction_id: &mut TransactionId,
    nft_contract_id: &ContractId,
    ft_contract_id: &ContractId,
    new_owner: &ActorId,
    payer: &ActorId,
    token_id: TokenId,
    item: &mut Item,
    price: u128,
    payouts: &Payout,
) {
    // get transaction id
    let mut transaction_id = match item.transaction_id {
        // The transaction has already taken place but has not completed
        Some(transaction_id) => transaction_id,
        // New transaction
        None => {
            let transaction_id = current_transaction_id.clone();
            // We account the transfer of NFT to the contract program and transfer NFT to the buyer
            // the transfer of tokens to the contract program and then sending tokens to royalties, seller and treasury accounts
            *current_transaction_id = current_transaction_id.wrapping_add(2 + payouts.len() as u64);
            item.transaction_id = Some(transaction_id);
            transaction_id
        }
    };

    // transfer NFT to the marketplace account
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

    // transfer tokens to the marketplace account
    if transfer_tokens(
        transaction_id,
        &ft_contract_id,
        payer,
        &exec::program_id(),
        price,
    )
    .await
    .is_err()
    {
        // if there is a fail during the token transfer
        // we transfer NFT back to the seller
        transaction_id = transaction_id.wrapping_add(1);
        if nft_transfer(transaction_id, nft_contract_id, &item.owner, token_id)
            .await
            .is_err()
        {
            // if it fails here we have to rerun transaction in order to return NFT to the owner
            reply_rerun_transaction();
            return;
        }
        reply_transaction_failed();
        return;
    }

    // send tokens to the seller, royalties and tresuary account
    // since tokens are on the marketplace account, the error can be only due the lack of gas
    for (account, amount) in payouts.iter() {
        transaction_id = transaction_id.wrapping_add(1);
        if transfer_tokens(
            transaction_id,
            &ft_contract_id,
            &exec::program_id(),
            account,
            *amount,
        )
        .await
        .is_err()
        {
            reply_rerun_transaction();
            return;
        };
    }

    // transfer NFT to the buyer
    if nft_transfer(transaction_id, &nft_contract_id, new_owner, token_id)
        .await
        .is_err()
    {
        reply_rerun_transaction();
        return;
    }
    item.owner = *new_owner;
    item.price = None;
    item.transaction_id = None;

    msg::reply(
        MarketEvent::ItemSold {
            owner: *new_owner,
            nft_contract_id: *nft_contract_id,
            token_id,
        },
        0,
    )
    .expect("Error in reply [MarketEvent::ItemSold]");
}

pub async fn sale_with_value(
    current_transaction_id: &mut TransactionId,
    nft_contract_id: &ContractId,
    new_owner: &ActorId,
    token_id: TokenId,
    item: &mut Item,
    price: u128,
    payouts: &Payout,
) {
    assert_eq!(msg::value(), price, "Not enough value for buying NFT");
    // get transaction id
    let mut transaction_id = match item.transaction_id {
        // The transaction has already taken place but has not completed
        Some(transaction_id) => transaction_id,
        // New transaction
        None => {
            let transaction_id = current_transaction_id.clone();
            // We account the transfer of NFT to the contract program and transfer NFT to the buyer
            *current_transaction_id = current_transaction_id.wrapping_add(2);
            item.transaction_id = Some(transaction_id);
            transaction_id
        }
    };

    // transfer NFT to the
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

    // send tokens to the seller, royalties and tresuary account
    // since tokens are on the marketplace account, the error can be only due the lack of gas
    for (account, amount) in payouts.iter() {
        if account != &exec::program_id()
            && price > MINIMUM_VALUE.into()
            && msg::send(*account, "", *amount).is_err()
        {
            reply_rerun_transaction();
            return;
        };
    }

    transaction_id = transaction_id.wrapping_add(1);
    // transfer NFT to the buyer
    if nft_transfer(transaction_id, &nft_contract_id, &msg::source(), token_id)
        .await
        .is_err()
    {
        reply_rerun_transaction();
        return;
    }
    item.owner = *new_owner;
    item.price = None;
    item.transaction_id = None;

    msg::reply(
        MarketEvent::ItemSold {
            owner: *new_owner,
            nft_contract_id: *nft_contract_id,
            token_id,
        },
        0,
    )
    .expect("Error in reply [MarketEvent::ItemSold]");
}

pub fn reply_transaction_failed() {
    msg::reply(MarketEvent::TransactionFailed, 0)
        .expect("Error in a reply `NFTEvent::TransactionFailed`");
}

pub fn reply_rerun_transaction() {
    msg::reply(MarketEvent::RerunTransaction, 0)
        .expect("Error in reply [MarketEvent::RerunTransaction]");
}
