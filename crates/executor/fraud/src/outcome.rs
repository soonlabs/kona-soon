use kona_soon_primitives::blocks::L2BlockInfo;
use litesvm::types::TransactionResult;
use solana_program::fee_calculator::FeeRateGovernor;
use soon_mpt_primitives::B256;

/// The outcome of a block building operation, returning the sealed block [`Header`] and the
/// [`BlockExecutionResult`].
#[derive(Debug, Clone, Default)]
pub struct BlockBuildingOutcome {
    /// The block header.
    pub block_info: L2BlockInfo,
    /// The state root
    pub state_root: B256,
    /// The block execution result.
    pub execution_result: Vec<TransactionResult>,
    /// The number of signatures in the block.
    pub signature_count: u64,
    /// The fee rate governor for the block.
    pub fee_rate_governor: FeeRateGovernor,
}
