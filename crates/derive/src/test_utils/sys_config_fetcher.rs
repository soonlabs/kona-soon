//! Implements a mock [L2SystemConfigFetcher] for testing.

use crate::{
    errors::{PipelineError, PipelineErrorKind},
    traits::L2ChainProvider,
};
use alloc::boxed::Box;
use alloy_primitives::map::HashMap;
use anyhow::Result;
use async_trait::async_trait;
use soon_primitives::{blocks::L2BlockInfo, l2blocks::L2Block, system::SystemConfig};
use thiserror::Error;

/// A mock implementation of the `SystemConfigL2Fetcher` for testing.
#[derive(Debug, Default)]
pub struct TestSystemConfigL2Fetcher {
    /// A map from [u64] block number to a [SystemConfig].
    pub system_configs: HashMap<u64, SystemConfig>,
}

impl TestSystemConfigL2Fetcher {
    /// Inserts a new system config into the mock fetcher with the given block number.
    pub fn insert(&mut self, number: u64, config: SystemConfig) {
        self.system_configs.insert(number, config);
    }

    /// Clears all system configs from the mock fetcher.
    pub fn clear(&mut self) {
        self.system_configs.clear();
    }
}

/// An error returned by the [TestSystemConfigL2Fetcher].
#[derive(Error, Debug, PartialEq, Eq)]
pub enum TestSystemConfigL2FetcherError {
    /// The system config was not found.
    #[error("system config not found: {0}")]
    NotFound(u64),
}

impl From<TestSystemConfigL2FetcherError> for PipelineErrorKind {
    fn from(val: TestSystemConfigL2FetcherError) -> Self {
        PipelineError::Provider(val.to_string()).temp()
    }
}

#[async_trait]
impl L2ChainProvider for TestSystemConfigL2Fetcher {
    type Error = TestSystemConfigL2FetcherError;

    async fn system_config_by_number(&mut self, number: u64) -> Result<SystemConfig, Self::Error> {
        self.system_configs
            .get(&number)
            .cloned()
            .ok_or_else(|| TestSystemConfigL2FetcherError::NotFound(number))
    }

    async fn l2_block_info_by_number(&mut self, _: u64) -> Result<L2BlockInfo, Self::Error> {
        unimplemented!()
    }

    async fn block_by_number(&mut self, _: u64) -> Result<L2Block, Self::Error> {
        unimplemented!()
    }
}
