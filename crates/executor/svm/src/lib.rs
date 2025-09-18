#![allow(missing_docs)]

pub mod accounts_callback;
pub mod error;
pub mod genesis;
pub mod sysvar;
pub mod types;

mod accounts_db;
mod builtin;
// mod spl;
mod block;
mod blockhash_queue;
mod entry;
mod leader_schedule;
mod utils;

use crate::{
    accounts_db::AccountsDb,
    builtin::BUILTINS,
    entry::*,
    error::LiteSVMError,
    genesis::*,
    types::{ExecutionResult, FailedTransactionMetadata, TransactionMetadata, TransactionResult},
    utils::rent::RentState,
};
use accounts_callback::AccountsCallback;
use solana_bpf_loader_program::syscalls::create_program_runtime_environment_v1;
use solana_compute_budget::{
    compute_budget::ComputeBudget,
    compute_budget_processor::{ComputeBudgetLimits, process_compute_budget_instructions},
};
use solana_loader_v4_program::create_program_runtime_environment_v2;
use solana_program::{
    clock::{Clock, Epoch, INITIAL_RENT_EPOCH, MAX_PROCESSING_AGE, Slot},
    epoch_schedule::EpochSchedule,
    fee_calculator::FeeRateGovernor,
    hash::Hash,
    nonce, sysvar as solana_sysvar,
    sysvar::recent_blockhashes::IntoIterSorted,
};
#[cfg(not(target_os = "zkvm"))]
use solana_program_runtime::timings::ExecuteTimings;
use solana_program_runtime::{
    invoke_context::{EnvironmentConfig, InvokeContext},
    loaded_programs::{ProgramCacheEntry, ProgramRuntimeEnvironments},
    log_collector::LogCollector,
};
use solana_sdk::{
    account::{Account, AccountSharedData, ReadableAccount, WritableAccount},
    feature_set,
    feature_set::{
        FeatureSet, include_loaded_accounts_data_size_in_fee_calculation,
        remove_rounding_in_fee_calculation,
    },
    fee::FeeStructure,
    inner_instruction::InnerInstructionsList,
    message::SanitizedMessage,
    native_loader,
    nonce::{NONCED_TX_MARKER_IX_INDEX, state::DurableNonce},
    nonce_account,
    pubkey::Pubkey,
    rent::Rent,
    rent_collector::RentCollector,
    reserved_account_keys::ReservedAccountKeys,
    signature::Signature,
    sysvar::{Sysvar, SysvarId},
    transaction::{MessageHash, SanitizedTransaction, TransactionError, VersionedTransaction},
    transaction_context::{ExecutionRecord, IndexOfAccount, TransactionContext},
};
use solana_svm::{
    account_loader::{
        CheckedTransactionDetails, TransactionCheckResult, collect_rent_from_account,
    },
    message_processor::MessageProcessor,
    nonce_info::NoncePartial,
};
use solana_system_program::{SystemAccountKind, get_system_account_kind};
use std::{cell::RefCell, collections::BinaryHeap, fmt::Debug, rc::Rc, sync::Arc};
use tracing::error;
use types::SimulatedTransactionInfo;
use utils::{
    construct_instructions_account,
    inner_instructions::inner_instructions_list_from_instruction_trace,
};

pub use block::{L2Block, L2Transaction, RawBlock};
pub use blockhash_queue::BlockhashQueue;
pub use leader_schedule::LeaderSchedule;

// The test code doesn't actually get run because it's not
// what doctest expects but at least it
// compiles it so we'll see if there's a compile-time error.
#[doc = include_str!("../../README.md")]
#[cfg(doctest)]
pub struct ReadmeDoctests;

pub struct LiteSVM<CB: AccountsCallback> {
    accounts: AccountsDb<CB>,
    log_collector: Option<Rc<RefCell<LogCollector>>>,

    sig_verify: bool,
    blockhash_verify: bool,

    // configurations
    compute_budget: ComputeBudget,
    feature_set: FeatureSet,
    slots_per_year: f64,
    ticks_per_slot: u64,
    hashes_per_tick: u64,
    genesis_hash: Hash,
    fee_structure: FeeStructure,
    epoch_schedule: EpochSchedule,
    leader_schedule: LeaderSchedule,
    rent: Rent,

    // internal state
    slot: Slot,
    epoch: Epoch,
    signature_count: u64,
    fee_rate_governor: FeeRateGovernor,
    parent_blockhash: Option<Hash>,
    blockhash: Option<Hash>,
    blockhash_queue: BlockhashQueue,
    rent_collector: RentCollector,

    // witness variables
    clock_timestamp: i64,
    parent_slot: Slot,
    parent_bank_hash: Hash,
}

impl<CB: AccountsCallback> Default for LiteSVM<CB> {
    fn default() -> Self {
        Self {
            accounts: Default::default(),
            log_collector: None,
            sig_verify: false,
            blockhash_verify: false,
            compute_budget: Default::default(),
            feature_set: Default::default(),
            slots_per_year: soon_slots_per_year(),
            ticks_per_slot: 64,
            hashes_per_tick: 0,
            genesis_hash: Default::default(),
            epoch_schedule: Default::default(),
            leader_schedule: Default::default(),
            rent: Default::default(),
            slot: 0,
            epoch: 0,
            signature_count: 0,
            fee_rate_governor: Default::default(),
            fee_structure: Default::default(),
            parent_blockhash: None,
            blockhash: None,
            blockhash_queue: Default::default(),
            rent_collector: Default::default(),
            clock_timestamp: 0,
            parent_slot: 0,
            parent_bank_hash: Hash::default(),
        }
    }
}

