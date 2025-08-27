use crate::utils::sol_account_encoder;
use alloy_primitives::B256;
use kona_mpt::ordered_trie_with_encoder;
use litesvm::accounts_callback::MemoryAccountsCallback;
use serde::{Deserialize, Serialize};
use solana_sdk::{account::AccountSharedData, pubkey::Pubkey};
use soon_primitives::mpt::{TrieSolanaAccount as MptAccount, TrieSolanaPubkey};
use std::collections::BTreeMap;

pub type AccountPairs = Vec<(Pubkey, AccountSharedData)>;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SoonAccounts {
    pub accounts: AccountPairs,
}

impl SoonAccounts {
    pub fn state_root(&self) -> B256 {
        let mut mpt_accounts: Vec<(B256, MptAccount)> = self.clone().into();
        mpt_accounts.sort_unstable_by_key(|(k, _)| *k);
        let accounts = mpt_accounts.into_iter().map(|(_, m)| m).collect::<Vec<_>>();
        let mut hash_builder = ordered_trie_with_encoder(&accounts, |account, buf| {
            let encoded = sol_account_encoder(account);
            buf.put_slice(&encoded);
        });
        hash_builder.root()
    }
}

impl From<AccountPairs> for SoonAccounts {
    fn from(accounts: AccountPairs) -> Self {
        Self { accounts }
    }
}

impl From<SoonAccounts> for Vec<(B256, MptAccount)> {
    fn from(val: SoonAccounts) -> Self {
        let mut accounts = Self::new();
        for (pubkey, account) in val.accounts {
            accounts.push((TrieSolanaPubkey(pubkey).into(), account.into()));
        }
        accounts
    }
}

impl From<SoonAccounts> for MemoryAccountsCallback {
    fn from(val: SoonAccounts) -> Self {
        val.accounts.into()
    }
}

impl From<BTreeMap<Pubkey, AccountSharedData>> for SoonAccounts {
    fn from(val: BTreeMap<Pubkey, AccountSharedData>) -> Self {
        Self { accounts: val.into_iter().collect() }
    }
}

impl From<SoonAccounts> for BTreeMap<Pubkey, AccountSharedData> {
    fn from(val: SoonAccounts) -> Self {
        val.accounts.into_iter().collect()
    }
}
