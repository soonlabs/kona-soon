use crate::{
    errors::PipelineError,
    prelude::DataAvailabilityProvider,
    sources::CalldataSource,
    traits::{ChainProvider, DAProvider},
    types::PipelineResult,
};
use alloc::{boxed::Box, collections::VecDeque};
use alloy_primitives::{Address, Bytes};
use async_trait::async_trait;
use soon_primitives::blocks::BlockInfo;

/// A data iterator that reads from da server.
#[derive(Debug, Clone)]
pub struct DAServerSource<CP, DA>
where
    CP: ChainProvider + Send,
    DA: DAProvider + Send,
{
    /// The calldata source
    pub calldata_source: CalldataSource<CP>,
    /// server data provider
    pub server_provider: DA,
    /// Current calldata.
    pub data: VecDeque<Bytes>,
}

impl<CP: ChainProvider + Send, DA: DAProvider + Send> DAServerSource<CP, DA> {
    /// Creates a new da server source.
    pub fn new(chain_provider: CP, server_provider: DA, batch_inbox_address: Address) -> Self {
        let calldata_source = CalldataSource::new(chain_provider, batch_inbox_address);

        Self { calldata_source, server_provider, data: VecDeque::new() }
    }

    /// Loads the image data from da server
    async fn load_da_server_data(&mut self, commitment: Bytes) -> PipelineResult<()> {
        let image_data =
            self.server_provider.get_input(commitment.to_vec()).await.map_err(Into::into)?;
        self.data.push_back(Bytes::from(image_data));
        Ok(())
    }
}

#[async_trait]
impl<CP: ChainProvider + Send, DA: DAProvider + Send> DataAvailabilityProvider
    for DAServerSource<CP, DA>
{
    type Item = Bytes;

    async fn next(
        &mut self,
        block_ref: &BlockInfo,
        batcher_address: Address,
    ) -> PipelineResult<Self::Item> {
        let commitment = self.calldata_source.next(block_ref, batcher_address).await?;
        self.load_da_server_data(commitment.clone()).await?;
        self.data.pop_front().ok_or(PipelineError::Eof.temp())
    }

    fn clear(&mut self) {
        self.data.clear();
        self.calldata_source.clear();
    }
}
