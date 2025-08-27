//! Test Utilities for chain provider traits

use crate::errors::{PipelineError, PipelineErrorKind};
use crate::traits::{ChainProvider, DAProvider, L2ChainProvider};
use alloc::{sync::Arc, vec::Vec};
use alloy_consensus::{Header, Receipt};
use alloy_eips::BlockNumberOrTag;
use alloy_primitives::{B256, keccak256, map::HashMap};
use async_trait::async_trait;
use solana_sdk::hash::Hash;
use solana_transaction_status::VersionedConfirmedBlock;
use soon_primitives::blocks::{BlockInfo, L1Header, L1Transaction, L2BlockInfo};
use soon_primitives::l2blocks::L2Block;
use soon_primitives::system::SystemConfig;
use std::sync::Mutex;

/// A mock chain provider for testing.
#[derive(Debug, Clone, Default)]
pub struct TestChainProvider {
    /// Maps block numbers to block information using a tuple list.
    pub blocks: Vec<(u64, BlockInfo)>,
    /// Maps block hashes to header information using a tuple list.
    pub headers: Vec<(B256, Header)>,
    /// Maps block hashes to receipts using a tuple list.
    pub receipts: Vec<(B256, Vec<Receipt>)>,
    /// Maps block hashes to transactions using a tuple list.
    pub transactions: Vec<(B256, Vec<L1Transaction>)>,
}

impl TestChainProvider {
    /// Insert a block into the mock chain provider.
    pub fn insert_block(&mut self, number: u64, block: BlockInfo) {
        self.blocks.push((number, block));
    }

    /// Insert a block with transactions into the mock chain provider.
    pub fn insert_block_with_transactions(
        &mut self,
        number: u64,
        block: BlockInfo,
        txs: Vec<L1Transaction>,
    ) {
        self.blocks.push((number, block));
        self.transactions.push((block.hash, txs));
    }

    /// Insert receipts into the mock chain provider.
    pub fn insert_receipts(&mut self, hash: B256, receipts: Vec<Receipt>) {
        self.receipts.push((hash, receipts));
    }

    /// Insert a header into the mock chain provider.
    pub fn insert_header(&mut self, hash: B256, header: Header) {
        self.headers.push((hash, header));
    }
}

/// An error for the [TestChainProvider] and [TestL2ChainProvider].
#[derive(Debug, thiserror::Error)]
pub enum TestProviderError {
    /// The block was not found.
    #[error("Block not found")]
    BlockNotFound,
    /// The header was not found.
    #[error("Header not found")]
    HeaderNotFound,
    /// The receipts were not found.
    #[error("Receipts not found")]
    ReceiptsNotFound,
    /// The L2 block was not found.
    #[error("L2 Block not found")]
    L2BlockNotFound,
    /// The system config was not found.
    #[error("System config not found")]
    SystemConfigNotFound(u64),
}

impl From<TestProviderError> for PipelineErrorKind {
    fn from(val: TestProviderError) -> Self {
        PipelineError::Provider(val.to_string()).temp()
    }
}

#[async_trait]
impl ChainProvider for TestChainProvider {
    type Error = TestProviderError;

    async fn header_by_hash(&self, hash: B256) -> Result<L1Header, Self::Error> {
        if let Some((_, header)) = self.headers.iter().find(|(_, b)| b.hash_slow() == hash) {
            Ok(L1Header {
                hash: header.hash_slow(),
                number: header.number,
                parent_hash: header.parent_hash,
                timestamp: header.timestamp,
                state_root: header.state_root,
                transactions_root: header.transactions_root,
                receipts_root: header.receipts_root,
            })
        } else {
            Err(TestProviderError::HeaderNotFound)
        }
    }

    async fn block_info_by_hash(&self, hash: B256) -> Result<BlockInfo, Self::Error> {
        if let Some((_, header)) = self.headers.iter().find(|(_, b)| b.hash_slow() == hash) {
            Ok(BlockInfo {
                hash: header.hash_slow(),
                number: header.number,
                parent_hash: header.parent_hash,
                timestamp: header.timestamp,
            })
        } else {
            Err(TestProviderError::HeaderNotFound)
        }
    }

    async fn block_info_by_number(
        &self,
        number: BlockNumberOrTag,
    ) -> Result<BlockInfo, Self::Error> {
        if let Some((_, block)) =
            self.blocks.iter().find(|(n, _)| *n == number.as_number().unwrap())
        {
            Ok(*block)
        } else {
            Err(TestProviderError::BlockNotFound)
        }
    }

