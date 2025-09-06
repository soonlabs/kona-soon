//! This module contains an implementation of an in-memory Trie DB for [`revm`], that allows for
//! incremental updates through fetching node preimages on the fly during execution.

use crate::errors::{TrieDBError, TrieDBResult};
use alloc::{format, string::ToString, vec::Vec};
use alloy_primitives::{B256, keccak256};
use alloy_rlp::Decodable;
use kona_mpt::{Nibbles, TrieHinter, TrieNode};
use litesvm::accounts_callback::AccountsCallback;
use solana_sdk::account::{AccountSharedData, ReadableAccount};
use solana_sdk::pubkey::Pubkey;
use soon_primitives::mpt::encoder::{sol_account_encoder, withdrawal_account_encoder};
use soon_primitives::{
    blocks::L2BlockHeader, mpt::WrappedSolanaAccount, mpt::account_from_solana_native,
};

mod traits;
pub use traits::{NoopTrieDBProvider, TrieDBProvider};

/// A Trie DB that caches open state in-memory.
///
/// When accounts that don't already exist within the cached [`TrieNode`] are queried, the database
/// fetches the preimages of the trie nodes on the path to the account using the `PreimageFetcher`
/// (`F` generic). This allows for data to be fetched in a verifiable manner given an initial
/// trusted state root as it is needed during execution.
///
/// The [`TrieDB`] is intended to be wrapped by a [`State`], which is then used by [`revm`] to
/// capture state transitions during block execution.
///
/// **Behavior**:
/// - When an account is queried and the trie path has not already been opened by [Self::basic], we
///   fall through to the `PreimageFetcher` to fetch the preimages of the trie nodes on the path to
///   the account. After it has been fetched, the path will be cached until the next call to
///   [Self::state_root].
/// - When querying for the code hash of an account, the [`TrieDBProvider`] is consulted to fetch
///   the code hash of the account.
/// - When a [`BundleState`] changeset is committed to the parent [`State`] database, the changes
///   are first applied to the [`State`]'s cache, then the trie hash is recomputed with
///   [Self::state_root].
/// - When the block hash of a block number is needed via [Self::block_hash], the
///   `HeaderByHashFetcher` is consulted to walk back to the desired block number by revealing the
///   parent hash of block headers until the desired block number is reached, up to a maximum of
///   [BLOCK_HASH_HISTORY] blocks back relative to the current parent block hash.
///
#[derive(Debug, Clone)]
pub struct TrieDB<F, H>
where
    F: TrieDBProvider,
    H: TrieHinter,
{
    /// The [`TrieNode`] representation of the root node.
    root_node: TrieNode,
    /// withdrawal trie.
    withdrawal_node: TrieNode,
    /// The parent block header of the current block.
    parent_block_header: L2BlockHeader,
    /// The [`TrieDBProvider`]
    pub fetcher: F,
    /// The [`TrieHinter`]
    pub hinter: H,
}

