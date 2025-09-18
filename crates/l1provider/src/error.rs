use alloy_eips::BlockId;
use alloy_primitives::B256;
use alloy_transport::{RpcError, TransportErrorKind};
use soon_derive::errors::{PipelineError, PipelineErrorKind};

/// An error for the [AlloyChainProvider].
#[allow(clippy::enum_variant_names)]
#[derive(Debug, thiserror::Error)]
pub enum AlloyChainProviderError {
    /// Transport error
    #[error(transparent)]
    Transport(#[from] RpcError<TransportErrorKind>),
    /// Block not found.
    #[error("Block not found: {0}")]
    BlockNotFound(BlockId),
    /// Failed to convert RPC receipts into consensus receipts.
    #[error("Failed to convert RPC receipts into consensus receipts {0}")]
    ReceiptsConversion(B256),
}

impl From<AlloyChainProviderError> for PipelineErrorKind {
    fn from(e: AlloyChainProviderError) -> Self {
        match e {
            AlloyChainProviderError::Transport(e) => PipelineErrorKind::Temporary(
                PipelineError::Provider(format!("Transport error: {e}")),
            ),
            AlloyChainProviderError::BlockNotFound(id) => PipelineErrorKind::Temporary(
                PipelineError::Provider(format!("L1 Block not found: {id}")),
            ),
            AlloyChainProviderError::ReceiptsConversion(_) => {
                PipelineErrorKind::Temporary(PipelineError::Provider(
                    "Failed to convert RPC receipts into consensus receipts".to_string(),
                ))
            }
        }
    }
}
