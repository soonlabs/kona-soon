use crate::error::AlloyChainProviderError;
use crate::l1_provider::L1Client;
use alloy::network::primitives::HeaderResponse;
use alloy::network::Ethereum;
use alloy::providers::{Identity, Provider, ProviderBuilder, RootProvider};
use alloy_consensus::{Receipt, Transaction};
use alloy_eips::{BlockId, BlockNumberOrTag, RpcBlockHash};
use async_trait::async_trait;
use reqwest::Url;
use soon_primitives::blocks::{BlockInfo, L1Header, L1Transaction};
use alloy_primitives::{B256, BlockHash};

type HttpProvider = RootProvider<Ethereum>;

#[derive(Debug)]
pub struct EthProviderImpl {
    inner: HttpProvider,
}

impl EthProviderImpl {
    pub fn new_with_url(l1_url: Url) -> Self {
        let provider =
            ProviderBuilder::<Identity, Identity, Ethereum>::default().connect_http(l1_url);

        Self { inner: provider }
    }
}

#[async_trait]
impl L1Client for EthProviderImpl {
    async fn get_block_number(&self) -> Result<u64, AlloyChainProviderError> {
        Ok(self.inner.get_block_number().await?)
    }

    async fn get_header_by_hash(&self, hash: B256) -> Result<L1Header, AlloyChainProviderError> {
        let res_block = self.inner.get_block_by_hash(hash).hashes().await?;
        match res_block {
            Some(block) => Ok(block.header.into_consensus().into()),
            None => Err(AlloyChainProviderError::BlockNotFound(BlockId::Hash(
                RpcBlockHash::from_hash(hash, None),
            ))),
        }
    }

    async fn get_header_by_number(
        &self,
        number: BlockNumberOrTag,
    ) -> Result<L1Header, AlloyChainProviderError> {
        let res_block = self.inner.get_block_by_number(number).hashes().await?;
        match res_block {
            Some(block) => Ok(block.header.into_consensus().into()),
            None => Err(AlloyChainProviderError::BlockNotFound(BlockId::Number(
                number,
            ))),
        }
    }

    async fn get_block_receipts_by_hash(
        &self,
        hash: B256,
    ) -> Result<Vec<Receipt>, AlloyChainProviderError> {
        let receipts = self
            .inner
            .get_block_receipts(BlockId::Hash(RpcBlockHash::from_hash(hash, Some(true))))
            .await?
            .ok_or(AlloyChainProviderError::BlockNotFound(hash.into()))?;
        let consensus_receipts = receipts
            .into_iter()
            .map(|r| r.inner.into_primitives_receipt().as_receipt().cloned())
            .collect::<Option<Vec<_>>>()
            .ok_or(AlloyChainProviderError::ReceiptsConversion(hash))?;
        Ok(consensus_receipts)
    }

    async fn get_full_block_by_hash(
        &self,
        hash: BlockHash,
    ) -> Result<(BlockInfo, Vec<L1Transaction>), AlloyChainProviderError> {
        let res_block = self.inner.get_block_by_hash(hash).full().await?;
        match res_block {
            Some(block) => {
                let block_info = BlockInfo {
                    hash: block.header.hash(),
                    number: block.header.number,
                    parent_hash: block.header.parent_hash,
                    timestamp: block.header.timestamp,
                };

                let txs = block
                    .transactions
                    .into_transactions()
                    .map(|tx| L1Transaction {
                        hash: tx.block_hash.unwrap_or_default(),
                        from: tx.inner.signer(),
                        to: tx.inner.to(),
                        input: tx.inner.input().to_vec(),
                    })
                    .collect();
                Ok((block_info, txs))
            }
            None => Err(AlloyChainProviderError::BlockNotFound(BlockId::Hash(
                RpcBlockHash::from_hash(hash, None),
            ))),
        }
    }
}
