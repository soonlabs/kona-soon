//! Contains the [EthereumDataSource], which is a concrete implementation of the
//! [DataAvailabilityProvider] trait for the Ethereum protocol.

use crate::{
    sources::da_server::DAServerSource,
    traits::{ChainProvider, DAProvider, DataAvailabilityProvider},
    types::PipelineResult,
};
use alloc::{boxed::Box, fmt::Debug};
use alloy_primitives::{Address, Bytes};
use async_trait::async_trait;
use soon_primitives::{blocks::BlockInfo, rollup_config::SoonRollupConfig};

/// A factory for creating an Ethereum data source provider.
#[derive(Debug, Clone)]
pub struct EthereumDataSource<C, DA>
where
    C: ChainProvider + Send + Clone,
    DA: DAProvider + Send + Debug,
{
    /// The chain provider to use for the factory.
    pub chain_provider: C,
    /// The batch inbox address.
    pub batch_inbox_address: Address,
    /// The da proxy source.
    pub da_source: DAServerSource<C, DA>,
}

impl<C, DA> EthereumDataSource<C, DA>
where
    C: ChainProvider + Send + Clone + Debug,
    DA: DAProvider + Send + Clone + Debug,
{
    /// Creates a new factory.
    pub fn new(provider: C, da_provider: DA, cfg: &SoonRollupConfig) -> Self {
        let da_source =
            DAServerSource::new(provider.clone(), da_provider.clone(), cfg.batch_inbox_address);
        Self { chain_provider: provider, batch_inbox_address: cfg.batch_inbox_address, da_source }
    }
}

#[async_trait]
impl<C, DA> DataAvailabilityProvider for EthereumDataSource<C, DA>
where
    C: ChainProvider + Send + Sync + Clone + Debug,
    DA: DAProvider + Send + Clone + Debug,
{
    type Item = Bytes;

    async fn next(
        &mut self,
        block_ref: &BlockInfo,
        batcher_address: Address,
    ) -> PipelineResult<Self::Item> {
        self.da_source.next(block_ref, batcher_address).await
    }

    fn clear(&mut self) {
        self.da_source.clear();
    }
}
