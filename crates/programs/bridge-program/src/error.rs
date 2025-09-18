//! Error types

use {
    num_derive::FromPrimitive,
    solana_program::{
        decode_error::DecodeError,
        msg,
        program_error::{PrintProgramError, ProgramError},
    },
    thiserror::Error,
};

/// Errors that may be returned by the bridge program.
#[derive(Clone, Debug, Eq, Error, FromPrimitive, PartialEq)]
pub enum BridgeError {
    // 0
    /// Invalid instruction data or account
    #[error("Invalid instruction")]
    InvalidInstruction,
    /// Invalid Withdrawal Counter Account
    #[error("Invalid Withdrawal Counter Account")]
    InvalidWithdrawalCounterAccount,
    /// Invalid Withdrawal Transaction Account
    #[error("Invalid Withdrawal Transaction Account")]
    InvalidWithdrawalTransactionAccount,
    /// Invalid vault account
    #[error("Invalid vault account")]
    InvalidVaultAccount,
    /// Invalid depositor account
    #[error("Invalid depositor account")]
    InvalidDepositorAccount,
    /// Invalid deposit type
    #[error("Invalid deposit type")]
    InvalidDepositType,
    /// Deposit account is full, need to expand
    #[error("Deposit account is full, need to expand")]
    DepositAccountIsFull,
    /// Invalid SPL Token Info Account
    #[error("Invalid SPL Token Owner Account")]
    InvalidSPLTokenOwnerAccount,
    /// Invalid SPL Token Mint Account
    #[error("Invalid SPL Token Mint Account")]
    InvalidSPLTokenMintAccount,
    /// Invalid Depositor Associated Token Account
    #[error("Invalid Depositor Associated Token Account")]
    InvalidDepositorAssociatedTokenAccount,
    /// Invalid Remote Token Account
    #[error("Invalid Remote Token Account")]
    InvalidRemoteTokenAccount,
    /// Invalid SPL Token Program Id
    #[error("Invalid SPL Token Program Id")]
    InvalidSPLTokenProgramId,
    /// Invalid MPL Token Metadata Account
    #[error("Invalid MPL Token Metadata Account")]
    InvalidMPLTokenMetadataAccount,
    /// Invalid Bridge Config Account
    #[error("Invalid Bridge Config Account")]
    InvalidBridgeConfigAccount,
    /// Invalid Associated Token Program Id
    #[error("Invalid Associated Token Program Id")]
    InvalidAssociatedTokenProgramId,
    /// String length should be less than 256
    #[error("String length should be less than 256")]
    StringLengthTooLong,
    /// Invalid NoSigTxPayer
    #[error("Invalid no-sig transaction payer detected")]
    InvalidNoSigTxPayer,
    /// Invalid Bridge Owner Account
    #[error("Invalid Bridge Owner Account")]
    InvalidBridgeOwnerAccount,
    /// Invalid Admin
    #[error("Invalid Bridge Admin")]
    InvalidAdmin,
    /// Gas limit for cross-chain call is too low
    #[error("Gas limit too low")]
    GasLimitTooLow,
    /// Decimal for spl token is too high
    #[error("decimal for spl token is too high")]
    InvalidSPLTokenDecimal,
    /// Invalid Metadata Program Id
    #[error("Invalid Metadata Program Id")]
    InvalidMetadataProgramId,
    /// Invalid Metadata Account
    #[error("Invalid Metadata Account")]
    InvalidMetadataAccount,
    /// Invalid Deposit Amount
    #[error("Invalid Deposit Amount")]
    InvalidDepositAmount,
    /// Invalid Withdraw Amount
    #[error("Invalid Withdraw Amount")]
    InvalidWithdrawAmount,
    /// Unmatched Token Account Owner
    #[error("Unmatched Token Account Owner")]
    UnmatchedTokenAccountOwner,
}
impl From<BridgeError> for ProgramError {
    fn from(e: BridgeError) -> Self {
        ProgramError::Custom(e as u32)
    }
}
impl<T> DecodeError<T> for BridgeError {
    fn type_of() -> &'static str {
        "TokenError"
    }
}

impl PrintProgramError for BridgeError {
    fn print<E>(&self)
    where
        E: 'static
            + std::error::Error
            + DecodeError<E>
            + PrintProgramError
            + num_traits::FromPrimitive,
    {
        match self {
            BridgeError::InvalidInstruction => msg!("Error: Invalid instruction"),
            BridgeError::InvalidWithdrawalCounterAccount => {
                msg!("Error: Invalid Withdrawal Counter Account")
            }
            BridgeError::InvalidWithdrawalTransactionAccount => {
                msg!("Error: Invalid Withdrawal Transaction Account")
            }
            BridgeError::InvalidVaultAccount => {
                msg!("Error: Invalid Vault Account")
            }
            BridgeError::InvalidDepositorAccount => {
                msg!("Error: Invalid depositor account")
            }
            BridgeError::InvalidDepositType => msg!("Error: Invalid deposit type"),
            BridgeError::DepositAccountIsFull => {
                msg!("Error: Deposit account is full, need to expand")
            }
            BridgeError::InvalidSPLTokenOwnerAccount => {
                msg!("Error: Invalid SPL Token Owner Account")
            }
            BridgeError::InvalidSPLTokenMintAccount => {
                msg!("Error: Invalid SPL Token Mint Account")
            }
            BridgeError::InvalidDepositorAssociatedTokenAccount => {
                msg!("Error: Invalid Depositor Associated Token Account")
            }
            BridgeError::InvalidRemoteTokenAccount => {
                msg!("Error: Invalid Remote Token Account")
            }
            BridgeError::InvalidSPLTokenProgramId => {
                msg!("Error: Invalid SPL Token Program Id")
            }
            BridgeError::InvalidMPLTokenMetadataAccount => {
                msg!("Error: Invalid MPL Token Metadata Account")
            }
            BridgeError::InvalidBridgeConfigAccount => {
                msg!("Error: Invalid Bridge Config Account")
            }
            BridgeError::InvalidAssociatedTokenProgramId => {
                msg!("Error: Invalid Associated Token Program Id")
            }
            BridgeError::StringLengthTooLong => {
                msg!("Error: String length should be less than 256")
            }
            BridgeError::InvalidNoSigTxPayer => {
                msg!("Error: Invalid no-sig transaction payer detected")
            }
            BridgeError::InvalidBridgeOwnerAccount => {
                msg!("Error: Invalid Bridge Owner Account")
            }
            BridgeError::InvalidAdmin => {
                msg!("Error: Invalid Admin")
            }
            BridgeError::GasLimitTooLow => {
                msg!("Error: Gas limit too low")
            }
            BridgeError::InvalidSPLTokenDecimal => {
                msg!("Error: decimal for spl token should be less than 10")
            }
            BridgeError::InvalidMetadataProgramId => {
                msg!("Error: Invalid Metadata Program Id")
            }
            BridgeError::InvalidMetadataAccount => {
                msg!("Error: Invalid Metadata Account")
            }
            BridgeError::InvalidDepositAmount => {
                msg!("Error: Invalid Deposit Amount")
            }
            BridgeError::InvalidWithdrawAmount => {
                msg!("Error: Invalid Withdraw Amount")
            }
            BridgeError::UnmatchedTokenAccountOwner => {
                msg!("Error: Unmatched Token Account Owner")
            }
        }
    }
}
