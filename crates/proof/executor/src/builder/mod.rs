//! Stateless OP Stack block builder implementation.
use alloc::sync::Arc;
use alloy_consensus::{Header, Sealed};
use alloy_evm::block::BlockExecutionResult;
use alloy_primitives::B256;
use kona_mpt::TrieHinter;
use op_alloy_consensus::OpReceiptEnvelope;
use op_alloy_rpc_types_engine::OpPayloadAttributes;
use soon_primitives::{blocks::L2BlockInfo, rollup_config::SoonRollupConfig};

mod core;
pub use core::StatelessL2Builder;

mod offchain;
pub use offchain::{INIT_ACCOUNTS_HASH, OffchainL2Builder, cal_extra_accounts_hash};

use crate::{ExecutorResult, TrieDBProvider};

/// A trait for building L2 blocks.
pub trait L2BlockBuilder<P, H>
where
    P: TrieDBProvider,
    H: TrieHinter,
{
    /// Creates a new block builder.
    fn new(
        config: Arc<SoonRollupConfig>,
        provider: P,
        hinter: H,
        parent_header: L2BlockInfo,
    ) -> Self;

    /// Initializes the block builder.
    fn init(&mut self) -> ExecutorResult<()>;

    /// Builds a new block on top of the parent state, using the given [`OpPayloadAttributes`].
    fn build_block(&mut self, attrs: OpPayloadAttributes) -> ExecutorResult<L2BlockInfo>;

    /// Computes the current output root of the latest executed block, based on the parent header
    /// and the underlying state trie.
    fn compute_output_root(&mut self) -> ExecutorResult<B256>;
}

/// The outcome of a block building operation, returning the sealed block [`Header`] and the
/// [`BlockExecutionResult`].
#[derive(Debug, Clone)]
pub struct BlockBuildingOutcome {
    /// The block header.
    pub header: L2BlockInfo,
    /// The block execution result.
    pub execution_result: BlockExecutionResult<OpReceiptEnvelope>,
}

impl From<(L2BlockInfo, BlockExecutionResult<OpReceiptEnvelope>)> for BlockBuildingOutcome {
    fn from(
        (header, execution_result): (L2BlockInfo, BlockExecutionResult<OpReceiptEnvelope>),
    ) -> Self {
        Self { header, execution_result }
    }
}
