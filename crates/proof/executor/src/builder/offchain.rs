use crate::alloc::string::ToString;
use crate::{ExecutorError, ExecutorResult, L2BlockBuilder, TrieDBProvider};
use alloc::sync::Arc;
use alloc::vec::Vec;
use alloy_primitives::{B256, Keccak256};
use core::marker::PhantomData;
use fraud_executor::accounts::SoonAccounts;
use fraud_executor::block::SimpleBlock;
use fraud_executor::executor::FraudExecutor;
use fraud_executor::outcome::BlockBuildingOutcome;
use fraud_executor::utils::analyze_account_sets;
use kona_mpt::TrieHinter;
use litesvm::LiteSVM;
use litesvm::accounts_callback::AccountsCallback;
use op_alloy_rpc_types_engine::OpPayloadAttributes;
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::transaction::VersionedTransaction;
use soon_primitives::blocks::L2BlockInfo;
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
    pub(crate) parent_header: L2BlockInfo,
    pub(crate) diff_accounts: SoonAccounts,
    _a: PhantomData<A>,
}

impl<P, H, A> L2BlockBuilder<P, H> for OffchainL2Builder<P, H, A>
where
    P: TrieDBProvider,
    H: TrieHinter,
    A: AccountsCallback + Default + serde::de::DeserializeOwned,
{
    fn new(
        config: Arc<SoonRollupConfig>,
        provider: P,
        hinter: H,
        parent_header: L2BlockInfo,
    ) -> Self {
        Self {
            _config: config,
            provider,
            _hinter: hinter,
            parent_header,
            diff_accounts: SoonAccounts::default(),
            _a: PhantomData,
        }
    }

    fn init(&mut self) -> ExecutorResult<()> {
        Ok(())
    }

    fn build_block(&mut self, attrs: OpPayloadAttributes) -> ExecutorResult<BlockBuildingOutcome> {
        // Step 1. Set up the execution environment using genesis

        // Step 2. Create the executor, using the trie database.
        let init_accounts_code =
            self.provider.bytecode_by_hash(cal_init_accounts_hash(self.init_slot())).map_err(
                |_| ExecutorError::FraudInitError("Failed to get init accounts code".to_string()),
            )?;
        let svm_start_up_meta_code = self
            .provider
            .bytecode_by_hash(cal_svm_start_up_meta_hash(self.init_slot()))
            .map_err(|_| {
                ExecutorError::FraudInitError("Failed to get svm start up meta code".to_string())
            })?;
        let svm_start_up_meta: SvmStartUpMeta = bincode::deserialize(&svm_start_up_meta_code)
            .map_err(|e| ExecutorError::FraudInitError(e.to_string()))?;

        let accounts_callback: A = bincode::deserialize(&init_accounts_code)
            .map_err(|e| ExecutorError::FraudInitError(e.to_string()))?;
        let mut svm = LiteSVM::new_soon()
            .with_slot_and_epoch(self.init_slot(), svm_start_up_meta.epoch)
            .with_fee_collector(Some(svm_start_up_meta.fee_collector))
            .with_sigverify(false)
            .with_blockhash_check(false)
            .with_accounts_callback(accounts_callback);
        svm.finish_init().map_err(|e| ExecutorError::FraudExecutorError(e.into()))?;
        let mut executor = FraudExecutor::new(svm);

        // check state root
        {
            let init_state_root = self
                .provider
                .bytecode_by_hash(cal_init_state_root_hash(self.init_slot()))
                .map_err(|_| {
                    ExecutorError::FraudInitError("Failed to get init state root".to_string())
                })?;
            let init_state_root = B256::try_from(init_state_root.to_vec().as_slice()).unwrap();
            let diff_accounts = executor.export_diff_accounts();
            let actual_state_root = SoonAccounts::from(diff_accounts).state_root();
            if init_state_root != actual_state_root {
                error!(
                    "init state root mismatch, expected: {}, actual: {}",
                    init_state_root, actual_state_root
                );
            } else {
                info!(
                    "init state root match, expected: {}, actual: {}",
                    init_state_root, actual_state_root
                );
            }
        }

        // Step 3. Execute the block containing the transactions within the payload attributes.
        let block = self.convert_block(attrs)?;
        let l2_info = executor.execute_block(block)?;

        // Step 4. Store data to calculate output root
        let accounts = executor.export_diff_accounts();
        info!("exported diff {} accounts", accounts.len());
        self.diff_accounts = SoonAccounts::from(accounts);

        // check execution account states
        {
            let new_accounts_data = self
                .provider
                .bytecode_by_hash(cal_init_accounts_hash(self.current_slot()))
                .map_err(|_| {
                    ExecutorError::FraudInitError("Failed to get init state root".to_string())
                })?;
            let new_block_accounts: SoonAccounts = bincode::deserialize(&new_accounts_data)
                .map_err(|e| ExecutorError::FraudInitError(e.to_string()))?;
            let soon_state_root = new_block_accounts.state_root();
            let litesvm_state_root = self.diff_accounts.state_root();
            if soon_state_root == litesvm_state_root {
                info!("state root match, both are: {}", soon_state_root);
            } else {
                info!(
                    "state root mismatch, expected: {}, actual: {}",
                    soon_state_root, litesvm_state_root
                );
                let (_, _, _, analyze) =
                    analyze_account_sets(&new_block_accounts, &self.diff_accounts);
                info!("check execution account states, analyze: {}", analyze);
            }
        }

        Ok(l2_info)
    }

    fn compute_output_root(&mut self) -> ExecutorResult<B256> {
        if self.diff_accounts.accounts.len() == 0 {
            let init_accounts_code = self
                .provider
                .bytecode_by_hash(cal_init_accounts_hash(self.init_slot()))
                .map_err(|_| {
                    ExecutorError::FraudInitError("Failed to get init accounts code".to_string())
                })?;
            let soon_accounts: SoonAccounts = bincode::deserialize(&init_accounts_code)
                .map_err(|e| ExecutorError::FraudInitError(e.to_string()))?;
            Ok(soon_accounts.state_root())
        } else {
            Ok(self.diff_accounts.state_root())
        }
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

    const fn init_slot(&self) -> u64 {
        self.parent_header.block_info.number
    }

    fn convert_block(&self, attrs: OpPayloadAttributes) -> ExecutorResult<SimpleBlock> {
        let slot = self.current_slot();
        let (hash, parent_hash) = self.fetch_slot_hash_pair(slot)?;

        Ok(SimpleBlock {
            slot,
            hash,
            parent_hash,
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

    fn fetch_slot_hash_pair(&self, slot: u64) -> ExecutorResult<(B256, B256)> {
        let data = self
            .provider
            .bytecode_by_hash(slot_hash_pair_hash(slot))
            .map_err(|e| ExecutorError::FraudInitError(e.to_string()))?;
        let slot_hash_pair: (B256, B256) = bincode::deserialize(&data)
            .map_err(|e| ExecutorError::FraudInitError(e.to_string()))?;
        Ok(slot_hash_pair)
    }
}

/// Calculate the hash of the init accounts for the given slot.
pub fn cal_init_accounts_hash(slot: u64) -> B256 {
    slot_spec_hash(slot, b"init_accounts")
}

/// Calculate the hash of the init state root for the given slot.
pub fn cal_init_state_root_hash(slot: u64) -> B256 {
    slot_spec_hash(slot, b"init_state_root")
}

/// Calculate the hash of the slot hash pair for the given slot.
pub fn slot_hash_pair_hash(slot: u64) -> B256 {
    slot_spec_hash(slot, b"slot_hash_set")
}

/// Calculate the hash of the svm start up meta for the given slot.
pub fn cal_svm_start_up_meta_hash(slot: u64) -> B256 {
    slot_spec_hash(slot, b"svm_start_up_meta")
}

fn slot_spec_hash(slot: u64, suffix: &[u8]) -> B256 {
    let mut hasher = Keccak256::new();
    hasher.update(slot.to_be_bytes());
    hasher.update(suffix);
    hasher.finalize()
}

/// The svm start up meta data for the given slot.
#[allow(missing_docs)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SvmStartUpMeta {
    pub epoch: u64,
    pub fee_collector: Pubkey,
}
