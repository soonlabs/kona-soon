use serde::{Deserialize, Serialize};
use solana_program::clock::Slot;
use solana_program::fee_calculator::FeeRateGovernor;
use solana_program::hash::Hash;

#[derive(Debug, Default, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct ParentInfo {
    pub slot: Slot,
    pub bank_hash: Hash,
    pub fee_rate_governor: FeeRateGovernor,
    pub signature_count: u64,
}

impl ParentInfo {
    pub fn next_fee_rate_governor(&self) -> FeeRateGovernor {
        FeeRateGovernor::new_derived(&self.fee_rate_governor, self.signature_count)
    }

    pub fn next_slot(&self) -> Slot {
        self.slot + 1
    }
}
