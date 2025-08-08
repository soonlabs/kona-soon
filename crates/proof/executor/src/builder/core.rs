//! The [StatelessL2Builder] is a block builder that pulls state from a [TrieDB] during execution.

use crate::ExecutorError;
use crate::{ExecutorResult, TrieDB, TrieDBProvider, builder::L2BlockBuilder};
use alloc::string::ToString;
use alloc::sync::Arc;
use alloc::vec::Vec;
use alloy_primitives::B256;
use fraud_executor::outcome::BlockBuildingOutcome;
use fraud_executor::{accounts::SoonAccounts, block::SimpleBlock, executor::FraudExecutor};
use kona_mpt::TrieHinter;
use op_alloy_rpc_types_engine::OpPayloadAttributes;
use solana_sdk::transaction::VersionedTransaction;
use soon_primitives::{blocks::L2BlockInfo, rollup_config::SoonRollupConfig};
use soon_primitives::blocks::L2BlockHeader;
use litesvm::LiteSVM;

/// The [`StatelessL2Builder`] is an OP Stack block builder that traverses a merkle patricia trie
/// via the [`TrieDB`] during execution.
#[derive(Debug)]
pub struct StatelessL2Builder<P, H>
where
    P: TrieDBProvider + Clone,
    H: TrieHinter + Clone,
{
    /// The [SoonRollupConfig].
    #[allow(dead_code)]
    pub(crate) config: Arc<SoonRollupConfig>,
    /// The inner trie database.
    #[allow(dead_code)]
    pub(crate) trie_db: TrieDB<P, H>,
    /// The executor factory, used to create new [`op_revm::OpEvm`] instances for block building
    /// routines.
    #[allow(dead_code)]
    pub(crate) factory: Option<bool>,

    pub(crate) accounts_diff: SoonAccounts,

    pub(crate) last_accounts_diff: SoonAccounts,
}

impl<P, H> StatelessL2Builder<P, H>
where
    P: TrieDBProvider + Clone,
    H: TrieHinter + Clone,
{
    fn convert_block(&self, attrs: OpPayloadAttributes) -> ExecutorResult<SimpleBlock> {
        Ok(SimpleBlock {
            hash: B256::ZERO,        // TODO: get hash from oracle
            parent_hash: B256::ZERO, // TODO: get parent hash from oracle
            slot: 0,                 // TODO: get current slot
            transactions: attrs
                .transactions
                .unwrap_or_default()
                .into_iter()
                .map(|tx| {
                    let tx: VersionedTransaction = bincode::deserialize(&tx)
                        .map_err(|e| ExecutorError::FraudInitError(e.to_string()))?;
                    Ok(tx)
                })
                .collect::<ExecutorResult<Vec<VersionedTransaction>>>()?,
        })
    }
}

impl<P, H> L2BlockBuilder<P, H> for StatelessL2Builder<P, H>
where
    P: TrieDBProvider + Clone,
    H: TrieHinter + Clone,
{
    /// Creates a new [StatelessL2Builder] instance.
    fn new(
        config: Arc<SoonRollupConfig>,
        provider: P,
        hinter: H,
        parent_header: L2BlockHeader,
        last_accounts_diff: SoonAccounts,
    ) -> Self {
        let trie_db = TrieDB::new(parent_header, provider, hinter);
        Self {
            config,
            trie_db,
            factory: None,
            accounts_diff: SoonAccounts::default(),
            last_accounts_diff,
        }
    }

    /// Initializes the block builder.
    fn init(&mut self) -> ExecutorResult<()> {
        Ok(())
    }

    /// Builds a new block on top of the parent state, using the given [`OpPayloadAttributes`].
    fn build_block(&mut self, attrs: OpPayloadAttributes) -> ExecutorResult<BlockBuildingOutcome> {
        // Step 1. Set up the execution environment using genesis

        // Step 2. Create the executor, using the trie database.
        // TODO: import using trie db later
        // TODO: svm should be correctly initialized
        let mut svm = LiteSVM::new_soon()
            .with_accounts_callback(self.trie_db.clone())
            .with_init_account(self.last_accounts_diff.accounts.clone());
        svm.finish_init().map_err(|e| ExecutorError::FraudExecutorError(e.into()))?;
        let mut executor = FraudExecutor::new(svm);

        // Step 3. Execute the block containing the transactions within the payload attributes.
        let block = self.convert_block(attrs)?;
        let mut outcome = executor.execute_block(block)?;

        // Step 4. Store data to calculate output root
        let accounts = executor.export_diff_accounts();
        self.accounts_diff = SoonAccounts::from(accounts);

        outcome.state_root = self.trie_db.state_root(&self.accounts_diff)?;

        Ok(outcome)
    }

    /// Computes the current output root of the latest executed block, based on the parent header
    /// and the underlying state trie.
    fn compute_output_root(&mut self) -> ExecutorResult<B256> {
        Ok(self.accounts_diff.state_root())
    }

    fn account_diff(&self) -> SoonAccounts {
        return self.accounts_diff.clone()
    }
}
