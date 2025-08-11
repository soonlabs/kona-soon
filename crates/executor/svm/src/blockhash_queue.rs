use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use solana_program::clock::MAX_RECENT_BLOCKHASHES;
use solana_program::fee_calculator::FeeCalculator;
use solana_program::hash::Hash;
use solana_program::sysvar;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockhashQueue {
    /// index of last hash to be registered
    last_hash_index: u64,
    /// last hash to be registered
    last_hash: Option<Hash>,
    hashes: HashMap<Hash, HashInfo>,
    /// hashes older than `max_age` will be dropped from the queue
    max_age: usize,
}

impl BlockhashQueue {
    pub fn new(max_age: usize) -> Self {
        Self {
            hashes: HashMap::new(),
            last_hash_index: 0,
            last_hash: None,
            max_age,
        }
    }

    pub fn last_hash(&self) -> Hash {
        self.last_hash.expect("no hash has been set")
    }

    pub fn get_lamports_per_signature(&self, hash: &Hash) -> Option<u64> {
        self.hashes
            .get(hash)
            .map(|hash_age| hash_age.fee_calculator.lamports_per_signature)
    }

    /// Check if the age of the hash is within the specified age
    pub fn is_hash_valid_for_age(&self, hash: &Hash, max_age: usize) -> bool {
        self.get_hash_info_if_valid(hash, max_age).is_some()
    }

    /// Get hash info for the specified hash if it is in the queue and its age
    /// of the hash is within the specified age
    pub(crate) fn get_hash_info_if_valid(&self, hash: &Hash, max_age: usize) -> Option<&HashInfo> {
        self.hashes.get(hash).filter(|info| {
            Self::is_hash_index_valid(self.last_hash_index, max_age, info.hash_index)
        })
    }

    pub fn get_hash_age(&self, hash: &Hash) -> Option<u64> {
        self.hashes
            .get(hash)
            .map(|info| self.last_hash_index - info.hash_index)
    }

    pub(crate) fn register_hash(&mut self, hash: Hash, lamports_per_signature: u64) {
        self.last_hash_index += 1;
        if self.hashes.len() >= self.max_age {
            self.hashes.retain(|_, info| {
                Self::is_hash_index_valid(self.last_hash_index, self.max_age, info.hash_index)
            });
        }

        self.hashes.insert(
            hash,
            HashInfo {
                fee_calculator: FeeCalculator::new(lamports_per_signature),
                hash_index: self.last_hash_index,
                timestamp: 0,
            },
        );

        self.last_hash = Some(hash);
    }

    #[allow(deprecated)]
    pub(crate) fn get_recent_blockhashes(&self) -> impl Iterator<Item = sysvar::recent_blockhashes::IterItem<'_>> {
        self.hashes.iter().map(|(k, v)| {
            sysvar::recent_blockhashes::IterItem(v.hash_index, k, v.fee_calculator.lamports_per_signature)
        })
    }

    fn is_hash_index_valid(last_hash_index: u64, max_age: usize, hash_index: u64) -> bool {
        last_hash_index - hash_index <= max_age as u64
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub(crate) struct HashInfo {
    fee_calculator: FeeCalculator,
    hash_index: u64,
    timestamp: u64,
}

impl HashInfo {
    pub(crate) fn lamports_per_signature(&self) -> u64 {
        self.fee_calculator.lamports_per_signature
    }
}

impl Default for BlockhashQueue {
    fn default() -> Self {
        Self::new(MAX_RECENT_BLOCKHASHES)
    }
}