impl<CB: AccountsCallback> Debug for LiteSVM<CB> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LiteSVM")?;
        write!(f, "compute_budget: {:?}", self.compute_budget)?;
        write!(f, "sig_verify: {}", self.sig_verify)?;
        write!(f, "fee_structure: {:?}", self.fee_structure)?;
        Ok(())
    }
}

impl<CB: AccountsCallback> LiteSVM<CB> {
    /// Creates the basic test environment.
    pub fn new_soon() -> Self {
        Self::default()
            .with_builtins()
            .with_compute_budget(soon_compute_budget())
            .with_epoch_schedule(soon_epoch_schedule())
            .with_rent(soon_rent())
            .with_feature_set(soon_feature_set())
    }

    pub fn finish_init(&mut self) -> Result<(), LiteSVMError> {
        // set slot and epoch from parent
        self.slot = self.parent_slot + 1;
        self.epoch = self.epoch_schedule.get_epoch(self.slot);
        self.accounts.set_slot(self.slot);
        self.accounts.set_epoch(self.epoch);
        // update fee rate governer
        let fee_rate_governor =
            self.get_sysvar::<sysvar::fee_rate_governor::SoonFeeRateGovernor>()?;
        self.fee_rate_governor = fee_rate_governor.new_derived();

        // create environments
        let program_runtime_v1 = create_program_runtime_environment_v1(
            &self.feature_set,
            &self.compute_budget,
            false,
            false,
        )
        .unwrap();
        let program_runtime_v2 = create_program_runtime_environment_v2(&self.compute_budget, false);
        self.accounts.set_environments(ProgramRuntimeEnvironments {
            program_runtime_v1: Arc::new(program_runtime_v1),
            program_runtime_v2: Arc::new(program_runtime_v2),
        });

        // update sysvars
        self.update_slot_hashes()?;
        self.update_clock()?;
        self.update_slot_history()?;

        // update blockhash queue
        let mut recent_blockhashes =
            self.get_sysvar::<sysvar::recent_blockhashes::SoonRecentBlockhashes>()?;
        if recent_blockhashes.is_empty() && self.parent_slot == 0 {
            // genesis case, fill with genesis hash
            #[allow(deprecated)]
            recent_blockhashes.push(solana_sysvar::recent_blockhashes::Entry {
                blockhash: self.genesis_hash,
                fee_calculator: Default::default(),
            })
        }
        self.blockhash_queue = recent_blockhashes.into();
        self.parent_blockhash = Some(self.blockhash_queue.last_hash());

        // fill sysvars cache
        self.accounts.fill_sysvar_cache()?;

        // fill rent collector
        self.rent_collector = RentCollector::new(
            self.epoch,
            self.epoch_schedule.clone(),
            self.slots_per_year,
            self.rent.clone(),
        );

        Ok(())
    }

    pub fn with_log_collector(mut self, log_collector: Option<Rc<RefCell<LogCollector>>>) -> Self {
        self.log_collector = log_collector;
        self
    }

    pub const fn with_parent_slot(mut self, slot: Slot) -> Self {
        self.parent_slot = slot;
        self
    }

    pub const fn with_parent_bank_hash(mut self, bank_hash: Hash) -> Self {
        self.parent_bank_hash = bank_hash;
        self
    }

    pub const fn with_clock_timestamp(mut self, timestamp: i64) -> Self {
        self.clock_timestamp = timestamp;
        self
    }

    /// Sets the compute budget.
    pub const fn with_compute_budget(mut self, compute_budget: ComputeBudget) -> Self {
        self.compute_budget = compute_budget;
        self
    }

    /// Enables or disables sigverify.
    pub const fn with_sig_verify(mut self, sig_verify: bool) -> Self {
        self.sig_verify = sig_verify;
        self
    }

    pub const fn with_blockhash_verify(mut self, blockhash_verify: bool) -> Self {
        self.blockhash_verify = blockhash_verify;
        self
    }

    pub const fn with_genesis_hash(mut self, genesis_hash: Hash) -> Self {
        self.genesis_hash = genesis_hash;
        self
    }

    pub fn with_leader_schedule(mut self, leader_schedule: LeaderSchedule) -> Self {
        self.leader_schedule = leader_schedule;
        self
    }

    /// Sets the accounts db callback.
    pub fn with_accounts_callback(mut self, callback: CB) -> Self {
        self.accounts.set_callback(callback);
        self
    }

    pub fn with_init_account(mut self, init_accounts: Vec<(Pubkey, AccountSharedData)>) -> Self {
        self.accounts.set_init_accounts(init_accounts);
        self
    }

    /// Sets the feature set for the test environment.
    pub fn with_feature_set(mut self, feature_set: FeatureSet) -> Self {
        self.feature_set = feature_set;
        self
    }

    pub fn with_fee_structure(mut self, fee_structure: FeeStructure) -> Self {
        self.fee_structure = fee_structure;
        self
    }

    pub fn with_epoch_schedule(mut self, epoch_schedule: EpochSchedule) -> Self {
        self.epoch_schedule = epoch_schedule;
        self
    }

    pub fn with_rent(mut self, rent: Rent) -> Self {
        self.rent = rent.clone();
        self
    }

    /// Changes the default builtins.
    pub fn with_builtins(mut self) -> Self {
        BUILTINS.iter().for_each(|builtin| {
            if let Some(feature_id) = builtin.feature_id {
                if !self.feature_set.is_active(&feature_id) {
                    return;
                }
            }
            let loaded_program =
                ProgramCacheEntry::new_builtin(0, builtin.name.len(), builtin.entrypoint);
            self.accounts.programs_cache.replenish(builtin.program_id, Arc::new(loaded_program));
        });
        self
    }

