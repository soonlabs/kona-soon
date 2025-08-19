use serde::{Deserialize, Serialize};
use solana_sdk::transaction::VersionedTransaction;

/// `L2Transaction` is an enum wraps versioned transaction with a label info in a tx batch.
#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum L2Transaction {
    /// Head transaction contains the length of the batch and the transaction itself.
    Head(u64, VersionedTransaction),
    /// Body transaction is part of a batch.
    Body(VersionedTransaction),
}

impl L2Transaction {
    pub fn new_from_derived_tx(tx: VersionedTransaction) -> Self {
        Self::Head(1, tx)
    }

    pub fn new_head(length: u64, tx: VersionedTransaction) -> Self {
        Self::Head(length, tx)
    }

    pub fn new_body(tx: VersionedTransaction) -> Self {
        Self::Body(tx)
    }

    /// Create a batch of `L2Transaction` from an `EntrySummary` and a `VersionedConfirmedBlock`.
    pub fn from_tx_batch(tx_batch: Vec<VersionedTransaction>) -> Vec<Self> {
        let batch_size = tx_batch.len() as u64;
        tx_batch
            .into_iter()
            .enumerate()
            .map(|(index, tx)| if index == 0 { Self::Head(batch_size, tx) } else { Self::Body(tx) })
            .collect::<Vec<Self>>()
    }

    pub fn is_head(&self) -> bool {
        matches!(self, Self::Head(_, _))
    }

    pub fn is_body(&self) -> bool {
        matches!(self, Self::Body(_))
    }

    pub fn batch_length(&self) -> Option<u64> {
        match self {
            Self::Head(length, _) => Some(*length),
            Self::Body(_) => None,
        }
    }

    pub fn transaction(&self) -> &VersionedTransaction {
        match self {
            Self::Head(_, tx) => tx,
            Self::Body(tx) => tx,
        }
    }
}

impl From<L2Transaction> for VersionedTransaction {
    fn from(value: L2Transaction) -> Self {
        match value {
            L2Transaction::Head(_, tx) => tx,
            L2Transaction::Body(tx) => tx,
        }
    }
}

impl Default for L2Transaction {
    fn default() -> Self {
        Self::Body(VersionedTransaction::default())
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct RawBlock(pub Vec<Vec<VersionedTransaction>>);

impl RawBlock {
    pub fn from_single_batch(batch: Vec<VersionedTransaction>) -> Self {
        Self(vec![batch])
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct L2Block(pub Vec<L2Transaction>);

impl From<RawBlock> for L2Block {
    fn from(raw_block: RawBlock) -> Self {
        let transactions = raw_block.0.into_iter().flat_map(L2Transaction::from_tx_batch).collect();
        Self(transactions)
    }
}

impl From<L2Block> for RawBlock {
    fn from(value: L2Block) -> Self {
        let mut batches = vec![];
        let mut tx_batch = vec![];

        for tx in value.0 {
            match tx {
                L2Transaction::Head(length, tx) => {
                    if !tx_batch.is_empty() {
                        batches.push(tx_batch);
                    }
                    tx_batch = Vec::with_capacity(length as usize);
                    tx_batch.push(tx);
                }
                L2Transaction::Body(tx) => {
                    tx_batch.push(tx);
                }
            }
        }

        if !tx_batch.is_empty() {
            batches.push(tx_batch);
        }

        Self(batches)
    }
}
