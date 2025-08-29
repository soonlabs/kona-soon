//! Contains the [TrieProvider] trait for fetching trie node preimages, contract bytecode, and
//! headers.

use crate::TrieNode;
use alloc::string::String;
use alloy_primitives::B256;
use core::fmt::Display;
use solana_sdk::pubkey::Pubkey;

/// The [TrieProvider] trait defines the synchronous interface for fetching trie node preimages.
pub trait TrieProvider {
    /// The error type for fetching trie node preimages.
    type Error: Display;

    /// Fetches the preimage for the given trie node hash.
    ///
    /// ## Takes
    /// - `key`: The key of the trie node to fetch.
    ///
    /// ## Returns
    /// - Ok(TrieNode): The trie node preimage.
    /// - Err(Self::Error): If the trie node preimage could not be fetched.
    fn trie_node_by_hash(&self, key: B256) -> Result<TrieNode, Self::Error>;

    /// Fetches the bank hash for the given block number.
    ///
    /// ## Takes
    /// - `block_number` - The block number at which the bank hash is to be fetched.
    ///
    /// ## Returns
    /// - Ok(B256): The bank hash.
    fn bank_hash(&self, block_number: u64) -> Result<B256, Self::Error>;

    /// Fetches the block time for the given block number.
    ///
    /// ## Takes
    /// - `block_number` - The block number at which the block time is to be fetched.
    ///
    /// ## Returns
    /// - Ok(u64): The block time.
    fn block_time(&self, block_number: u64) -> Result<i64, Self::Error>;
}

/// The [TrieHinter] trait defines the synchronous interface for hinting the host to fetch trie
/// node preimages.
pub trait TrieHinter {
    /// The error type for hinting trie node preimages.
    type Error: Display;

    /// Hints the host to fetch the trie node preimage by hash.
    ///
    /// ## Takes
    /// - `hash`: The hash of the trie node to hint.
    ///
    /// ## Returns
    /// - Ok(()): If the hint was successful.
    fn hint_trie_node(&self, hash: B256) -> Result<(), Self::Error>;

    /// Hints the host to fetch the trie node preimages on the path to the given address.
    ///
    /// ## Takes
    /// - `pubkey` - The pubkey of the account whose trie node preimages are to be fetched.
    /// - `block_number` - The block number at which the trie node preimages are to be fetched.
    ///
    /// ## Returns
    /// - Ok(()): If the hint was successful.
    /// - Err(Self::Error): If the hint was unsuccessful.
    fn hint_account_proof(&self, pubkey: &Pubkey, block_number: u64) -> Result<(), Self::Error>;

    /// Hints the host to fetch the bank hash for the given block number.
    ///
    /// ## Takes
    /// - `block_number` - The block number at which the bank hash is to be fetched.
    ///
    /// ## Returns
    /// - Ok(()): If the hint was successful.
    fn hint_bank_hash(&self, block_number: u64) -> Result<(), Self::Error>;

    /// Hints the host to fetch the block time for the given block number.
    ///
    /// ## Takes
    /// - `block_number` - The block number at which the block time is to be fetched.
    ///
    /// ## Returns
    /// - Ok(()): If the hint was successful.
    fn hint_block_time(&self, block_number: u64) -> Result<(), Self::Error>;
}