    async fn receipts_by_hash(&self, _hash: B256) -> Result<Vec<Receipt>, Self::Error> {
        if let Some((_, receipts)) = self.receipts.iter().find(|(h, _)| *h == _hash) {
            Ok(receipts.clone())
        } else {
            Err(TestProviderError::ReceiptsNotFound)
        }
    }

    async fn get_block_transactions_by_hash(
        &self,
        hash: B256,
    ) -> Result<Vec<L1Transaction>, Self::Error> {
        let txs = self
            .transactions
            .iter()
            .find(|(h, _)| *h == hash)
            .map(|(_, txs)| txs.clone())
            .unwrap_or_default();
        Ok(txs)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct VersionedConfirmedBlockWithEntries {
    block: VersionedConfirmedBlock,
    entries: Vec<EntrySummary>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EntrySummary {
    num_hashes: u64,
    hash: Hash,
    num_transactions: u64,
    starting_transaction_index: usize,
}

/// An [L2ChainProvider] implementation for testing.
#[derive(Debug, Default, Clone)]
pub struct TestL2ChainProvider {
    /// Blocks
    pub blocks: Vec<L2BlockInfo>,
    /// Short circuit the block return to be the first block.
    pub short_circuit: bool,
    /// Blocks
    pub op_blocks: Vec<VersionedConfirmedBlockWithEntries>,
    /// System configs
    pub system_configs: HashMap<u64, SystemConfig>,
}

impl TestL2ChainProvider {
    /// Creates a new [MockBlockFetcher] with the given origin and batches.
    pub const fn new(
        blocks: Vec<L2BlockInfo>,
        op_blocks: Vec<VersionedConfirmedBlockWithEntries>,
        system_configs: HashMap<u64, SystemConfig>,
    ) -> Self {
        Self { blocks, short_circuit: false, op_blocks, system_configs }
    }
}

#[async_trait]
impl L2ChainProvider for TestL2ChainProvider {
    type Error = TestProviderError;

    async fn l2_block_info_by_number(&mut self, number: u64) -> Result<L2BlockInfo, Self::Error> {
        if self.short_circuit {
            return self.blocks.first().copied().ok_or_else(|| TestProviderError::BlockNotFound);
        }
        self.blocks
            .iter()
            .find(|b| b.block_info.number == number)
            .cloned()
            .ok_or_else(|| TestProviderError::BlockNotFound)
    }

    async fn block_by_number(&mut self, number: u64) -> Result<L2Block, Self::Error> {
        self.op_blocks
            .iter()
            .find(|p| p.block.parent_slot + 1 == number)
            .cloned()
            .ok_or_else(|| TestProviderError::L2BlockNotFound)
            .map(|block_with_entries| {
                let block_with_entries =
                    solana_transaction_status::VersionedConfirmedBlockWithEntries {
                        block: block_with_entries.block,
                        entries: block_with_entries
                            .entries
                            .into_iter()
                            .map(|entry| solana_transaction_status::EntrySummary {
                                num_hashes: entry.num_hashes,
                                hash: entry.hash,
                                num_transactions: entry.num_transactions,
                                starting_transaction_index: entry.starting_transaction_index,
                            })
                            .collect(),
                    };
                block_with_entries.try_into().expect("must convert")
            })
    }

    async fn system_config_by_number(&mut self, number: u64) -> Result<SystemConfig, Self::Error> {
        self.system_configs
            .get(&number)
            .ok_or_else(|| TestProviderError::SystemConfigNotFound(number))
            .cloned()
    }
}

/// An error for the [TestDAServerProvider]
#[derive(Debug, thiserror::Error)]
pub enum TestDAServerProviderError {
    #[error("empty error")]
    EMPTY,
}

impl From<TestDAServerProviderError> for PipelineErrorKind {
    fn from(val: TestDAServerProviderError) -> Self {
        PipelineError::Provider(val.to_string()).temp()
    }
}

#[derive(Debug, Clone, Default)]
pub struct TestDAServerProvider {
    image_map: Arc<Mutex<HashMap<Vec<u8>, Vec<u8>>>>,
}

#[async_trait]
impl DAProvider for TestDAServerProvider {
    type Error = TestDAServerProviderError;
    async fn set_input(&self, data: Vec<u8>) -> Result<Vec<u8>, Self::Error> {
        let hash = keccak256(data.clone()).to_vec();
        let data_guard = self.image_map.lock();
        let _ = data_guard.unwrap().insert(hash.clone(), data);
        Ok(hash.to_vec())
    }

    async fn get_input(&self, commitment: Vec<u8>) -> Result<Vec<u8>, Self::Error> {
        let data_guard = self.image_map.lock();
        let data = data_guard.unwrap().get(&commitment).unwrap().to_vec();
        Ok(data)
    }
}