impl<F, H> TrieDB<F, H>
where
    F: TrieDBProvider,
    H: TrieHinter,
{
    /// Creates a new [TrieDB] with the given root node.
    pub fn new(parent_block_header: L2BlockHeader, fetcher: F, hinter: H) -> Self {
        Self {
            root_node: TrieNode::new_blinded(parent_block_header.account_root),
            withdrawal_node: TrieNode::new_blinded(parent_block_header.widthdraw_root),
            parent_block_header,
            fetcher,
            hinter,
        }
    }

    /// Consumes `Self` and takes the current state root of the trie DB.
    pub fn take_root_node(self) -> TrieNode {
        self.root_node
    }

    /// Returns a shared reference to the root [TrieNode] of the trie DB.
    pub const fn root(&self) -> &TrieNode {
        &self.root_node
    }

    /// Returns a reference to the current parent block header of the trie DB.
    pub const fn parent_block_header(&self) -> &L2BlockHeader {
        &self.parent_block_header
    }

    /// Sets the parent block header of the trie DB. Should be called after a block has been
    /// executed and the Header has been created.
    ///
    /// ## Takes
    /// - `parent_block_header`: The parent block header of the current block.
    pub fn set_parent_block_header(&mut self, parent_block_header: L2BlockHeader) {
        self.parent_block_header = parent_block_header;
    }

    /// Applies a [BundleState] changeset to the [TrieNode] and recomputes the world status.
    ///
    /// ## Takes
    /// - `bundle`: The [BundleState] changeset to apply to the trie DB.
    ///
    /// ## Returns
    /// - `Ok((B256, B256))`: The new state root hash + withdrawal root hash of the trie DB.
    /// - `Err(_)`: If the state root hash could not be computed.
    pub fn world_states<'a>(
        &mut self,
        account_diff: impl IntoIterator<Item = (&'a Pubkey, &'a AccountSharedData)>,
    ) -> TrieDBResult<(B256, B256)> {
        debug!(target: "client_executor", "Recomputing state root");

        // Update the accounts in the trie with the changeset.
        self.update_accounts(account_diff)?;

        // Recompute the root hash of the trie.
        let state_root = self.root_node.blind();
        let withdrawal_root = self.withdrawal_node.blind();

        info!(
            target: "client_executor",
            "block {} recomputed state root: {state_root}, withdrawal root: {withdrawal_root}",
            self.parent_block_header.block_info.number + 1
        );

        // Extract the new state root from the root node.
        Ok((state_root, withdrawal_root))
    }

    /// Modifies the accounts in the storage trie with the given [BundleState] changeset.
    ///
    /// ## Takes
    /// - `bundle`: The [BundleState] changeset to apply to the trie DB.
    ///
    /// ## Returns
    /// - `Ok(())` if the accounts were successfully updated.
    /// - `Err(_)` if the accounts could not be updated.
    fn update_accounts<'a>(
        &mut self,
        account_diff: impl IntoIterator<Item = (&'a Pubkey, &'a AccountSharedData)>,
    ) -> TrieDBResult<()> {
        // Sort the account keys prior to applying the changeset, to ensure that the order of
        // application is deterministic between runs.
        let mut sorted_state =
            account_diff.into_iter().map(|(k, v)| (k, keccak256(*k), v)).collect::<Vec<_>>();
        sorted_state.sort_by_key(|(_, hashed_addr, _)| *hashed_addr);

        for (pubkey, hashed_address, bundle_account) in sorted_state {
            // Compute the path to the account in the trie.
            info!("update accounts: {:?} <-> {}", pubkey, hashed_address);
            let account_path = Nibbles::unpack(hashed_address.as_slice());
            let is_withdrawal = bundle_account.owner().to_bytes()
                == soon_primitives::mpt::WITHDRAWAL_PROGRAM_PUBKEY.to_bytes();

            // If the account was destroyed, delete it from the trie.
            if bundle_account.lamports() == 0 {
                self.root_node.delete(&account_path, &self.fetcher, &self.hinter)?;
                if is_withdrawal {
                    self.withdrawal_node.delete(&account_path, &self.fetcher, &self.hinter)?;
                }
                continue;
            }

            // RLP encode the trie account for insertion.
            let mpt_account = account_from_solana_native(bundle_account);
            let account_buf = sol_account_encoder()(&mpt_account);

            // Insert or update the account in the trie.
            self.root_node.insert(&account_path, account_buf.into(), &self.fetcher)?;
            if is_withdrawal {
                let buf = withdrawal_account_encoder()(&mpt_account);
                self.withdrawal_node.insert(&account_path, buf.into(), &self.fetcher)?;
            }
        }

        Ok(())
    }

    /// Fetches the bank hash for the given block number.
    ///
    /// ## Takes
    /// - `block_number` - The block number at which the bank hash is to be fetched.
    ///
    /// ## Returns
    /// - Ok(B256): The bank hash.
    pub fn bank_hash(&self, block_number: u64) -> TrieDBResult<B256> {
        self.hinter
            .hint_bank_hash(block_number)
            .map_err(|e| TrieDBError::Provider(e.to_string()))?;
        Ok(self
            .fetcher
            .bank_hash(block_number)
            .map_err(|e| TrieDBError::Provider(e.to_string()))?)
    }

    /// Fetches the block time for the given block number.
    ///
    /// ## Takes
    /// - `block_number` - The block number at which the block time is to be fetched.
    ///
    /// ## Returns
    /// - Ok(u64): The block time.
    pub fn block_time(&self, block_number: u64) -> TrieDBResult<i64> {
        self.hinter
            .hint_block_time(block_number)
            .map_err(|e| TrieDBError::Provider(e.to_string()))?;
        Ok(self
            .fetcher
            .block_time(block_number)
            .map_err(|e| TrieDBError::Provider(e.to_string()))?)
    }
}

