use crate::utils::add_trie_account;
use alloy_primitives::{B256, keccak256};
use litesvm::accounts_callback::MemoryAccountsCallback;
use serde::{Deserialize, Serialize};
use solana_sdk::{account::AccountSharedData, pubkey::Pubkey};
use soon_primitives::mpt::{
    account::TrieSolanaAccount as MptAccount, encoder::sol_account_encoder,
};
use std::collections::BTreeMap;

pub type AccountPairs = Vec<(Pubkey, AccountSharedData)>;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SoonAccounts {
    pub accounts: AccountPairs,
}

impl SoonAccounts {
    pub fn state_root(&self) -> B256 {
        let mpt_accounts: Vec<(B256, MptAccount)> = self.clone().into();
        triehash::trie_root::<KeccakHasher, _, _, _>(
            mpt_accounts
                .into_iter()
                .map(|(address, account)| (address, sol_account_encoder()(&account))),
        )
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
            add_trie_account(&mut accounts, &pubkey, &account);
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

use hash_db::Hasher;
use plain_hasher::PlainHasher;

pub struct KeccakHasher;

impl Hasher for KeccakHasher {
    type Out = B256;
    type StdHasher = PlainHasher;

    const LENGTH: usize = 32;

    fn hash(x: &[u8]) -> Self::Out {
        keccak256(x)
    }
}
