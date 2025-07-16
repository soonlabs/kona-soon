use solana_program::epoch_schedule::EpochSchedule;
use solana_program::fee_calculator::{FeeRateGovernor, DEFAULT_TARGET_SIGNATURES_PER_SLOT};
use solana_program::rent::Rent;

pub fn default_epoch_schedule() -> EpochSchedule {
    EpochSchedule::custom(432000, 432000, false)
}

pub fn default_fee_rate_governor() -> FeeRateGovernor {
    const DEFAULT_TARGET_LAMPORTS_PER_SIGNATURE: u64 = 500;
    FeeRateGovernor {
        lamports_per_signature: DEFAULT_TARGET_LAMPORTS_PER_SIGNATURE,
        target_lamports_per_signature: DEFAULT_TARGET_LAMPORTS_PER_SIGNATURE,
        target_signatures_per_slot: DEFAULT_TARGET_SIGNATURES_PER_SLOT,
        min_lamports_per_signature: 0,
        max_lamports_per_signature: 0,
        burn_percent: 0,
    }
}

pub fn default_rent() -> Rent {
    const DEFAULT_LAMPORTS_PER_BYTE_YEAR: u64 = 1_000_000 / 100 * 365 / (1024 * 1024);
    Rent {
        lamports_per_byte_year: DEFAULT_LAMPORTS_PER_BYTE_YEAR,
        exemption_threshold: 2.0,
        burn_percent: 0,
    }
}
