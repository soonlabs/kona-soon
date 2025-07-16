use solana_program::pubkey::Pubkey;
use solana_sdk::instruction::InstructionError;
use solana_sdk::transaction::TransactionError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum InvalidSysvarDataError {
    #[error("Invalid Clock sysvar data.")]
    Clock,
    #[error("Invalid EpochRewards sysvar data.")]
    EpochRewards,
    #[error("Invalid EpochSchedule sysvar data.")]
    EpochSchedule,
    #[error("Invalid Fees sysvar data.")]
    Fees,
    #[error("Invalid LastRestartSlot sysvar data.")]
    LastRestartSlot,
    #[error("Invalid RecentBlockhashes sysvar data.")]
    RecentBlockhashes,
    #[error("Invalid Rent sysvar data.")]
    Rent,
    #[error("Invalid SlotHashes sysvar data.")]
    SlotHashes,
    #[error("Invalid StakeHistory sysvar data.")]
    StakeHistory,
}

#[derive(Error, Debug)]
pub enum LiteSVMError {
    #[error(transparent)]
    InvalidSysvarData(#[from] InvalidSysvarDataError),
    #[error(transparent)]
    Instruction(#[from] InstructionError),
    #[error(transparent)]
    Bincode(#[from] bincode::Error),
    #[error(transparent)]
    Transaction(#[from] TransactionError),

    #[error("Add overflow")]
    AddOverflow,
    #[error("Sub overflow")]
    SubOverflow,

    #[error("Account {0} not found in callback")]
    AccountNotFound(Pubkey),

}
