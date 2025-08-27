mod instruction;

use crate::error::NativeTransactionError;
use alloy_primitives::B256;
use solana_program::hash::Hash;
use solana_program::instruction::Instruction;
use solana_program::message::Message;
use solana_program::pubkey::Pubkey;
use solana_sdk::pubkey;
use solana_sdk::signature::{SIGNATURE_BYTES, Signature};
use solana_sdk::transaction::Transaction;

pub use instruction::HasNativeInstruction;
pub use instruction::get_instruction_data_array;

pub const NO_SIG_TX_PAYER: Pubkey = pubkey!("NoSigTxPayer1111111111111111111111111111111");

pub fn new_native_transaction(
    instructions: &[Instruction],
    source_hash: B256,
) -> Result<Transaction, NativeTransactionError> {
    // use `source_hash` as latest blockhash
    let blockhash = Hash::new_from_array(source_hash.0);
    let message = Message::new_with_blockhash(instructions, Some(&NO_SIG_TX_PAYER), &blockhash);
    let mut tx = Transaction::new_unsigned(message);
    // // should have none signatures in origin instructions
    // if tx.signatures.len() != 1 {
    //     return Err(NativeTransactionError::NotSupportSignatures);
    // }
    // derive signature from blockhash
    tx.signatures[0] = hash_to_signature(&blockhash);
    Ok(tx)
}

fn hash_to_signature(hash: &Hash) -> Signature {
    let mut sig = [0u8; SIGNATURE_BYTES];
    sig[0..32].copy_from_slice(hash.as_ref());
    sig.into()
}
