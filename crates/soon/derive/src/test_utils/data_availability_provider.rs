//! An implementation of the [DataAvailabilityProvider] trait for tests.

use crate::{
    errors::PipelineError,
    traits::{AsyncIterator, DataAvailabilityProvider},
    types::PipelineResult,
};
use alloc::{boxed::Box, vec::Vec};
use alloy_primitives::{Address, Bytes};
use async_trait::async_trait;
use core::fmt::Debug;
use soon_primitives::blocks::BlockInfo;

/// Mock data iterator
#[derive(Debug, Default, PartialEq)]
pub struct TestIter {
    /// Holds open data calls with args for assertions.
    pub(crate) open_data_calls: Vec<(BlockInfo, Address)>,
    /// A queue of results to return as the next iterated data.
    pub(crate) results: Vec<PipelineResult<Bytes>>,
}

#[async_trait]
impl AsyncIterator for TestIter {
    type Item = Bytes;

    async fn next(&mut self) -> PipelineResult<Self::Item> {
        self.results.pop().unwrap_or(Err(PipelineError::Eof.temp()))
    }
}

/// Mock data availability provider
#[derive(Debug, Default)]
pub struct TestDAP {
    /// Specifies the stage results.
    pub results: Vec<PipelineResult<Bytes>>,
}

#[async_trait]
impl DataAvailabilityProvider for TestDAP {
    type Item = Bytes;

    async fn next(&mut self, _: &BlockInfo, _: Address) -> PipelineResult<Self::Item> {
        self.results.pop().unwrap_or(Err(PipelineError::Eof.temp()))
    }

    fn clear(&mut self) {
        self.results.clear();
    }
}
