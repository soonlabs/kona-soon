use crate::{
    error::L2BlockError, native_tx::HasNativeInstruction, ui::UiConfirmedBlockWithEntries,
};
use alloy_rlp::{BufMut, Decodable, Encodable, RlpDecodable, RlpEncodable};
use serde::{Deserialize, Serialize};
use solana_sdk::{clock::Slot, transaction::VersionedTransaction};
use solana_transaction_status::{EntrySummary, VersionedConfirmedBlockWithEntries};

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

    pub fn is_deposit_tx(&self) -> bool {
        self.transaction().has_native_instruction()
    }

    /// Create a batch of `L2Transaction` from an `EntrySummary` and a `VersionedConfirmedBlock`.
    pub fn batch_from_entry_and_block_txs(
        entry: EntrySummary,
        block_txs: &[VersionedTransaction],
    ) -> Result<Vec<Self>, L2BlockError> {
        if entry.num_transactions == 0 {
            // Entry must be a tick if there are no transactions inner.
            return Ok(Vec::new());
        }

        let start_index = entry.starting_transaction_index;
        if start_index >= block_txs.len() {
            return Err(L2BlockError::InsufficientTransactions);
        }
        let end_index =
            entry.starting_transaction_index.saturating_add(entry.num_transactions as usize);
        if end_index > block_txs.len() {
            return Err(L2BlockError::InsufficientTransactions);
        }

        let l2_txs = block_txs[start_index..end_index]
            .iter()
            .enumerate()
            .map(|(index, tx)| {
                if index == 0 {
                    Self::Head(entry.num_transactions, tx.clone())
                } else {
                    Self::Body(tx.clone())
                }
            })
            .collect();
        Ok(l2_txs)
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

impl Encodable for L2Transaction {
    fn encode(&self, out: &mut dyn BufMut) {
        let tx_bytes = bincode::serialize(&self).unwrap_or_default();
        tx_bytes.encode(out);
    }
}

impl Decodable for L2Transaction {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let tx_bytes = Vec::<u8>::decode(buf)?;
        let tx = bincode::deserialize(&tx_bytes)
            .map_err(|_| alloy_rlp::Error::Custom("Failed to deserialize labeled transaction"))?;
        Ok(tx)
    }
}

#[derive(Default, Debug, Clone, RlpEncodable, RlpDecodable)]
pub struct L2Block {
    //base58
    pub previous_blockhash: String,
    //base58
    pub blockhash: String,
    pub parent_slot: Slot,
    pub block_time: u64,
    pub block_height: u64,
    pub transactions: Vec<L2Transaction>,
}

impl TryFrom<VersionedConfirmedBlockWithEntries> for L2Block {
    type Error = L2BlockError;

    fn try_from(value: VersionedConfirmedBlockWithEntries) -> Result<Self, Self::Error> {
        let VersionedConfirmedBlockWithEntries { block, entries } = value;
        let block_txs = block.transactions.into_iter().map(|tx| tx.transaction).collect::<Vec<_>>();
        let mut l2_txs = Vec::with_capacity(block_txs.len());
        for entry in entries {
            let tx_batch = L2Transaction::batch_from_entry_and_block_txs(entry, &block_txs)?;
            l2_txs.extend(tx_batch);
        }
        if l2_txs.len() != block_txs.len() {
            return Err(L2BlockError::UnmatchedEntriesAndTransactions);
        }

        Ok(L2Block {
            previous_blockhash: block.previous_blockhash,
            blockhash: block.blockhash,
            parent_slot: block.parent_slot,
            //TODO empty block_time and block_height
            block_time: block.block_time.unwrap_or(0) as u64,
            block_height: block.parent_slot + 1,
            transactions: l2_txs,
        })
    }
}

impl TryFrom<UiConfirmedBlockWithEntries> for L2Block {
    type Error = L2BlockError;

