use crate::accounts_callback::AccountsCallback;
use crate::error::{InvalidSysvarDataError, LiteSVMError};
use solana_program::clock::{Epoch, Slot};
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
    system_program,
};
use solana_program_runtime::loaded_programs::ProgramRuntimeEnvironments;
use solana_program_runtime::{
    loaded_programs::{LoadProgramMetrics, ProgramCacheEntry, ProgramCacheForTxBatch},
    sysvar_cache::SysvarCache,
};
use solana_sdk::{
    account::{AccountSharedData, ReadableAccount, WritableAccount},
    account_utils::StateMut,
    native_loader,
    pubkey::Pubkey,
};
use std::{collections::HashMap, sync::Arc};
use tracing::{error, warn};

pub(crate) struct AccountsDb<CB: AccountsCallback> {
    callback: Option<CB>,
    accounts_diff: HashMap<Pubkey, AccountSharedData>,
    pub(crate) slot: Slot,
    pub(crate) epoch: Epoch,
    pub(crate) programs_cache: ProgramCacheForTxBatch,
    pub(crate) sysvar_cache: SysvarCache,
}

impl<CB: AccountsCallback> Default for AccountsDb<CB> {
    fn default() -> Self {
        Self {
            callback: None,
            accounts_diff: HashMap::default(),
            slot: Slot::default(),
            epoch: Epoch::default(),
            programs_cache: ProgramCacheForTxBatch::default(),
            sysvar_cache: SysvarCache::default(),
        }
    }
}

impl<CB: AccountsCallback> AccountsDb<CB> {
    pub fn set_callback(&mut self, callback: CB) -> &mut Self {
        self.callback = Some(callback);
        self
    }

    pub fn set_init_accounts(&mut self, init_accounts: Vec<(Pubkey, AccountSharedData)>) -> &mut Self {
        for (pubkey, account) in init_accounts {
            self.accounts_diff.insert(pubkey, account);
        }
        self
    }

    pub fn set_slot(&mut self, slot: Slot) -> &mut Self {
        self.slot = slot;
        self.programs_cache.set_slot_for_tests(slot);
        self
    }

    pub fn set_epoch(&mut self, epoch: Epoch) -> &mut Self {
        self.epoch = epoch;
        self.programs_cache.latest_root_epoch = epoch;
        self
    }

    pub fn set_environments(&mut self, environments: ProgramRuntimeEnvironments) -> &mut Self {
        self.programs_cache.environments = environments;
        self
    }

    pub(crate) fn fill_sysvar_cache(&mut self) -> Result<(), LiteSVMError> {
        self.sysvar_cache.fill_missing_entries(|pubkey, set_sysvar| {
            if let Some(data) = self.accounts_diff.get(pubkey) {
                set_sysvar(data.data());
                return;
            }
            if let Some(callback) = &mut self.callback {
                if let Ok(Some(data)) = callback.get_account_data(pubkey) {
                    set_sysvar(data.data());
                } else {
                    warn!("Sysvar account {pubkey} not found in callback.");
                }
            } else {
                warn!("Sysvar account {pubkey} not found for none callback.");
            }
        });
        // check clock consistency
        if let Ok(clock) = self.sysvar_cache.get_clock() {
            if clock.slot != self.slot || clock.epoch != self.epoch {
                error!(
                    "Clock mismatch, slot: {}, epoch: {}, actual slot: {}, actual epoch: {}",
                    self.slot, self.epoch, clock.slot, clock.epoch
                );
                return Err(LiteSVMError::InvalidSysvarData(InvalidSysvarDataError::Clock));
            }
        }
        Ok(())
    }

    pub(crate) fn get_account(&mut self, pubkey: &Pubkey) -> Option<AccountSharedData> {
        self.accounts_diff
            .get(pubkey)
            .cloned()
            .or_else(|| {
                self.callback.as_mut()
                    .and_then(|callback| callback.get_account_data(pubkey).ok().flatten())
            })
    }

