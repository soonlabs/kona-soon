//! Contains the [`SystemConfig`] type.

use super::{
    CONFIG_UPDATE_TOPIC, SystemConfigLog, SystemConfigUpdateError, SystemConfigUpdateKind,
};
use alloy_consensus::{Eip658Value, Receipt};
use alloy_primitives::{Address, B64, Log, U256};

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

impl SystemConfig {
    /// Filters all L1 receipts to find config updates and applies the config updates.
    pub fn update_with_receipts(
        &mut self,
        receipts: &[Receipt],
        l1_system_config_address: Address,
    ) -> Result<(), SystemConfigUpdateError> {
        for receipt in receipts {
            if Eip658Value::Eip658(false) == receipt.status {
                continue;
            }

            receipt.logs.iter().try_for_each(|log| {
                let topics = log.topics();
                if log.address == l1_system_config_address
                    && !topics.is_empty()
                    && topics[0] == CONFIG_UPDATE_TOPIC
                {
                    // Safety: Error is bubbled up by the trailing `?`
                    self.process_config_update_log(log)?;
                }
                Ok::<(), SystemConfigUpdateError>(())
            })?;
        }
        Ok(())
    }

    /// Returns the eip1559 parameters from a [SystemConfig] encoded as a [B64].
    pub fn eip_1559_params(&self, _parent_timestamp: u64, _next_timestamp: u64) -> Option<B64> {
        Some(B64::ZERO)
    }

    /// Decodes an EVM log entry emitted by the system config contract and applies it as a
    /// [SystemConfig] change.
    ///
    /// Parse log data for:
    ///
    /// ```text
    /// event ConfigUpdate(
    ///    uint256 indexed version,
    ///    UpdateType indexed updateType,
    ///    bytes data
    /// );
    /// ```
    fn process_config_update_log(
        &mut self,
        log: &Log,
    ) -> Result<SystemConfigUpdateKind, SystemConfigUpdateError> {
        // Construct the system config log from the log.
        let log = SystemConfigLog::new(log.clone());

        // Construct the update type from the log.
        let update = log.build()?;

        // Apply the update to the system config.
        update.apply(self);

        // Return the update type.
        Ok(update.kind())
    }
}
