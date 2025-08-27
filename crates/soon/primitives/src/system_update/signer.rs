//! The unsafe block signer update.

use alloy_primitives::{Address, LogData};
use alloy_sol_types::{SolType, sol};

use crate::system::{SystemConfigLog, UnsafeBlockSignerUpdateError};

/// The unsafe block signer update type.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct UnsafeBlockSignerUpdate {
    /// The new unsafe block signer address.
    pub unsafe_block_signer: Address,
}

impl TryFrom<&SystemConfigLog> for UnsafeBlockSignerUpdate {
    type Error = UnsafeBlockSignerUpdateError;

    fn try_from(log: &SystemConfigLog) -> Result<Self, Self::Error> {
        let LogData { data, .. } = &log.log.data;
        if data.len() != 96 {
            return Err(UnsafeBlockSignerUpdateError::InvalidDataLen(data.len()));
        }

        let Ok(pointer) = <sol!(uint64)>::abi_decode_validate(&data[0..32]) else {
            return Err(UnsafeBlockSignerUpdateError::PointerDecodingError);
        };
        if pointer != 32 {
            return Err(UnsafeBlockSignerUpdateError::InvalidDataPointer(pointer));
        }

        let Ok(length) = <sol!(uint64)>::abi_decode_validate(&data[32..64]) else {
            return Err(UnsafeBlockSignerUpdateError::LengthDecodingError);
        };
        if length != 32 {
            return Err(UnsafeBlockSignerUpdateError::InvalidDataLength(length));
        }

        let Ok(unsafe_block_signer) = <sol!(address)>::abi_decode_validate(&data[64..]) else {
            return Err(UnsafeBlockSignerUpdateError::UnsafeBlockSignerAddressDecodingError);
        };

        Ok(Self { unsafe_block_signer })
    }
}
