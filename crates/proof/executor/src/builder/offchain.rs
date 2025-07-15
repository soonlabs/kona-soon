use crate::{ExecutorError, ExecutorResult, L2BlockBuilder, TrieDBProvider};
use alloc::sync::Arc;
use alloy_consensus::{Header, Sealed};
use alloy_primitives::B256;
use op_alloy_rpc_types_engine::OpPayloadAttributes;
use soon_primitives::blocks::L2BlockInfo;
use kona_mpt::TrieHinter;
use soon_primitives::rollup_config::SoonRollupConfig;
use fraud_executor::accounts::SoonAccounts;
use fraud_executor::block::SimpleBlock;
use fraud_executor::executor::FraudExecutor;

#[derive(Debug)]
pub struct OffchainL2Builder<P, H>
where
    P: TrieDBProvider,
    H: TrieHinter,
{
    pub(crate) config: Arc<SoonRollupConfig>,
    pub(crate) provider: P,
    pub(crate) hinter: H,
    pub(crate) parent_header: L2BlockInfo,
    pub(crate) accounts: SoonAccounts,
}

impl<P, H> L2BlockBuilder<P, H> for OffchainL2Builder<P, H>
where
    P: TrieDBProvider,
    H: TrieHinter,
{
    fn new(
        config: Arc<SoonRollupConfig>,
        provider: P,
        hinter: H,
        parent_header: L2BlockInfo,
    ) -> Self {
        Self { config, provider, hinter, parent_header, accounts: SoonAccounts::default() }
    }

    fn init(&mut self) -> ExecutorResult<()> {
        Ok(())
    }

    fn build_block(
        &mut self,
        attrs: OpPayloadAttributes,
    ) -> ExecutorResult<L2BlockInfo> {
        // Step 1. Set up the execution environment using genesis

        // Step 2. Create the executor, using the trie database.
        // TODO: import using trie db later
        let mut executor = FraudExecutor::new(&Default::default())?;

        // Step 3. Execute the block containing the transactions within the payload attributes.
        let block = self.convert_block(attrs)?;
        let l2_info = executor.execute_block(block)?;

        // Step 4. Store data to calculate output root
        let accounts = executor.export_accounts();
        self.accounts = SoonAccounts::from(accounts);

        Ok(l2_info)
    }

    fn compute_output_root(&mut self) -> ExecutorResult<B256> {
        todo!()
    }
}

impl<P, H> OffchainL2Builder<P, H> where
    P: TrieDBProvider,
    H: TrieHinter,
{
    fn convert_block(&self, attrs: OpPayloadAttributes) -> ExecutorResult<SimpleBlock> {
        Ok(SimpleBlock {
            slot: 0,                            // TODO: get current slot
            transactions: Default::default(),   // TODO: get transactions from attrs.transactions
            extra_accounts: Default::default(), // TODO: get extra accounts from somewhere
        })
    }
}