#![allow(missing_docs)]

use crate::{
    accounts_db::AccountsDb,
    builtin::BUILTINS,
    error::LiteSVMError,
    history::TransactionHistory,
    // spl::load_spl_programs,
    types::{ExecutionResult, FailedTransactionMetadata, TransactionMetadata, TransactionResult},
    utils::rent::RentState,
};
use solana_bpf_loader_program::syscalls::create_program_runtime_environment_v1;
use solana_compute_budget::{
    compute_budget::ComputeBudget,
    compute_budget_processor::{process_compute_budget_instructions, ComputeBudgetLimits},
};
use solana_loader_v4_program::create_program_runtime_environment_v2;
use solana_program_runtime::{
    invoke_context::{EnvironmentConfig, InvokeContext},
    loaded_programs::ProgramCacheEntry,
    log_collector::LogCollector,
    timings::ExecuteTimings,
};
use solana_sdk::{
    account::{Account, AccountSharedData, ReadableAccount, WritableAccount},
    feature_set, feature_set::{
        include_loaded_accounts_data_size_in_fee_calculation, remove_rounding_in_fee_calculation,
        FeatureSet,
    },
    fee::FeeStructure,
    inner_instruction::InnerInstructionsList,
    message::SanitizedMessage,
    native_loader,
    nonce::{state::DurableNonce, NONCED_TX_MARKER_IX_INDEX},
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
use solana_svm::{account_loader::collect_rent_from_account, message_processor::MessageProcessor};
use solana_system_program::{get_system_account_kind, SystemAccountKind};
use std::fmt::Debug;
use std::{cell::RefCell, rc::Rc, sync::Arc};
use std::collections::BinaryHeap;
use solana_program::clock::{Clock, Epoch, Slot, INITIAL_RENT_EPOCH};
use solana_program::epoch_schedule::EpochSchedule;
use solana_program::fee_calculator::FeeRateGovernor;
use solana_program::hash::Hash;
use solana_program::nonce;
use solana_program::sysvar;
use solana_program::sysvar::recent_blockhashes::IntoIterSorted;
use solana_program_runtime::loaded_programs::ProgramRuntimeEnvironments;
use solana_svm::account_loader::{CheckedTransactionDetails, TransactionCheckResult};
use solana_svm::nonce_info::NoncePartial;
use tracing::error;
use types::SimulatedTransactionInfo;
use utils::{
    construct_instructions_account,
    inner_instructions::inner_instructions_list_from_instruction_trace,
};
use accounts_callback::AccountsCallback;
use crate::genesis::*;

pub mod error;
pub mod types;
pub mod accounts_callback;
pub mod genesis;

mod accounts_db;
mod builtin;
mod history;
// mod spl;
mod utils;
mod blockhash_queue;
mod settings;
mod parent;

pub use settings::Settings;
pub use blockhash_queue::BlockhashQueue;
pub use parent::ParentState;

// The test code doesn't actually get run because it's not
// what doctest expects but at least it
// compiles it so we'll see if there's a compile-time error.
#[doc = include_str!("../../README.md")]
#[cfg(doctest)]
pub struct ReadmeDoctests;

pub struct LiteSVM<CB: AccountsCallback> {
    settings: Settings,
    accounts: AccountsDb<CB>,
    feature_set: FeatureSet,
    log_collector: Rc<RefCell<LogCollector>>,
    history: TransactionHistory,
    compute_budget: Option<ComputeBudget>,
    sigverify: bool,
    blockhash_verify: bool,

    slots_per_year: f64,
    fee_rate_governor: FeeRateGovernor,
    fee_structure: FeeStructure,
    epoch_schedule: EpochSchedule,
    rent: Rent,

    slot: Slot,
    epoch: Epoch,

    clock_timestamp: i64,
    // TODO: blockhash should be computed by: transactions -> data entries -> blockhash
    blockhash: Hash,
    parent_state: ParentState,

    log_bytes_limit: Option<usize>,
    rent_collector: RentCollector,
    fee_collector: Option<Pubkey>,
}

impl<CB: AccountsCallback> Default for LiteSVM<CB> {
    fn default() -> Self {
        Self {
            settings: Settings::default(),
            accounts: Default::default(),
            feature_set: Default::default(),
            log_collector: Default::default(),
            history: TransactionHistory::new(),
            compute_budget: None,
            sigverify: false,
            blockhash_verify: false,
            epoch_schedule: Default::default(),
            rent: Default::default(),
            slot: 0,
            epoch: 0,
            clock_timestamp: 0,
            blockhash: Default::default(),
            parent_state: ParentState::default(),
            slots_per_year: soon_slots_per_year(),
            fee_rate_governor: FeeRateGovernor::default(),
            fee_structure: Default::default(),
            log_bytes_limit: Some(10_000),
            rent_collector: Default::default(),
            fee_collector: None,
        }
    }
}

impl<CB: AccountsCallback> Debug for LiteSVM<CB> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LiteSVM")?;
        write!(f, "compute_budget: {:?}", self.compute_budget)?;
        write!(f, "sigverify: {}", self.sigverify)?;
        write!(f, "fee_structure: {:?}", self.fee_structure)?;
        write!(f, "log_bytes_limit: {:?}", self.log_bytes_limit)?;
        Ok(())
    }
}

