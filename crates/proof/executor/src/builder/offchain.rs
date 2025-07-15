use crate::{ExecutorError, ExecutorResult, L2BlockBuilder, TrieDBProvider};
use alloc::sync::Arc;
use alloy_primitives::{B256, Keccak256, b256};
use fraud_executor::accounts::{AccountPairs, SoonAccounts};
use fraud_executor::block::SimpleBlock;
use fraud_executor::executor::FraudExecutor;
use kona_mpt::TrieHinter;
use op_alloy_rpc_types_engine::OpPayloadAttributes;
use solana_sdk::transaction::VersionedTransaction;
use soon_primitives::blocks::L2BlockInfo;
use soon_primitives::rollup_config::SoonRollupConfig;

pub const INIT_ACCOUNTS_HASH: B256 =
    b256!("8b4b5e2a2b0a0d3c1f8e7d4a9c6b5a2d8e1f0c9b6a3d7e0f8b4a5c2d9e6f1b8a");

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

    fn build_block(&mut self, attrs: OpPayloadAttributes) -> ExecutorResult<L2BlockInfo> {
        // Step 1. Set up the execution environment using genesis

        // Step 2. Create the executor, using the trie database.
        let init_accounts_code =
            self.provider.bytecode_by_hash(INIT_ACCOUNTS_HASH).map_err(|_| {
                ExecutorError::FraudInitError("Failed to get init accounts code".to_string())
            })?;
        let soon_accounts: SoonAccounts = bincode::deserialize(&init_accounts_code)
            .map_err(|e| ExecutorError::FraudInitError(e.to_string()))?;

        let mut executor = FraudExecutor::new(&soon_accounts)?;

        // Step 3. Execute the block containing the transactions within the payload attributes.
        let block = self.convert_block(attrs)?;
        let l2_info = executor.execute_block(block)?;

        // Step 4. Store data to calculate output root
        let accounts = executor.export_accounts();
        self.accounts = SoonAccounts::from(accounts);

        Ok(l2_info)
    }

    fn compute_output_root(&mut self) -> ExecutorResult<B256> {
        Ok(self.accounts.state_root())
    }
}

impl<P, H> OffchainL2Builder<P, H>
where
    P: TrieDBProvider,
    H: TrieHinter,
{
    fn convert_block(&self, attrs: OpPayloadAttributes) -> ExecutorResult<SimpleBlock> {
        let slot = self.parent_header.block_info.number + 1;

        Ok(SimpleBlock {
            slot,
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
            extra_accounts: self.fetch_extra_accounts(slot)?,
        })
    }

    fn fetch_extra_accounts(&self, slot: u64) -> ExecutorResult<AccountPairs> {
        let data = self
            .provider
            .bytecode_by_hash(cal_extra_accounts_hash(slot))
            .map_err(|e| ExecutorError::FraudInitError(e.to_string()))?;
        let soon_accounts: AccountPairs = bincode::deserialize(&data)
            .map_err(|e| ExecutorError::FraudInitError(e.to_string()))?;
        Ok(soon_accounts)
    }
}

/// Calculate the hash of the extra accounts for the given slot.
pub fn cal_extra_accounts_hash(slot: u64) -> B256 {
    let mut hasher = Keccak256::new();
    hasher.update(slot.to_be_bytes());
    hasher.update(b"extra_accounts");
    hasher.finalize()
}
