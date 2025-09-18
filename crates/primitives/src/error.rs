use alloy_primitives::hex::FromHexError;
use solana_program::{hash::ParseHashError, pubkey::ParsePubkeyError};
use solana_sdk::transaction::TransactionError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum BlockError {
    #[error("Invalid new block: {0}")]
    InvalidNewBlock(String),
    #[error(transparent)]
    InvalidTransaction(#[from] TransactionError),
    #[error(transparent)]
    InvalidNativeTransaction(#[from] NativeTransactionError),
    #[error(transparent)]
    DepositError(#[from] DepositError),
}

#[derive(Error, Debug, Eq, PartialEq)]
pub enum DepositError {
    #[error("Derive deposit tx error: {0}")]
    ParseDepositTxError(String),
    #[error("Derive l1 block info tx error: {0}")]
    ParseL1BlockInfoTxError(String),
    #[error("Invalid pubkey")]
    InvalidPubKey,
    #[error("Invalid decimals")]
    InvalidDecimals,
    #[error(transparent)]
    InvalidTransaction(#[from] TransactionError),
    #[error(transparent)]
    InvalidNativeTransaction(#[from] NativeTransactionError),
    #[error("invalid l1 cross domain messenger: {0}")]
    InvalidDomainMessenger(String),
    #[error("invalid l2 cross domain messenger: {0}")]
    InvalidL2CrossDomainMessenger(String),
    #[error("invalid l2 standard bridge: {0}")]
    InvalidL2StandardBridge(String),
    #[error("invalid l1 standard bridge: {0}")]
    InvalidL1StandardBridge(String),
    #[error("Invalid relay selector: {0}")]
    InvalidRelaySelector(String),
    #[error("Invalid deposit selector: {0}")]
    InvalidDepositSelector(String),
    #[error("Invalid data length")]
    InvalidDataLength,
}

#[derive(Error, Debug, Eq, PartialEq)]
pub enum NativeTransactionError {
    #[error("Not support signatures")]
    NotSupportSignatures,
}

#[derive(Error, Debug)]
pub enum RollupConfigError {
    #[error(transparent)]
    Address(#[from] FromHexError),
    #[error(transparent)]
    PubKey(#[from] ParsePubkeyError),
    #[error(transparent)]
    Hash(#[from] ParseHashError),
}

#[derive(Error, Debug)]
pub enum L2BlockError {
    #[error("Insufficient transactions in block")]
    InsufficientTransactions,
    #[error("Block entries and transactions are not matched")]
    UnmatchedEntriesAndTransactions,
    #[error(transparent)]
    Hash(#[from] ParseHashError),
}
