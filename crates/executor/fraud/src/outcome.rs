use litesvm::types::TransactionResult;
use soon_primitives::blocks::L2BlockInfo;

/// The outcome of a block building operation, returning the sealed block [`Header`] and the
/// [`BlockExecutionResult`].
#[derive(Debug, Clone, Default)]
pub struct BlockBuildingOutcome {
    /// The block header.
    pub header: L2BlockInfo,
    /// The block execution result.
    pub execution_result: Vec<TransactionResult>,
}

impl From<(L2BlockInfo, Vec<TransactionResult>)> for BlockBuildingOutcome {
    fn from((header, execution_result): (L2BlockInfo, Vec<TransactionResult>)) -> Self {
        Self { header, execution_result }
    }
}