    pub(crate) fn load_account(
        &mut self,
        pubkey: &Pubkey,
    ) -> Result<Option<AccountSharedData>, InstructionError> {
        let account = self.get_account(pubkey);

        if let Some(account) = &account {
            // if account is program account, add it into programs cache
            if account.executable() && !native_loader::check_id(account.owner()) {
                let loaded_program = self.load_program(&account)?;
                self.programs_cache.replenish(*pubkey, Arc::new(loaded_program));
            }
        }

        Ok(account)
    }

    pub(crate) fn add_diff_account(
        &mut self,
        check_rent_exemption: bool,
        pubkey: Pubkey,
        account: AccountSharedData,
    ) -> Result<(), LiteSVMError> {
        if check_rent_exemption {
            let rent_exemption =
                self.sysvar_cache.get_rent()?.minimum_balance(account.data().len());
            if account.lamports() < rent_exemption {
                return Err(LiteSVMError::InsufficientLamports);
            }
        }
        self.accounts_diff.insert(pubkey, account);
        Ok(())
    }

    pub(crate) fn clear_diff_accounts(&mut self) {
        self.accounts_diff.clear();
    }

    /// Get all accounts in the database.
    pub(crate) fn export_diff_accounts(&self) -> Vec<(Pubkey, AccountSharedData)> {
        self.accounts_diff.iter().map(|(pubkey, account)| (*pubkey, account.clone())).collect()
    }

    pub(crate) fn clean_zero_accounts(&mut self) {
        self.accounts_diff.retain(|_, account| account.lamports() > 0);
    }

    fn load_program(
        &mut self,
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
        &mut self,
        address_table_lookup: &MessageAddressTableLookup,
    ) -> Result<LoadedAddresses, AddressLookupError> {
        let table_account = self
            .get_account(&address_table_lookup.account_key)
            .ok_or(AddressLookupError::LookupTableAccountNotFound)?;

        if table_account.owner() == &address_lookup_table::program::id() {
            let slot_hashes =
                self.sysvar_cache.get_slot_hashes().expect("Slot hashes sysvar not found");
            let lookup_table =
                AddressLookupTable::deserialize(table_account.data()).map_err(|e| {
                    error!("Error loading lookup table: {:?}", e);
                    AddressLookupError::InvalidAccountData
                })?;

            Ok(LoadedAddresses {
                writable: lookup_table.lookup(
                    self.slot,
                    &address_table_lookup.writable_indexes,
                    &slot_hashes,
                )?,
                readonly: lookup_table.lookup(
                    self.slot,
                    &address_table_lookup.readonly_indexes,
                    &slot_hashes,
                )?,
            })
        } else {
            Err(AddressLookupError::InvalidAccountOwner)
        }
    }

    pub(crate) fn mint(
        &mut self,
        check_rent_exemption: bool,
        pubkey: &Pubkey,
        lamports: u64,
    ) -> Result<(), LiteSVMError> {
        let account = if let Some(mut account) = self.get_account(pubkey) {
            account.checked_add_lamports(lamports)?;
            account
        } else {
            AccountSharedData::new(lamports, 0, &system_program::id())
        };
        self.add_diff_account(check_rent_exemption, *pubkey, account)?;
        Ok(())
    }

    pub(crate) fn burn(
        &mut self,
        check_rent_exemption: bool,
        pubkey: &Pubkey,
        lamports: u64,
    ) -> Result<(), LiteSVMError> {
        if let Some(mut account) = self.get_account(pubkey) {
            account.checked_sub_lamports(lamports)?;
            self.add_diff_account(check_rent_exemption, *pubkey, account)?;
            Ok(())
        } else {
            error!("Account {pubkey} not found when trying to withdraw.");
            Err(LiteSVMError::MissingAccount(*pubkey))
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
        // Since we can't mutate self, we'll return an error for now
        // This is a limitation of the current design
        Err(AddressLoaderError::LookupTableAccountNotFound)
    }
}
