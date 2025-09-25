use bridge::instruction::BridgeInstruction;
use solana_program::{instruction::CompiledInstruction, pubkey::Pubkey};
use solana_sdk::transaction::{SanitizedTransaction, Transaction, VersionedTransaction};

/// Trait to check if a transaction has native instructions
pub trait HasNativeInstruction {
    /// Check if the transaction has l1_block_info related instructions, including normal transfer
    /// transactions interacting with l1_block_info
    fn has_update_l1_block_info_instruction(&self) -> bool;

    /// Check if the transaction has forbidden bridge instruction
    fn has_derived_bridge_instruction(&self) -> bool;

    /// Check if the transaction has native instructions, including bridge instructions and
    /// l1_block_info instructions
    fn has_native_instruction(&self) -> bool {
        self.has_update_l1_block_info_instruction() || self.has_derived_bridge_instruction()
    }
}

impl HasNativeInstruction for Transaction {
    fn has_update_l1_block_info_instruction(&self) -> bool {
        // never allow user to create a transaction with l1_block_info related instructions
        self.message().program_ids().contains(&&l1_block_info::id())
    }

    fn has_derived_bridge_instruction(&self) -> bool {
        let message = self.message();
        message.instructions.iter().any(|ix| {
            message.account_keys[ix.program_id_index as usize] == bridge::id()
                && BridgeInstruction::unpack(&ix.data).map(|ix| ix.is_derived()).unwrap_or(false)
        })
    }
}

impl HasNativeInstruction for SanitizedTransaction {
    fn has_update_l1_block_info_instruction(&self) -> bool {
        // never allow user to create a transaction with l1_block_info related instructions
        self.message()
            .program_instructions_iter()
            .any(|(program, _)| program == &l1_block_info::id())
    }

    fn has_derived_bridge_instruction(&self) -> bool {
        self.message().program_instructions_iter().any(|(program, ix)| {
            program == &bridge::id()
                && BridgeInstruction::unpack(&ix.data).map(|ix| ix.is_derived()).unwrap_or(false)
        })
    }
}

impl HasNativeInstruction for VersionedTransaction {
    fn has_update_l1_block_info_instruction(&self) -> bool {
        // never allow user to create a transaction with l1_block_info related instructions
        self.message.instructions().iter().any(|ix| {
            self.message.static_account_keys()[ix.program_id_index as usize] == l1_block_info::id()
        })
    }

    fn has_derived_bridge_instruction(&self) -> bool {
        self.message.instructions().iter().any(|ix| {
            self.message.static_account_keys()[ix.program_id_index as usize] == bridge::id()
                && BridgeInstruction::unpack(&ix.data).map(|ix| ix.is_derived()).unwrap_or(false)
        })
    }
}

