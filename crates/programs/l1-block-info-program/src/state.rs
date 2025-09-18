//! State transition types

use solana_program::{
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack, Sealed},
};
use std::convert::TryInto;

/// L1 deposit info
#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct L1BlockInfo {
    /// The L1 epoch block number
    pub number: u64,
    /// The L1 epoch block timestamp
    pub timestamp: u64,
    /// The L1 epoch base fee
    pub base_fee: u128,
    /// The L1 epoch block hash
    pub hash: [u8; 32],
    /// The L2 block's position in the epoch
    pub sequence_number: u64,
    /// A versioned hash of the current authorized batcher sender.
    pub batcher_hash: [u8; 32],
    /// The current L1 fee overhead to apply to L2 transactions cost computation. Unused after Ecotone hard fork.
    pub fee_overhead: u128,
    /// The current L1 fee scalar to apply to L2 transactions cost computation. Unused after Ecotone hard fork.
    pub fee_scalar: u128,
    /// Gas limit: 1_000_000 if post-Regolith, otherwise 150_000_000
    pub gas: u64,
    /// False if post-Regolith, otherwise true
    pub is_system_tx: bool,
}

impl Sealed for L1BlockInfo {}

impl IsInitialized for L1BlockInfo {
    fn is_initialized(&self) -> bool {
        true
    }
}

impl Pack for L1BlockInfo {
    const LEN: usize = 8 + 8 + 16 + 32 + 8 + 32 + 16 + 16 + 8 + 1;

    fn pack_into_slice(&self, target: &mut [u8]) {
        target[0..8].copy_from_slice(&self.number.to_be_bytes());
        target[8..16].copy_from_slice(&self.timestamp.to_be_bytes());
        target[16..32].copy_from_slice(&self.base_fee.to_be_bytes());
        target[32..64].copy_from_slice(&self.hash);
        target[64..72].copy_from_slice(&self.sequence_number.to_be_bytes());
        target[72..104].copy_from_slice(&self.batcher_hash);
        target[104..120].copy_from_slice(&self.fee_overhead.to_be_bytes());
        target[120..136].copy_from_slice(&self.fee_scalar.to_be_bytes());
        target[136..144].copy_from_slice(&self.gas.to_be_bytes());
        target[144] = match self.is_system_tx {
            false => 0,
            true => 1,
        };
    }

    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        let number =
            u64::from_be_bytes(src[..8].try_into().map_err(|_| ProgramError::InvalidAccountData)?);
        let timestamp = u64::from_be_bytes(
            src[8..16].try_into().map_err(|_| ProgramError::InvalidAccountData)?,
        );
        let base_fee = u128::from_be_bytes(
            src[16..32].try_into().map_err(|_| ProgramError::InvalidAccountData)?,
        );
        let hash = src[32..64].try_into().map_err(|_| ProgramError::InvalidAccountData)?;
        let sequence_number = u64::from_be_bytes(
            src[64..72].try_into().map_err(|_| ProgramError::InvalidAccountData)?,
        );
        let batcher_hash = src[72..104].try_into().map_err(|_| ProgramError::InvalidAccountData)?;
        let fee_overhead = u128::from_be_bytes(
            src[104..120].try_into().map_err(|_| ProgramError::InvalidAccountData)?,
        );
        let fee_scalar = u128::from_be_bytes(
            src[120..136].try_into().map_err(|_| ProgramError::InvalidAccountData)?,
        );
        let gas = u64::from_be_bytes(
            src[136..144].try_into().map_err(|_| ProgramError::InvalidAccountData)?,
        );
        let is_system_tx = match src[144] {
            0 => false,
            1 => true,
            _ => return Err(ProgramError::InvalidAccountData),
        };

        Ok(Self {
            number,
            timestamp,
            base_fee,
            hash,
            sequence_number,
            batcher_hash,
            fee_overhead,
            fee_scalar,
            gas,
            is_system_tx,
        })
    }
}
