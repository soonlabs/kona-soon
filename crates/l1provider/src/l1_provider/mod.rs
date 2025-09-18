mod base_provider;
mod eth_provider;

use crate::error::AlloyChainProviderError;
use crate::l1_provider::base_provider::BaseProviderImpl;
use crate::l1_provider::eth_provider::EthProviderImpl;
use alloy::transports::http::reqwest::Url;
use alloy_consensus::Receipt;
use alloy_eips::BlockNumberOrTag;
use async_trait::async_trait;
use soon_primitives::blocks::{BlockInfo, L1Header, L1Transaction};
use std::fmt::Debug;
use alloy_primitives::{B256, BlockHash};

#[async_trait]
pub(crate) trait L1Client: Send + Sync + Debug {
    async fn get_block_number(&self) -> Result<u64, AlloyChainProviderError>;
    async fn get_header_by_hash(&self, hash: B256) -> Result<L1Header, AlloyChainProviderError>;

    async fn get_header_by_number(
        &self,
        number: BlockNumberOrTag,
    ) -> Result<L1Header, AlloyChainProviderError>;

    async fn get_block_receipts_by_hash(
        &self,
        hash: B256,
    ) -> Result<Vec<Receipt>, AlloyChainProviderError>;

    async fn get_full_block_by_hash(
        &self,
        hash: BlockHash,
    ) -> Result<(BlockInfo, Vec<L1Transaction>), AlloyChainProviderError>;
}

pub fn create_l1_provider(chain_id: u64, l1_url: Url) -> Box<dyn L1Client> {
    if chain_id == 8453 || chain_id == 84532 {
        Box::new(BaseProviderImpl::new_with_url(l1_url))
    } else {
        Box::new(EthProviderImpl::new_with_url(l1_url))
    }
}
