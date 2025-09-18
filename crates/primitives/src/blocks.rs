use crate::deposit::{AttributesDeposited, UserDeposited};
use crate::derive::OpAttributesWithParent;
use crate::error::BlockError;
use crate::payload::BlockPayload;
use alloy_primitives::{B256, BlockHash};
// use alloy::rpc::types::Block;
use alloy_consensus::Header;
use alloy_eips::BlockNumHash;
use alloy_primitives::{Address, BlockNumber, Sealed};
use alloy_rlp::{RlpDecodable, RlpEncodable};
use solana_program::pubkey::Pubkey;
use solana_sdk::bs58;
use solana_sdk::transaction::SanitizedTransaction;
use solana_transaction_status::VersionedConfirmedBlock;
use std::collections::HashSet;

#[derive(
    Debug, Clone, Copy, Eq, Hash, PartialEq, Default, serde::Serialize, serde::Deserialize,
)]
pub struct BlockInfo {
    /// The block hash
    pub hash: B256,
    /// The block number
    pub number: u64,
    /// The parent block hash
    pub parent_hash: B256,
    /// The block timestamp
    pub timestamp: u64,
}

impl BlockInfo {
    /// Instantiates a new [BlockInfo].
    pub const fn new(hash: B256, number: u64, parent_hash: B256, timestamp: u64) -> Self {
        Self { hash, number, parent_hash, timestamp }
    }

    /// Returns the block ID.
    pub const fn id(&self) -> BlockNumHash {
        BlockNumHash { hash: self.hash, number: self.number }
    }
}

impl core::fmt::Display for BlockInfo {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "BlockInfo {{ hash: {}, number: {}, parent_hash: {}, timestamp: {} }}",
            self.hash, self.number, self.parent_hash, self.timestamp
        )
    }
}

#[derive(Clone, Debug, Default, Copy)]
pub struct L1Header {
    pub hash: B256,
    pub parent_hash: B256,
    pub number: BlockNumber,
    pub timestamp: u64,
    pub state_root: B256,
    pub transactions_root: B256,
    pub receipts_root: B256,
}

impl L1Header {
    pub const fn seal(self, hash: B256) -> Sealed<Self> {
        Sealed::new_unchecked(self, hash)
    }
}

impl From<L1Header> for BlockInfo {
    fn from(header: L1Header) -> Self {
        Self {
            hash: header.hash,
            number: header.number,
            parent_hash: header.parent_hash,
            timestamp: header.timestamp,
        }
    }
}

impl From<Header> for L1Header {
    fn from(header: Header) -> Self {
        Self {
            hash: header.hash_slow(),
            parent_hash: header.parent_hash,
            number: header.number,
            timestamp: header.timestamp,
            state_root: header.state_root,
            transactions_root: header.transactions_root,
            receipts_root: header.receipts_root,
        }
    }
}

#[derive(Clone, Debug, RlpEncodable, RlpDecodable)]
#[rlp(trailing)]
pub struct L1Transaction {
    pub hash: B256,
    pub from: Address,
    pub input: Vec<u8>,
    pub to: Option<Address>,
}

#[derive(Clone)]
pub struct RawBlock {
    pub l1_origin: BlockInfo,
    pub sequence_number: u64,
    pub deposit_txs_count: u64,
    /// contains: l1_block_info_tx + l2_deposit_tx
    pub transactions: Vec<SanitizedTransaction>,
}

#[derive(
    Debug, Clone, Copy, Hash, Eq, PartialEq, Default, serde::Serialize, serde::Deserialize,
)]
pub struct L2BlockHeader {
    /// The base [BlockInfo]
    pub block_info: BlockInfo,
    /// Account state root
    pub account_root: B256,
    /// Withdraw account state root
    pub widthdraw_root: B256,
}

#[derive(
    Debug, Clone, Copy, Hash, Eq, PartialEq, Default, serde::Serialize, serde::Deserialize,
)]
pub struct L2BlockInfo {
    /// The base [BlockInfo]
    pub block_info: BlockInfo,
    /// The L1 origin [BlockNumHash]
    pub l1_origin: BlockNumHash,
    /// The sequence number of the L2 block
    pub seq_num: u64,
}

impl L2BlockInfo {
    /// Instantiates a new [op_alloy_protocol::L2BlockInfo].
    pub const fn new(block_info: BlockInfo, l1_origin: BlockNumHash, seq_num: u64) -> Self {
        Self { block_info, l1_origin, seq_num }
    }
}

#[derive(Clone, Default)]
pub struct L2Block {
    pub l1_origin: BlockInfo,
    pub sequence_number: u64,
    pub payload: BlockPayload,
}

#[derive(Debug, Clone, Copy)]
pub enum DeriveEvents {
    NewHead(BlockInfo),
    Finalized(BlockInfo),
    Safe(BlockInfo),
    Reorg(u64),
}

#[derive(Debug, Clone)]
pub struct DeriveBlock {
    pub l1_origin: BlockInfo,
    pub l1_info_tx: AttributesDeposited,
    pub deposit_txs: Vec<UserDeposited>,
    pub finalized: bool,
}

impl TryFrom<RawBlock> for L2Block {
    type Error = BlockError;

    fn try_from(value: RawBlock) -> Result<Self, Self::Error> {
        Ok(Self {
            l1_origin: value.l1_origin,
            sequence_number: value.sequence_number,
            payload: BlockPayload::new_with_l1_derived(value.transactions, value.deposit_txs_count),
        })
    }
}

impl RawBlock {
    pub fn try_init(
        derive_block: DeriveBlock,
        sequence_number: u64,
        reserved_account_keys: &HashSet<Pubkey>,
    ) -> Result<Self, BlockError> {
        let DeriveBlock { l1_origin, l1_info_tx, deposit_txs, finalized: _ } = derive_block;

        let mut transactions = vec![];

        // Step 1: prepare fixed transaction
        let tx = l1_info_tx.to_sanitized_transaction(reserved_account_keys)?;
        transactions.push(tx);

        // Step 2: for each l1 deposit tx, generate a l2 deposit tx.
        let deposit_txs_count = deposit_txs.len();
        for deposit in deposit_txs {
            let sanitized_tx = deposit.to_sanitized_transaction(reserved_account_keys)?;
            transactions.push(sanitized_tx);
        }

        Ok(Self {
            l1_origin,
            sequence_number,
            transactions,
            deposit_txs_count: deposit_txs_count as u64,
        })
    }
}

impl TryFrom<OpAttributesWithParent> for RawBlock {
    type Error = BlockError;

    fn try_from(_value: OpAttributesWithParent) -> Result<Self, Self::Error> {
        // TODO: convert to RawBlock when batcher is ready
        todo!()
    }
}

pub fn str_block_hash_to(str_hash: &str) -> BlockHash {
    if let Ok(data) = bs58::decode(str_hash).into_vec() {
        B256::from_slice(data.as_slice())
    } else {
        panic!("invalid block hash:{}", str_hash)
    }
}

pub fn to_l2_block_info(
    block: Option<&VersionedConfirmedBlock>,
    l1_origin: BlockNumHash,
    seq_num: u64,
    slot: u64,
) -> L2BlockInfo {
    let l2_block_info = block.map(|block| {
        BlockInfo::new(
            str_block_hash_to(block.blockhash.as_str()),
            slot,
            str_block_hash_to(block.previous_blockhash.as_str()),
            block.block_time.unwrap_or_default().try_into().unwrap(),
        )
    });
    L2BlockInfo { block_info: l2_block_info.unwrap_or_default(), l1_origin, seq_num }
}

pub const fn current_derived_tx_version() -> u64 {
    0
}
