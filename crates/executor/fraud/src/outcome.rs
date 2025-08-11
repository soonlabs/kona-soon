use soon_mpt_primitives::B256;
use litesvm::types::TransactionResult;
use soon_primitives::blocks::L2BlockInfo;

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
}

impl From<(L2BlockInfo, B256, Vec<TransactionResult>)> for BlockBuildingOutcome {
    fn from((block_info, state_root, execution_result): (L2BlockInfo, B256, Vec<TransactionResult>)) -> Self {
        Self { block_info, state_root, execution_result }
    }
}
