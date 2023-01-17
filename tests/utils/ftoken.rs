use super::{prelude::*};
use ft_logic_io::{Action, FTLogicEvent};
use ft_main_io::*;
use gstd::ActorId;
use gtest::{Log, Program as InnerProgram, System};

pub struct FungibleToken<'a>(InnerProgram<'a>);

impl Program for FungibleToken<'_> {
    fn inner_program(&self) -> &InnerProgram {
        &self.0
    }
}

impl<'a> FungibleToken<'a> {
    pub fn initialize(system: &'a System) -> Self {

        let program = InnerProgram::from_file(system, "./target/ft_main.wasm");
        let storage_code_hash: [u8; 32] = system.submit_code("./target/ft_storage.opt.wasm").into();
        let ft_logic_code_hash: [u8; 32] = system.submit_code("./target/ft_logic.opt.wasm").into();

        // let program = InnerProgram::from_file(system, "./sharded-fungible-token/target/wasm32-unknown-unknown/release/ft_main.opt.wasm");
        // let storage_code_hash: [u8; 32] = system.submit_code("./sharded-fungible-token/target/wasm32-unknown-unknown/release/ft_storage.opt.wasm").into();
        // let ft_logic_code_hash: [u8; 32] = system.submit_code("./sharded-fungible-token/target/wasm32-unknown-unknown/release/ft_logic.opt.wasm").into();


        assert!(!program
            .send(
                ADMIN,
                InitFToken {
                    storage_code_hash: storage_code_hash.into(),
                    ft_logic_code_hash: ft_logic_code_hash.into(),
                },
            )
            .main_failed());

        Self(program)
    }

    pub fn mint(&self, transaction_id: u64, from: u64, amount: u128) {
        let payload = Action::Mint {
            recipient: from.into(),
            amount,
        }
        .encode();
        assert!(self
            .0
            .send(from, FTokenAction::Message {
                transaction_id,
                payload,
            })
            .contains(&Log::builder().payload(FTokenEvent::Ok)));
    }

    pub fn approve(&self, transaction_id: u64, from: u64, to: ActorId, amount: u128) {
        let payload = Action::Approve {
            approved_account: to,
            amount,
        }
        .encode();
        assert!(self
            .0
            .send(from, FTokenAction::Message {
                transaction_id, payload,
            })
            .contains(&Log::builder().payload(FTokenEvent::Ok)));
    }

}