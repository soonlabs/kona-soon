use crate::utils::add_trie_account;
use litesvm::LiteSVM;
use soon_mpt_primitives::{Account as MptAccount, B256};
use soon_mpt_trie::{encoder::sol_account_encoder, test_utils::state_root_prehashed};

#[derive(Clone, Debug)]
pub struct SoonAccounts {
    pub accounts: Vec<(solana_sdk::pubkey::Pubkey, solana_sdk::account::AccountSharedData)>,
}

impl SoonAccounts {
    pub fn state_root(&self) -> B256 {
        let mpt_accounts: Vec<(B256, MptAccount)> = self.clone().into();
        state_root_prehashed(mpt_accounts, sol_account_encoder())
    }
}

impl From<SoonAccounts> for Vec<(B256, MptAccount)> {
    fn from(val: SoonAccounts) -> Self {
        let mut accounts = Vec::new();
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
