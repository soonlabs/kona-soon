use crate::{ExecutorError, ExecutorResult, L2BlockBuilder, TrieDBProvider};
use alloc::sync::Arc;
use alloy_consensus::{Header, Sealed};
use alloy_primitives::B256;
use soon_primitives::blocks::L2BlockInfo;
use kona_mpt::TrieHinter;
use soon_primitives::rollup_config::SoonRollupConfig;

#[derive(Debug, Clone)]
pub struct OffchainL2Builder<P, H>
where
    P: TrieDBProvider,
    H: TrieHinter,
{
    pub(crate) config: Arc<SoonRollupConfig>,
    pub(crate) provider: P,
    pub(crate) hinter: H,
    pub(crate) parent_header: L2BlockInfo,
}

impl<P, H> L2BlockBuilder<P, H> for OffchainL2Builder<P, H>
where
    P: TrieDBProvider,
    H: TrieHinter,
{
    fn new(
        config: Arc<SoonRollupConfig>,
        provider: P,
        hinter: H,
        parent_header: L2BlockInfo,
    ) -> Self {
        Self { config, provider, hinter, parent_header }
    }

    fn init(&mut self) -> ExecutorResult<()> {
        Ok(())
    }

    fn build_block(
        &mut self,
        attrs: op_alloy_rpc_types_engine::OpPayloadAttributes,
    ) -> ExecutorResult<soon_primitives::blocks::L2BlockInfo> {
        todo!()
    }

    fn compute_output_root(&mut self) -> ExecutorResult<B256> {
        todo!()
    }
}
