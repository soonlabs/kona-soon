use alloy_primitives::{Address, B256};
use core::fmt::Display;

pub trait TrieHinter {
    /// The error type for hinting trie node preimages.
    type Error: Display;

    /// Hints the host to fetch the trie node preimage by hash.
    ///
    /// ## Takes
    /// - `hash`: The hash of the trie node to hint.
    fn hint_trie_node(&self, hash: B256) -> Result<(), Self::Error>;

    /// Hints the host to fetch the trie node preimages on the path to the given address.
    ///
    /// ## Takes
    /// - `address` - The address of the contract whose trie node preimages are to be fetched.
    /// - `block_number` - The block number at which the trie node preimages are to be fetched.
    fn hint_account_proof(&self, address: Address, block_number: u64) -> Result<(), Self::Error>;

    /// Hints the host to fetch the withdrawal trie node preimages on the Bridge Accounts.
    ///
    /// ## Takes
    /// - `address` - The withdrawal PDA accounts.
    /// - `block_number` - The block number at which the trie node preimages are to be fetched.
    fn hint_withdrawal_proof(&self, address: Address, block_number: u64)
    -> Result<(), Self::Error>;
}
