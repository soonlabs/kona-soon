//! The [StatelessL2Builder] is a block builder that pulls state from a [TrieDB] during execution.

use crate::ExecutorError;
use crate::{ExecutorResult, TrieDB, TrieDBProvider, builder::L2BlockBuilder};
use alloc::collections::BTreeMap;
use alloc::string::ToString;
use alloc::sync::Arc;
use alloy_primitives::B256;
use fraud_executor::outcome::BlockBuildingOutcome;
use fraud_executor::{accounts::SoonAccounts, executor::FraudExecutor};
use kona_mpt::TrieHinter;
use litesvm::{L2Block, L2Transaction, LiteSVM};
use op_alloy_rpc_types_engine::OpPayloadAttributes;
use solana_sdk::account::AccountSharedData;
use solana_sdk::pubkey::Pubkey;
use soon_primitives::blocks::L2BlockHeader;
use soon_primitives::rollup_config::SoonRollupConfig;

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

    accounts_diff: BTreeMap<Pubkey, AccountSharedData>,
    last_accounts_diff: SoonAccounts,
    parent_slot: u64,
}

impl<P, H> StatelessL2Builder<P, H>
where
    P: TrieDBProvider + Clone,
    H: TrieHinter + Clone,
{
    fn convert_block(&self, attrs: OpPayloadAttributes) -> ExecutorResult<L2Block> {
        Ok(L2Block(
            attrs
                .transactions
                .unwrap_or_default()
                .into_iter()
                .map(|tx| {
                    let tx: L2Transaction = bincode::deserialize(&tx)
                        .map_err(|e| ExecutorError::FraudInitError(e.to_string()))?;
                    Ok(tx)
                })
                .collect::<ExecutorResult<_>>()?,
        ))
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
        let parent_slot = parent_header.block_info.number;
        let trie_db = TrieDB::new(parent_header, provider, hinter);
        Self {
            config,
            trie_db,
            factory: None,
            accounts_diff: last_accounts_diff.accounts.iter().cloned().collect(),
            last_accounts_diff,
            parent_slot,
        }
    }

    /// Initializes the block builder.
    fn init(&mut self) -> ExecutorResult<()> {
        // TODO: litesvm should be initialized here
        Ok(())
    }

    /// Builds a new block on top of the parent state, using the given [`OpPayloadAttributes`].
    fn build_block(&mut self, attrs: OpPayloadAttributes) -> ExecutorResult<BlockBuildingOutcome> {
        // Step 1. Set up the execution environment using genesis

        // Step 2. Get the parent bank hash and clock timestamp
        let parent_bank_hash = self.trie_db.bank_hash(self.parent_slot)?;
        let clock_timestamp = self.trie_db.block_time(self.parent_slot + 1)?;
        info!(
            "building block for slot: {:?}, parent_bank_hash: {:?}, clock_timestamp: {:?}",
            self.parent_slot + 1,
            parent_bank_hash,
            clock_timestamp
        );

        // Step 2. Create the executor, using the trie database.
        let mut svm = LiteSVM::new_soon()
            .with_parent_slot(self.parent_slot)
            .with_parent_bank_hash((*parent_bank_hash).into())
            .with_clock_timestamp(clock_timestamp)
            .with_leader_schedule(self.config.sequencer_schedules.clone().into_iter().collect())
            .with_sig_verify(false)
            .with_blockhash_verify(true)
            .with_accounts_callback(self.trie_db.clone())
            .with_init_account(self.last_accounts_diff.accounts.clone());
        svm.finish_init().map_err(|e| ExecutorError::FraudExecutorError(e.into()))?;
        let mut executor = FraudExecutor::new(svm);

        // Step 3. Execute the block containing the transactions within the payload attributes.
        let block = self.convert_block(attrs)?;
        let mut outcome = executor.execute_block(block)?;

        // Step 4. Store data to calculate output root
        let diff_accounts = executor.export_diff_accounts();
        self.accounts_diff.extend(diff_accounts);
        outcome.state_root = self.trie_db.state_root(self.accounts_diff.iter())?;
        // TODO calculate withdraw root
        outcome.withdraw_root = B256::ZERO;

        Ok(outcome)
    }

    /// Computes the current output root of the latest executed block, based on the parent header
    /// and the underlying state trie.
    fn compute_output_root(&mut self) -> ExecutorResult<B256> {
        // TODO calculate output root
        Ok(B256::ZERO)
    }

    fn account_diff(&self) -> SoonAccounts {
        SoonAccounts::from(self.accounts_diff.clone())
    }
}