    /// Returns all information associated with the account of the provided pubkey.
    pub fn get_account(&mut self, pubkey: &Pubkey) -> Option<Account> {
        self.accounts.get_account(pubkey).map(Into::into)
    }

    pub fn set_rent_collector(&mut self, rent_collector: RentCollector) {
        self.rent_collector = rent_collector;
    }

    pub fn clear_diff_accounts(&mut self) {
        self.accounts.clear_diff_accounts();
    }

    pub fn export_diff_accounts(&self) -> Vec<(Pubkey, AccountSharedData)> {
        self.accounts.export_diff_accounts()
    }

    pub fn slot(&self) -> Slot {
        self.slot
    }

    pub fn epoch(&self) -> Epoch {
        self.epoch
    }

    pub fn blockhash(&self) -> Result<Hash, LiteSVMError> {
        self.blockhash.ok_or(LiteSVMError::NoBlockhash)
    }

    pub fn parent_blockhash(&mut self) -> Result<Hash, LiteSVMError> {
        self.parent_blockhash.ok_or(LiteSVMError::NoBlockhash)
    }

    pub fn feature_set(&self) -> &FeatureSet {
        &self.feature_set
    }

    pub fn fee_rate_governor(&self) -> &FeeRateGovernor {
        &self.fee_rate_governor
    }

    pub fn signature_count(&self) -> u64 {
        self.signature_count
    }

    pub fn get_lamports_per_signature(&self) -> u64 {
        self.fee_rate_governor.lamports_per_signature
    }

    pub fn get_blockhash_queue(&self) -> &BlockhashQueue {
        &self.blockhash_queue
    }

    /// Gets the balance of the provided account pubkey.
    pub fn get_balance(&mut self, pubkey: &Pubkey) -> Option<u64> {
        self.accounts.get_account(pubkey).map(|x| x.lamports())
    }

    /// Gets a sysvar from the test environment.
    pub fn get_sysvar<T>(&mut self) -> Result<T, LiteSVMError>
    where
        T: Sysvar + SysvarId,
    {
        let account =
            self.accounts.get_account(&T::id()).ok_or(LiteSVMError::MissingAccount(T::id()))?;
        bincode::deserialize(account.data()).map_err(|e| LiteSVMError::Bincode(e))
    }

    fn update_sysvar<T>(&mut self, sysvar: T) -> Result<(), LiteSVMError>
    where
        T: Sysvar + SysvarId,
    {
        let mut account = Account::new(
            self.rent.minimum_balance(T::size_of()),
            T::size_of(),
            &solana_sysvar::id(),
        );
        solana_sdk::account::to_account::<_, Account>(&sysvar, &mut account).unwrap();
        account.rent_epoch = INITIAL_RENT_EPOCH;

        // add to accounts db
        self.accounts.add_diff_account(false, T::id(), account.into())?;
        Ok(())
    }

    fn create_transaction_context(
        &self,
        compute_budget: ComputeBudget,
        accounts: Vec<(Pubkey, AccountSharedData)>,
    ) -> TransactionContext {
        TransactionContext::new(
            accounts,
            self.rent.clone(),
            compute_budget.max_instruction_stack_depth,
            compute_budget.max_instruction_trace_length,
        )
    }

    fn sanitize_transaction_no_verify_inner(
        &mut self,
        tx: VersionedTransaction,
    ) -> Result<SanitizedTransaction, TransactionError> {
        SanitizedTransaction::try_create(
            tx,
            MessageHash::Compute,
            Some(false),
            &self.accounts,
            &ReservedAccountKeys::empty_key_set(),
        )
    }

    fn sanitize_transaction_no_verify(
        &mut self,
        tx: VersionedTransaction,
    ) -> Result<SanitizedTransaction, ExecutionResult> {
        self.sanitize_transaction_no_verify_inner(tx)
            .map_err(|err| ExecutionResult { tx_result: Err(err), ..Default::default() })
    }

    fn sanitize_transaction(
        &mut self,
        tx: VersionedTransaction,
    ) -> Result<SanitizedTransaction, ExecutionResult> {
        self.sanitize_transaction_inner(tx)
            .map_err(|err| ExecutionResult { tx_result: Err(err), ..Default::default() })
    }

    fn sanitize_transaction_inner(
        &mut self,
        tx: VersionedTransaction,
    ) -> Result<SanitizedTransaction, TransactionError> {
        let tx = self.sanitize_transaction_no_verify_inner(tx)?;

        tx.verify()?;
        tx.verify_precompiles(&self.feature_set)?;

        Ok(tx)
    }

