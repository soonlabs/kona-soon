use solana_program::{
    address_lookup_table::{self, error::AddressLookupError, state::AddressLookupTable},
    bpf_loader, bpf_loader_deprecated,
    bpf_loader_upgradeable::{self, UpgradeableLoaderState},
    instruction::InstructionError,
    loader_v4::{self, LoaderV4State},
    message::{
        AddressLoader, AddressLoaderError,
        v0::{LoadedAddresses, MessageAddressTableLookup},
    },
};
use solana_program_runtime::{
    loaded_programs::{LoadProgramMetrics, ProgramCacheEntry, ProgramCacheForTxBatch},
    sysvar_cache::SysvarCache,
};
use solana_sdk::{
    account::{AccountSharedData, ReadableAccount, WritableAccount},
    account_utils::StateMut,
    native_loader, nonce,
    pubkey::Pubkey,
    transaction::TransactionError,
};
use solana_system_program::{SystemAccountKind, get_system_account_kind};
use std::{collections::HashMap, sync::Arc};
use solana_program::clock::{Epoch, Slot};
use solana_program_runtime::loaded_programs::ProgramRuntimeEnvironments;
use tracing::{warn, error};
use crate::accounts_callback::AccountsCallback;
use crate::error::LiteSVMError;

#[derive(Default)]
pub(crate) struct AccountsDb<CB: AccountsCallback> {
    callback: CB,
    accounts_cache: HashMap<Pubkey, AccountSharedData>,
    pub(crate) slot: Slot,
    pub(crate) programs_cache: ProgramCacheForTxBatch,
    pub(crate) sysvar_cache: SysvarCache,
}

impl<CB: AccountsCallback> AccountsDb<CB> {
    pub fn set_callback(&mut self, callback: CB) -> &mut Self {
        self.callback = callback;
        self
    }

    pub fn set_slot(&mut self, slot: Slot) -> &mut Self {
        self.slot = slot;
        self.programs_cache.set_slot_for_tests(slot);
        self
    }

    pub fn set_epoch(&mut self, epoch: Epoch) -> &mut Self {
        self.programs_cache.latest_root_epoch = epoch;
        self
    }

    pub fn set_environments(&mut self, environments: ProgramRuntimeEnvironments) -> &mut Self {
        self.programs_cache.environments = environments;
        self
    }

    pub(crate) fn fill_sysvar_cache(&mut self) {
        self.sysvar_cache.fill_missing_entries(|pubkey, set_sysvar| {
            if let Some(data) = self.accounts_cache.get(pubkey) {
                set_sysvar(data.data());
                return
            }
            if let Some(data) = self.callback.get_account_data(self.slot, pubkey) {
                set_sysvar(data.data());
                self.accounts_cache.insert(*pubkey, data);
            } else {
                warn!("Sysvar account {pubkey} not found in callback.");
            }
        });
    }

    pub(crate) fn get_account(
        &self,
        pubkey: &Pubkey,
    ) -> Option<AccountSharedData> {
        self.accounts_cache
            .get(pubkey)
            .cloned()
            .or_else(|| self.callback.get_account_data(self.slot, pubkey))
    }

    /// We should only use this when we know we're not touching any executable or sysvar accounts,
    /// or have already handled such cases.
    pub(crate) fn add_account_no_checks(&mut self, pubkey: Pubkey, account: AccountSharedData) {
        self.accounts_cache.insert(pubkey, account);
    }

    pub(crate) fn add_account(
        &mut self,
        pubkey: Pubkey,
        account: AccountSharedData,
    ) -> Result<(), LiteSVMError> {
        if account.executable() && !native_loader::check_id(account.owner()) {
            let loaded_program = self.load_program(&account)?;
            self.programs_cache.replenish(pubkey, Arc::new(loaded_program));
        }
        self.add_account_no_checks(pubkey, account);
        Ok(())
    }

