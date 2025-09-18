//! The gas limit update type.

use alloy_primitives::{LogData, U64, U256};
use alloy_sol_types::{SolType, sol};

use crate::system::{GasLimitUpdateError, SystemConfig, SystemConfigLog};

/// The gas limit update type.
#[derive(Debug, Default, Clone, Hash, PartialEq, Eq)]
pub struct GasLimitUpdate {
    /// The gas limit.
    pub gas_limit: u64,
}

impl GasLimitUpdate {
    /// Applies the update to the [`SystemConfig`].
    pub fn apply(&self, config: &mut SystemConfig) {
        config.gas_limit = self.gas_limit;
    }
}

impl TryFrom<&SystemConfigLog> for GasLimitUpdate {
    type Error = GasLimitUpdateError;

    fn try_from(log: &SystemConfigLog) -> Result<Self, Self::Error> {
        let LogData { data, .. } = &log.log.data;
        if data.len() != 96 {
            return Err(GasLimitUpdateError::InvalidDataLen(data.len()));
        }

        let Ok(pointer) = <sol!(uint64)>::abi_decode_validate(&data[0..32]) else {
            return Err(GasLimitUpdateError::PointerDecodingError);
        };
        if pointer != 32 {
            return Err(GasLimitUpdateError::InvalidDataPointer(pointer));
        }

        let Ok(length) = <sol!(uint64)>::abi_decode_validate(&data[32..64]) else {
            return Err(GasLimitUpdateError::LengthDecodingError);
        };
        if length != 32 {
            return Err(GasLimitUpdateError::InvalidDataLength(length));
        }

        let Ok(gas_limit) = <sol!(uint256)>::abi_decode_validate(&data[64..]) else {
            return Err(GasLimitUpdateError::GasLimitDecodingError);
        };

        // Prevent overflows here.
        let max = U256::from(u64::MAX as u128);
        if gas_limit > max {
            return Err(GasLimitUpdateError::GasLimitDecodingError);
        }

        Ok(Self { gas_limit: U64::from(gas_limit).saturating_to::<u64>() })
    }
}
