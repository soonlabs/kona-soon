use solana_sdk::epoch_schedule::EpochSchedule;
use crate::block::SimpleBlock;
use crate::error::Result;
use litesvm::LiteSVM;
use solana_sdk::fee::FeeStructure;
#[cfg(feature = "dev")]
use solana_sdk::pubkey::Pubkey;
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

    // pub fn new(accounts: &SoonAccounts) -> Result<Self> {
    //     let mut executor = Self::default();
    //
    //
    //
    //
    //     litesvm_import_accounts(&mut executor.litesvm, accounts)?;
    //     Ok(executor)
    // }

    pub fn execute_block(&mut self, block: SimpleBlock) -> Result<L2BlockInfo> {
        // self.prepare_block(&block)?;
        self.litesvm.import_accounts(block.extra_accounts)?;
        let _results = self.litesvm.execute_block_transactions(block.transactions)?;
        self.get_l2_block_info(block.slot)
    }

    pub fn export_accounts(&self) -> AccountPairs {
        self.litesvm.export_accounts()
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

    fn get_l2_block_info(&self, slot: u64) -> Result<L2BlockInfo> {
        Ok(L2BlockInfo {
            // TODO: set zero hash here, calculate is needed later
            block_info: BlockInfo::new(B256::ZERO, slot, B256::ZERO, 0),
            l1_origin: BlockNumHash::default(),
            seq_num: 0,
        })
    }
}

// #[cfg(feature = "dev")]
// impl<CB: AccountsCallback> FraudExecutor<CB> {
//     pub fn reset_fee_collector(&mut self, fee_collector: Pubkey) {
//         self.fee_collector = fee_collector;
//     }
//
//     pub fn reset_fee_structure(&mut self, fee_structure: Option<FeeStructure>) {
//         self.fee_structure = fee_structure;
//     }
// }
