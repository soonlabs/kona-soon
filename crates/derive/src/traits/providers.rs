//! Chain providers for the derivation pipeline.

use crate::errors::PipelineErrorKind;
use alloc::{boxed::Box, vec::Vec};
use alloy_consensus::Receipt;
use alloy_eips::BlockNumberOrTag;
use alloy_primitives::B256;
use async_trait::async_trait;
use core::fmt::Display;
use soon_primitives::blocks::{BlockInfo, L1Header, L1Transaction, L2BlockInfo};
use soon_primitives::l2blocks::L2Block;
use soon_primitives::system::SystemConfig;

#[async_trait]
pub trait DAProvider {
    /// The error type for the [DAProvider].
    type Error: Display + Into<PipelineErrorKind>;

    /// submit data to da provider, return data can be used as data id for later fetch operation
    async fn set_input(&self, data: Vec<u8>) -> Result<Vec<u8>, Self::Error>;
    /// fetch data by key from DA provider
    async fn get_input(&self, key: Vec<u8>) -> Result<Vec<u8>, Self::Error>;
}

/// Describes the functionality of a data source that can provide information from the blockchain.
#[async_trait]
pub trait ChainProvider {
    /// The error type for the [ChainProvider].
    type Error: Display + Into<PipelineErrorKind>;

    /// Fetch the L1 [Header] for the given [B256] hash.
    async fn header_by_hash(&self, hash: B256) -> Result<L1Header, Self::Error>;

    /// Fetch the L1 [BlockInfo] for the given [B256] hash.
    async fn block_info_by_hash(&self, hash: B256) -> Result<BlockInfo, Self::Error>;

    /// Returns the block at the given number, or an error if the block does not exist in the data
    /// source.
    async fn block_info_by_number(
        &self,
        number: BlockNumberOrTag,
    ) -> Result<BlockInfo, Self::Error>;

    /// Returns all receipts in the block with the given hash, or an error if the block does not
    /// exist in the data source.
    async fn receipts_by_hash(&self, hash: B256) -> Result<Vec<Receipt>, Self::Error>;

    /// Returns the [BlockInfo] and list of [Transaction]s from the given block hash.
    async fn get_block_transactions_by_hash(
        &self,
        hash: B256,
    ) -> Result<Vec<L1Transaction>, Self::Error>;
}

/// Describes the functionality of a data source that fetches safe blocks.
#[async_trait]
pub trait L2ChainProvider {
    /// The error type for the [L2ChainProvider].
    type Error: Display + Into<PipelineErrorKind>;

    /// Returns the L2 block info given a block number.
    /// Errors if the block does not exist.
    async fn l2_block_info_by_number(&mut self, number: u64) -> Result<L2BlockInfo, Self::Error>;

    /// Returns the block for a given number.
    /// Errors if no block is available for the given block number.
    async fn block_by_number(&mut self, number: u64) -> Result<L2Block, Self::Error>;

    /// Returns the [SystemConfig] by L2 number.
    async fn system_config_by_number(&mut self, number: u64) -> Result<SystemConfig, Self::Error>;
}
