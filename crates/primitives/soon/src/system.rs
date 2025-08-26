//! Contains the [`SystemConfig`] type.

use alloy_primitives::{Address, U256};

/// System configuration.
#[derive(
    Debug, Copy, Clone, Default, Hash, Eq, PartialEq, serde::Serialize, serde::Deserialize,
)]
pub struct SystemConfig {
    /// Batcher address
    pub batcher_address: Address,
    /// Fee overhead value
    pub overhead: U256,
    /// Fee scalar value
    pub scalar: U256,
    /// Gas limit value
    pub gas_limit: u64,
    /// Base fee scalar value
    pub base_fee_scalar: Option<u64>,
    /// Blob base fee scalar value
    pub blob_base_fee_scalar: Option<u64>,
    /// EIP-1559 denominator
    pub eip1559_denominator: Option<u32>,
    /// EIP-1559 elasticity
    pub eip1559_elasticity: Option<u32>,
    /// The operator fee scalar (isthmus hardfork)
    pub operator_fee_scalar: Option<u32>,
    /// The operator fee constant (isthmus hardfork)
    pub operator_fee_constant: Option<u64>,
}
