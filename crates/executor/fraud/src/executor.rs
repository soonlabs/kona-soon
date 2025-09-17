use crate::{accounts::AccountPairs, error::Result, outcome::BlockBuildingOutcome};
use alloy_eips::BlockNumHash;
use alloy_primitives::B256;
use l1_block_info::state::L1BlockInfo;
use litesvm::{LiteSVM, RawBlock, accounts_callback::AccountsCallback};
use solana_sdk::{account::ReadableAccount, clock::Clock, program_pack::Pack};
use soon_primitives::blocks::{BlockInfo, L2BlockInfo};

#[derive(Debug, Default)]
pub struct FraudExecutor<CB: AccountsCallback> {
    pub svm: LiteSVM<CB>,
}

impl<CB: AccountsCallback> FraudExecutor<CB> {
    pub fn new(litesvm: LiteSVM<CB>) -> Self {
        Self { svm: litesvm }
    }

    pub fn execute_block(&mut self, block: impl Into<RawBlock>) -> Result<BlockBuildingOutcome> {
        let execution_result = self.svm.execute_block(block.into())?;
        let l2_block_info = self.get_l2_block_info()?;
        Ok(BlockBuildingOutcome {
            block_info: l2_block_info,
            state_root: B256::ZERO,
            withdraw_root: B256::ZERO,
            execution_result,
            signature_count: self.svm.signature_count(),
            fee_rate_governor: self.svm.fee_rate_governor().clone(),
        })
    }

    pub fn export_diff_accounts(&self) -> AccountPairs {
        self.svm.export_diff_accounts()
    }

    fn get_l2_block_info(&mut self) -> Result<L2BlockInfo> {
        let l1_block_info = self.get_l1_block_info().unwrap_or_default();
        let clock = self.svm.get_sysvar::<Clock>()?;
        let slot = self.svm.slot();
        let parent_hash = self.svm.parent_blockhash()?.to_bytes();
        let hash = self.svm.blockhash()?.to_bytes();
        Ok(L2BlockInfo {
            block_info: BlockInfo::new(
                hash.into(),
                slot,
                parent_hash.into(),
                clock.unix_timestamp as u64,
            ),
            l1_origin: BlockNumHash::new(l1_block_info.number, l1_block_info.hash.into()),
            seq_num: l1_block_info.sequence_number,
        })
    }

    fn get_l1_block_info(&mut self) -> Option<L1BlockInfo> {
        let l1_info_account = l1_block_info::pda::l1_block_info_pubkey();
        let l1_data = self.svm.get_account(&l1_info_account);
        let l1_block_info = l1_data
            .and_then(|l1_data| l1_block_info::state::L1BlockInfo::unpack(l1_data.data()).ok());
        l1_block_info
    }
}
