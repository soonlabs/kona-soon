use crate::utils::add_trie_account;
use litesvm::LiteSVM;
use solana_sdk::{account::AccountSharedData, pubkey::Pubkey};
use soon_mpt_primitives::{Account as MptAccount, B256};
use soon_mpt_trie::{encoder::sol_account_encoder, test_utils::state_root_prehashed};

pub type AccountPairs = Vec<(Pubkey, AccountSharedData)>;

#[derive(Clone, Debug)]
pub struct SoonAccounts {
    pub accounts: AccountPairs,
}

impl SoonAccounts {
    pub fn state_root(&self) -> B256 {
        let mpt_accounts: Vec<(B256, MptAccount)> = self.clone().into();
        state_root_prehashed(mpt_accounts, sol_account_encoder())
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

impl From<&LiteSVM> for SoonAccounts {
    fn from(litesvm: &LiteSVM) -> Self {
        Self { accounts: litesvm.export_accounts() }
    }
}
