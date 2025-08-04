use solana_sdk::transaction::VersionedTransaction;
use soon_mpt_primitives::B256;
use soon_primitives::blocks::RawBlock;

#[derive(Debug)]
pub struct SimpleBlock {
    pub slot: u64,
    pub hash: B256,
    pub parent_hash: B256,
    pub transactions: Vec<VersionedTransaction>,
}

impl SimpleBlock {
    pub const fn new(
        slot: u64,
        hash: B256,
        parent_hash: B256,
        transactions: Vec<VersionedTransaction>,
    ) -> Self {
        Self {
            slot,
            hash,
            parent_hash,
            transactions,
        }
    }

    pub fn new_from_raw_block(
        slot: u64,
        hash: B256,
        parent_hash: B256,
        raw_block: RawBlock,
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
        }
    }
}