impl<CB: AccountsCallback> LiteSVM<CB> {
    /// Creates the basic test environment.
    pub fn new_soon() -> Self {
        Self::default()
            .with_builtins()
            // .with_sigverify(true)
            .with_compute_budget(soon_compute_budget())
            .with_rent(soon_rent())
            .with_feature_set(soon_feature_set())
    }

    pub fn new_soon_from_parent(parent: ParentState) -> Self {
        Self::default()
            .with_builtins()
            // .with_sigverify(true)
            .with_compute_budget(soon_compute_budget())
            .with_rent(soon_rent())
            .with_feature_set(soon_feature_set())
            .with_parent_state(parent)
    }

    pub fn finish_init(&mut self) -> Result<(), LiteSVMError> {
        // set slot and epoch from parent
        self.slot = self.parent_state.new_slot();
        self.epoch = self.parent_state.new_epoch();
        self.accounts.set_slot(self.slot);
        self.accounts.set_epoch(self.epoch);
        // update fee rate governer
        self.fee_rate_governor = self.parent_state.new_fee_rate_governor();

        // create environments
        let program_runtime_v1 = create_program_runtime_environment_v1(
            &self.feature_set,
            &self.compute_budget.unwrap_or_default(),
            false,
            false,
        )
        .unwrap();
        let program_runtime_v2 =
            create_program_runtime_environment_v2(&self.compute_budget.unwrap_or_default(), false);
        self.accounts.set_environments(ProgramRuntimeEnvironments {
            program_runtime_v1: Arc::new(program_runtime_v1),
            program_runtime_v2: Arc::new(program_runtime_v2),
        });

        // update sysvars
        self.update_slot_hashes()?;
        self.update_clock()?;
        self.update_slot_history()?;

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

    /// Sets the slot and epoch for the accounts db.
    pub fn with_slot_and_epoch(mut self, slot: Slot, epoch: Epoch) -> Self {
        self.slot = slot;
        self.epoch = epoch;
        self.accounts.set_slot(slot);
        self.accounts.set_epoch(epoch);
        self
    }

    pub fn with_parent_state(mut self, parent: ParentState) -> Self {
        self.parent_state = parent;
        self
    }

    pub fn with_clock_timestamp(mut self, timestamp: i64) -> Self {
        self.clock_timestamp = timestamp;
        self
    }

    pub fn with_blockhash(mut self, blockhash: Hash) -> Self {
        self.blockhash = blockhash;
        self
    }

    pub const fn with_settings(mut self, settings: Settings) -> Self {
        self.settings = settings;
        self
    }

    /// Sets the compute budget.
    pub const fn with_compute_budget(mut self, compute_budget: ComputeBudget) -> Self {
        self.compute_budget = Some(compute_budget);
        self
    }

    /// Enables or disables sigverify.
    pub const fn with_sigverify(mut self, sigverify: bool) -> Self {
        self.sigverify = sigverify;
        self
    }

    pub const fn with_blockhash_verify(mut self, blockhash_verify: bool) -> Self {
        self.blockhash_verify = blockhash_verify;
        self
    }

    pub const fn with_fee_collector(mut self, collector: Option<Pubkey>) -> Self {
        self.fee_collector = collector;
        self
    }

    /// Sets the accounts db callback.
    pub fn with_accounts_callback(mut self, callback: CB) -> Self {
        self.accounts.set_callback(callback);
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

    // /// Includes the standard SPL programs.
    // pub fn with_spl_programs(mut self) -> Self {
    //     load_spl_programs(&mut self);
    //     self
    // }

    /// Changes the capacity of the transaction history.
    /// Set this to 0 to disable transaction history and allow duplicate transactions.
    pub fn with_transaction_history(mut self, capacity: usize) -> Self {
        self.history.set_capacity(capacity);
        self
    }

    pub const fn with_log_bytes_limit(mut self, limit: usize) -> Self {
        self.log_bytes_limit = Some(limit);
        self
    }

    /// Returns all information associated with the account of the provided pubkey.
    pub fn get_account(&self, pubkey: &Pubkey) -> Option<Account> {
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

    pub fn feature_set(&self) -> &FeatureSet {
        &self.feature_set
    }

    pub fn get_lamports_per_signature(&self) -> u64 {
        self.fee_rate_governor.lamports_per_signature
    }

    pub fn get_blockhash_queue(&self) -> &BlockhashQueue {
        &self.parent_state.blockhash_queue
    }

    /// Gets the balance of the provided account pubkey.
    pub fn get_balance(&self, pubkey: &Pubkey) -> Option<u64> {
        self.accounts.get_account(pubkey).map(|x| x.lamports())
    }

    /// Gets a sysvar from the test environment.
    pub fn get_sysvar<T>(&self) -> Result<T, LiteSVMError>
    where
        T: Sysvar + SysvarId,
    {
        let account = self
            .accounts
            .get_account(&T::id())
            .ok_or(LiteSVMError::MissingAccount(T::id()))?;
        bincode::deserialize(account.data()).map_err(|e| LiteSVMError::Bincode(e))
    }

    fn update_sysvar<T>(&mut self, sysvar: T) -> Result<(), LiteSVMError>
    where
        T: Sysvar + SysvarId,
    {
        let mut account = Account::new(
            self.rent.minimum_balance(T::size_of()),
            T::size_of(),
            &sysvar::id(),
        );
        solana_sdk::account::to_account::<_, Account>(&sysvar, &mut account).unwrap();
        account.rent_epoch = INITIAL_RENT_EPOCH;

        // add to accounts db
        self.accounts.add_diff_account(false, T::id(), account.into())?;
        Ok(())
    }

    /// Gets a transaction from the transaction history.
    pub fn get_transaction(&self, signature: &Signature) -> Option<&TransactionResult> {
        self.history.get_transaction(signature)
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
        &self,
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
        &self,
        tx: VersionedTransaction,
    ) -> Result<SanitizedTransaction, ExecutionResult> {
        self.sanitize_transaction_no_verify_inner(tx)
            .map_err(|err| ExecutionResult { tx_result: Err(err), ..Default::default() })
    }

    fn sanitize_transaction(
        &self,
        tx: VersionedTransaction,
    ) -> Result<SanitizedTransaction, ExecutionResult> {
        self.sanitize_transaction_inner(tx)
            .map_err(|err| ExecutionResult { tx_result: Err(err), ..Default::default() })
    }

    fn sanitize_transaction_inner(
        &self,
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
            nonce, // TODO: where should use the nonce?
            lamports_per_signature,
        } = tx_details;
        let compute_budget = self.compute_budget.unwrap_or_else(|| ComputeBudget {
            compute_unit_limit: u64::from(compute_budget_limits.compute_unit_limit),
            heap_size: compute_budget_limits.updated_heap_bytes,
            ..ComputeBudget::default()
        });
        let blockhash = tx.message().recent_blockhash();
        let mut accumulated_consume_units = 0;
        let message = tx.message();
        let account_keys = message.account_keys();

        let fee = self.fee_structure.calculate_fee(
            message,
            lamports_per_signature,
            &compute_budget_limits.into(),
            self.feature_set
                .is_active(&include_loaded_accounts_data_size_in_fee_calculation::id()),
            self.feature_set.is_active(&remove_rounding_in_fee_calculation::id()),
        );
        let mut validated_fee_payer = false;
        let mut payer_key = None;
        let mut fee_payer_rent_debit = 0;
        let maybe_accounts = account_keys
            .iter()
            .enumerate()
            .map(|(i, key)| {
                let account = if sysvar::instructions::check_id(key) {
                    construct_instructions_account(message)
                } else {
                    let mut account = self
                        .accounts
                        .load_account(key)
                        .map_err(|_| TransactionError::AccountNotFound)?
                        .unwrap_or_default();
                    if !validated_fee_payer
                        && (!message.is_invoked(i) || message.is_instruction_account(i))
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
                let mut context = self.create_transaction_context(compute_budget, accounts);
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
                        Some(self.log_collector.clone()),
                        compute_budget,
                    ),
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
        &self,
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
        map_sanitize_result(self.sanitize_transaction_no_verify(tx), |s_tx| {
            self.execute_sanitized_transaction(s_tx)
        })
    }

    fn execute_transaction(&mut self, tx: VersionedTransaction) -> ExecutionResult {
        map_sanitize_result(self.sanitize_transaction(tx), |s_tx| {
            self.execute_sanitized_transaction(s_tx)
        })
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
                    fee_payer_rent_debit, // TODO: return rent debit if tx failed
                },
            fee,
            payer_key,
        } = match self.check_and_process_transaction(&sanitized_tx) {
            Ok(value) => value,
            Err(value) => return value,
        };
        if let Some(ctx) = context {
            let tx_result = self.check_tx_result(result, payer_key, fee);
            execution_result_if_context(sanitized_tx, ctx, tx_result, compute_units_consumed, fee)
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
            execution_result_if_context(sanitized_tx, ctx, result, compute_units_consumed, fee)
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
            .map_err(|e| ExecutionResult {
                tx_result: Err(e),
                ..Default::default()
            })?;
        let compute_budget_limits = get_compute_budget_limits(sanitized_tx)?;
        self.maybe_history_check(sanitized_tx)?;
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

    fn maybe_history_check(
        &self,
        sanitized_tx: &SanitizedTransaction,
    ) -> Result<(), ExecutionResult> {
        if self.history.check_transaction(sanitized_tx.signature()) {
            return Err(ExecutionResult {
                tx_result: Err(TransactionError::AlreadyProcessed),
                ..Default::default()
            });
        }
        Ok(())
    }

    fn maybe_blockhash_check(
        &self,
        sanitized_tx: &SanitizedTransaction,
    ) -> TransactionCheckResult {
        if self.blockhash_verify {
            let blockhash_queue = self.get_blockhash_queue();
            let last_blockhash = blockhash_queue.last_hash();
            let next_durable_nonce = DurableNonce::from_blockhash(&last_blockhash);
            self.check_transaction_age(blockhash_queue, sanitized_tx, &next_durable_nonce)
        } else {
            Ok(CheckedTransactionDetails {
                nonce: None,
                lamports_per_signature: self.get_lamports_per_signature(),
            })
        }
    }

    fn execute_transaction_readonly(&mut self, tx: VersionedTransaction) -> ExecutionResult {
        map_sanitize_result(self.sanitize_transaction(tx), |s_tx| {
            self.execute_sanitized_transaction_readonly(s_tx)
        })
    }

    fn execute_transaction_no_verify_readonly(&mut self, tx: VersionedTransaction) -> ExecutionResult {
        map_sanitize_result(self.sanitize_transaction_no_verify(tx), |s_tx| {
            self.execute_sanitized_transaction_readonly(s_tx)
        })
    }

    pub fn seal_block(
        &mut self,
        results: &[TransactionResult],
    ) -> Result<(), LiteSVMError> {
        let mut fees = 0;
        results.iter().for_each(|r| match r {
            Ok(meta) => fees += meta.fee,
            Err(err) => fees += err.meta.fee,
        });
        if fees > 0 && self.fee_collector.is_some() {
            let validate_fee_collector =
                self.feature_set.is_active(&feature_set::validate_fee_collector_account::id());
            self.accounts.mint(validate_fee_collector, &self.fee_collector.unwrap(), fees)?;
        }

        self.accounts.clean_zero_accounts();
        self.update_slot_history()?;
        self.register_recent_blockhash()?;

        Ok(())
    }

    pub fn execute_block_transactions(
        &mut self,
        txs: Vec<impl Into<VersionedTransaction>>,
    ) -> Result<Vec<TransactionResult>, LiteSVMError> {
        let mut results = Vec::with_capacity(txs.len());
        for tx in txs {
            let res = self.send_transaction(tx.into());
            results.push(res);
        }
        self.seal_block(&results)?;
        Ok(results)
    }

    /// Submits a signed transaction.
    pub fn send_transaction(&mut self, tx: impl Into<VersionedTransaction>) -> TransactionResult {
        let vtx: VersionedTransaction = tx.into();
        let ExecutionResult {
            post_accounts,
            tx_result,
            signature,
            compute_units_consumed,
            inner_instructions,
            return_data,
            included,
            fee,
        } = if self.sigverify {
            self.execute_transaction(vtx)
        } else {
            self.execute_transaction_no_verify(vtx)
        };

        let meta = TransactionMetadata {
            logs: self
                .log_collector
                .replace(LogCollector { bytes_limit: self.log_bytes_limit, ..Default::default() })
                .into_messages(),
            inner_instructions,
            compute_units_consumed,
            return_data,
            signature,
            fee,
        };

        if let Err(tx_err) = tx_result {
            let err = Err(FailedTransactionMetadata { err: tx_err, meta });
            if included {
                self.history.add_new_transaction(signature, err.clone());
            }
            err
        } else {
            self.history.add_new_transaction(signature, Ok(meta.clone()));
            for (key, mut account) in post_accounts {
                // TODO: is putting collect rent here correct?
                // collect rent
                collect_rent_from_account(
                    &self.feature_set,
                    &self.rent_collector,
                    &key,
                    &mut account,
                );

                self.accounts
                    .add_diff_account(false, key, account)
                    .map_err(|_| FailedTransactionMetadata {
                        err: TransactionError::InvalidRentPayingAccount,
                        meta: meta.clone(),
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
        } = if self.sigverify {
            self.execute_transaction_readonly(tx.into())
        } else {
            self.execute_transaction_no_verify_readonly(tx.into())
        };

        let meta = TransactionMetadata {
            signature,
            logs: self
                .log_collector
                .replace(LogCollector { bytes_limit: self.log_bytes_limit, ..Default::default() })
                .into_messages(),
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
    pub fn get_compute_budget(&self) -> Option<ComputeBudget> {
        self.compute_budget
    }

    #[cfg(feature = "internal-test")]
    pub fn get_feature_set(&self) -> Arc<FeatureSet> {
        self.feature_set.clone()
    }

    // fn check_transaction_age(&self, tx: &SanitizedTransaction) -> Result<(), ExecutionResult> {
    //     self.check_transaction_age_inner(tx)
    //         .map_err(|e| ExecutionResult { tx_result: Err(e), ..Default::default() })
    // }

    fn check_transaction_age(
        &self,
        blockhash_queue: &BlockhashQueue,
        tx: &SanitizedTransaction,
        next_durable_nonce: &DurableNonce,
    ) -> TransactionCheckResult {
        let recent_blockhash = tx.message().recent_blockhash();
        if let Some(hash_info) = blockhash_queue.get_hash_info_if_valid(recent_blockhash, self.settings.max_age) {
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
            // TODO: should use native transaction verification logic, only use fee payer is not correct
            // Native transaction do not pay fees
            Ok(CheckedTransactionDetails {
                nonce: None,
                lamports_per_signature: 0,
            })
        } else {
            Err(TransactionError::BlockhashNotFound)
        }
    }

    fn check_and_load_message_nonce_account(
        &self,
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
        let epoch_start_timestamp = if self.parent_state.epoch != self.epoch {
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
        let mut slot_history: solana_program::slot_history::SlotHistory = self.get_sysvar().unwrap_or_default();
        slot_history.add(self.slot);
        self.update_sysvar(slot_history)?;
        Ok(())
    }

    fn update_slot_hashes(&mut self) -> Result<(), LiteSVMError> {
        let mut slot_hashes: solana_program::slot_hashes::SlotHashes = self.get_sysvar().unwrap_or_default();
        slot_hashes.add(self.parent_state.slot, self.parent_state.hash);
        self.update_sysvar(slot_hashes)?;
        Ok(())
    }

    #[allow(deprecated)]
    fn update_recent_blockhashes_locked(&mut self) -> Result<(), LiteSVMError> {
        let recent_blockhash_iter = self.get_blockhash_queue().get_recent_blockhashes();
        let sorted = BinaryHeap::from_iter(recent_blockhash_iter);
        let recent_blockhashes: sysvar::recent_blockhashes::RecentBlockhashes = IntoIterSorted::new(sorted)
            .take(sysvar::recent_blockhashes::MAX_ENTRIES)
            .collect();
        self.update_sysvar(recent_blockhashes)?;
        Ok(())
    }

    fn register_recent_blockhash(&mut self) -> Result<(), LiteSVMError> {
        self.parent_state
            .blockhash_queue
            .register_hash(self.blockhash, self.fee_rate_governor.lamports_per_signature);
        self.update_recent_blockhashes_locked()?;
        Ok(())
    }
}

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
    sanitized_tx: SanitizedTransaction,
    ctx: TransactionContext,
    result: Result<(), TransactionError>,
    compute_units_consumed: u64,
    fee: u64,
) -> ExecutionResult {
    let (signature, return_data, inner_instructions, post_accounts) =
        execute_tx_helper(sanitized_tx, ctx);
    ExecutionResult {
        tx_result: result,
        signature,
        post_accounts,
        inner_instructions,
        compute_units_consumed,
        return_data,
        included: true,
        fee,
    }
}

fn execute_tx_helper(
    sanitized_tx: SanitizedTransaction,
    ctx: TransactionContext,
) -> (
    Signature,
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
    (signature, return_data, inner_instructions, post_accounts)
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
            rent.minimum_balance(solana_sdk::nonce::State::size())
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
    if !solana_sdk::incinerator::check_id(address)
        && !post_rent_state.transition_allowed_from(pre_rent_state)
    {
        let account_index = account_index as u8;
        error!("Transaction would leave account {address} with insufficient funds for rent");
        Err(TransactionError::InsufficientFundsForRent { account_index })
    } else {
        Ok(())
    }
}

fn map_sanitize_result<F>(
    res: Result<SanitizedTransaction, ExecutionResult>,
    op: F,
) -> ExecutionResult
where
    F: FnOnce(SanitizedTransaction) -> ExecutionResult,
{
    match res {
        Ok(s_tx) => op(s_tx),
        Err(e) => e,
    }
}
