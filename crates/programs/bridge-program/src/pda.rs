//! Program derived address (PDA) utilities

use ethabi::Address;
use mpl_token_metadata::accounts::Metadata;
use solana_program::pubkey::Pubkey;

const VAULT_SEED: &[u8; 5] = b"vault";
const USER_WITHDRAWAL_COUNTER_SEED: &[u8; 20] = b"svm-withdraw-counter";
const BRIDGE_CONFIG_SEED: &[u8; 13] = b"bridge-config";
const SPL_OWNER_SEED: &[u8; 9] = b"spl-owner";
const SPL_MINT_SEED: &[u8; 8] = b"spl-mint";
const BRIDGE_OWNER_SEED: &[u8; 12] = b"bridge-owner";

/// Bridge vault pubkey
pub fn vault_pubkey() -> Pubkey {
    vault_pubkey_and_bump(&crate::ID).0
}

pub(crate) fn vault_signer_seeds(bump: &[u8]) -> [&[u8]; 2] {
    [VAULT_SEED, bump]
}

pub(crate) fn vault_pubkey_and_bump(program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[VAULT_SEED], program_id)
}

/// User withdrawal counter pubkey
pub fn user_withdrawal_counter_pubkey(user_key: &Pubkey) -> Pubkey {
    user_withdrawal_counter_pubkey_and_bump(&crate::ID, user_key).0
}

pub(crate) fn user_withdrawal_counter_pubkey_and_bump(
    program_id: &Pubkey,
    user_key: &Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[USER_WITHDRAWAL_COUNTER_SEED, user_key.as_ref()], program_id)
}

pub(crate) fn user_withdrawal_counter_seeds<'a>(
    user_key: &'a Pubkey,
    bump: &'a [u8],
) -> [&'a [u8]; 3] {
    [USER_WITHDRAWAL_COUNTER_SEED, user_key.as_ref(), bump]
}

/// Withdrawal transaction pubkey
pub fn withdrawal_transaction_pubkey(user_key: &Pubkey, counter: u64) -> Pubkey {
    withdrawal_transaction_pubkey_and_bump(user_key, counter, &crate::ID).0
}

pub(crate) fn withdrawal_transaction_signer_seeds<'a>(
    user_key: &'a Pubkey,
    counter_seed: &'a [u8],
    bump: &'a [u8],
) -> [&'a [u8]; 3] {
    [user_key.as_ref(), counter_seed, bump]
}

pub(crate) fn withdrawal_transaction_pubkey_and_bump(
    user_key: &Pubkey,
    counter: u64,
    program_id: &Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[user_key.as_ref(), &counter.to_le_bytes()], program_id)
}

/// Bridge config pubkey
pub fn config_pubkey() -> Pubkey {
    config_pubkey_and_bump(&crate::ID).0
}

pub(crate) fn config_pubkey_and_bump(program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[BRIDGE_CONFIG_SEED], program_id)
}

/// SPL token owner pubkey
pub fn spl_token_owner_pubkey(remote_token: &Address) -> Pubkey {
    spl_token_owner_pubkey_and_bump(remote_token, &crate::ID).0
}

pub(crate) fn spl_token_owner_signer_seeds<'a>(
    remote_token: &'a Address,
    bump: &'a [u8],
) -> [&'a [u8]; 3] {
    [SPL_OWNER_SEED, remote_token.as_ref(), bump]
}

pub(crate) fn spl_token_owner_pubkey_and_bump(
    remote_token: &Address,
    program_id: &Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[SPL_OWNER_SEED, remote_token.as_ref()], program_id)
}

/// SPL token mint pubkey
pub fn spl_token_mint_pubkey(remote_token: &Address) -> Pubkey {
    spl_token_mint_pubkey_and_bump(remote_token, &crate::ID).0
}

/// SPL token metadata pubkey
pub fn spl_token_metadata_pubkey(remote_token: &Address) -> Pubkey {
    let mint_key = spl_token_mint_pubkey_and_bump(remote_token, &crate::ID).0;
    Metadata::find_pda(&mint_key).0
}

pub(crate) fn spl_token_mint_signer_seeds<'a>(
    remote_token: &'a Address,
    bump: &'a [u8],
) -> [&'a [u8]; 3] {
    [SPL_MINT_SEED, remote_token.as_ref(), bump]
}

pub(crate) fn spl_token_mint_pubkey_and_bump(
    remote_token: &Address,
    program_id: &Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[SPL_MINT_SEED, remote_token.as_ref()], program_id)
}

/// Bridge owner pubkey
pub fn bridge_owner_pubkey() -> Pubkey {
    bridge_owner_pubkey_and_bump(&crate::ID).0
}

pub(crate) fn bridge_owner_pubkey_and_bump(program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[BRIDGE_OWNER_SEED], program_id)
}
