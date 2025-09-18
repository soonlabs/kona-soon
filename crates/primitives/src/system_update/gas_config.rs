//! The gas config update type.

use alloy_primitives::{LogData, U256};
use alloy_sol_types::{SolType, sol};

use crate::system::{GasConfigUpdateError, SystemConfig, SystemConfigLog};

/// The gas config update type.
#[derive(Debug, Default, Clone, Hash, PartialEq, Eq)]
pub struct GasConfigUpdate {
    /// The scalar.
    pub scalar: Option<U256>,
    /// The overhead.
    pub overhead: Option<U256>,
}

impl GasConfigUpdate {
    /// Applies the update to the [`SystemConfig`].
    pub fn apply(&self, config: &mut SystemConfig) {
        if let Some(scalar) = self.scalar {
            config.scalar = scalar;
        }
        if let Some(overhead) = self.overhead {
            config.overhead = overhead;
        }
    }
}

impl TryFrom<&SystemConfigLog> for GasConfigUpdate {
    type Error = GasConfigUpdateError;

    fn try_from(sys_log: &SystemConfigLog) -> Result<Self, Self::Error> {
        let LogData { data, .. } = &sys_log.log.data;
        if data.len() != 128 {
            return Err(GasConfigUpdateError::InvalidDataLen(data.len()));
        }

        let Ok(pointer) = <sol!(uint64)>::abi_decode_validate(&data[0..32]) else {
            return Err(GasConfigUpdateError::PointerDecodingError);
        };
        if pointer != 32 {
            return Err(GasConfigUpdateError::InvalidDataPointer(pointer));
        }

        let Ok(length) = <sol!(uint64)>::abi_decode_validate(&data[32..64]) else {
            return Err(GasConfigUpdateError::LengthDecodingError);
        };
        if length != 64 {
            return Err(GasConfigUpdateError::InvalidDataLength(length));
        }

        let Ok(overhead) = <sol!(uint256)>::abi_decode_validate(&data[64..96]) else {
            return Err(GasConfigUpdateError::OverheadDecodingError);
        };
        let Ok(scalar) = <sol!(uint256)>::abi_decode_validate(&data[96..]) else {
            return Err(GasConfigUpdateError::ScalarDecodingError);
        };

        Ok(Self { scalar: Some(scalar), overhead: Some(overhead) })
    }
}