    fn process_transaction(
        &mut self,
        tx: &SanitizedTransaction,
        tx_details: CheckedTransactionDetails,
        compute_budget_limits: ComputeBudgetLimits,
    ) -> (
        Result<(), TransactionError>,
        u64,
        Option<TransactionContext>,
        u64,
        Option<Pubkey>,
        u64, // fee_payer_rent_debit
    ) {
        let CheckedTransactionDetails {
            nonce: _, // TODO: where should use the nonce?
            lamports_per_signature,
        } = tx_details;
        let blockhash = tx.message().recent_blockhash();
        let mut accumulated_consume_units = 0;
        let message = tx.message();
        let account_keys = message.account_keys();

        let fee = self.fee_structure.calculate_fee(
            message,
            lamports_per_signature,
            &compute_budget_limits.into(),
            self.feature_set.is_active(&include_loaded_accounts_data_size_in_fee_calculation::id()),
            self.feature_set.is_active(&remove_rounding_in_fee_calculation::id()),
        );
        let mut validated_fee_payer = false;
        let mut payer_key = None;
        let mut fee_payer_rent_debit = 0;
        let maybe_accounts = account_keys
            .iter()
            .enumerate()
            .map(|(i, key)| {
                let account = if solana_sysvar::instructions::check_id(key) {
                    construct_instructions_account(message)
                } else {
                    let mut account = self
                        .accounts
                        .load_account(key)
                        .map_err(|_| TransactionError::AccountNotFound)?
                        .unwrap_or_default();
                    if !validated_fee_payer &&
                        (!message.is_invoked(i) || message.is_instruction_account(i))
                    {
                        fee_payer_rent_debit = collect_rent_from_account(
                            &self.feature_set,
                            &self.rent_collector,
                            key,
                            &mut account,
                        )
                        .rent_amount;

                        validate_fee_payer(
                            key,
                            &mut account,
                            i as IndexOfAccount,
                            &self.rent,
                            fee,
                        )?;
                        validated_fee_payer = true;
                        payer_key = Some(*key);
                    } else if message.is_writable(i) {
                        fee_payer_rent_debit = collect_rent_from_account(
                            &self.feature_set,
                            &self.rent_collector,
                            key,
                            &mut account,
                        )
                        .rent_amount;
                    }
                    account
                };

                Ok((*key, account))
            })
            .collect::<solana_sdk::transaction::Result<Vec<_>>>();
        let mut accounts = match maybe_accounts {
            Ok(accs) => accs,
            Err(e) => {
                return (
                    Err(e),
                    accumulated_consume_units,
                    None,
                    fee,
                    payer_key,
                    fee_payer_rent_debit,
                );
            }
        };
        if !validated_fee_payer {
            error!("Failed to validate fee payer");
            return (
                Err(TransactionError::AccountNotFound),
                accumulated_consume_units,
                None,
                fee,
                payer_key,
                fee_payer_rent_debit,
            );
        }
        let builtins_start_index = accounts.len();
        let maybe_program_indices = tx
            .message()
            .instructions()
            .iter()
            .map(|c| {
                let mut account_indices: Vec<u16> = Vec::with_capacity(2);
                let program_index = c.program_id_index as usize;
                // This may never error, because the transaction is sanitized
                let (program_id, program_account) = accounts.get(program_index).unwrap();
                if native_loader::check_id(program_id) {
                    return Ok(account_indices);
                }
                if !program_account.executable() {
                    error!("Program account {program_id} is not executable.");
                    return Err(TransactionError::InvalidProgramForExecution);
                }
                account_indices.insert(0, program_index as IndexOfAccount);

                let owner_id = program_account.owner();
                if native_loader::check_id(owner_id) {
                    return Ok(account_indices);
                }
                if !accounts
                    .get(builtins_start_index..)
                    .ok_or(TransactionError::ProgramAccountNotFound)?
                    .iter()
                    .any(|(key, _)| key == owner_id)
                {
                    let owner_account = self
                        .accounts
                        .load_account(owner_id)
                        .map_err(|_| TransactionError::AccountNotFound)?
                        .ok_or(TransactionError::AccountNotFound)?;
                    if !native_loader::check_id(owner_account.owner()) {
                        error!(
                            "Owner account {owner_id} is not owned by the native loader program."
                        );
                        return Err(TransactionError::InvalidProgramForExecution);
                    }
                    if !owner_account.executable() {
                        error!("Owner account {owner_id} is not executable");
                        return Err(TransactionError::InvalidProgramForExecution);
                    }
                    accounts.push((*owner_id, owner_account));
                }
                Ok(account_indices)
            })
            .collect::<Result<Vec<Vec<u16>>, TransactionError>>();
        match maybe_program_indices {
            Ok(program_indices) => {
                //reload program cache
                let mut program_cache_for_tx_batch = self.accounts.programs_cache.clone();
                let mut context = self.create_transaction_context(self.compute_budget, accounts);
                let mut tx_result = MessageProcessor::process_message(
                    tx.message(),
                    &program_indices,
                    &mut InvokeContext::new(
                        &mut context,
                        &mut program_cache_for_tx_batch,
                        EnvironmentConfig::new(
                            *blockhash,
                            None,
                            None,
                            Arc::new(self.feature_set.clone()),
                            0,
                            &self.accounts.sysvar_cache,
                        ),
                        self.log_collector.clone(),
                        self.compute_budget,
                    ),
                    #[cfg(not(target_os = "zkvm"))]
                    &mut ExecuteTimings::default(),
                    &mut accumulated_consume_units,
                )
                .map(|_| ());

                if let Err(err) = self.check_accounts_rent(tx, &context) {
                    tx_result = Err(err);
                };

                (
                    tx_result,
                    accumulated_consume_units,
                    Some(context),
                    fee,
                    payer_key,
                    fee_payer_rent_debit,
                )
            }
            Err(e) => {
                (Err(e), accumulated_consume_units, None, fee, payer_key, fee_payer_rent_debit)
            }
        }
    }

    fn check_accounts_rent(
        &mut self,
        tx: &SanitizedTransaction,
        context: &TransactionContext,
    ) -> Result<(), TransactionError> {
        for index in 0..tx.message().account_keys().len() {
            if tx.message().is_writable(index) {
                let account = context
                    .get_account_at_index(index as IndexOfAccount)
                    .map_err(|err| TransactionError::InstructionError(index as u8, err))?
                    .borrow();
                let pubkey = context
                    .get_key_of_account_at_index(index as IndexOfAccount)
                    .map_err(|err| TransactionError::InstructionError(index as u8, err))?;

                if !account.data().is_empty() {
                    let post_rent_state = RentState::from_account(&account, &self.rent);
                    let pre_rent_state = RentState::from_account(
                        &self.accounts.get_account(pubkey).unwrap_or_default(),
                        &self.rent,
                    );

                    if !post_rent_state.transition_allowed_from(&pre_rent_state) {
                        return Err(TransactionError::InsufficientFundsForRent {
                            account_index: index as u8,
                        });
                    }
                }
            }
        }
        Ok(())
    }