    fn try_from(value: UiConfirmedBlockWithEntries) -> Result<Self, Self::Error> {
        let UiConfirmedBlockWithEntries { block, entries } = value;

        let l2_txs = if let Some(transactions) = block.transactions {
            let block_txs = transactions
                .into_iter()
                .filter_map(|tx| tx.transaction.decode())
                .collect::<Vec<_>>();
            let mut l2_txs = Vec::with_capacity(block_txs.len());
            for entry in entries {
                let entry = entry.try_into()?;
                let tx_batch = L2Transaction::batch_from_entry_and_block_txs(entry, &block_txs)?;
                l2_txs.extend(tx_batch);
            }
            if l2_txs.len() != block_txs.len() {
                return Err(L2BlockError::UnmatchedEntriesAndTransactions);
            }
            l2_txs
        } else {
            Vec::new()
        };

        Ok(L2Block {
            previous_blockhash: block.previous_blockhash,
            blockhash: block.blockhash,
            parent_slot: block.parent_slot,
            //TODO empty block_time and block_height
            block_time: block.block_time.unwrap_or(0) as u64,
            block_height: block.parent_slot + 1,
            transactions: l2_txs,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_rlp::{BytesMut, Decodable, Encodable};
    use solana_sdk::{
        message::VersionedMessage,
        signature::{Keypair, Signer},
        transaction::VersionedTransaction,
    };

    fn create_test_transaction() -> VersionedTransaction {
        let keypair = Keypair::new();
        let message = VersionedMessage::Legacy(solana_sdk::message::Message::new(
            &[solana_sdk::system_instruction::transfer(
                &keypair.pubkey(),
                &Keypair::new().pubkey(),
                1000,
            )],
            Some(&keypair.pubkey()),
        ));
        VersionedTransaction::try_new(message, &[&keypair]).unwrap()
    }

    fn create_test_l2_transaction() -> L2Transaction {
        L2Transaction::Body(create_test_transaction())
    }

    fn create_block_txs_and_entry_summaries() -> (Vec<VersionedTransaction>, Vec<EntrySummary>) {
        let mut block_txs = Vec::new();
        let mut entry_summaries = Vec::new();

        // Entry 1 with 2 transactions
        let entry1_start = block_txs.len();
        for _ in 0..2 {
            let tx = create_test_transaction();
            block_txs.push(tx);
        }
        entry_summaries.push(EntrySummary {
            num_hashes: 0,
            starting_transaction_index: entry1_start,
            num_transactions: 2,
            hash: Default::default(),
        });

        // Entry 2 with 0 transactions (tick)
        entry_summaries.push(EntrySummary {
            num_hashes: 0,
            starting_transaction_index: block_txs.len(),
            num_transactions: 0,
            hash: Default::default(),
        });

        // Entry 3 with 2 transactions
        let entry3_start = block_txs.len();
        for _ in 0..3 {
            let tx = create_test_transaction();
            block_txs.push(tx);
        }
        entry_summaries.push(EntrySummary {
            num_hashes: 0,
            starting_transaction_index: entry3_start,
            num_transactions: 3,
            hash: Default::default(),
        });

        (block_txs, entry_summaries)
    }

    #[test]
    fn test_l2_transaction_rlp_roundtrip() {
        let l2_tx = create_test_l2_transaction();

        let mut out_buf = BytesMut::default();
        l2_tx.encode(&mut out_buf);

        let decoded_tx = L2Transaction::decode(&mut out_buf.as_ref()).unwrap();

        assert_eq!(l2_tx.transaction().message, decoded_tx.transaction().message);
        assert_eq!(l2_tx.transaction().signatures, decoded_tx.transaction().signatures);
    }

    #[test]
    fn test_l2_block_rlp_roundtrip() {
        let test_block = L2Block {
            previous_blockhash: "5nHHXGcCAhbpewehi9eGiLuobYMaFzdVzU6bfKyu62vE".to_string(),
            blockhash: "5nHHXGcCAhbpewehi9eGiLuobYMaFzdVzU6bfKyu62vE".to_string(),
            parent_slot: 100,
            block_time: 1640995200, // 2022-01-01 00:00:00 UTC
            block_height: 101,
            transactions: vec![create_test_l2_transaction(), create_test_l2_transaction()],
        };

        let mut out_buf = BytesMut::default();
        test_block.encode(&mut out_buf);

        let decoded_block = L2Block::decode(&mut out_buf.as_ref()).unwrap();

        assert_eq!(test_block.previous_blockhash, decoded_block.previous_blockhash);
        assert_eq!(test_block.blockhash, decoded_block.blockhash);
        assert_eq!(test_block.parent_slot, decoded_block.parent_slot);
        assert_eq!(test_block.block_time, decoded_block.block_time);
        assert_eq!(test_block.block_height, decoded_block.block_height);
        assert_eq!(test_block.transactions.len(), decoded_block.transactions.len());

        for (i, (original_tx, decoded_tx)) in
            test_block.transactions.iter().zip(decoded_block.transactions.iter()).enumerate()
        {
            assert_eq!(
                original_tx.transaction().message,
                decoded_tx.transaction().message,
                "Transaction {} message mismatch",
                i
            );
            assert_eq!(
                original_tx.transaction().signatures,
                decoded_tx.transaction().signatures,
                "Transaction {} signatures mismatch",
                i
            );
        }
    }

    #[test]
    fn test_l2_block_empty_transactions() {
        let test_block = L2Block {
            previous_blockhash: "5nHHXGcCAhbpewehi9eGiLuobYMaFzdVzU6bfKyu62vE".to_string(),
            blockhash: "5nHHXGcCAhbpewehi9eGiLuobYMaFzdVzU6bfKyu62vE".to_string(),
            parent_slot: 100,
            block_time: 1640995200,
            block_height: 101,
            transactions: vec![],
        };

        let mut out_buf = BytesMut::default();
        test_block.encode(&mut out_buf);

        let decoded_block = L2Block::decode(&mut out_buf.as_ref()).unwrap();

        assert_eq!(test_block.previous_blockhash, decoded_block.previous_blockhash);
        assert_eq!(test_block.blockhash, decoded_block.blockhash);
        assert_eq!(test_block.parent_slot, decoded_block.parent_slot);
        assert_eq!(test_block.block_time, decoded_block.block_time);
        assert_eq!(test_block.block_height, decoded_block.block_height);
        assert_eq!(test_block.transactions.len(), decoded_block.transactions.len());
        assert!(decoded_block.transactions.is_empty());
    }

    #[test]
    fn test_l2_block_default_values() {
        let test_block = L2Block::default();

        let mut out_buf = BytesMut::default();
        test_block.encode(&mut out_buf);

        let decoded_block = L2Block::decode(&mut out_buf.as_ref()).unwrap();

        assert_eq!(test_block.previous_blockhash, decoded_block.previous_blockhash);
        assert_eq!(test_block.blockhash, decoded_block.blockhash);
        assert_eq!(test_block.parent_slot, decoded_block.parent_slot);
        assert_eq!(test_block.block_time, decoded_block.block_time);
        assert_eq!(test_block.block_height, decoded_block.block_height);
        assert_eq!(test_block.transactions.len(), decoded_block.transactions.len());
    }

    #[test]
    fn test_l2_transaction_is_deposit_tx() {
        let l2_tx = create_test_l2_transaction();
        assert!(!l2_tx.is_deposit_tx());
    }

    #[test]
    fn test_l2_transaction_batch_from_entry_and_block_txs() {
        let (block_txs, entry_summaries) = create_block_txs_and_entry_summaries();
        let mut l2_txs = Vec::new();

        for entry in entry_summaries {
            let tx_batch = L2Transaction::batch_from_entry_and_block_txs(entry, &block_txs)
                .expect("Failed to create L2 transactions from entry and block txs");
            l2_txs.extend(tx_batch);
        }

        for (block_tx, l2_tx) in block_txs.iter().zip(l2_txs.iter()) {
            assert_eq!(
                block_tx,
                l2_tx.transaction(),
                "Transaction mismatch between block and L2 transaction",
            )
        }
    }
}