    pub fn clear_cache_accounts(&mut self) {
        self.accounts_cache.clear();
    }

    /// Get all accounts in the database.
    pub(crate) fn all_cached_accounts(&self) -> Vec<(Pubkey, AccountSharedData)> {
        self.accounts_cache.iter().map(|(pubkey, account)| (*pubkey, account.clone())).collect()
    }

    pub(crate) fn clean_zero_accounts(&mut self) {
        self.accounts_cache.retain(|_, account| account.lamports() > 0);
    }

    pub(crate) fn sync_accounts(
        &mut self,
        mut accounts: Vec<(Pubkey, AccountSharedData)>,
    ) -> Result<(), LiteSVMError> {
        // need to add programdata accounts first if there are any
        itertools::partition(&mut accounts, |(_, account)| {
            account.owner() == &bpf_loader_upgradeable::id()
                && account.data().first().is_some_and(|byte| *byte == 3)
        });
        for (pubkey, acc) in accounts {
            self.add_account(pubkey, acc)?;
        }
        Ok(())
    }

    fn load_program(
        &self,
        program_account: &AccountSharedData,
    ) -> Result<ProgramCacheEntry, InstructionError> {
        let metrics = &mut LoadProgramMetrics::default();

        let owner = program_account.owner();
        let program_runtime_v1 = self.programs_cache.environments.program_runtime_v1.clone();

        if bpf_loader::check_id(owner) | bpf_loader_deprecated::check_id(owner) {
            ProgramCacheEntry::new(
                owner,
                self.programs_cache.environments.program_runtime_v1.clone(),
                self.slot,
                self.slot,
                program_account.data(),
                program_account.data().len(),
                &mut LoadProgramMetrics::default(),
            )
            .map_err(|e| {
                error!("Error loading program: {:?}", e);
                InstructionError::InvalidAccountData
            })
        } else if bpf_loader_upgradeable::check_id(owner) {
            let Ok(UpgradeableLoaderState::Program { programdata_address }) =
                program_account.state()
            else {
                error!(
                    "Program account data does not deserialize to UpgradeableLoaderState::Program"
                );
                return Err(InstructionError::InvalidAccountData);
            };
            let programdata_account = self.get_account(&programdata_address).ok_or_else(|| {
                error!("Program data account {programdata_address} not found");
                InstructionError::MissingAccount
            })?;
            let program_data = programdata_account.data();
            if let Some(programdata) =
                program_data.get(UpgradeableLoaderState::size_of_programdata_metadata()..)
            {
                ProgramCacheEntry::new(
                    owner,
                    program_runtime_v1,
                    self.slot,
                    self.slot,
                    programdata,
                    program_account
                        .data()
                        .len()
                        .saturating_add(program_data.len()),
                    metrics).map_err(|_| {
                        error!("Error encountered when calling ProgramCacheEntry::new() for bpf_loader_upgradeable.");
                        InstructionError::InvalidAccountData
                    })
            } else {
                error!("Index out of bounds using bpf_loader_upgradeable.");
                Err(InstructionError::InvalidAccountData)
            }
        } else if loader_v4::check_id(owner) {
            if let Some(elf_bytes) =
                program_account.data().get(LoaderV4State::program_data_offset()..)
            {
                ProgramCacheEntry::new(
                    &loader_v4::id(),
                    program_runtime_v1,
                    self.slot,
                    self.slot,
                    elf_bytes,
                    program_account.data().len(),
                    metrics,
                )
                .map_err(|_| {
                    error!("Error encountered when calling LoadedProgram::new() for loader_v4.");
                    InstructionError::InvalidAccountData
                })
            } else {
                error!("Index out of bounds using loader_v4.");
                Err(InstructionError::InvalidAccountData)
            }
        } else {
            error!(
                "Owner does not match any expected loader. program_account: {:?}, owner: {:?}",
                program_account, owner
            );
            Err(InstructionError::IncorrectProgramId)
        }
    }

