use crate::accounts::{AccountPairs, SoonAccounts};
use crate::block::SimpleBlock;
use crate::error::Result;
use crate::outcome::BlockBuildingOutcome;
use crate::utils::litesvm_import_accounts;
use l1_block_info::state::L1BlockInfo;
use litesvm::LiteSVM;
use solana_sdk::account::ReadableAccount;
use solana_sdk::clock::Clock;
use solana_sdk::fee::FeeStructure;
use solana_sdk::program_pack::Pack;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::rent_collector::RentCollector;
use soon_mpt_primitives::B256;
use soon_mpt_primitives::alloy::eips::BlockNumHash;
use soon_primitives::blocks::{BlockInfo, L2BlockInfo};

#[derive(Debug)]
pub struct FraudExecutor {
    pub litesvm: LiteSVM,
    // TODO: init fee collector from genesis
    fee_collector: Pubkey,
    // TODO: init rent collector from genesis
    rent_collector: RentCollector,
    // TODO: init fee structure from genesis
    fee_structure: Option<FeeStructure>,
}

impl Default for FraudExecutor {
    fn default() -> Self {
        Self {
            litesvm: LiteSVM::default().with_builtins(None).with_sysvars().with_precompiles(None),
            fee_collector: Pubkey::default(),
            rent_collector: RentCollector::default(),
            fee_structure: None,
        }
    }
}

impl FraudExecutor {
    pub fn new(accounts: &SoonAccounts) -> Result<Self> {
        let mut executor = Self::default();
        litesvm_import_accounts(&mut executor.litesvm, accounts)?;
        Ok(executor)
    }

    pub fn execute_block(&mut self, block: SimpleBlock) -> Result<BlockBuildingOutcome> {
        self.prepare_block(&block)?;
        let execution_result =
            self.litesvm.execute_block_transactions(block.transactions, self.fee_collector)?;
        self.litesvm.import_accounts(block.extra_accounts)?;
        self.cleanup_block();

        Ok(BlockBuildingOutcome {
            header: self.get_l2_block_info(block.slot, block.hash, block.parent_hash)?,
            execution_result,
        })
    }

    pub fn export_accounts(&self) -> AccountPairs {
        self.litesvm.export_accounts()
    }

    fn prepare_block(&mut self, _block: &SimpleBlock) -> Result<()> {
        // TODO: prepare slot based environment
        // TODO: new rent collector according to new slot
        // TODO: new fee structure according to new slot
        self.litesvm.set_rent_collector(Some(self.rent_collector.clone()));
        self.litesvm.set_fee_structure(self.fee_structure.clone());
        Ok(())
    }

    fn cleanup_block(&mut self) {
        self.litesvm.set_rent_collector(None);
    }

    fn get_l2_block_info(&self, slot: u64, hash: B256, parent_hash: B256) -> Result<L2BlockInfo> {
        let l1_block_info = self.get_l1_block_info().unwrap_or_default();
        let clock = self.litesvm.get_sysvar::<Clock>();
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

#[cfg(feature = "dev")]
impl FraudExecutor {
    pub fn reset_fee_collector(&mut self, fee_collector: Pubkey) {
        self.fee_collector = fee_collector;
    }

    pub fn reset_fee_structure(&mut self, fee_structure: Option<FeeStructure>) {
        self.fee_structure = fee_structure;
    }
}
