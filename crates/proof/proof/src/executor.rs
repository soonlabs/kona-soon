//! An executor constructor.

use alloc::{boxed::Box, sync::Arc};
use alloy_primitives::B256;
use async_trait::async_trait;
use fraud_executor::accounts::SoonAccounts;
use fraud_executor::outcome::BlockBuildingOutcome;
use kona_driver::Executor;
pub use kona_executor::{L2BlockBuilder, OffchainL2Builder, StatelessL2Builder};
use kona_executor::{TrieDB, TrieDBProvider};
use kona_mpt::TrieHinter;
use op_alloy_rpc_types_engine::OpPayloadAttributes;
use soon_primitives::{blocks::L2BlockHeader, rollup_config::SoonRollupConfig};

/// An executor wrapper type.
#[derive(Debug)]
pub struct KonaExecutor<P, H, E>
where
    P: TrieDBProvider + Send + Sync + Clone,
    H: TrieHinter + Send + Sync + Clone,
    E: L2BlockBuilder<P, H> + Send + Sync,
{
    /// The rollup config for the executor.
    rollup_config: Arc<SoonRollupConfig>,
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
        rollup_config: Arc<SoonRollupConfig>,
        trie_provider: P,
        trie_hinter: H,
        inner: Option<E>,
    ) -> Self {
        Self { rollup_config, trie_provider, trie_hinter, inner }
    }

    /// Returns the inner builder.
    pub fn inner_builder(&self) -> Option<&E> {
        self.inner.as_ref()
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
    fn update_safe_head(&mut self, header: L2BlockHeader) -> Result<(), Self::Error> {
        let (last_account_diff, trie_db) = match &self.inner {
            None => {
                let trie_db =
                    TrieDB::new(header, self.trie_provider.clone(), self.trie_hinter.clone());
                (SoonAccounts::default(), trie_db)
            }
            Some(builder) => (builder.account_diff(), builder.trie_db()),
        };
        let mut executor = E::new(
            self.rollup_config.clone(),
            self.trie_provider.clone(),
            header,
            last_account_diff,
            trie_db,
        );
        executor.init()?;
        self.inner = Some(executor);
        Ok(())
    }

    /// Execute the given payload attributes.
    async fn execute_payload(
        &mut self,
        attributes: OpPayloadAttributes,
    ) -> Result<BlockBuildingOutcome, Self::Error> {
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

    /// Reset the executor.
    fn reset(&mut self) {
        self.inner.take();
    }
}
