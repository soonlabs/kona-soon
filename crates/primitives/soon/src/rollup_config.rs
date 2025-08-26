use alloy_primitives::Address;
use solana_sdk::clock::Slot;
use solana_sdk::hash::Hash;
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;

/// `UpgradeSchedules` configures when network upgrades activate.
pub type UpgradeSchedules = HashMap<String, Option<Slot>>;

/// The Rollup configuration for Soon.
#[derive(Debug, Clone, Eq, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct SoonRollupConfig {
    /// The L1 chain ID
    pub l1_chain_id: u64,
    /// Sequencer batches may not be more than MaxSequencerDrift seconds after
    /// the L1 timestamp of the sequencing window end.
    pub max_sequencer_drift: u64,
    /// The sequencer window size.
    pub seq_window_size: u64,
    /// Number of L1 blocks between when a channel can be opened and when it can be closed.
    pub channel_timeout: u64,
    /// `batch_inbox_address` is the L1 address that batches are sent to.
    pub batch_inbox_address: Address,
    /// `optimism_portal_address` is the L1 address that deposits are sent to.
    pub optimism_portal_address: Address,
    /// `l1_system_config_address` is the L1 address that the system config is stored at.
    pub l1_system_config_address: Address,
    /// address to get l1 optimism standard bridge contract.
    pub l1_standard_bridge: Address,
    /// address to get l1 cross domain messenger contract.
    pub l1_cross_domain_messenger: Address,
    /// L1 block number to delay deriving L2 blocks
    pub derive_delay_l1_block_num: u64,
    /// The max frame size
    pub max_frame_size: usize,
    /// The rlp bytes per channel
    pub channel_size: usize,
    /// L2 block time
    pub block_time: u64,
    /// L2 shred version
    pub shred_version: Option<u16>,
    /// L2 genesis hash
    pub genesis_hash: Option<Hash>,
    /// Soon upgrade schedules
    pub upgrade_schedules: UpgradeSchedules,
    /// Soon sequencer schedules
    pub sequencer_schedules: Vec<(Slot, Pubkey)>,
}

impl SoonRollupConfig {
    pub fn new_for_genesis(
        l1_chain_id: u64,
        l1_system_config_address: Address,
        derive_delay_l1_block_num: u64,
    ) -> Self {
        Self {
            l1_chain_id,
            max_sequencer_drift: 86400,
            seq_window_size: 604800,
            channel_timeout: 1800,
            batch_inbox_address: Address::default(),
            optimism_portal_address: Address::default(),
            l1_system_config_address,
            l1_standard_bridge: Address::default(),
            l1_cross_domain_messenger: Address::default(),
            derive_delay_l1_block_num,
            max_frame_size: 2000000,
            channel_size: 30000000,
            block_time: 0,
            shred_version: None,
            genesis_hash: None,
            upgrade_schedules: UpgradeSchedules::default(),
            sequencer_schedules: Vec::new(),
        }
    }
}
