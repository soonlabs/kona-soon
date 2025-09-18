#![allow(clippy::arithmetic_side_effects)]
#![deny(missing_docs)]
#![cfg_attr(not(test), forbid(unsafe_code))]

//! A withdrawal bridge for the soon blockchain

pub mod error;
pub mod instruction;
pub mod pda;
pub mod processor;
pub mod state;
mod utils;

#[cfg(not(feature = "no-entrypoint"))]
mod entrypoint;
#[cfg(test)]
mod tests;

pub use solana_program;
use solana_program::{
    entrypoint::ProgramResult, program_error::ProgramError, pubkey, pubkey::Pubkey,
};

solana_program::declare_id!("Bridge1111111111111111111111111111111111111");

/// No-sig tx payer pubkey
pub const NO_SIG_TX_PAYER: Pubkey = pubkey!("NoSigTxPayer1111111111111111111111111111111");

/// Checks that the supplied program ID is the correct one for bridge
pub fn check_program_account(bridge_program_id: &Pubkey) -> ProgramResult {
    if bridge_program_id != &id() {
        return Err(ProgramError::IncorrectProgramId);
    }
    Ok(())
}
