#![no_std]

use gstd::{errors::Result as GstdResult, msg, prelude::*, ActorId, MessageId};
pub use market_io::*;
pub mod nft_messages;
use nft_messages::*;
pub mod auction;
pub mod offers;
pub mod payment;
pub mod sale;
pub mod state;
use state::*;

pub type ContractAndTokenId = String;

const MIN_TREASURY_FEE: u16 = 0;
const MAX_TREASURT_FEE: u16 = 5;
pub const BASE_PERCENT: u16 = 100;
pub const MINIMUM_VALUE: u64 = 500;

#[derive(Debug, Default, Encode, Decode, TypeInfo)]
#[codec(crate = gstd::codec)]
#[scale_info(crate = gstd::scale_info)]
pub struct Market {
    pub admin_id: ActorId,
    pub treasury_id: ActorId,
    pub treasury_fee: u16,
    pub items: BTreeMap<(ContractId, TokenId), Item>,
    pub approved_nft_contracts: BTreeSet<ActorId>,
    pub approved_ft_contracts: BTreeSet<ActorId>,
    pub tx_id: TransactionId,
}

static mut MARKET: Option<Market> = None;

impl Market {
    fn add_nft_contract(&mut self, nft_contract_id: &ContractId) -> Result<MarketEvent, MarketErr> {
        self.check_admin();
        self.approved_nft_contracts.insert(*nft_contract_id);
        Ok(MarketEvent::NftContractAdded(*nft_contract_id))
    }

    fn add_ft_contract(&mut self, ft_contract_id: &ContractId) -> Result<MarketEvent, MarketErr> {
        self.check_admin();
        self.approved_ft_contracts.insert(*ft_contract_id);
        Ok(MarketEvent::FtContractAdded(*ft_contract_id))
    }

    pub async fn add_market_data(
        &mut self,
        nft_contract_id: &ContractId,
        ft_contract_id: Option<ContractId>,
        token_id: TokenId,
        price: Option<Price>,
    ) -> Result<MarketEvent, MarketErr> {
        self.check_approved_nft_contract(nft_contract_id);
        self.check_approved_ft_contract(ft_contract_id);
        let contract_and_token_id = (*nft_contract_id, token_id);

        let owner = get_owner(nft_contract_id, token_id).await;
        assert_eq!(
            owner,
            msg::source(),
            "Only owner has a right to add NFT to the marketplace"
        );
        self.items
            .entry(contract_and_token_id)
            .and_modify(|item| {
                item.price = price;
                item.ft_contract_id = ft_contract_id
            })
            .or_insert(Item {
                owner,
                ft_contract_id,
                price,
                auction: None,
                offers: BTreeMap::new(),
                tx: None,
            });

        Ok(MarketEvent::MarketDataAdded {
            nft_contract_id: *nft_contract_id,
            token_id,
            price,
        })
    }

    pub fn check_admin(&self) {
        if msg::source() != self.admin_id {
            panic!("Only owner can make that action");
        }
    }

    pub fn check_approved_nft_contract(&self, nft_contract_id: &ActorId) {
        if !self.approved_nft_contracts.contains(nft_contract_id) {
            panic!("that nft contract is not approved");
        }
    }

    pub fn check_approved_ft_contract(&self, ft_contract_id: Option<ActorId>) {
        if ft_contract_id.is_some()
            && !self
                .approved_ft_contracts
                .contains(&ft_contract_id.expect("Must not be an error here"))
        {
            panic!("that ft contract is not approved");
        }
    }
}

#[gstd::async_main]
async fn main() {
    let action: MarketAction = msg::load().expect("Could not load Action");
    let market: &mut Market = unsafe { MARKET.get_or_insert(Market::default()) };
    let result = match action {
        MarketAction::AddNftContract(nft_contract_id) => market.add_nft_contract(&nft_contract_id),
        MarketAction::AddFTContract(nft_contract_id) => market.add_ft_contract(&nft_contract_id),
        MarketAction::AddMarketData {
            nft_contract_id,
            ft_contract_id,
            token_id,
            price,
        } => {
            market
                .add_market_data(&nft_contract_id, ft_contract_id, token_id, price)
                .await
        }
        MarketAction::BuyItem {
            nft_contract_id,
            token_id,
        } => market.buy_item(&nft_contract_id, token_id).await,
        MarketAction::AddOffer {
            nft_contract_id,
            ft_contract_id,
            token_id,
            price,
        } => {
            market
                .add_offer(&nft_contract_id, ft_contract_id, token_id, price)
                .await
        }
        MarketAction::AcceptOffer {
            nft_contract_id,
            token_id,
            ft_contract_id,
            price,
        } => {
            market
                .accept_offer(&nft_contract_id, token_id, ft_contract_id, price)
                .await
        }
        MarketAction::Withdraw {
            nft_contract_id,
            ft_contract_id,
            token_id,
            price,
        } => {
            market
                .withdraw(&nft_contract_id, token_id, ft_contract_id, price)
                .await
        }
        MarketAction::CreateAuction {
            nft_contract_id,
            ft_contract_id,
            token_id,
            min_price,
            bid_period,
            duration,
        } => {
            market
                .create_auction(
                    &nft_contract_id,
                    ft_contract_id,
                    token_id,
                    min_price,
                    bid_period,
                    duration,
                )
                .await
        }
        MarketAction::AddBid {
            nft_contract_id,
            token_id,
            price,
        } => market.add_bid(&nft_contract_id, token_id, price).await,

        MarketAction::SettleAuction {
            nft_contract_id,
            token_id,
        } => market.settle_auction(&nft_contract_id, token_id).await,
    };
    reply(result)
        .expect("Failed to encode or reply with `Result<MarketEvent, MarketErr>`");
}

#[no_mangle]
extern "C" fn init() {
    let config: InitMarket = msg::load().expect("Unable to decode InitConfig");
    if config.treasury_fee == MIN_TREASURY_FEE || config.treasury_fee > MAX_TREASURT_FEE {
        panic!("Wrong treasury fee");
    }
    let market = Market {
        admin_id: config.admin_id,
        treasury_id: config.treasury_id,
        treasury_fee: config.treasury_fee,
        ..Default::default()
    };
    unsafe { MARKET = Some(market) };
}

gstd::metadata! {
title: "NFTMarketplace",
    init:
        input: InitMarket,
    handle:
        input: MarketAction,
        output: MarketEvent,
    state:
        input: State,
        output: StateReply,
}

#[no_mangle]
extern "C" fn meta_state() -> *mut [i32; 2] {
    let state: State = msg::load().expect("failed to decode input argument");
    let market: &mut Market = unsafe { MARKET.get_or_insert(Market::default()) };
    let encoded = match state {
        State::AllItems => StateReply::AllItems(market.items.values().cloned().collect()).encode(),
        State::ItemInfo {
            nft_contract_id,
            token_id,
        } => {
            if let Some(item) = market.items.get(&(nft_contract_id, token_id)) {
                StateReply::ItemInfo(item.clone()).encode()
            } else {
                StateReply::ItemInfo(Item::default()).encode()
            }
        }
    };
    gstd::util::to_leak_ptr(encoded)
}

fn reply(payload: impl Encode) -> GstdResult<MessageId> {
    msg::reply(payload, 0)
}
