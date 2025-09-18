//! Defines the interface for the core derivation pipeline.

use super::OriginProvider;
use crate::{errors::PipelineErrorKind, types::StepResult};
use alloc::boxed::Box;
use async_trait::async_trait;
use core::iter::Iterator;
use soon_primitives::{
    blocks::L2BlockInfo, derive::OpAttributesWithParent, rollup_config::SoonRollupConfig,
    system::SystemConfig,
};

/// This trait defines the interface for interacting with the derivation pipeline.
#[async_trait]
pub trait Pipeline: OriginProvider + Iterator<Item = OpAttributesWithParent> {
    /// Peeks at the next [OpAttributesWithParent] from the pipeline.
    fn peek(&self) -> Option<&OpAttributesWithParent>;

    /// Attempts to progress the pipeline.
    async fn step(&mut self, cursor: L2BlockInfo) -> StepResult;

    /// Returns the rollup config.
    fn rollup_config(&self) -> &SoonRollupConfig;

    /// Returns the [SystemConfig] by L2 number.
    async fn system_config_by_number(
        &mut self,
        number: u64,
    ) -> Result<SystemConfig, PipelineErrorKind>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ActivationSignal, ResetSignal, Signal};

    #[test]
    fn test_reset_signal() {
        let signal = ResetSignal::default();
        assert_eq!(signal.signal(), Signal::Reset(signal));
    }

    #[test]
    fn test_activation_signal() {
        let signal = ActivationSignal::default();
        assert_eq!(signal.signal(), Signal::Activation(signal));
    }

    #[test]
    fn test_signal_with_system_config() {
        let signal = ResetSignal::default();
        let system_config = SystemConfig::default();
        assert_eq!(
            signal.with_system_config(system_config).signal(),
            Signal::Reset(ResetSignal { system_config: Some(system_config), ..signal })
        );

        let signal = ActivationSignal::default();
        let system_config = SystemConfig::default();
        assert_eq!(
            signal.with_system_config(system_config).signal(),
            Signal::Activation(ActivationSignal { system_config: Some(system_config), ..signal })
        );

        assert_eq!(Signal::FlushChannel.with_system_config(system_config), Signal::FlushChannel);
    }
}