    fn execute_transaction_no_verify(&mut self, tx: VersionedTransaction) -> ExecutionResult {
        match self.sanitize_transaction_no_verify(tx) {
            Ok(sanitized_tx) => self.execute_sanitized_transaction(sanitized_tx),
            Err(execution_result) => execution_result,
        }
    }

    fn execute_transaction(&mut self, tx: VersionedTransaction) -> ExecutionResult {
        match self.sanitize_transaction(tx) {
            Ok(sanitized_tx) => self.execute_sanitized_transaction(sanitized_tx),
            Err(execution_result) => execution_result,
        }
    }

    fn execute_sanitized_transaction(
        &mut self,
        sanitized_tx: SanitizedTransaction,
    ) -> ExecutionResult {
        let CheckAndProcessTransactionSuccess {
            core:
                CheckAndProcessTransactionSuccessCore {
                    result,
                    compute_units_consumed,
                    context,
                    fee_payer_rent_debit: _, // TODO: return rent debit if tx failed
                },
            fee,
            payer_key,
        } = match self.check_and_process_transaction(&sanitized_tx) {
            Ok(value) => value,
            Err(value) => return value,
        };
        if let Some(ctx) = context {
            let tx_result = self.check_tx_result(result, payer_key, fee);
            execution_result_if_context(&sanitized_tx, ctx, tx_result, compute_units_consumed, fee)
        } else {
            ExecutionResult::result_and_compute_units(result, compute_units_consumed, fee)
        }
    }

    fn execute_sanitized_transaction_readonly(
        &mut self,
        sanitized_tx: SanitizedTransaction,
    ) -> ExecutionResult {
        let CheckAndProcessTransactionSuccess {
            core:
                CheckAndProcessTransactionSuccessCore {
                    result,
                    compute_units_consumed,
                    context,
                    fee_payer_rent_debit: _,
                },
            fee,
            ..
        } = match self.check_and_process_transaction(&sanitized_tx) {
            Ok(value) => value,
            Err(value) => return value,
        };
        if let Some(ctx) = context {
            execution_result_if_context(&sanitized_tx, ctx, result, compute_units_consumed, fee)
        } else {
            ExecutionResult::result_and_compute_units(result, compute_units_consumed, fee)
        }
    }

    fn check_tx_result(
        &mut self,
        result: Result<(), TransactionError>,
        payer_key: Option<Pubkey>,
        fee: u64,
    ) -> Result<(), TransactionError> {
        if result.is_ok() {
            result
        } else if let Some(payer) = payer_key {
            self.accounts
                .burn(false, &payer, fee)
                .map_err(|_| TransactionError::InsufficientFundsForFee)
        } else {
            result
        }
    }

    fn check_and_process_transaction(
        &mut self,
        sanitized_tx: &SanitizedTransaction,
    ) -> Result<CheckAndProcessTransactionSuccess, ExecutionResult> {
        let tx_details = self
            .maybe_blockhash_check(sanitized_tx)
            .map_err(|e| ExecutionResult { tx_result: Err(e), ..Default::default() })?;
        let compute_budget_limits = get_compute_budget_limits(sanitized_tx)?;
        let (result, compute_units_consumed, context, fee, payer_key, fee_payer_rent_debit) =
            self.process_transaction(sanitized_tx, tx_details, compute_budget_limits);
        Ok(CheckAndProcessTransactionSuccess {
            core: {
                CheckAndProcessTransactionSuccessCore {
                    result,
                    compute_units_consumed,
                    context,
                    fee_payer_rent_debit,
                }
            },
            fee,
            payer_key,
        })
    }

    fn maybe_blockhash_check(
        &mut self,
        sanitized_tx: &SanitizedTransaction,
    ) -> TransactionCheckResult {
        if self.blockhash_verify {
            let last_blockhash = self.get_blockhash_queue().last_hash();
            let next_durable_nonce = DurableNonce::from_blockhash(&last_blockhash);
            self.check_transaction_age(sanitized_tx, &next_durable_nonce)
        } else {
            Ok(CheckedTransactionDetails {
                nonce: None,
                lamports_per_signature: self.get_lamports_per_signature(),
            })
        }
    }

    fn execute_transaction_readonly(&mut self, tx: VersionedTransaction) -> ExecutionResult {
        match self.sanitize_transaction(tx) {
            Ok(sanitized_tx) => self.execute_sanitized_transaction_readonly(sanitized_tx),
            Err(execution_result) => execution_result,
        }
    }

    fn execute_transaction_no_verify_readonly(
        &mut self,
        tx: VersionedTransaction,
    ) -> ExecutionResult {
        match self.sanitize_transaction_no_verify(tx) {
            Ok(sanitized_tx) => self.execute_sanitized_transaction_readonly(sanitized_tx),
            Err(execution_result) => execution_result,
        }
    }

    fn seal_block(&mut self, results: &[TransactionResult]) -> Result<(), LiteSVMError> {
        let mut fees = 0;
        results.iter().for_each(|r| match r {
            Ok(meta) => fees += meta.fee,
            Err(err) => fees += err.meta.fee,
        });
        let fee_collector = self.leader_schedule.leader_at_slot(self.slot);
        if fees > 0 && fee_collector.is_some() {
            let validate_fee_collector =
                self.feature_set.is_active(&feature_set::validate_fee_collector_account::id());
            self.accounts.mint(validate_fee_collector, &fee_collector.unwrap(), fees)?;
        }
        self.update_slot_history()?;
        self.update_fee_rate_governor()?;

        Ok(())
    }

