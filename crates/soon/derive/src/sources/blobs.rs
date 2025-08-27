//! Blob Data Source

use crate::{
    errors::{BlobProviderError, PipelineError},
    sources::BlobData,
    traits::{BlobProvider, ChainProvider, DataAvailabilityProvider},
    types::PipelineResult,
};
use alloc::{boxed::Box, string::ToString, vec::Vec};
use alloy_consensus::TxEnvelope;
use alloy_eips::eip4844::IndexedBlobHash;
use alloy_primitives::{Address, Bytes};
use async_trait::async_trait;
use soon_primitives::blocks::BlockInfo;
use tracing::warn;

/// A data iterator that reads from a blob.
#[derive(Debug, Clone)]
pub struct BlobSource<F, B>
where
    F: ChainProvider + Send,
    B: BlobProvider + Send,
{
    /// Chain provider.
    pub chain_provider: F,
    /// Fetches blobs.
    pub blob_fetcher: B,
    /// The address of the batcher contract.
    pub batcher_address: Address,
    /// Data.
    pub data: Vec<BlobData>,
    /// Whether the source is open.
    pub open: bool,
}

impl<F, B> BlobSource<F, B>
where
    F: ChainProvider + Send,
    B: BlobProvider + Send,
{
    /// Creates a new blob source.
    pub const fn new(chain_provider: F, blob_fetcher: B, batcher_address: Address) -> Self {
        Self { chain_provider, blob_fetcher, batcher_address, data: Vec::new(), open: false }
    }

    fn extract_blob_data(
        &self,
        _txs: Vec<TxEnvelope>,
        _batcher_address: Address,
    ) -> (Vec<BlobData>, Vec<IndexedBlobHash>) {
        // soon used alternative da, we just return empty data.
        (vec![], vec![])
    }

    /// Loads blob data into the source if it is not open.
    async fn load_blobs(
        &mut self,
        block_ref: &BlockInfo,
        batcher_address: Address,
    ) -> Result<(), BlobProviderError> {
        if self.open {
            return Ok(());
        }

        let (mut data, blob_hashes) = self.extract_blob_data(vec![], batcher_address);

        // If there are no hashes, set the calldata and return.
        if blob_hashes.is_empty() {
            self.open = true;
            self.data = data;
            return Ok(());
        }

        let blobs = self.blob_fetcher.get_blobs(block_ref, &blob_hashes).await.map_err(|e| {
            warn!(target: "blob_source", "Failed to fetch blobs: {e}");
            BlobProviderError::Backend(e.to_string())
        })?;

        // Fill the blob pointers.
        let mut blob_index = 0;
        for blob in data.iter_mut() {
            match blob.fill(&blobs, blob_index) {
                Ok(should_increment) => {
                    if should_increment {
                        blob_index += 1;
                    }
                }
                Err(e) => {
                    return Err(e.into());
                }
            }
        }

        self.open = true;
        self.data = data;
        Ok(())
    }

    /// Extracts the next data from the source.
    fn next_data(&mut self) -> Result<BlobData, PipelineResult<Bytes>> {
        if self.data.is_empty() {
            return Err(Err(PipelineError::Eof.temp()));
        }

        Ok(self.data.remove(0))
    }
}

#[async_trait]
impl<F, B> DataAvailabilityProvider for BlobSource<F, B>
where
    F: ChainProvider + Sync + Send,
    B: BlobProvider + Sync + Send,
{
    type Item = Bytes;

    async fn next(
        &mut self,
        block_ref: &BlockInfo,
        batcher_address: Address,
    ) -> PipelineResult<Self::Item> {
        self.load_blobs(block_ref, batcher_address).await?;

        let next_data = match self.next_data() {
            Ok(d) => d,
            Err(e) => return e,
        };
        if let Some(c) = next_data.calldata {
            return Ok(c);
        }

        // Decode the blob data to raw bytes.
        // Otherwise, ignore blob and recurse next.
        match next_data.decode() {
            Ok(d) => Ok(d),
            Err(_) => {
                warn!(target: "blob_source", "Failed to decode blob data, skipping");
                self.next(block_ref, batcher_address).await
            }
        }
    }

    fn clear(&mut self) {
        self.data.clear();
        self.open = false;
    }
}
