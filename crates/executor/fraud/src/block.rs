use solana_sdk::transaction::VersionedTransaction;
use soon_mpt_primitives::B256;
use soon_primitives::blocks::RawBlock;

#[derive(Debug)]
pub struct SimpleBlock {
    pub transactions: Vec<VersionedTransaction>,
}

impl SimpleBlock {
    pub const fn new(transactions: Vec<VersionedTransaction>) -> Self {
        Self { transactions }
    }

    pub fn new_from_raw_block(raw_block: RawBlock) -> Self {
        Self {
            transactions: raw_block
                .transactions
                .into_iter()
                .map(|tx| tx.to_versioned_transaction())
                .collect(),
        }
    }
}
