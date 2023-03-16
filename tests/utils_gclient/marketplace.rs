use super::common;
use gclient::{EventListener, EventProcessor, GearApi};
use gstd::{prelude::*, ActorId};
use market_io::{InitMarket, Market, MarketAction, MarketErr, MarketEvent, TokenId};

const MARKETPLACE_WASM_PATH: &str =
    "./target/wasm32-unknown-unknown/debug/nft_marketplace.opt.wasm";
pub const TREASURY_FEE: u16 = 3;

pub async fn init(api: &GearApi, admin: &ActorId, treasury: &ActorId) -> gclient::Result<ActorId> {
    let mut listener = api.subscribe().await?;
    assert!(listener.blocks_running().await?);

    let init_marketplace_config = InitMarket {
        admin_id: *admin,
        treasury_id: *treasury,
        treasury_fee: TREASURY_FEE,
    }
    .encode();

    let gas_info = api
        .calculate_upload_gas(
            None,
            gclient::code_from_os(MARKETPLACE_WASM_PATH)?,
            init_marketplace_config.clone(),
            0,
            true,
        )
        .await?;

    let (message_id, program_id, _hash) = api
        .upload_program_bytes(
            gclient::code_from_os(MARKETPLACE_WASM_PATH)?,
            gclient::now_in_micros().to_le_bytes(),
            init_marketplace_config,
            gas_info.min_limit,
            0,
        )
        .await?;
    assert!(listener.message_processed(message_id).await?.succeed());

    let program_id: common::Hash = program_id
        .encode()
        .try_into()
        .expect("Unexpected invalid program id.");

    Ok(program_id.into())
}

pub async fn add_nft_contract(
    api: &GearApi,
    listener: &mut EventListener,
    program_id: &ActorId,
    nft_contract: &ActorId,
    should_fail: bool,
) -> gclient::Result<()> {
    let reply = send_message(
        api,
        listener,
        program_id,
        MarketAction::AddNftContract(*nft_contract),
        0,
    )
    .await?;

    if !should_fail {
        let MarketEvent::NftContractAdded(_) = MarketEvent::decode(&mut reply.as_ref()).expect("Unexpected invalid `MarketEvent` data.") else {
            panic!("Unexpected invalid `MarketEvent`.");
        };
    } else {
        MarketErr::decode(&mut reply.as_ref()).expect("Unexpected invalid `MarketErr` data.");
    }

    Ok(())
}

pub async fn add_ft_contract(
    api: &GearApi,
    listener: &mut EventListener,
    program_id: &ActorId,
    ft_contract: &ActorId,
    should_fail: bool,
) -> gclient::Result<()> {
    let reply = send_message(
        api,
        listener,
        program_id,
        MarketAction::AddFTContract(*ft_contract),
        0,
    )
    .await?;

    if !should_fail {
        let MarketEvent::FtContractAdded(_) = MarketEvent::decode(&mut reply.as_ref()).expect("Unexpected invalid `MarketEvent` data.") else {
            panic!("Unexpected invalid `MarketEvent`.");
        };
    } else {
        MarketErr::decode(&mut reply.as_ref()).expect("Unexpected invalid `MarketErr` data.");
    }

    Ok(())
}

pub async fn add_market_data(
    api: &GearApi,
    listener: &mut EventListener,
    program_id: &ActorId,
    nft_contract: &ActorId,
    ft_contract: Option<ActorId>,
    token_id: TokenId,
    price: Option<u128>,
    should_fail: bool,
) -> gclient::Result<()> {
    let reply = send_message(
        api,
        listener,
        program_id,
        MarketAction::AddMarketData {
            nft_contract_id: *nft_contract,
            ft_contract_id: ft_contract,
            token_id,
            price,
        },
        0,
    )
    .await?;

    if !should_fail {
        let MarketEvent::MarketDataAdded { nft_contract_id: _, token_id: _, price: _ } = MarketEvent::decode(&mut reply.as_ref()).expect("Unexpected invalid `MarketEvent` data.") else {
            panic!("Unexpected invalid `MarketEvent`.");
        };
    } else {
        MarketErr::decode(&mut reply.as_ref()).expect("Unexpected invalid `MarketErr` data.");
    }

    Ok(())
}

