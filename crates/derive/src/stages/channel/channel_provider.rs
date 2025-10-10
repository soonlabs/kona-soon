//! This module contains the [ChannelProvider] stage.

use super::{ChannelAssembler, ChannelBank, ChannelReaderProvider, NextFrameProvider};
use crate::{
    errors::PipelineError,
    traits::{OriginAdvancer, OriginProvider, SignalReceiver},
    types::{PipelineResult, Signal},
};
use alloc::{boxed::Box, sync::Arc};
use alloy_primitives::Bytes;
use async_trait::async_trait;
use core::fmt::Debug;
use soon_primitives::{blocks::BlockInfo, rollup_config::SoonRollupConfig};

/// The [ChannelProvider] stage is a mux between the [ChannelBank] and [ChannelAssembler] stages.
///
/// Rules:
/// When Holocene is not active, the [ChannelBank] is used.
/// When Holocene is active, the [ChannelAssembler] is used.
#[derive(Debug)]
pub struct ChannelProvider<P>
where
    P: NextFrameProvider + OriginAdvancer + OriginProvider + SignalReceiver + Debug,
{
    /// The rollup configuration.
    pub cfg: Arc<SoonRollupConfig>,
    /// The previous stage of the derivation pipeline.
    ///
    /// If this is set to [None], the multiplexer has been activated and the active stage
    /// owns the previous stage.
    ///
    /// Must be [None] if `channel_bank` or `channel_assembler` is [Some].
    pub prev: Option<P>,
    /// The channel bank stage of the provider.
    ///
    /// Must be [None] if `prev` or `channel_assembler` is [Some].
    pub channel_bank: Option<ChannelBank<P>>,
    /// The channel assembler stage of the provider.
    ///
    /// Must be [None] if `prev` or `channel_bank` is [Some].
    pub channel_assembler: Option<ChannelAssembler<P>>,
}

impl<P> ChannelProvider<P>
where
    P: NextFrameProvider + OriginAdvancer + OriginProvider + SignalReceiver + Debug,
{
    /// Creates a new [ChannelProvider] with the given configuration and previous stage.
    pub const fn new(cfg: Arc<SoonRollupConfig>, prev: P) -> Self {
        Self { cfg, prev: Some(prev), channel_bank: None, channel_assembler: None }
    }

    /// Attempts to update the active stage of the mux.
    pub(crate) fn attempt_update(&mut self) -> PipelineResult<()> {
        if let Some(prev) = self.prev.take() {
            self.channel_bank = Some(ChannelBank::new(self.cfg.clone(), prev));
        }
        Ok(())
    }
}

#[async_trait]
impl<P> OriginAdvancer for ChannelProvider<P>
where
    P: NextFrameProvider + OriginAdvancer + OriginProvider + SignalReceiver + Send + Debug,
{
    async fn advance_origin(&mut self) -> PipelineResult<()> {
        self.attempt_update()?;

        if let Some(channel_assembler) = self.channel_assembler.as_mut() {
            channel_assembler.advance_origin().await
        } else if let Some(channel_bank) = self.channel_bank.as_mut() {
            channel_bank.advance_origin().await
        } else {
            Err(PipelineError::NotEnoughData.temp())
        }
    }
}

impl<P> OriginProvider for ChannelProvider<P>
where
    P: NextFrameProvider + OriginAdvancer + OriginProvider + SignalReceiver + Debug,
{
    fn origin(&self) -> Option<BlockInfo> {
        self.channel_assembler.as_ref().map_or_else(
            || {
                self.channel_bank.as_ref().map_or_else(
                    || self.prev.as_ref().and_then(|prev| prev.origin()),
                    |channel_bank| channel_bank.origin(),
                )
            },
            |channel_assembler| channel_assembler.origin(),
        )
    }
}

#[async_trait]
impl<P> SignalReceiver for ChannelProvider<P>
where
    P: NextFrameProvider + OriginAdvancer + OriginProvider + SignalReceiver + Send + Debug,
{
    async fn signal(&mut self, signal: Signal) -> PipelineResult<()> {
        self.attempt_update()?;

        if let Some(channel_assembler) = self.channel_assembler.as_mut() {
            channel_assembler.signal(signal).await
        } else if let Some(channel_bank) = self.channel_bank.as_mut() {
            channel_bank.signal(signal).await
        } else {
            Err(PipelineError::NotEnoughData.temp())
        }
    }
}

#[async_trait]
impl<P> ChannelReaderProvider for ChannelProvider<P>
where
    P: NextFrameProvider + OriginAdvancer + OriginProvider + SignalReceiver + Send + Debug,
{
    async fn next_data(&mut self) -> PipelineResult<Option<Bytes>> {
        self.attempt_update()?;

        if let Some(channel_assembler) = self.channel_assembler.as_mut() {
            channel_assembler.next_data().await
        } else if let Some(channel_bank) = self.channel_bank.as_mut() {
            channel_bank.next_data().await
        } else {
            Err(PipelineError::NotEnoughData.temp())
        }
    }
}

#[cfg(test)]
mod test {
    use super::ChannelProvider;
    use crate::test_utils::TestNextFrameProvider;
    use alloc::{sync::Arc, vec};
    use soon_primitives::rollup_config::SoonRollupConfig;

    #[test]
    fn test_channel_provider_bank_active() {
        let provider = TestNextFrameProvider::new(vec![]);
        let cfg = Arc::new(SoonRollupConfig::default());
        let mut channel_provider = ChannelProvider::new(cfg, provider);

        assert!(channel_provider.attempt_update().is_ok());
        assert!(channel_provider.prev.is_none());
        assert!(channel_provider.channel_bank.is_some());
        assert!(channel_provider.channel_assembler.is_none());
    }
}
