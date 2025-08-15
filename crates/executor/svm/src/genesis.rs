use solana_compute_budget::compute_budget::ComputeBudget;
use solana_program::clock::{DEFAULT_TICKS_PER_SECOND, DEFAULT_TICKS_PER_SLOT};
use solana_program::epoch_schedule::EpochSchedule;
use solana_program::pubkey::Pubkey;
use solana_program::rent::Rent;
use solana_program::{pubkey, unchecked_div_by_const};
use solana_sdk::feature_set::*;
use solana_sdk::timing::years_as_slots;
use std::time::Duration;

pub const NO_SIG_TX_PAYER: Pubkey = pubkey!("NoSigTxPayer1111111111111111111111111111111");
pub const HASHES_PER_TICK: u64 = 0;

pub fn soon_compute_budget() -> ComputeBudget {
    Default::default()
}

pub fn soon_epoch_schedule() -> EpochSchedule {
    EpochSchedule::custom(432000, 432000, false)
}

pub fn soon_rent() -> Rent {
    const DEFAULT_LAMPORTS_PER_BYTE_YEAR: u64 = 1_000_000 / 100 * 365 / (1024 * 1024);
    Rent {
        lamports_per_byte_year: DEFAULT_LAMPORTS_PER_BYTE_YEAR,
        exemption_threshold: 2.0,
        burn_percent: 0,
    }
}

pub fn soon_feature_set() -> FeatureSet {
    let features = vec![
        blake3_syscall_enabled::id(),
        disable_fees_sysvar::id(),
        zk_token_sdk_enabled::id(),
        curve25519_syscall_enabled::id(),
        libsecp256k1_fail_on_bad_count::id(),
        libsecp256k1_fail_on_bad_count2::id(),
        error_on_syscall_bpf_function_hash_collisions::id(),
        reject_callx_r10::id(),
        disable_deploy_of_alloc_free_syscall::id(),
        add_shred_type_to_shred_seed::id(),
        skip_rent_rewrites::id(),
        loosen_cpi_size_restriction::id(),
        relax_authority_signer_check_for_lookup_table_creation::id(),
        increase_tx_account_lock_limit::id(),
        enable_bpf_loader_set_authority_checked_ix::id(),
        enable_alt_bn128_syscall::id(),
        simplify_alt_bn128_syscall_error_codes::id(),
        enable_big_mod_exp_syscall::id(),
        apply_cost_tracker_during_replay::id(),
        switch_to_new_elf_parser::id(),
        include_loaded_accounts_data_size_in_fee_calculation::id(),
        simplify_writable_program_account_check::id(),
        bpf_account_data_direct_mapping::id(),
        last_restart_slot_sysvar::id(),
        enable_poseidon_syscall::id(),
        remaining_compute_units_syscall_enabled::id(),
        enable_program_runtime_v2_and_loader_v4::id(),
        better_error_codes_for_tx_lamport_check::id(),
        enable_alt_bn128_compression_syscall::id(),
        // validate_fee_collector_account::id(),
        disable_rent_fees_collection::id(),
        enable_zk_transfer_with_fee::id(),
        add_new_reserved_account_keys::id(),
        merkle_conflict_duplicate_proofs::id(),
        enable_zk_proof_from_account::id(),
        cost_model_requested_write_lock_cost::id(),
        enable_gossip_duplicate_proof_ingestion::id(),
        enable_chained_merkle_shreds::id(),
        remove_rounding_in_fee_calculation::id(),
        chained_merkle_conflict_duplicate_proofs::id(),
        reward_full_priority_fee::id(),
        abort_on_invalid_curve::id(),
        get_sysvar_syscall_enabled::id(),
        migrate_feature_gate_program_to_core_bpf::id(),
        migrate_config_program_to_core_bpf::id(),
        migrate_address_lookup_table_program_to_core_bpf::id(),
        zk_elgamal_proof_program_enabled::id(),
        ed25519_precompile_verify_strict::id(),
    ];
    let mut feature_set = FeatureSet::default();
    for feature in features {
        feature_set.activate(&feature, 0);
    }
    feature_set
}

pub fn soon_slots_per_year() -> f64 {
    const TARGET_TICK_DURATION: Duration =
        Duration::from_micros(unchecked_div_by_const!(1000 * 1000, DEFAULT_TICKS_PER_SECOND));
    years_as_slots(1.0, &TARGET_TICK_DURATION, DEFAULT_TICKS_PER_SLOT)
}
