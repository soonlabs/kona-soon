//! The EIP-1559 update type.

use alloy_primitives::LogData;
use alloy_sol_types::{SolType, sol};

use crate::system::{EIP1559UpdateError, SystemConfig, SystemConfigLog};

/// The EIP-1559 update type.
#[derive(Debug, Default, Clone, Hash, PartialEq, Eq)]
pub struct Eip1559Update {
    /// The EIP-1559 denominator.
    pub eip1559_denominator: u32,
    /// The EIP-1559 elasticity multiplier.
    pub eip1559_elasticity: u32,
}

impl Eip1559Update {
    /// Applies the update to the [`SystemConfig`].
    pub fn apply(&self, config: &mut SystemConfig) {
        config.eip1559_denominator = Some(self.eip1559_denominator);
        config.eip1559_elasticity = Some(self.eip1559_elasticity);
    }
}

impl TryFrom<&SystemConfigLog> for Eip1559Update {
    type Error = EIP1559UpdateError;

    fn try_from(log: &SystemConfigLog) -> Result<Self, Self::Error> {
        let LogData { data, .. } = &log.log.data;
        if data.len() != 96 {
            return Err(EIP1559UpdateError::InvalidDataLen(data.len()));
        }

        let Ok(pointer) = <sol!(uint64)>::abi_decode_validate(&data[0..32]) else {
            return Err(EIP1559UpdateError::PointerDecodingError);
        };
        if pointer != 32 {
            return Err(EIP1559UpdateError::InvalidDataPointer(pointer));
        }

        let Ok(length) = <sol!(uint64)>::abi_decode_validate(&data[32..64]) else {
            return Err(EIP1559UpdateError::LengthDecodingError);
        };
        if length != 32 {
            return Err(EIP1559UpdateError::InvalidDataLength(length));
        }

        let Ok(eip1559_params) = <sol!(uint64)>::abi_decode_validate(&data[64..96]) else {
            return Err(EIP1559UpdateError::EIP1559DecodingError);
        };

        Ok(Self {
            eip1559_denominator: (eip1559_params >> 32) as u32,
            eip1559_elasticity: eip1559_params as u32,
        })
    }
}
