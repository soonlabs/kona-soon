use crate::builtin::BUILTINS;
use serde::{Deserialize, Serialize};
use solana_program::clock::INITIAL_RENT_EPOCH;
use solana_program::pubkey::Pubkey;
use solana_program::rent::Rent;
use solana_program::sysvar::{Sysvar, SysvarId};
use solana_sdk::account::{AccountSharedData, WritableAccount};
use solana_sdk::feature_set::FeatureSet;
use solana_sdk::native_loader;
use solana_sdk::precompiles::get_precompiles;
use std::collections::HashMap;
use std::fmt::Debug;
use tracing::debug;

/// Fetch account data for a given public key at a specific slot.
pub trait AccountsCallback {
    /// The error type for the AccountsCallback.
    type Error;
    /// Get account data for a specific slot and public key.
    fn get_account_data(
        &mut self,
        _pubkey: &Pubkey,
    ) -> Result<Option<AccountSharedData>, Self::Error> {
        Ok(None)
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct MemoryAccountsCallback {
    accounts: HashMap<Pubkey, AccountSharedData>,
}

impl MemoryAccountsCallback {
    pub fn insert(&mut self, pubkey: Pubkey, account: AccountSharedData) {
        self.accounts.insert(pubkey, account);
    }

    pub fn with_precompiles(mut self, feature_set: &FeatureSet) -> Self {
        let mut account = AccountSharedData::default();
        account.set_owner(native_loader::id());
        account.set_lamports(1);
        account.set_executable(true);

        for precompile in get_precompiles() {
            if precompile.feature.map_or(true, |feature_id| feature_set.is_active(&feature_id)) {
                self.insert(precompile.program_id, account.clone());
            }
        }

        self
    }

    pub fn with_builtins(mut self, feature_set: &FeatureSet) -> Self {
        for builtin in BUILTINS {
            if let Some(feature_id) = builtin.feature_id {
                if !feature_set.is_active(&feature_id) {
                    debug!("Builtin feature {} not active", builtin.name);
                    continue;
                }
            }
            self.insert(
                builtin.program_id,
                native_loader::create_loadable_account_with_fields(
                    builtin.name,
                    (1, INITIAL_RENT_EPOCH),
                ),
            );
        }
        self
    }

    pub fn with_minting_to(mut self, accounts: impl IntoIterator<Item = (Pubkey, u64)>) -> Self {
        for (key, lamports) in accounts {
            self.insert(key, AccountSharedData::new(lamports, 0, &key));
        }
        self
    }

    pub fn with_sysvar_clock(
        mut self,
        rent: &Rent,
        clock: solana_program::clock::Clock,
    ) -> Result<Self, bincode::Error> {
        self.add_sysvar(rent, clock)?;
        Ok(self)
    }

    pub fn with_sysvar_slot_history(
        mut self,
        rent: &Rent,
        slot_history: solana_program::slot_history::SlotHistory,
    ) -> Result<Self, bincode::Error> {
        self.add_sysvar(rent, slot_history)?;
        Ok(self)
    }

    pub fn with_sysvar_rent(mut self, rent: &Rent) -> Result<Self, bincode::Error> {
        self.add_sysvar(rent, rent.clone())?;
        Ok(self)
    }

    pub fn with_sysvar_stake_history(
        mut self,
        rent: &Rent,
        stake_history: solana_program::stake_history::StakeHistory,
    ) -> Result<Self, bincode::Error> {
        self.add_sysvar(rent, stake_history)?;
        Ok(self)
    }

    pub fn with_sysvar_last_restart_slot(
        mut self,
        rent: &Rent,
        last_restart_slot: solana_program::last_restart_slot::LastRestartSlot,
    ) -> Result<Self, bincode::Error> {
        self.add_sysvar(rent, last_restart_slot)?;
        Ok(self)
    }

    pub fn with_sysvar_slot_hashes(
        mut self,
        rent: &Rent,
        slot_hashes: solana_program::slot_hashes::SlotHashes,
    ) -> Result<Self, bincode::Error> {
        self.add_sysvar(rent, slot_hashes)?;
        Ok(self)
    }

    pub fn with_sysvar_epoch_schedule(
        mut self,
        rent: &Rent,
        epoch_schedule: solana_program::epoch_schedule::EpochSchedule,
    ) -> Result<Self, bincode::Error> {
        self.add_sysvar(rent, epoch_schedule)?;
        Ok(self)
    }

    fn add_sysvar<T: Sysvar + SysvarId>(
        &mut self,
        rent: &Rent,
        sysvar: T,
    ) -> Result<(), bincode::Error> {
        let account = AccountSharedData::new_data(
            rent.minimum_balance(T::size_of()),
            &sysvar,
            &solana_sdk::sysvar::id(),
        )?;
        self.insert(T::id(), account);
        Ok(())
    }
}

impl<I: IntoIterator<Item = (Pubkey, AccountSharedData)>> From<I> for MemoryAccountsCallback {
    fn from(accounts: I) -> Self {
        let mut callback = Self::default();
        for (pubkey, account) in accounts {
            callback.insert(pubkey, account);
        }
        callback
    }
}

impl AccountsCallback for MemoryAccountsCallback {
    type Error = ();
    fn get_account_data(
        &mut self,
        pubkey: &Pubkey,
    ) -> Result<Option<AccountSharedData>, Self::Error> {
        Ok(self.accounts.get(pubkey).cloned())
    }
}
