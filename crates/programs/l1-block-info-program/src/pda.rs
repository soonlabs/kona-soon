//! Program derived address (PDA) utilities

use solana_program::pubkey::Pubkey;

/// L1 block info seed
const L1_BLOCK_INFO_SEED: &[u8; 13] = b"l1-block-info";

/// L1 block info pubkey
pub fn l1_block_info_pubkey() -> Pubkey {
    l1_block_info_pubkey_and_bump(&crate::ID).0
}

pub(crate) fn l1_block_info_pubkey_and_bump(program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[L1_BLOCK_INFO_SEED], program_id)
}
