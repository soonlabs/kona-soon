use solana_sdk::transaction::VersionedTransaction;
use soon_primitives::blocks::RawBlock;

use crate::accounts::AccountPairs;

#[derive(Debug)]
pub struct SimpleBlock {
    pub slot: u64,
    pub transactions: Vec<VersionedTransaction>,
    pub extra_accounts: AccountPairs,
}

impl SimpleBlock {
    pub const fn new(
        slot: u64,
        transactions: Vec<VersionedTransaction>,
        extra_accounts: AccountPairs,
    ) -> Self {
        Self { slot, transactions, extra_accounts }
    }

    pub fn new_from_raw_block(
        slot: u64,
        raw_block: RawBlock,
        extra_accounts: AccountPairs,
    ) -> Self {
        Self {
            slot,
            transactions: raw_block
                .transactions
                .into_iter()
                .map(|tx| tx.to_versioned_transaction())
                .collect(),
            extra_accounts,
        }
    }
}
