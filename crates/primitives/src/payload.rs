use crate::scheduler::SchedulingBatch;
use solana_sdk::transaction::SanitizedTransaction;

pub type SanitizedTransactions = Vec<SanitizedTransaction>;

/// BlockPayload stores either l1_derived_txs or l2_txs.
/// l1_derived_txs contains l1_block_info and l1_deposits
/// l2_txs is raised by common users.
#[derive(Default, Debug, Clone)]
pub struct BlockPayload {
    pub deposit_txs_count: u64,
    pub transactions: Vec<SchedulingBatch>,
}

impl BlockPayload {
    pub fn new_with_l1_derived(
        transactions: Vec<SanitizedTransaction>,
        deposit_txs_count: u64,
    ) -> Self {
        // NOTE: every l1_derived_tx is filled with a writable placeholder `NoSigTxPayer` account
        // as a fake fee payer to implement no-sig l1 vote, to let other validator finish
        // independent derivation check and fraud proof.
        // So each transaction need to be put in one SchedulingBatch to avoid account conflict.
        // TODO: can free this after svm allow conflict accounts txs in one entry.

        Self {
            deposit_txs_count,
            transactions: transactions
                .into_iter()
                .map(|tx| SchedulingBatch::from_derived(vec![tx]))
                .collect::<Vec<_>>(),
        }
    }

    pub fn set_l2(&mut self, batches: Vec<SchedulingBatch>) {
        self.transactions = batches;
    }

    pub fn new_with_l2_batch(transactions: Vec<SchedulingBatch>, deposit_txs_count: u64) -> Self {
        Self { deposit_txs_count, transactions }
    }

    /// return (batch_count, total_tx_count)
    pub fn block_tx_count(&self) -> (usize, usize) {
        (
            self.transactions.len(),
            self.transactions.iter().map(|item| item.transactions.len()).sum(),
        )
    }

    pub fn transactions_ref(&self) -> Vec<&[SanitizedTransaction]> {
        self.transactions.iter().map(|tx| tx.transactions.as_slice()).collect()
    }
}