    fn execute_batch_transactions(
        &mut self,
        batch_txs: &[VersionedTransaction],
    ) -> Vec<TransactionResult> {
        // TODO: verify batch txs conflict or not?
        batch_txs.into_iter().map(|tx| self.send_transaction(tx.clone())).collect()
    }

    pub fn execute_block(
        &mut self,
        block: RawBlock,
    ) -> Result<Vec<TransactionResult>, LiteSVMError> {
        let batch_txs = block.0;
        let mut results = Vec::with_capacity(batch_txs.len());
        for tx_batch in batch_txs.iter() {
            let res = self.execute_batch_transactions(tx_batch);
            results.extend(res);
        }
        self.seal_block(&results)?;
        self.process_entries_and_register_blockhash(&batch_txs)?;
        Ok(results)
    }

    /// Submits a signed transaction.
    pub fn send_transaction(&mut self, tx: VersionedTransaction) -> TransactionResult {
        let ExecutionResult {
            post_accounts,
            tx_result,
            signature,
            signature_count,
            compute_units_consumed,
            inner_instructions,
            return_data,
            fee,
        } = if self.sig_verify {
            self.execute_transaction(tx)
        } else {
            self.execute_transaction_no_verify(tx)
        };

        // add signature
        self.signature_count += signature_count as u64;

        let meta = TransactionMetadata {
            logs: self
                .log_collector
                .as_ref()
                .map(|log_collector| {
                    log_collector
                        .replace_with(|old| LogCollector {
                            messages: Vec::new(),
                            bytes_written: old.bytes_written,
                            bytes_limit: old.bytes_limit,
                            limit_warning: old.limit_warning,
                        })
                        .into_messages()
                })
                .unwrap_or_else(Vec::new),
            inner_instructions,
            compute_units_consumed,
            return_data,
            signature,
            fee,
        };

        if let Err(tx_err) = tx_result {
            let err = Err(FailedTransactionMetadata { err: tx_err, meta });
            err
        } else {
            for (key, mut account) in post_accounts {
                // TODO: is putting collect rent here correct?
                // collect rent
                collect_rent_from_account(
                    &self.feature_set,
                    &self.rent_collector,
                    &key,
                    &mut account,
                );

                self.accounts.add_diff_account(false, key, account).map_err(|_| {
                    FailedTransactionMetadata {
                        err: TransactionError::InvalidRentPayingAccount,
                        meta: meta.clone(),
                    }
                })?;
            }
            Ok(meta)
        }
    }

    /// Simulates a transaction.
    pub fn simulate_transaction(
        &mut self,
        tx: impl Into<VersionedTransaction>,
    ) -> Result<SimulatedTransactionInfo, FailedTransactionMetadata> {
        let ExecutionResult {
            post_accounts,
            tx_result,
            signature,
            compute_units_consumed,
            inner_instructions,
            return_data,
            fee,
            ..
        } = if self.sig_verify {
            self.execute_transaction_readonly(tx.into())
        } else {
            self.execute_transaction_no_verify_readonly(tx.into())
        };

        let meta = TransactionMetadata {
            signature,
            logs: self
                .log_collector
                .as_ref()
                .map(|log_collector| {
                    log_collector
                        .replace_with(|old| LogCollector {
                            messages: Vec::new(),
                            bytes_written: old.bytes_written,
                            bytes_limit: old.bytes_limit,
                            limit_warning: old.limit_warning,
                        })
                        .into_messages()
                })
                .unwrap_or_else(Vec::new),
            inner_instructions,
            compute_units_consumed,
            return_data,
            fee,
        };

        if let Err(tx_err) = tx_result {
            Err(FailedTransactionMetadata { err: tx_err, meta })
        } else {
            Ok(SimulatedTransactionInfo { meta, post_accounts })
        }
    }

    /// Gets the current compute budget.
    pub fn get_compute_budget(&self) -> &ComputeBudget {
        &self.compute_budget
    }

    #[cfg(feature = "internal-test")]
    pub fn get_feature_set(&self) -> Arc<FeatureSet> {
        self.feature_set.clone()
    }

    fn check_transaction_age(
        &mut self,
        tx: &SanitizedTransaction,
        next_durable_nonce: &DurableNonce,
    ) -> TransactionCheckResult {
        let recent_blockhash = tx.message().recent_blockhash();
        if let Some(hash_info) =
            self.get_blockhash_queue().get_hash_info_if_valid(recent_blockhash, MAX_PROCESSING_AGE)
        {
            Ok(CheckedTransactionDetails {
                nonce: None,
                lamports_per_signature: hash_info.lamports_per_signature(),
            })
        } else if let Some((nonce, nonce_data)) =
            self.check_and_load_message_nonce_account(tx.message(), next_durable_nonce)
        {
            Ok(CheckedTransactionDetails {
                nonce: Some(nonce),
                lamports_per_signature: nonce_data.get_lamports_per_signature(),
            })
        } else if tx.message().fee_payer() == &NO_SIG_TX_PAYER {
            // TODO: should use native transaction verification logic, only use fee payer is not
            // correct Native transaction do not pay fees
            Ok(CheckedTransactionDetails { nonce: None, lamports_per_signature: 0 })
        } else {
            Err(TransactionError::BlockhashNotFound)
        }
    }

