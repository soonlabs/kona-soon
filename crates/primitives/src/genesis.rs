use crate::deposit::AttributesDeposited;
use crate::error::DepositError;
use alloy_primitives::U256;
use solana_program::epoch_schedule::EpochSchedule;
use solana_program::fee_calculator::{DEFAULT_TARGET_SIGNATURES_PER_SLOT, FeeRateGovernor};
use solana_program::program_pack::Pack;
use solana_program::rent::Rent;
use solana_sdk::account::ReadableAccount;
use solana_sdk::genesis_config::{ClusterType, GenesisConfig};
use solana_sdk::inflation::Inflation;
use solana_sdk::transaction::SanitizedTransaction;

pub fn default_genesis_config() -> GenesisConfig {
    GenesisConfig {
        fee_rate_governor: default_fee_rate_governor(),
        rent: default_rent(),
        inflation: Inflation::new_disabled(),
        epoch_schedule: default_epoch_schedule(),
        // TODO: is this meaningful? We do not use this field
        cluster_type: ClusterType::Development,
        ..GenesisConfig::default()
    }
}

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

pub fn parse_l1_block_info_tx_from_genesis_config(
    genesis_config: &GenesisConfig,
) -> Result<SanitizedTransaction, DepositError> {
    let l1_info_data = genesis_config.accounts.get(&l1_block_info::pda::l1_block_info_pubkey());
    if let Some(l1_info_data) = l1_info_data {
        let l1_info = l1_block_info::state::L1BlockInfo::unpack(l1_info_data.data())
            .map_err(|e| DepositError::ParseL1BlockInfoTxError(e.to_string()))?;
        let attribute = AttributesDeposited {
            l1_block_num: l1_info.number,
            timestamp: l1_info.timestamp,
            base_fee: l1_info.base_fee,
            l1_block_hash: l1_info.hash.into(),
            sequence_number: l1_info.sequence_number,
            batcher_hash: U256::from_le_bytes(l1_info.batcher_hash),
            fee_overhead: l1_info.fee_overhead.try_into().unwrap(),
            fee_scalar: l1_info.fee_scalar.try_into().unwrap(),
            gas: l1_info.gas,
            is_system_tx: l1_info.is_system_tx,
        };
        attribute.to_sanitized_transaction(&Default::default())
    } else {
        Err(DepositError::ParseL1BlockInfoTxError("l1 block info account not found".to_string()))
    }
}
