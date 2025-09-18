//! Program state processor

use crate::{
    error::L1BlockInfoError, instruction::L1BlockInfoInstruction,
    pda::l1_block_info_pubkey_and_bump, state::L1BlockInfo,
};
use arrayref::array_ref;
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    program_memory::sol_memcmp,
    program_pack::Pack,
    pubkey::{Pubkey, PUBKEY_BYTES},
};

/// Program state handler.
pub struct Processor {}
impl Processor {
    #[deprecated(note = "This instruction is no longer used")]
    fn create_l1_block_info_account(
        _program_id: &Pubkey,
        _accounts: &[AccountInfo],
    ) -> ProgramResult {
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn update_l1_block_info(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
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
    ) -> ProgramResult {
        const NUM_FIXED: usize = 2;
        let accounts = array_ref![accounts, 0, NUM_FIXED];
        let [
            l1_block_info_account,      // write
            no_sig_tx_payer_account,    // read,signer
        ] = accounts;

        if !Self::cmp_pubkeys(no_sig_tx_payer_account.key, &crate::NO_SIG_TX_PAYER) ||
            !no_sig_tx_payer_account.is_signer
        {
            return Err(L1BlockInfoError::InvalidNoSigTxPayer.into());
        }

        let (l1_block_info_key, _) = l1_block_info_pubkey_and_bump(program_id);
        if !Self::cmp_pubkeys(&l1_block_info_key, l1_block_info_account.key) {
            return Err(L1BlockInfoError::InvalidL1BlockInfoAccount.into());
        }

        // Store deposit info
        let l1_block_info = L1BlockInfo {
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
        };
        L1BlockInfo::pack(l1_block_info, &mut l1_block_info_account.data.borrow_mut())?;

        Ok(())
    }

    /// Processes an [Instruction](enum.Instruction.html).
    pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], input: &[u8]) -> ProgramResult {
        let instruction = L1BlockInfoInstruction::unpack(input)?;

        match instruction {
            #[allow(deprecated)]
            L1BlockInfoInstruction::CreateL1BlockInfoAccount => {
                msg!("Deprecated Instruction: CreateL1BlockInfoAccount");
                Self::create_l1_block_info_account(program_id, accounts)
            }
            L1BlockInfoInstruction::UpdateL1BlockInfo {
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
                msg!("Instruction: UpdateL1BlockInfo");
                Self::update_l1_block_info(
                    program_id,
                    accounts,
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
                )
            }
        }
    }

    /// Checks two pubkeys for equality in a computationally cheap way using `sol_memcmp`
    pub fn cmp_pubkeys(a: &Pubkey, b: &Pubkey) -> bool {
        sol_memcmp(a.as_ref(), b.as_ref(), PUBKEY_BYTES) == 0
    }
}