    fn check_and_load_message_nonce_account(
        &mut self,
        message: &SanitizedMessage,
        next_durable_nonce: &DurableNonce,
    ) -> Option<(NoncePartial, nonce::state::Data)> {
        if message.recent_blockhash() == next_durable_nonce.as_hash() {
            return None;
        }

        let nonce_address = message.get_durable_nonce()?;
        let nonce_account = self.accounts.get_account(nonce_address)?;
        let nonce_data =
            nonce_account::verify_nonce_account(&nonce_account, message.recent_blockhash())?;

        let nonce_is_authorized = message
            .get_ix_signers(NONCED_TX_MARKER_IX_INDEX as usize)
            .any(|signer| signer == &nonce_data.authority);
        if !nonce_is_authorized {
            return None;
        }

        Some((NoncePartial::new(*nonce_address, nonce_account), nonce_data))
    }

    fn update_clock(&mut self) -> Result<(), LiteSVMError> {
        let epoch_start_timestamp = if self.epoch_schedule.get_epoch(self.parent_slot) != self.epoch
        {
            todo!("Epoch change not supported yet")
        } else {
            let clock: Clock = self.get_sysvar()?;
            clock.epoch_start_timestamp
        };

        let clock = Clock {
            slot: self.slot,
            epoch: self.epoch,
            leader_schedule_epoch: self.epoch_schedule.get_leader_schedule_epoch(self.slot),
            unix_timestamp: self.clock_timestamp,
            epoch_start_timestamp,
        };
        self.update_sysvar(clock)?;
        Ok(())
    }

    fn update_slot_history(&mut self) -> Result<(), LiteSVMError> {
        let mut slot_history: solana_program::slot_history::SlotHistory =
            self.get_sysvar().unwrap_or_default();
        slot_history.add(self.slot);
        self.update_sysvar(slot_history)?;
        Ok(())
    }

    fn update_slot_hashes(&mut self) -> Result<(), LiteSVMError> {
        let mut slot_hashes: solana_program::slot_hashes::SlotHashes =
            self.get_sysvar().unwrap_or_default();
        slot_hashes.add(self.parent_slot, self.parent_bank_hash);
        self.update_sysvar(slot_hashes)?;
        Ok(())
    }

    #[allow(deprecated)]
    fn update_recent_blockhashes_locked(&mut self) -> Result<(), LiteSVMError> {
        let recent_blockhash_iter = self.get_blockhash_queue().get_recent_blockhashes();
        let sorted = BinaryHeap::from_iter(recent_blockhash_iter);
        // solana recent blockhashes sysvar
        let solana_recent_blockhashes: solana_sysvar::recent_blockhashes::RecentBlockhashes =
            IntoIterSorted::new(sorted.clone())
                .take(solana_sysvar::recent_blockhashes::MAX_ENTRIES)
                .collect();
        // soon recent blockhashes sysvar
        let soon_recent_blockhashes: sysvar::recent_blockhashes::SoonRecentBlockhashes =
            IntoIterSorted::new(sorted).take(sysvar::recent_blockhashes::MAX_ENTRIES).collect();
        // update both sysvars
        self.update_sysvar(solana_recent_blockhashes)?;
        self.update_sysvar(soon_recent_blockhashes)?;

        Ok(())
    }

    fn update_fee_rate_governor(&mut self) -> Result<(), LiteSVMError> {
        let fee_rate_governor = sysvar::fee_rate_governor::SoonFeeRateGovernor {
            fee_rate_governor: self.fee_rate_governor.clone(),
            signature_count: self.signature_count,
        };
        self.update_sysvar(fee_rate_governor)?;
        Ok(())
    }

    fn process_entries_and_register_blockhash(
        &mut self,
        batch_txs: &[Vec<VersionedTransaction>],
    ) -> Result<(), LiteSVMError> {
        // process entries
        let data_entries = self.batches_to_data_entries(batch_txs)?;
        // complete entries
        let entries = self.complete_entries(data_entries)?;
        // register ticks
        let mut count = 0;
        for entry in entries {
            if entry.is_tick() {
                count += 1;
                if count == self.ticks_per_slot {
                    // the hash of the entry is current blockhash !
                    self.blockhash = Some(entry.hash);
                    self.register_recent_blockhash(entry.hash)?;
                    break;
                }
            }
        }
        Ok(())
    }

    fn register_recent_blockhash(&mut self, blockhash: Hash) -> Result<(), LiteSVMError> {
        self.blockhash_queue
            .register_hash(blockhash, self.fee_rate_governor.lamports_per_signature);
        self.update_recent_blockhashes_locked()?;
        Ok(())
    }

    fn complete_entries(&self, mut data_entries: Vec<Entry>) -> Result<Vec<Entry>, LiteSVMError> {
        let last_entry = data_entries.last().ok_or(LiteSVMError::NoEntries)?;
        let mut start_hash = last_entry.hash;

        let tick_count = self
            .ticks_per_slot
            .saturating_sub(data_entries.iter().filter(|entry| entry.is_tick()).count() as u64);

        for _ in 0..tick_count {
            let entry = new_entry(&start_hash, self.hashes_per_tick, vec![]);
            start_hash = entry.hash;
            data_entries.push(entry);
        }

        Ok(data_entries)
    }

    fn batches_to_data_entries(
        &self,
        batch_txs: &[Vec<VersionedTransaction>],
    ) -> Result<Vec<Entry>, LiteSVMError> {
        let mut data_entries = Vec::with_capacity(batch_txs.len());
        let mut start_hash = None;
        for executed_txs in batch_txs.iter() {
            // maybe there's no successful execution transactions in one entry.
            // this entry is actually a tick, we should ignore this tick instead of add it.
            // otherwise the ticks may be over limit(64/slot).
            if executed_txs.is_empty() {
                continue;
            }
            let entry = self.transactions_to_entry(executed_txs.clone(), start_hash)?;
            start_hash = Some(entry.hash);
            data_entries.push(entry);
        }
        Ok(data_entries)
    }

