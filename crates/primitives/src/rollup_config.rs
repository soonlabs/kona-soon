use crate::error::RollupConfigError;
use alloy_primitives::Address;
use core::str::FromStr;
use solana_program::clock::Slot;
use solana_program::hash::Hash;
use solana_program::pubkey::{ParsePubkeyError, Pubkey};
use std::collections::HashMap;
use std::default::Default;

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
    pub max_frame_size: u32,
    /// The rlp bytes per channel
    pub channel_size: u32,
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

/// The Origin rollup configuration parsed from json file.
#[derive(Debug, Clone, Eq, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct OriginSoonRollupConfig {
    /// The L1 chain ID
    pub l1_chain_id: u64,
    /// Sequencer batches may not be more than MaxSequencerDrift seconds after
    /// the L1 timestamp of the sequencing window end.
    pub max_sequencer_drift: u64,
    /// The sequencer window size.
    pub seq_window_size: u64,
    /// Number of L1 blocks between when a channel can be opened and when it can be closed.
    pub channel_timeout: u64,
    /// `l1_system_config_address` is the L1 address that the system config is stored at.
    pub l1_system_config_address: String,
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
    pub genesis_hash: Option<String>,
    /// Soon upgrade schedules
    #[serde(default)]
    pub upgrade_schedules: UpgradeSchedules,
    /// Soon sequencer schedules
    #[serde(default)]
    pub sequencer_schedules: Vec<(Slot, String)>,
}

impl TryFrom<OriginSoonRollupConfig> for SoonRollupConfig {
    type Error = RollupConfigError;

    fn try_from(value: OriginSoonRollupConfig) -> Result<Self, Self::Error> {
        Ok(SoonRollupConfig {
            l1_chain_id: value.l1_chain_id,
            max_sequencer_drift: value.max_sequencer_drift,
            seq_window_size: value.seq_window_size,
            channel_timeout: value.channel_timeout,
            l1_system_config_address: value.l1_system_config_address.parse()?,
            derive_delay_l1_block_num: value.derive_delay_l1_block_num,
            max_frame_size: value.max_frame_size as u32,
            channel_size: value.channel_size as u32,
            block_time: value.block_time,
            shred_version: value.shred_version,
            genesis_hash: value.genesis_hash.map(|s| Hash::from_str(&s)).transpose()?,
            upgrade_schedules: value.upgrade_schedules,
            sequencer_schedules: value
                .sequencer_schedules
                .into_iter()
                .map(|(slot, pubkey)| Ok((slot, Pubkey::from_str(&pubkey)?)))
                .collect::<Result<Vec<_>, ParsePubkeyError>>()?,
            ..Default::default()
        })
    }
}

impl From<SoonRollupConfig> for OriginSoonRollupConfig {
    fn from(value: SoonRollupConfig) -> Self {
        Self {
            l1_chain_id: value.l1_chain_id,
            max_sequencer_drift: value.max_sequencer_drift,
            seq_window_size: value.seq_window_size,
            channel_timeout: value.channel_timeout,
            l1_system_config_address: value.l1_system_config_address.to_string(),
            derive_delay_l1_block_num: value.derive_delay_l1_block_num,
            max_frame_size: value.max_frame_size as usize,
            channel_size: value.channel_size as usize,
            block_time: value.block_time,
            shred_version: value.shred_version,
            genesis_hash: value.genesis_hash.map(|h| h.to_string()),
            upgrade_schedules: value.upgrade_schedules,
            sequencer_schedules: value
                .sequencer_schedules
                .into_iter()
                .map(|(slot, pubkey)| (slot, pubkey.to_string()))
                .collect(),
        }
    }
}

pub fn mock_roll_up_config() -> SoonRollupConfig {
    SoonRollupConfig {
        max_sequencer_drift: 1800,
        seq_window_size: 1000,
        channel_timeout: 1800,
        l1_chain_id: 11155111,
        channel_size: 300,
        ..Default::default()
    }
}
