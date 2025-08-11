use solana_program::clock::{Epoch, Slot};
use solana_program::fee_calculator::FeeRateGovernor;
use solana_program::hash::Hash;
use crate::BlockhashQueue;

#[derive(Debug, Default)]
pub struct ParentState {
    pub slot: Slot,
    pub epoch: Epoch,
    pub hash: Hash,
    pub fee_rate_governor: FeeRateGovernor,
    pub blockhash_queue: BlockhashQueue,
    pub signature_count: u64,
}

impl ParentState {
    pub fn new_fee_rate_governor(&self) -> FeeRateGovernor {
        FeeRateGovernor::new_derived(&self.fee_rate_governor, self.signature_count)
    }

    pub fn new_slot(&self) -> Slot {
        self.slot + 1
    }

    pub fn new_epoch(&self) -> Epoch {
        // TODO: generate next epoch correctly
        self.epoch
    }
}
