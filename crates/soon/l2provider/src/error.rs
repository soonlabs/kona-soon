use soon_derive::errors::{PipelineError, PipelineErrorKind};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum L2ChainProviderError {
    #[error("fetch l2 block with entries failed: {0}")]
    FetchBlockWithEntriesFailed(String),
    #[error("fetch l2 block info failed: {0}")]
    FetchBlockInfoFailed(String),
    #[error("fetch l2 system config failed: {0}")]
    FetchSystemConfigFailed(String),
    #[error("fetch output at block failed: {0}")]
    FetchOutputAtBlockFailed(String),
    #[error("fetch tried account at block failed: {0}")]
    FetchTriedAccountFailed(String),
}

impl From<L2ChainProviderError> for PipelineErrorKind {
    fn from(e: L2ChainProviderError) -> Self {
        match e {
            L2ChainProviderError::FetchBlockWithEntriesFailed(e) => {
                PipelineErrorKind::Temporary(PipelineError::Provider(e.to_string()))
            }
            L2ChainProviderError::FetchBlockInfoFailed(e) => {
                PipelineErrorKind::Temporary(PipelineError::Provider(e.to_string()))
            }
            L2ChainProviderError::FetchSystemConfigFailed(e) => {
                PipelineErrorKind::Temporary(PipelineError::Provider(e.to_string()))
            }
            L2ChainProviderError::FetchOutputAtBlockFailed(e) => {
                PipelineErrorKind::Temporary(PipelineError::Provider(e.to_string()))
            }
            L2ChainProviderError::FetchTriedAccountFailed(e) => {
                PipelineErrorKind::Temporary(PipelineError::Provider(e.to_string()))
            }
        }
    }
}
