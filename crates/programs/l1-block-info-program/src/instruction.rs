//! Instruction types

use crate::pda::l1_block_info_pubkey;
use {
    crate::error::L1BlockInfoError,
    solana_program::{
        instruction::{AccountMeta, Instruction},
        program_error::ProgramError,
        pubkey::Pubkey,
    },
    std::convert::TryInto,
};

const U64_BYTES: usize = 8;
const U128_BYTES: usize = 16;

/// Instructions supported by the l1 block info program.
#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub enum L1BlockInfoInstruction {
    /// Create l1 block info account.
    ///
    /// Accounts expected by this instruction:
    ///
    ///   0. `[]` System program.
    ///   1. `[]` Rent sysvar.
    ///   2. `[writable]` L1 block info account to be created.
    ///   3. `[signer]` Payer.
    #[deprecated(note = "This instruction is no longer used")]
    CreateL1BlockInfoAccount,

    /// Update l1 block info.
    ///
    /// Accounts expected by this instruction:
    ///
    ///   0. `[writable]` Deposit info account.
    UpdateL1BlockInfo {
        /// The L1 epoch block number
        number: u64,
        /// The L1 epoch block timestamp
        timestamp: u64,
        /// The L1 epoch base fee
        base_fee: u128,
        /// The L1 epoch block hash
        hash: [u8; 32],
        /// The L2 block's position in the epoch
        sequence_number: u64,
        /// A versioned hash of the current authorized batcher sender.
        batcher_hash: [u8; 32],
        /// The current L1 fee overhead to apply to L2 transactions cost computation. Unused after Ecotone hard fork.
        fee_overhead: u128,
        /// The current L1 fee scalar to apply to L2 transactions cost computation. Unused after Ecotone hard fork.
        fee_scalar: u128,
        /// Gas limit: 1_000_000 if post-Regolith, otherwise 150_000_000
        gas: u64,
        /// False if post-Regolith, otherwise true
        is_system_tx: bool,
    },
}

impl L1BlockInfoInstruction {
    /// Unpacks a byte buffer into a SvmWithdrawBridgeInstruction
    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        use L1BlockInfoError::InvalidInstruction;

        let (&tag, rest) = input.split_first().ok_or(InvalidInstruction)?;
        Ok(match tag {
            #[allow(deprecated)]
            0 => Self::CreateL1BlockInfoAccount,
            1 => {
                let (number, rest) = Self::unpack_u64(rest)?;
                let (timestamp, rest) = Self::unpack_u64(rest)?;
                let (base_fee, rest) = Self::unpack_u128(rest)?;
                let (hash, rest) = Self::unpack_bytes32(rest)?;
                let (sequence_number, rest) = Self::unpack_u64(rest)?;
                let (batcher_hash, rest) = Self::unpack_bytes32(rest)?;
                let (fee_overhead, rest) = Self::unpack_u128(rest)?;
                let (fee_scalar, rest) = Self::unpack_u128(rest)?;
                let (gas, rest) = Self::unpack_u64(rest)?;
                let (is_system_tx, _rest) = Self::unpack_bool(rest)?;
                Self::UpdateL1BlockInfo {
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
                }
            }

            _ => return Err(InvalidInstruction.into()),
        })
    }

    fn unpack_bytes32(input: &[u8]) -> Result<([u8; 32], &[u8]), ProgramError> {
        if input.len() >= 32 {
            let (data, rest) = input.split_at(32);
            let address: [u8; 32] =
                <[u8; 32]>::try_from(data).map_err(|_| L1BlockInfoError::InvalidInstruction)?;
            Ok((address, rest))
        } else {
            Err(L1BlockInfoError::InvalidInstruction.into())
        }
    }

    fn unpack_u128(input: &[u8]) -> Result<(u128, &[u8]), ProgramError> {
        let value = input
            .get(..U128_BYTES)
            .and_then(|slice| slice.try_into().ok())
            .map(u128::from_le_bytes)
            .ok_or(L1BlockInfoError::InvalidInstruction)?;
        Ok((value, &input[U128_BYTES..]))
    }

    fn unpack_u64(input: &[u8]) -> Result<(u64, &[u8]), ProgramError> {
        let value = input
            .get(..U64_BYTES)
            .and_then(|slice| slice.try_into().ok())
            .map(u64::from_le_bytes)
            .ok_or(L1BlockInfoError::InvalidInstruction)?;
        Ok((value, &input[U64_BYTES..]))
    }

    fn unpack_bool(input: &[u8]) -> Result<(bool, &[u8]), ProgramError> {
        let byte_value = input.first().ok_or(L1BlockInfoError::InvalidInstruction)?;
        let value = match *byte_value {
            0 => false,
            1 => true,
            _ => return Err(L1BlockInfoError::InvalidInstruction.into()),
        };
        Ok((value, &input[1..]))
    }

    fn pack(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(std::mem::size_of::<Self>());
        match self {
            #[allow(deprecated)]
            Self::CreateL1BlockInfoAccount => {
                buf.push(0);
            }
            Self::UpdateL1BlockInfo {
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
            } => {
                buf.push(1);
                buf.extend_from_slice(&number.to_le_bytes());
                buf.extend_from_slice(&timestamp.to_le_bytes());
                buf.extend_from_slice(&base_fee.to_le_bytes());
                buf.extend_from_slice(hash);
                buf.extend_from_slice(&sequence_number.to_le_bytes());
                buf.extend_from_slice(batcher_hash);
                buf.extend_from_slice(&fee_overhead.to_le_bytes());
                buf.extend_from_slice(&fee_scalar.to_le_bytes());
                buf.extend_from_slice(&gas.to_le_bytes());
                buf.push(*is_system_tx as u8);
            }
        }
        buf
    }
}

/// Creates a `CreateL1BlockInfoAccount` instruction.
#[deprecated(note = "This instruction is no longer used")]
pub fn create_l1_block_info_account(payer: Pubkey) -> Instruction {
    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new_readonly(solana_program::system_program::ID, false),
            AccountMeta::new_readonly(solana_program::sysvar::rent::ID, false),
            AccountMeta::new(l1_block_info_pubkey(), false),
            AccountMeta::new(payer, true),
        ],
        #[allow(deprecated)]
        data: L1BlockInfoInstruction::CreateL1BlockInfoAccount.pack(),
    }
}

/// Creates a `UpdateL1BlockInfo` instruction.
#[allow(clippy::too_many_arguments)]
pub fn update_l1_block_info(
    number: u64,
    timestamp: u64,
    base_fee: u128,
    hash: [u8; 32],
    sequence_number: u64,
    batcher_hash: [u8; 32],
    fee_overhead: u128,
    fee_scalar: u128,
    gas: u64,
    is_system_tx: bool,
) -> Instruction {
    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new(l1_block_info_pubkey(), false),
            AccountMeta::new_readonly(crate::NO_SIG_TX_PAYER, true),
        ],
        data: L1BlockInfoInstruction::UpdateL1BlockInfo {
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
        }
        .pack(),
    }
}
