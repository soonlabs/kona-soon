use serde::{Deserialize, Serialize};
use solana_program::declare_id;
use solana_program::fee_calculator::FeeRateGovernor;
use solana_program::pubkey::Pubkey;
use solana_program::sysvar::{Sysvar, SysvarId};

declare_id!("SoonFeeRateGovernor111111111111111111111111");

impl SysvarId for SoonFeeRateGovernor {
    fn id() -> Pubkey {
        id()
    }

    fn check_id(pubkey: &Pubkey) -> bool {
        check_id(pubkey)
    }
}

impl Sysvar for SoonFeeRateGovernor {
    fn size_of() -> usize {
        // 41 + 8
        49
    }
}

/// A Soon Sysvar to provide the next `FeeRateGovernor`.
/// This sysvar is mainly to be used for fraud proof.
/// NOTE: `SoonFeeRateGovernor` should be overridden after `commit_transactions` in `Bank`, since
/// signature count is updated after that.
#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Debug, Default)]
pub struct SoonFeeRateGovernor {
    pub fee_rate_governor: FeeRateGovernor,
    pub signature_count: u64,
}

impl SoonFeeRateGovernor {
    pub fn new_derived(&self) -> FeeRateGovernor {
        FeeRateGovernor::new_derived(&self.fee_rate_governor, self.signature_count)
    }
}
