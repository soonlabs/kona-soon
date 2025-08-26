use alloy_primitives::{B256, keccak256};
use kona_mpt_primitives::account::{TrieSolanaAccount as MptAccount, account_from_solana_native};
use solana_sdk::account::AccountSharedData;
use solana_sdk::pubkey::Pubkey;

pub(crate) fn add_trie_account(
    target: &mut Vec<(B256, MptAccount)>,
    pubkey: &Pubkey,
    account: &AccountSharedData,
) {
    let hashed_pubkey = keccak256(pubkey);
    let mpt_account = account_from_solana_native(account);
    target.push((hashed_pubkey, mpt_account));
}
