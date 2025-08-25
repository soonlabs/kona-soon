//! Stateless OP Stack block builder implementation.
use alloc::sync::Arc;
use alloy_primitives::B256;
use kona_mpt::TrieHinter;
use op_alloy_rpc_types_engine::OpPayloadAttributes;
use soon_primitives::blocks::L2BlockHeader;
use soon_primitives::rollup_config::SoonRollupConfig;

mod core;
pub use core::StatelessL2Builder;
use fraud_executor::accounts::SoonAccounts;
use fraud_executor::outcome::BlockBuildingOutcome;

mod offchain;
pub use offchain::{
    OffchainL2Builder, cal_init_state_root_hash, cal_soon_accounts_hash, cal_svm_clock_timestamp,
    cal_svm_leader, cal_svm_parent_info,
};

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
        parent_header: L2BlockHeader,
        last_accounts_diff: SoonAccounts,
    ) -> Self;

    /// Initializes the block builder.
    fn init(&mut self) -> ExecutorResult<()>;

    /// Builds a new block on top of the parent state, using the given [`OpPayloadAttributes`].
    fn build_block(&mut self, attrs: OpPayloadAttributes) -> ExecutorResult<BlockBuildingOutcome>;

    /// Computes the current output root of the latest executed block, based on the parent header
    /// and the underlying state trie.
    fn compute_output_root(&mut self) -> ExecutorResult<B256>;

    /// Get builder account diff
    fn account_diff(&self) -> SoonAccounts;
}
