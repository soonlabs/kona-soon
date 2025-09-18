//! Error types

use num_derive::FromPrimitive;
use solana_program::{
    decode_error::DecodeError,
    msg,
    program_error::{PrintProgramError, ProgramError},
};
use thiserror::Error;

/// Errors that may be returned by the L1BlockInfo program.
#[derive(Clone, Debug, Eq, Error, FromPrimitive, PartialEq)]
pub enum L1BlockInfoError {
    // 0
    /// Invalid instruction data or account
    #[error("Invalid instruction")]
    InvalidInstruction,
    /// Invalid l1 block info account
    #[error("Invalid l1 block info account")]
    InvalidL1BlockInfoAccount,
    /// Invalid NoSigTxPayer
    #[error("Invalid no-sig transaction payer detected")]
    InvalidNoSigTxPayer,
}
impl From<L1BlockInfoError> for ProgramError {
    fn from(e: L1BlockInfoError) -> Self {
        ProgramError::Custom(e as u32)
    }
}
impl<T> DecodeError<T> for L1BlockInfoError {
    fn type_of() -> &'static str {
        "TokenError"
    }
}

impl PrintProgramError for L1BlockInfoError {
    fn print<E>(&self)
    where
        E: 'static
            + std::error::Error
            + DecodeError<E>
            + PrintProgramError
            + num_traits::FromPrimitive,
    {
        match self {
            L1BlockInfoError::InvalidInstruction => msg!("Error: Invalid instruction"),
            L1BlockInfoError::InvalidL1BlockInfoAccount => {
                msg!("Error: Invalid l1 block info account")
            }
            L1BlockInfoError::InvalidNoSigTxPayer => {
                msg!("Error: Invalid no-sig transaction payer detected")
            }
        }
    }
}
