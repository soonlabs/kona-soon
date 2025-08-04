use l1_block_info::state::L1BlockInfo;
use solana_sdk::account::ReadableAccount;
use solana_sdk::clock::Clock;
use solana_sdk::epoch_schedule::EpochSchedule;
use crate::block::SimpleBlock;
use crate::error::Result;
use crate::outcome::BlockBuildingOutcome;
use litesvm::LiteSVM;
use solana_sdk::fee::FeeStructure;
use solana_sdk::program_pack::Pack;
use solana_sdk::rent_collector::RentCollector;
use soon_mpt_primitives::B256;
use soon_mpt_primitives::alloy::eips::BlockNumHash;
use soon_primitives::blocks::{BlockInfo, L2BlockInfo};
use litesvm::accounts_callback::AccountsCallback;
use crate::accounts::AccountPairs;

#[derive(Debug, Default)]
pub struct FraudExecutor<CB: AccountsCallback> {
    pub litesvm: LiteSVM<CB>,
    // TODO: init fee collector from genesis
    epoch_schedule: EpochSchedule,
    // TODO: init rent collector from genesis
    rent_collector: RentCollector,
    // TODO: init fee structure from genesis
    fee_structure: Option<FeeStructure>,
}

impl<CB: AccountsCallback> FraudExecutor<CB> {
    pub fn new(litesvm: LiteSVM<CB>) -> Self {
        Self {
            litesvm,
            ..Default::default()
        }
    }

    pub fn execute_block(&mut self, block: SimpleBlock) -> Result<BlockBuildingOutcome> {
        // self.prepare_block(&block)?;
        // self.litesvm.import_accounts(block.extra_accounts)?;
        let execution_result = self.litesvm.execute_block_transactions(block.transactions)?;

        Ok(BlockBuildingOutcome {
            header: self.get_l2_block_info(block.slot, block.hash, block.parent_hash)?,
            execution_result,
        })
    }

    pub fn export_diff_accounts(&self) -> AccountPairs {
        self.litesvm.export_diff_accounts()
    }

    // fn prepare_block(&mut self, _block: &SimpleBlock) -> Result<()> {
    //     // TODO: prepare slot based environment
    //     // TODO: new rent collector according to new slot
    //     // TODO: new fee structure according to new slot
    //     self.litesvm.set_rent_collector(Some(self.rent_collector.clone()));
    //     self.litesvm.set_fee_structure(self.fee_structure.clone());
    //     Ok(())
    // }
    //
    // fn clear_block(&mut self) {
    //     self.litesvm.set_rent_collector(None);
    // }

    fn get_l2_block_info(&self, slot: u64, hash: B256, parent_hash: B256) -> Result<L2BlockInfo> {
        let l1_block_info = self.get_l1_block_info().unwrap_or_default();
        let clock = self.litesvm.get_sysvar::<Clock>()?;
        Ok(L2BlockInfo {
            block_info: BlockInfo::new(hash, slot, parent_hash, clock.unix_timestamp as u64),
            l1_origin: BlockNumHash::new(l1_block_info.number, l1_block_info.hash.into()),
            seq_num: l1_block_info.sequence_number,
        })
    }

    fn get_l1_block_info(&self) -> Option<L1BlockInfo> {
        let l1_info_account = l1_block_info::pda::l1_block_info_pubkey();
        let l1_data = self.litesvm.get_account(&l1_info_account);
        let l1_block_info = l1_data
            .and_then(|l1_data| l1_block_info::state::L1BlockInfo::unpack(l1_data.data()).ok());
        l1_block_info
    }
}
