use solana_sdk::transaction::VersionedTransaction;
use soon_primitives::blocks::RawBlock;

#[derive(Debug)]
pub struct SimpleBlock {
    pub slot: u64,
    pub transactions: Vec<VersionedTransaction>,
}

impl SimpleBlock {
    pub const fn new(
        slot: u64,
        transactions: Vec<VersionedTransaction>,
    ) -> Self {
        Self { slot, transactions }
    }

    pub fn new_from_raw_block(
        slot: u64,
        raw_block: RawBlock,
    ) -> Self {
        Self {
            slot,
            transactions: raw_block
                .transactions
                .into_iter()
                .map(|tx| tx.to_versioned_transaction())
                .collect(),
        }
    }
}
