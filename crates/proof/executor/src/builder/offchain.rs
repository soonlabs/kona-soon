use crate::{ExecutorError, ExecutorResult, L2BlockBuilder, TrieDBProvider};
use alloc::collections::BTreeMap;
use alloc::string::ToString;
use alloc::sync::Arc;
use alloy_primitives::{B256, Keccak256, keccak256};
use core::marker::PhantomData;
use fraud_executor::accounts::SoonAccounts;
use fraud_executor::executor::FraudExecutor;
use fraud_executor::outcome::BlockBuildingOutcome;
use kona_mpt::TrieHinter;
use litesvm::accounts_callback::AccountsCallback;
use litesvm::{L2Block, L2Transaction, LiteSVM, ParentInfo};
use op_alloy_rpc_types_engine::OpPayloadAttributes;
use solana_sdk::pubkey::Pubkey;
use soon_primitives::blocks::L2BlockHeader;
use soon_primitives::rollup_config::SoonRollupConfig;

/// The [`OffchainL2Builder`] is an OP Stack block builder that uses the offchain data to build a
/// block.
#[derive(Debug)]
pub struct OffchainL2Builder<P, H, A>
where
    P: TrieDBProvider,
    H: TrieHinter,
    A: AccountsCallback,
{
    pub(crate) _config: Arc<SoonRollupConfig>,
    pub(crate) provider: P,
    pub(crate) _hinter: H,
    pub(crate) parent_header: L2BlockHeader,
    pub(crate) diff_accounts: SoonAccounts,
    pub(crate) state_root: B256,
    _a: PhantomData<A>,
}

impl<P, H, A> L2BlockBuilder<P, H> for OffchainL2Builder<P, H, A>
where
    P: TrieDBProvider,
    H: TrieHinter,
    A: AccountsCallback + Default + serde::de::DeserializeOwned + From<SoonAccounts>,
{
    fn new(
        config: Arc<SoonRollupConfig>,
        provider: P,
        hinter: H,
        parent_header: L2BlockHeader,
        _last_accounts_diff: SoonAccounts,
    ) -> Self {
        Self {
            _config: config,
            provider,
            _hinter: hinter,
            parent_header,
            diff_accounts: SoonAccounts::default(),
            state_root: B256::ZERO,
            _a: PhantomData,
        }
    }

    fn init(&mut self) -> ExecutorResult<()> {
        // TODO: litesvm should be initialized here
        Ok(())
    }

    fn build_block(&mut self, attrs: OpPayloadAttributes) -> ExecutorResult<BlockBuildingOutcome> {
        // Step 1. Set up the execution environment using genesis

        // Step 2. Create the executor, using the trie database.
        let soon_accounts = self.get_init_accounts()?;
        let mut soon_accounts_map: BTreeMap<_, _> = soon_accounts.clone().into();
        let parent_info =
            self.provider.data_by_hash(cal_svm_parent_info(self.parent_slot())).map_err(|_| {
                ExecutorError::FraudInitError("Failed to get svm parent info code".to_string())
            })?;
        let parent_info: ParentInfo = bincode::deserialize(&parent_info)
            .map_err(|e| ExecutorError::FraudInitError(e.to_string()))?;
        let clock_timestamp =
            self.provider.data_by_hash(cal_svm_clock_timestamp(self.current_slot())).map_err(
                |_| ExecutorError::FraudInitError("Failed to get clock timestamp".to_string()),
            )?;
        let clock_timestamp: i64 = bincode::deserialize(&clock_timestamp)
            .map_err(|e| ExecutorError::FraudInitError(e.to_string()))?;
        let leader = self
            .provider
            .data_by_hash(cal_svm_leader())
            .map_err(|_| ExecutorError::FraudInitError("Failed to get svm leader".to_string()))?;
        let leader: Pubkey = bincode::deserialize(&leader)
            .map_err(|e| ExecutorError::FraudInitError(e.to_string()))?;

        let mut svm: LiteSVM<A> = LiteSVM::new_soon()
            .with_parent_info(parent_info)
            .with_leader_schedule(leader.into())
            .with_sig_verify(false)
            .with_blockhash_verify(true)
            .with_accounts_callback(soon_accounts.into())
            .with_clock_timestamp(clock_timestamp);
        svm.finish_init().map_err(|e| ExecutorError::FraudExecutorError(e.into()))?;
        let mut executor = FraudExecutor::new(svm);

        // Step 3. Execute the block containing the transactions within the payload attributes.
        let block = self.convert_block(attrs)?;
        let mut outcome = executor.execute_block(block)?;

        // Step 4. Store data to calculate output root
        let accounts_diff = executor.export_diff_accounts();
        info!("exported diff {} accounts", accounts_diff.len());
        self.diff_accounts = SoonAccounts::from(accounts_diff);

        // compute state root
        {
            for (key, account) in self.diff_accounts.accounts.iter() {
                soon_accounts_map.insert(*key, account.clone());
            }
            let soon_accounts: SoonAccounts = soon_accounts_map.into();
            let state_root = soon_accounts.state_root();
            self.state_root = state_root;
            outcome.state_root = state_root;

            let actual_root =
                self.provider.data_by_hash(cal_init_state_root_hash(self.current_slot())).map_err(
                    |_| ExecutorError::FraudInitError("Failed to get init state root".to_string()),
                )?;
            let actual_root = B256::try_from(actual_root.to_vec().as_slice()).unwrap();

            if state_root != actual_root {
                warn!("state root mismatch, expected: {}, actual: {}", actual_root, state_root);
            } else {
                info!("state root matches: {}", state_root);
            }
        }

        Ok(outcome)
    }

    fn compute_output_root(&mut self) -> ExecutorResult<B256> {
        if self.state_root == B256::ZERO {
            let soon_accounts = self.get_init_accounts()?;
            self.state_root = soon_accounts.state_root();
        }
        Ok(self.state_root)
    }

    fn account_diff(&self) -> SoonAccounts {
        self.diff_accounts.clone()
    }
}

