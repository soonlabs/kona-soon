#![allow(clippy::arithmetic_side_effects)]
#![deny(missing_docs)]
#![cfg_attr(not(test), forbid(unsafe_code))]

//! A withdrawal bridge for the soon blockchain

pub mod error;
pub mod instruction;
pub mod pda;
pub mod processor;
pub mod state;

#[cfg(not(feature = "no-entrypoint"))]
mod entrypoint;

pub use solana_program;
use solana_program::{
    entrypoint::ProgramResult, program_error::ProgramError, pubkey, pubkey::Pubkey,
};

solana_program::declare_id!("L1BLockinfo11111111111111111111111111111111");

/// No-sig tx payer pubkey
pub const NO_SIG_TX_PAYER: Pubkey = pubkey!("NoSigTxPayer1111111111111111111111111111111");

/// Checks that the supplied program ID is the correct one for svm withdraw bridge
pub fn check_program_account(svm_deposit_bridge_program_id: &Pubkey) -> ProgramResult {
    if svm_deposit_bridge_program_id != &id() {
        return Err(ProgramError::IncorrectProgramId);
    }
    Ok(())
}
