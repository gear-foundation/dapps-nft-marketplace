#![no_std]

use gmeta::{metawasm, Metadata};
use gstd::{prelude::*, ActorId};
use market_io::*;

#[metawasm]
pub trait Metawasm {
    type State = <AppMetadata as Metadata>::State;

    fn all_items(account: ActorId, state: Self::State) -> Vec<Item> {
        market_io::all_items(state)
    }

    fn item_info(nft_contract_id: &ActorId, token_id: U256, state: Self::State) -> Item {
        market_io::item_info(state, nft_contract_id, token_id).expect("Item not found")
    }
}