impl<P, H, A> OffchainL2Builder<P, H, A>
where
    P: TrieDBProvider,
    H: TrieHinter,
    A: AccountsCallback,
{
    const fn current_slot(&self) -> u64 {
        self.parent_header.block_info.number + 1
    }

    const fn parent_slot(&self) -> u64 {
        self.parent_header.block_info.number
    }

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

    fn get_init_accounts(&self) -> ExecutorResult<SoonAccounts> {
        let soon_accounts =
            self.provider.data_by_hash(cal_soon_accounts_hash(self.parent_slot())).map_err(
                |_| ExecutorError::FraudInitError("Failed to get soon accounts code".to_string()),
            )?;
        let soon_accounts: SoonAccounts = bincode::deserialize(&soon_accounts)
            .map_err(|e| ExecutorError::FraudInitError(e.to_string()))?;

        Ok(soon_accounts)
    }
}

/// Calculate the hash of the init accounts for the given slot.
pub fn cal_soon_accounts_hash(slot: u64) -> B256 {
    slot_spec_hash(slot, b"soon_accounts")
}

/// Calculate the hash of the init state root for the given slot.
pub fn cal_init_state_root_hash(slot: u64) -> B256 {
    slot_spec_hash(slot, b"init_state_root")
}

/// Calculate the hash of the SVM parent info for the given slot.
pub fn cal_svm_parent_info(slot: u64) -> B256 {
    slot_spec_hash(slot, b"svm_parent_info")
}

/// Calculate the hash of the SVM clock timestamp for the given slot.
pub fn cal_svm_clock_timestamp(slot: u64) -> B256 {
    slot_spec_hash(slot, b"svm_clock_timestamp")
}

/// Calculate the hash of the SVM leader.
pub fn cal_svm_leader() -> B256 {
    keccak256(b"svm_leader")
}

fn slot_spec_hash(slot: u64, suffix: &[u8]) -> B256 {
    let mut hasher = Keccak256::new();
    hasher.update(slot.to_be_bytes());
    hasher.update(suffix);
    hasher.finalize()
}