    fn load_lookup_table_addresses(
        &self,
        address_table_lookup: &MessageAddressTableLookup,
    ) -> Result<LoadedAddresses, AddressLookupError> {
        let table_account = self
            .get_account(&address_table_lookup.account_key)
            .ok_or(AddressLookupError::LookupTableAccountNotFound)?;

        if table_account.owner() == &address_lookup_table::program::id() {
            let slot_hashes = self.sysvar_cache.get_slot_hashes().unwrap();
            let current_slot = self.sysvar_cache.get_clock().unwrap().slot;
            let lookup_table =
                AddressLookupTable::deserialize(table_account.data()).map_err(|e| {
                    error!("Error loading lookup table: {:?}", e);
                    AddressLookupError::InvalidAccountData
                })?;

            Ok(LoadedAddresses {
                writable: lookup_table.lookup(
                    current_slot,
                    &address_table_lookup.writable_indexes,
                    &slot_hashes,
                )?,
                readonly: lookup_table.lookup(
                    current_slot,
                    &address_table_lookup.readonly_indexes,
                    &slot_hashes,
                )?,
            })
        } else {
            Err(AddressLookupError::InvalidAccountOwner)
        }
    }

    pub(crate) fn withdraw(
        &mut self,
        pubkey: &Pubkey,
        lamports: u64,
    ) -> solana_sdk::transaction::Result<()> {
        if let Some(account) = self.accounts_cache.get_mut(pubkey) {
            let min_balance = match get_system_account_kind(account) {
                Some(SystemAccountKind::Nonce) => {
                    self.sysvar_cache.get_rent().unwrap().minimum_balance(nonce::State::size())
                }
                _ => 0,
            };

            lamports
                .checked_add(min_balance)
                .filter(|required_balance| *required_balance <= account.lamports())
                .ok_or(TransactionError::InsufficientFundsForFee)?;
            account
                .checked_sub_lamports(lamports)
                .map_err(|_| TransactionError::InsufficientFundsForFee)?;

            return Ok(())
        }

        if let Some(mut account) = self.callback.get_account_data(self.slot, pubkey) {
            let min_balance = match get_system_account_kind(&account) {
                Some(SystemAccountKind::Nonce) => {
                    self.sysvar_cache.get_rent().unwrap().minimum_balance(nonce::State::size())
                }
                _ => 0,
            };

            lamports
                .checked_add(min_balance)
                .filter(|required_balance| *required_balance <= account.lamports())
                .ok_or(TransactionError::InsufficientFundsForFee)?;
            account
                .checked_sub_lamports(lamports)
                .map_err(|_| TransactionError::InsufficientFundsForFee)?;

            // add the account back to cache
            self.add_account_no_checks(*pubkey, account);
            Ok(())
        } else {
            error!("Account {pubkey} not found when trying to withdraw.");
            Err(TransactionError::AccountNotFound)
        }
    }
}

fn into_address_loader_error(err: AddressLookupError) -> AddressLoaderError {
    match err {
        AddressLookupError::LookupTableAccountNotFound => {
            AddressLoaderError::LookupTableAccountNotFound
        }
        AddressLookupError::InvalidAccountOwner => AddressLoaderError::InvalidAccountOwner,
        AddressLookupError::InvalidAccountData => AddressLoaderError::InvalidAccountData,
        AddressLookupError::InvalidLookupIndex => AddressLoaderError::InvalidLookupIndex,
    }
}

impl<CB: AccountsCallback> AddressLoader for &AccountsDb<CB> {
    fn load_addresses(
        self,
        lookups: &[MessageAddressTableLookup],
    ) -> Result<LoadedAddresses, AddressLoaderError> {
        lookups
            .iter()
            .map(|lookup| {
                self.load_lookup_table_addresses(lookup).map_err(into_address_loader_error)
            })
            .collect()
    }
}
