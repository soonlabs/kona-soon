use serde::{Deserialize, Serialize};
use solana_sdk::transaction::SanitizedTransaction;
use solana_sdk::{
    account::AccountSharedData,
    inner_instruction::InnerInstructionsList,
    instruction::InstructionError,
    program_error::ProgramError,
    pubkey::Pubkey,
    signature::Signature,
    transaction::{Result, TransactionError},
    transaction_context::TransactionReturnData,
};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TransactionMetadata {
    pub signature: Signature,
    pub logs: Vec<String>,
    pub inner_instructions: InnerInstructionsList,
    pub compute_units_consumed: u64,
    pub return_data: TransactionReturnData,
    pub fee: u64,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SimulatedTransactionInfo {
    pub meta: TransactionMetadata,
    pub post_accounts: Vec<(Pubkey, AccountSharedData)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedTransactionMetadata {
    pub err: TransactionError,
    pub meta: TransactionMetadata,
}

impl From<ProgramError> for FailedTransactionMetadata {
    fn from(value: ProgramError) -> Self {
        FailedTransactionMetadata {
            err: TransactionError::InstructionError(
                0,
                InstructionError::Custom(u64::from(value) as u32),
            ),
            meta: Default::default(),
        }
    }
}

pub type TransactionResult = std::result::Result<TransactionMetadata, FailedTransactionMetadata>;

pub(crate) struct ExecutionResult {
    pub(crate) sanitized_tx: Option<SanitizedTransaction>,
    pub(crate) post_accounts: Vec<(Pubkey, AccountSharedData)>,
    pub(crate) tx_result: Result<()>,
    pub(crate) signature: Signature,
    pub(crate) signature_count: u8,
    pub(crate) compute_units_consumed: u64,
    pub(crate) inner_instructions: InnerInstructionsList,
    pub(crate) return_data: TransactionReturnData,
    /// Whether the transaction can be included in a block
    pub(crate) included: bool,
    pub(crate) fee: u64,
}

impl Default for ExecutionResult {
    fn default() -> Self {
        Self {
            sanitized_tx: None,
            post_accounts: Default::default(),
            tx_result: Err(TransactionError::UnsupportedVersion),
            signature: Default::default(),
            signature_count: 0,
            compute_units_consumed: Default::default(),
            inner_instructions: Default::default(),
            return_data: Default::default(),
            included: false,
            fee: 0,
        }
    }
}

impl ExecutionResult {
    pub(crate) fn result_and_compute_units(
        sanitized_tx: SanitizedTransaction,
        tx_result: Result<()>,
        compute_units_consumed: u64,
        fee: u64,
    ) -> Self {
        Self {
            sanitized_tx: Some(sanitized_tx),
            tx_result,
            compute_units_consumed,
            fee,
            ..Default::default()
        }
    }
}