pub fn get_instruction_data_array<'a, 'b>(
    program_id: Pubkey,
    instructions: impl Iterator<Item = &'b CompiledInstruction>,
    account_index_getter: impl Fn(u8) -> Option<&'a Pubkey>,
) -> Vec<Vec<u8>> {
    instructions
        .filter_map(|ix| {
            let id = account_index_getter(ix.program_id_index);
            let data = id.map(|id| if id == &program_id { Some(ix.data.clone()) } else { None });
            data.flatten()
        })
        .collect()
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{error::NativeTransactionError, native_tx::new_native_transaction};
    use alloy_primitives::B256;
    use anyhow::Result;
    use bridge::instruction::{deposit_erc20, deposit_eth, withdraw_spl};
    use l1_block_info::instruction::update_l1_block_info;
    use solana_program::{
        instruction::{AccountMeta, Instruction},
        pubkey::Pubkey,
    };
    use solana_sdk::{
        reserved_account_keys::ReservedAccountKeys,
        signature::{Keypair, Signer},
        system_transaction,
    };

    fn mock_update_l1_block_info_raw() -> Result<Transaction, NativeTransactionError> {
        let instruction = update_l1_block_info(
            Default::default(),
            Default::default(),
            Default::default(),
            Default::default(),
            Default::default(),
            Default::default(),
            Default::default(),
            Default::default(),
            Default::default(),
            Default::default(),
        );
        new_native_transaction(&[instruction], B256::random())
    }

    fn mock_deposit_eth(data_valid: bool) -> Result<Transaction, NativeTransactionError> {
        let instruction = if data_valid {
            deposit_eth(
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                Pubkey::new_unique(),
                Default::default(),
                Default::default(),
            )
        } else {
            Instruction {
                program_id: bridge::id(),
                accounts: vec![
                    AccountMeta::new_readonly(solana_program::system_program::id(), false),
                    AccountMeta::new(Default::default(), false),
                    AccountMeta::new(Default::default(), false),
                ],
                data: vec![],
            }
        };
        new_native_transaction(&[instruction], B256::random())
    }

    fn mock_deposit_erc20(data_valid: bool) -> Result<Transaction, NativeTransactionError> {
        let instruction = if data_valid {
            deposit_erc20(
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                10000,
            )
        } else {
            Instruction { program_id: bridge::id(), accounts: vec![], data: vec![] }
        };
        new_native_transaction(&[instruction], B256::random())
    }

    fn check_non_native_instruction(tx: Transaction) -> Result<()> {
        assert!(!tx.has_native_instruction());
        let tx = SanitizedTransaction::try_from_legacy_transaction(
            tx,
            &ReservedAccountKeys::empty_key_set(),
        )?;
        assert!(!tx.has_native_instruction());

        Ok(())
    }

    #[test]
    fn test_common_transfer() -> Result<()> {
        let tx = system_transaction::transfer(
            &Keypair::new(),
            &Keypair::new().pubkey(),
            1000,
            Default::default(),
        );
        check_non_native_instruction(tx)?;

        // transfer to l1 block info is allowed
        let tx = system_transaction::transfer(
            &Keypair::new(),
            &l1_block_info::id(),
            1000,
            Default::default(),
        );
        check_non_native_instruction(tx)?;

        // transfer to bridge is allowed
        let tx =
            system_transaction::transfer(&Keypair::new(), &bridge::id(), 1000, Default::default());
        check_non_native_instruction(tx)?;

        Ok(())
    }

    #[test]
    fn test_update_l1_block_info_instruction() -> Result<()> {
        // update l1 block info is not allowed
        let tx = mock_update_l1_block_info_raw()?;
        assert!(tx.has_update_l1_block_info_instruction());
        assert!(tx.has_native_instruction());
        let tx = SanitizedTransaction::try_from_legacy_transaction(
            tx,
            &ReservedAccountKeys::empty_key_set(),
        )?;
        assert!(tx.has_update_l1_block_info_instruction());
        assert!(tx.has_native_instruction());

        Ok(())
    }

    #[test]
    fn test_bride_derived_instructions() -> Result<()> {
        // test deposit_eth instruction
        let tx = mock_deposit_eth(true)?;
        assert!(tx.has_native_instruction());
        assert!(tx.has_derived_bridge_instruction());
        let tx = SanitizedTransaction::try_from_legacy_transaction(
            tx,
            &ReservedAccountKeys::empty_key_set(),
        )?;
        assert!(tx.has_native_instruction());
        assert!(tx.has_derived_bridge_instruction());

        // test deposit_eth instruction with invalid data
        // we allow this pass since it's will be rejected by the bridge program)
        let tx = mock_deposit_eth(false)?;
        assert!(!tx.has_native_instruction());
        assert!(!tx.has_derived_bridge_instruction());
        let tx = SanitizedTransaction::try_from_legacy_transaction(
            tx,
            &ReservedAccountKeys::empty_key_set(),
        )?;
        assert!(!tx.has_native_instruction());
        assert!(!tx.has_derived_bridge_instruction());

        // test deposit_er20 instruction
        let tx = mock_deposit_erc20(true)?;
        assert!(tx.has_native_instruction());
        assert!(tx.has_derived_bridge_instruction());
        let tx = SanitizedTransaction::try_from_legacy_transaction(
            tx,
            &ReservedAccountKeys::empty_key_set(),
        )?;
        assert!(tx.has_native_instruction());
        assert!(tx.has_derived_bridge_instruction());

        // test deposit_erc20 instruction with invalid data
        // we allow this pass since it's will be rejected by the bridge program)
        let tx = mock_deposit_erc20(false)?;
        assert!(!tx.has_native_instruction());
        assert!(!tx.has_derived_bridge_instruction());
        let tx = SanitizedTransaction::try_from_legacy_transaction(
            tx,
            &ReservedAccountKeys::empty_key_set(),
        )?;
        assert!(!tx.has_native_instruction());
        assert!(!tx.has_derived_bridge_instruction());

        Ok(())
    }

    #[test]
    fn test_bridge_non_derived_instructions() -> Result<()> {
        let payer = Keypair::new();

        // test `withdraw_eth` instruction
        let instruction =
            bridge::instruction::withdraw_eth(Default::default(), payer.pubkey(), 1000, 1000, 0);
        let tx = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&payer.pubkey()),
            &[&payer],
            Default::default(),
        );
        check_non_native_instruction(tx)?;

        // test `withdraw_spl` instruction
        let instruction =
            withdraw_spl(Default::default(), payer.pubkey(), Default::default(), 1000, 1000, 0);
        let tx = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&payer.pubkey()),
            &[&payer],
            Default::default(),
        );
        check_non_native_instruction(tx)?;

        // test `create_spl` instruction
        let instruction = bridge::instruction::create_spl(
            Default::default(),
            payer.pubkey(),
            "Test Token",
            "TT",
            "https://ipfs.io/ipfs/QmXRVXSRbH9nKYPgVfakXRhDhEaXWs6QYu3rToadXhtHPr",
            9,
        )?;
        let tx = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&payer.pubkey()),
            &[&payer],
            Default::default(),
        );
        check_non_native_instruction(tx)?;

        Ok(())
    }
}
