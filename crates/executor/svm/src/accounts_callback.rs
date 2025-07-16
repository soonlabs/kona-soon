use std::fmt::Debug;
use solana_program::clock::Slot;
use solana_program::pubkey::Pubkey;
use solana_sdk::account::AccountSharedData;

/// Fetch account data for a given public key at a specific slot.
pub trait AccountsCallback: Debug + Default {
    /// Get account data for a specific slot and public key.
    fn get_account_data(&self, _slot: Slot, _pubkey: &Pubkey) -> Option<AccountSharedData> {
        None
    }
}

#[derive(Debug, Default)]
pub struct NoopAccountsCallback;

impl AccountsCallback for NoopAccountsCallback {}