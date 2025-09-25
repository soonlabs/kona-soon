use solana_program::clock::Slot;
use solana_sdk::transaction::SanitizedTransaction;
use std::fmt::Display;

/// A unique identifier for a transaction batch.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Default)]
pub struct TransactionBatchId(u64);

impl TransactionBatchId {
    pub fn new(index: u64) -> Self {
        Self(index)
    }

    pub fn value(&self) -> u64 {
        self.0
    }
}

impl Display for TransactionBatchId {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<u64> for TransactionBatchId {
    fn from(id: u64) -> Self {
        Self(id)
    }
}

/// A unique identifier for a transaction.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct TransactionId(u64);

impl TransactionId {
    pub fn new(index: u64) -> Self {
        Self(index)
    }

    pub fn value(&self) -> u64 {
        self.0
    }
}

impl Display for TransactionId {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<u64> for TransactionId {
    fn from(id: u64) -> Self {
        Self(id)
    }
}

/// Defined the task before into scheduling pipeline.
pub struct SchedulingTask {
    pub transactions: Vec<SanitizedTransaction>,
    pub priority_and_cost: Vec<(u64, u64)>,
    // a derived batch only contains l1 derived transaction,
    // and need to be executed isolated with l2 txs.
    // in derived batch, ids and batch_id is both empty.
    pub derived_batch: bool,
}

impl SchedulingTask {
    pub fn valid(&self) -> bool {
        self.derived_batch || (self.transactions.len() == self.priority_and_cost.len())
    }
}

/// Scheduling unit.
#[derive(Debug, Clone, Default)]
pub struct SchedulingBatch {
    pub batch_id: TransactionBatchId,
    pub ids: Vec<TransactionId>,
    pub transactions: Vec<SanitizedTransaction>,
    pub max_ages: Vec<MaxAge>,
    // a derived batch only contains l1 derived transaction,
    // and need to be executed isolated with l2 txs.
    // in derived batch, ids and batch_id is both empty.
    pub derived_batch: bool,
    /// 0-N to indicate svm process worker thread.
    pub thread_id: usize,
}

impl SchedulingBatch {
    pub fn valid(&self) -> bool {
        self.derived_batch
            || (self.transactions.len() == self.ids.len() && self.ids.len() == self.max_ages.len())
    }

    /// if it's a derived SchedulingBatch, only transaction field works.
    pub fn from_derived(transactions: Vec<SanitizedTransaction>) -> Self {
        Self {
            transactions,
            derived_batch: true,
            // all below is useless placeholder.
            batch_id: TransactionBatchId::new(0),
            ids: vec![],
            max_ages: vec![],
            thread_id: 0,
        }
    }
}

/// The scheduling result from worker one time.
/// Since the `SchedulingBatch` will be dispute to different subset to multi workers,
/// the `SchedulingBatchResult` is not 1-1 with SchedulingBatch.
/// One `batch_id` may occur mostly `num_of_worker` times.
pub struct SchedulingBatchResult {
    // workload.
    pub batch: SchedulingBatch,
    // time slice status for this batch job.
    pub retryable_indexes: Vec<usize>,
}

/// A TTL flag for a transaction.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub struct MaxAge {
    pub epoch_invalidation_slot: Slot,
    pub alt_invalidation_slot: Slot,
}
