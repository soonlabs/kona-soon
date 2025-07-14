//! An executor constructor.

use alloc::boxed::Box;
use alloy_consensus::{Header, Sealed};
use alloy_primitives::B256;
use async_trait::async_trait;
use kona_driver::Executor;
use kona_executor::TrieDBProvider;
use kona_mpt::TrieHinter;
use op_alloy_rpc_types_engine::OpPayloadAttributes;
use soon_primitives::{blocks::L2BlockInfo, rollup_config::SoonRollupConfig};

pub use kona_executor::{L2BlockBuilder, OffchainL2Builder, StatelessL2Builder};

/// An executor wrapper type.
#[derive(Debug)]
pub struct KonaExecutor<P, H, E>
where
    P: TrieDBProvider + Send + Sync + Clone,
    H: TrieHinter + Send + Sync + Clone,
    E: L2BlockBuilder<P, H> + Send + Sync,
{
    /// The rollup config for the executor.
    rollup_config: SoonRollupConfig,
    /// The trie provider for the executor.
    trie_provider: P,
    /// The trie hinter for the executor.
    trie_hinter: H,
    /// The executor.
    inner: Option<E>,
}

impl<P, H, E> KonaExecutor<P, H, E>
where
    P: TrieDBProvider + Send + Sync + Clone,
    H: TrieHinter + Send + Sync + Clone,
    E: L2BlockBuilder<P, H> + Send + Sync,
{
    /// Creates a new executor.
    pub const fn new(
        rollup_config: SoonRollupConfig,
        trie_provider: P,
        trie_hinter: H,
        inner: Option<E>,
    ) -> Self {
        Self { rollup_config, trie_provider, trie_hinter, inner }
    }
}

#[async_trait]
impl<P, H, E> Executor for KonaExecutor<P, H, E>
where
    P: TrieDBProvider + Send + Sync + Clone,
    H: TrieHinter + Send + Sync + Clone,
    E: L2BlockBuilder<P, H> + Send + Sync,
{
    type Error = kona_executor::ExecutorError;

    /// Waits for the executor to be ready.
    async fn wait_until_ready(&mut self) {
        /* no-op for the kona executor */
        /* This is used when an engine api is used instead of a stateless block executor */
    }

    /// Updates the safe header.
    ///
    /// Since the L2 block executor is stateless, on an update to the safe head,
    /// a new executor is created with the updated header.
    fn update_safe_head(&mut self, header: Sealed<Header>) -> Result<(), Self::Error> {
        let mut executor = E::new(
            self.rollup_config.clone(),
            self.trie_provider.clone(),
            self.trie_hinter.clone(),
            header,
        );
        executor.init()?;
        self.inner = Some(executor);
        Ok(())
    }

    /// Execute the given payload attributes.
    async fn execute_payload(
        &mut self,
        attributes: OpPayloadAttributes,
    ) -> Result<L2BlockInfo, Self::Error> {
        self.inner.as_mut().map_or_else(
            || Err(kona_executor::ExecutorError::MissingExecutor),
            |e| e.build_block(attributes),
        )
    }

    /// Computes the output root.
    fn compute_output_root(&mut self) -> Result<B256, Self::Error> {
        self.inner.as_mut().map_or_else(
            || Err(kona_executor::ExecutorError::MissingExecutor),
            |e| e.compute_output_root(),
        )
    }
}
