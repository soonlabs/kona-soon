use solana_sdk::transaction::VersionedTransaction;
use soon_mpt_primitives::B256;
use soon_primitives::blocks::RawBlock;

use crate::accounts::AccountPairs;

#[derive(Debug)]
pub struct SimpleBlock {
    pub slot: u64,
    pub hash: B256,
    pub parent_hash: B256,
    pub transactions: Vec<VersionedTransaction>,
    pub extra_accounts: AccountPairs,
}

impl SimpleBlock {
    pub const fn new(
        slot: u64,
        hash: B256,
        parent_hash: B256,
        transactions: Vec<VersionedTransaction>,
        extra_accounts: AccountPairs,
    ) -> Self {
        Self { slot, hash, parent_hash, transactions, extra_accounts }
    }

    pub fn new_from_raw_block(
        slot: u64,
        hash: B256,
        parent_hash: B256,
        raw_block: RawBlock,
        extra_accounts: AccountPairs,
    ) -> Self {
        Self {
            slot,
            hash,
            parent_hash,
            transactions: raw_block
                .transactions
                .into_iter()
                .map(|tx| tx.to_versioned_transaction())
                .collect(),
            extra_accounts,
        }
    }
}