pub async fn buy_item(
    api: &GearApi,
    listener: &mut EventListener,
    program_id: &ActorId,
    nft_contract: &ActorId,
    token_id: TokenId,
    should_fail: bool,
) -> gclient::Result<()> {
    let reply = send_message(
        api,
        listener,
        program_id,
        MarketAction::BuyItem {
            nft_contract_id: *nft_contract,
            token_id,
        },
        0,
    )
    .await?;

    if !should_fail {
        let MarketEvent::ItemSold { owner: _, token_id: _, nft_contract_id: _ } = MarketEvent::decode(&mut reply.as_ref()).expect("Unexpected invalid `MarketEvent` data.") else {
            panic!("Unexpected invalid `MarketEvent`.");
        };
    } else {
        MarketErr::decode(&mut reply.as_ref()).expect("Unexpected invalid `MarketErr` data.");
    }

    Ok(())
}

pub async fn create_auction(
    api: &GearApi,
    listener: &mut EventListener,
    program_id: &ActorId,
    nft_contract: &ActorId,
    ft_contract: Option<ActorId>,
    token_id: TokenId,
    min_price: u128,
    bid_period: u64,
    duration: u64,
    should_fail: bool,
) -> gclient::Result<()> {
    let reply = send_message(
        api,
        listener,
        program_id,
        MarketAction::CreateAuction {
            nft_contract_id: *nft_contract,
            ft_contract_id: ft_contract,
            token_id,
            min_price,
            bid_period,
            duration,
        },
        0,
    )
    .await?;

    if !should_fail {
        let MarketEvent::AuctionCreated { nft_contract_id: _, token_id: _, price: _ } = MarketEvent::decode(&mut reply.as_ref()).expect("Unexpected invalid `MarketEvent` data.") else {
            panic!("Unexpected invalid `MarketEvent`.");
        };
    } else {
        MarketErr::decode(&mut reply.as_ref()).expect("Unexpected invalid `MarketErr` data.");
    }

    Ok(())
}

pub async fn add_bid(
    api: &GearApi,
    listener: &mut EventListener,
    program_id: &ActorId,
    nft_contract: &ActorId,
    token_id: TokenId,
    price: u128,
    should_fail: bool,
) -> gclient::Result<()> {
    let reply = send_message(
        api,
        listener,
        program_id,
        MarketAction::AddBid {
            nft_contract_id: *nft_contract,
            token_id,
            price,
        },
        0,
    )
    .await?;

    if !should_fail {
        let MarketEvent::BidAdded { nft_contract_id: _, token_id: _, price: _ } = MarketEvent::decode(&mut reply.as_ref()).expect("Unexpected invalid `MarketEvent` data.") else {
            panic!("Unexpected invalid `MarketEvent`.");
        };
    } else {
        MarketErr::decode(&mut reply.as_ref()).expect("Unexpected invalid `MarketErr` data.");
    }

    Ok(())
}

pub async fn settle_auction(
    api: &GearApi,
    listener: &mut EventListener,
    program_id: &ActorId,
    nft_contract: &ActorId,
    token_id: TokenId,
    should_fail: bool,
) -> gclient::Result<()> {
    let reply = send_message(
        api,
        listener,
        program_id,
        MarketAction::SettleAuction {
            nft_contract_id: *nft_contract,
            token_id,
        },
        0,
    )
    .await?;

    if !should_fail {
        let MarketEvent::AuctionSettled { nft_contract_id: _, token_id: _, price: _ } = MarketEvent::decode(&mut reply.as_ref()).expect("Unexpected invalid `MarketEvent` data.") else {
            panic!("Unexpected invalid `MarketEvent`.");
        };
    } else {
        MarketErr::decode(&mut reply.as_ref()).expect("Unexpected invalid `MarketErr` data.");
    }

    Ok(())
}

