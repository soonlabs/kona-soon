use alloy_consensus::Header;
use alloy_eips::BlockNumHash;
use alloy_primitives::{Address, B256, BlockHash, BlockNumber};
use alloy_rlp::{RlpDecodable, RlpEncodable};
use solana_sdk::bs58;

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

#[derive(Clone, Debug, RlpEncodable, RlpDecodable)]
#[rlp(trailing)]
pub struct L1Transaction {
    pub hash: B256,
    pub from: Address,
    pub input: Vec<u8>,
    pub to: Option<Address>,
}

pub fn str_block_hash_to(str_hash: &str) -> BlockHash {
    if let Ok(data) = bs58::decode(str_hash).into_vec() {
        B256::from_slice(data.as_slice())
    } else {
        panic!("invalid block hash:{}", str_hash)
    }
}