impl<F, H> AccountsCallback for TrieDB<F, H>
where
    F: TrieDBProvider,
    H: TrieHinter,
{
    type Error = TrieDBError;
    fn get_account_data(
        &mut self,
        pubkey: &Pubkey,
    ) -> Result<Option<AccountSharedData>, Self::Error> {
        self.hinter
            .hint_account_proof(pubkey, self.parent_block_header.block_info.number)
            .map_err(|e| TrieDBError::Provider(e.to_string()))?;
        let account_bytes = self
            .fetcher
            .data_by_hash(keccak256(pubkey))
            .map_err(|e| TrieDBError::MissingAccountInfo)?;
        if account_bytes.len() == 0 {
            return Ok(None);
        }
        let account: WrappedSolanaAccount = Decodable::decode(&mut account_bytes.as_ref())
            .map_err(|e| TrieDBError::Provider(format!("fail to parse solana account: {}", e)))?;
        Ok(Some(account.0))
    }
}

// TODO: uncomment me when is ready
// #[cfg(test)]
// mod tests {
//     use super::*;
//     use alloy_consensus::Sealable;
//     use alloy_primitives::b256;
//     use kona_mpt::NoopTrieHinter;
//
//     fn new_test_db() -> TrieDB<NoopTrieDBProvider, NoopTrieHinter> {
//         TrieDB::new(Header::default().seal_slow(), NoopTrieDBProvider, NoopTrieHinter)
//     }
//
//     #[test]
//     fn test_trie_db_take_root_node() {
//         let db = new_test_db();
//         let root_node = db.take_root_node();
//         assert_eq!(root_node.blind(), EMPTY_ROOT_HASH);
//     }
//
//     #[test]
//     fn test_trie_db_root_node_ref() {
//         let db = new_test_db();
//         let root_node = db.root();
//         assert_eq!(root_node.blind(), EMPTY_ROOT_HASH);
//     }
//
//     #[test]
//     fn test_trie_db_storage_roots() {
//         let db = new_test_db();
//         let storage_roots = db.storage_roots();
//         assert!(storage_roots.is_empty());
//     }
//
//     #[test]
//     fn test_block_hash_above_range() {
//         let mut db = new_test_db();
//         db.parent_block_header = Header { number: 10, ..Default::default() }.seal_slow();
//         let block_number = 11;
//         let block_hash = db.block_hash(block_number).unwrap();
//         assert_eq!(block_hash, B256::default());
//     }
//
//     #[test]
//     fn test_block_hash_below_range() {
//         let mut db = new_test_db();
//         db.parent_block_header =
//             Header { number: BLOCK_HASH_HISTORY + 10, ..Default::default() }.seal_slow();
//         let block_number = 0;
//         let block_hash = db.block_hash(block_number).unwrap();
//         assert_eq!(block_hash, B256::default());
//     }
//
//     #[test]
//     fn test_block_hash_provider_missing_hash() {
//         let mut db = new_test_db();
//         db.parent_block_header = Header { number: 10, ..Default::default() }.seal_slow();
//         let block_number = 5;
//         let block_hash = db.block_hash(block_number).unwrap();
//         assert_eq!(
//             block_hash,
//             b256!("78dec18c6d7da925bbe773c315653cdc70f6444ed6c1de9ac30bdb36cff74c3b")
//         );
//     }
// }