pub async fn add_offer(
    api: &GearApi,
    listener: &mut EventListener,
    program_id: &ActorId,
    nft_contract: &ActorId,
    ft_contract: Option<ActorId>,
    token_id: TokenId,
    price: u128,
    value: u128,
    should_fail: bool,
) -> gclient::Result<()> {
    let reply = send_message(
        api,
        listener,
        program_id,
        MarketAction::AddOffer {
            nft_contract_id: *nft_contract,
            ft_contract_id: ft_contract,
            token_id,
            price,
        },
        value,
    )
    .await?;

    if !should_fail {
        let MarketEvent::OfferAdded { nft_contract_id: _, ft_contract_id:_, token_id: _, price: _ } = MarketEvent::decode(&mut reply.as_ref()).expect("Unexpected invalid `MarketEvent` data.") else {
            panic!("Unexpected invalid `MarketEvent`.");
        };
    } else {
        MarketErr::decode(&mut reply.as_ref()).expect("Unexpected invalid `MarketErr` data.");
    }

    Ok(())
}

pub async fn withdraw(
    api: &GearApi,
    listener: &mut EventListener,
    program_id: &ActorId,
    nft_contract: &ActorId,
    ft_contract: Option<ActorId>,
    token_id: TokenId,
    price: u128,
    should_fail: bool,
) -> gclient::Result<()> {
    let reply = send_message(
        api,
        listener,
        program_id,
        MarketAction::Withdraw {
            nft_contract_id: *nft_contract,
            ft_contract_id: ft_contract,
            token_id,
            price,
        },
        0,
    )
    .await?;

    if !should_fail {
        let MarketEvent::Withdraw { nft_contract_id: _, token_id: _, price: _ } = MarketEvent::decode(&mut reply.as_ref()).expect("Unexpected invalid `MarketEvent` data.") else {
            panic!("Unexpected invalid `MarketEvent`.");
        };
    } else {
        MarketErr::decode(&mut reply.as_ref()).expect("Unexpected invalid `MarketErr` data.");
    }

    Ok(())
}

pub async fn accept_offer(
    api: &GearApi,
    listener: &mut EventListener,
    program_id: &ActorId,
    nft_contract: &ActorId,
    ft_contract: Option<ActorId>,
    token_id: TokenId,
    price: u128,
    should_fail: bool,
) -> gclient::Result<()> {
    let reply = send_message(
        api,
        listener,
        program_id,
        MarketAction::AcceptOffer {
            nft_contract_id: *nft_contract,
            ft_contract_id: ft_contract,
            token_id,
            price,
        },
        0,
    )
    .await?;

    if !should_fail {
        let MarketEvent::OfferAccepted { nft_contract_id: _, token_id: _, new_owner:_, price: _ } = MarketEvent::decode(&mut reply.as_ref()).expect("Unexpected invalid `MarketEvent` data.") else {
            panic!("Unexpected invalid `MarketEvent`.");
        };
    } else {
        MarketErr::decode(&mut reply.as_ref()).expect("Unexpected invalid `MarketErr` data.");
    }

    Ok(())
}

pub async fn state(api: &GearApi, program_id: &ActorId) -> gclient::Result<Market> {
    let program_id_ref = program_id.as_ref();

    api.read_state(program_id_ref.into()).await
}

async fn send_message(
    api: &GearApi,
    listener: &mut EventListener,
    program_id: &ActorId,
    payload: MarketAction,
    value: u128,
) -> gclient::Result<Vec<u8>> {
    let program_id: common::Hash = program_id
        .encode()
        .try_into()
        .expect("Unexpected invalid program id.");

    let gas_info = api
        .calculate_handle_gas(None, program_id.into(), payload.encode(), value, true)
        .await?;

    let (message_id, _) = api
        .send_message(program_id.into(), payload, gas_info.min_limit * 2, value)
        .await?;

    // TODO: assert!(listener.message_processed(message_id).await?.succeed());

    let (_, reply_data_result, _) = listener.reply_bytes_on(message_id).await?;
    Ok(reply_data_result.expect("Unexpected invalid reply."))
}
