use crate::error::AlloyChainProviderError;
use crate::l1_provider::L1Client;
use alloy::network::primitives::TransactionResponse;
use alloy::providers::{Identity, Provider, ProviderBuilder, RootProvider};
use alloy_consensus::{Receipt, Transaction};
use alloy_eips::{BlockId, BlockNumberOrTag, RpcBlockHash};
use alloy_primitives::Log;
use alloy_primitives::{B256, BlockHash};
use async_trait::async_trait;
use op_alloy_consensus::OpReceiptEnvelope;
use op_alloy_network::Optimism;
use op_alloy_rpc_types::OpTransactionReceipt;
use reqwest::Url;
use soon_primitives::blocks::{BlockInfo, L1Header, L1Transaction};

type HttpProvider = RootProvider<Optimism>;

#[derive(Debug)]
pub struct BaseProviderImpl {
    inner: HttpProvider,
}

impl BaseProviderImpl {
    pub fn new_with_url(l1_url: Url) -> Self {
        let provider =
            ProviderBuilder::<Identity, Identity, Optimism>::default().connect_http(l1_url);

        Self { inner: provider }
    }
}

#[async_trait]
impl L1Client for BaseProviderImpl {
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
            None => Err(AlloyChainProviderError::BlockNotFound(BlockId::Number(number))),
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
            .map(|r: OpTransactionReceipt| convert_base_receipt_consensus_receipt(r))
            .collect::<Vec<_>>();
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
                    hash: block.header.hash,
                    number: block.header.number,
                    parent_hash: block.header.parent_hash,
                    timestamp: block.header.timestamp,
                };

                let txs = block
                    .transactions
                    .into_transactions()
                    .map(|tx| L1Transaction {
                        hash: tx.inner.tx_hash(),
                        from: tx.inner.from(),
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

fn convert_base_receipt_consensus_receipt(op_receipt: OpTransactionReceipt) -> Receipt {
    let receipt_envelope: OpReceiptEnvelope<Log> = op_receipt.into();
    receipt_envelope.as_receipt().cloned().unwrap()
}
