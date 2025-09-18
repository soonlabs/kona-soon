//! Contains the [`SystemConfigUpdate`].

use crate::{
    system::{SystemConfig, SystemConfigUpdateKind},
    system_update::{
        BatcherUpdate, Eip1559Update, GasConfigUpdate, GasLimitUpdate, OperatorFeeUpdate,
        UnsafeBlockSignerUpdate,
    },
};

/// The system config update is an update
/// of type [`SystemConfigUpdateKind`].
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum SystemConfigUpdate {
    /// The batcher update.
    Batcher(BatcherUpdate),
    /// The gas config update.
    GasConfig(GasConfigUpdate),
    /// The gas limit update.
    GasLimit(GasLimitUpdate),
    /// The unsafe block signer update.
    UnsafeBlockSigner(UnsafeBlockSignerUpdate),
    /// The EIP-1559 parameters update.
    Eip1559(Eip1559Update),
    /// The operator fee parameter update.
    OperatorFee(OperatorFeeUpdate),
}

impl SystemConfigUpdate {
    /// Applies the update to the [`SystemConfig`].
    pub fn apply(&self, config: &mut SystemConfig) {
        match self {
            Self::Batcher(update) => update.apply(config),
            Self::GasConfig(_) => { /* Ignored for SOON */ }
            Self::GasLimit(_) => { /* Ignored for SOON */ }
            Self::UnsafeBlockSigner(_) => { /* Ignored in derivation */ }
            Self::Eip1559(_) => { /* Ignored for SOON */ }
            Self::OperatorFee(_) => { /* Ignored for SOON */ }
        }
    }

    /// Returns the update kind.
    pub const fn kind(&self) -> SystemConfigUpdateKind {
        match self {
            Self::Batcher(_) => SystemConfigUpdateKind::Batcher,
            Self::GasConfig(_) => SystemConfigUpdateKind::GasConfig,
            Self::GasLimit(_) => SystemConfigUpdateKind::GasLimit,
            Self::UnsafeBlockSigner(_) => SystemConfigUpdateKind::UnsafeBlockSigner,
            Self::Eip1559(_) => SystemConfigUpdateKind::Eip1559,
            Self::OperatorFee(_) => SystemConfigUpdateKind::OperatorFee,
        }
    }
}