    fn transactions_to_entry(
        &self,
        transactions: Vec<VersionedTransaction>,
        start_hash: Option<Hash>,
    ) -> Result<Entry, LiteSVMError> {
        let start_hash = match start_hash {
            Some(start_hash) => start_hash,
            None => self.blockhash_queue.last_hash(),
        };
        Ok(new_entry(&start_hash, self.hashes_per_tick, transactions))
    }
}

#[allow(dead_code)]
struct CheckAndProcessTransactionSuccessCore {
    result: Result<(), TransactionError>,
    compute_units_consumed: u64,
    context: Option<TransactionContext>,
    fee_payer_rent_debit: u64,
}

struct CheckAndProcessTransactionSuccess {
    core: CheckAndProcessTransactionSuccessCore,
    fee: u64,
    payer_key: Option<Pubkey>,
}

fn execution_result_if_context(
    sanitized_tx: &SanitizedTransaction,
    ctx: TransactionContext,
    result: Result<(), TransactionError>,
    compute_units_consumed: u64,
    fee: u64,
) -> ExecutionResult {
    let (signature, signature_count, return_data, inner_instructions, post_accounts) =
        execute_tx_helper(sanitized_tx, ctx);
    ExecutionResult {
        tx_result: result,
        signature,
        signature_count,
        post_accounts,
        inner_instructions,
        compute_units_consumed,
        return_data,
        fee,
    }
}

fn execute_tx_helper(
    sanitized_tx: &SanitizedTransaction,
    ctx: TransactionContext,
) -> (
    Signature,
    u8,
    solana_sdk::transaction_context::TransactionReturnData,
    InnerInstructionsList,
    Vec<(Pubkey, AccountSharedData)>,
) {
    let signature = sanitized_tx.signature().to_owned();
    let inner_instructions = inner_instructions_list_from_instruction_trace(&ctx);
    let ExecutionRecord {
        accounts,
        return_data,
        touched_account_count: _,
        accounts_resize_delta: _,
    } = ctx.into();
    let msg = sanitized_tx.message();
    let post_accounts = accounts
        .into_iter()
        .enumerate()
        .filter_map(|(idx, pair)| msg.is_writable(idx).then_some(pair))
        .collect();
    (
        signature,
        msg.header().num_required_signatures,
        return_data,
        inner_instructions,
        post_accounts,
    )
}

fn get_compute_budget_limits(
    sanitized_tx: &SanitizedTransaction,
) -> Result<ComputeBudgetLimits, ExecutionResult> {
    let instructions = sanitized_tx.message().program_instructions_iter();
    process_compute_budget_instructions(instructions)
        .map_err(|e| ExecutionResult { tx_result: Err(e), ..Default::default() })
}

/// Lighter version of the one in the solana-svm crate.
///
/// Check whether the payer_account is capable of paying the fee. The
/// side effect is to subtract the fee amount from the payer_account
/// balance of lamports. If the payer_acount is not able to pay the
/// fee a specific error is returned.
fn validate_fee_payer(
    payer_address: &Pubkey,
    payer_account: &mut AccountSharedData,
    payer_index: IndexOfAccount,
    rent: &Rent,
    fee: u64,
) -> solana_sdk::transaction::Result<()> {
    if payer_account.lamports() == 0 {
        error!("Payer account {payer_address} not found.");
        return Err(TransactionError::AccountNotFound);
    }
    let system_account_kind = get_system_account_kind(payer_account).ok_or_else(|| {
        error!("Payer account {payer_address} is not a system account");
        TransactionError::InvalidAccountForFee
    })?;
    let min_balance = match system_account_kind {
        SystemAccountKind::System => 0,
        SystemAccountKind::Nonce => {
            // Should we ever allow a fees charge to zero a nonce account's
            // balance. The state MUST be set to uninitialized in that case
            rent.minimum_balance(nonce::State::size())
        }
    };

    let payer_lamports = payer_account.lamports();

    payer_lamports.checked_sub(min_balance).and_then(|v| v.checked_sub(fee)).ok_or_else(|| {
        error!(
            "Payer account {payer_address} has insufficient lamports for fee. Payer lamports: \
                {payer_lamports} min_balance: {min_balance} fee: {fee}"
        );
        TransactionError::InsufficientFundsForFee
    })?;

    let payer_pre_rent_state = RentState::from_account(payer_account, rent);
    // we already checked above if we have sufficient balance so this should never error.
    payer_account.checked_sub_lamports(fee).unwrap();

    let payer_post_rent_state = RentState::from_account(payer_account, rent);
    check_rent_state_with_account(
        &payer_pre_rent_state,
        &payer_post_rent_state,
        payer_address,
        payer_index,
    )
}

// modified version of the private fn in solana-svm
fn check_rent_state_with_account(
    pre_rent_state: &RentState,
    post_rent_state: &RentState,
    address: &Pubkey,
    account_index: IndexOfAccount,
) -> solana_sdk::transaction::Result<()> {
    if !solana_sdk::incinerator::check_id(address) &&
        !post_rent_state.transition_allowed_from(pre_rent_state)
    {
        let account_index = account_index as u8;
        error!("Transaction would leave account {address} with insufficient funds for rent");
        Err(TransactionError::InsufficientFundsForRent { account_index })
    } else {
        Ok(())
    }
}

fn new_entry(
    prev_hash: &Hash,
    mut num_hashes: u64,
    transactions: Vec<VersionedTransaction>,
) -> Entry {
    // If you passed in transactions, but passed in num_hashes == 0, then
    // next_hash will generate the next hash and set num_hashes == 1
    if num_hashes == 0 && !transactions.is_empty() {
        num_hashes = 1;
    }

    let hash = next_hash(prev_hash, num_hashes, &transactions);
    Entry { num_hashes, hash, transactions }
}
