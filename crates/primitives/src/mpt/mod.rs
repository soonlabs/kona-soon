pub mod account;
pub mod encoder;

use self::account::{TrieSolanaAccount as Account, TrieSolanaPubkey};
use alloy_primitives::U256;
use alloy_primitives::{B256, keccak256};
use alloy_rlp::{BufMut, Decodable, Encodable, RlpDecodable, RlpEncodable};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use solana_program::clock::Slot;
use solana_program::pubkey::Pubkey;
use solana_sdk::account::{AccountSharedData, ReadableAccount};

pub const WITHDRAWAL_PROGRAM: &str = "Bridge1111111111111111111111111111111111111";

pub static WITHDRAWAL_PROGRAM_PUBKEY: Lazy<Pubkey> = Lazy::new(|| {
    Pubkey::try_from(solana_sdk::bs58::decode(WITHDRAWAL_PROGRAM).into_vec().unwrap()).unwrap()
});

/// flatten data of ```crate::unique_entry::mpt_root::MptRoot```
pub type MptRootItem = (Slot, B256, B256);

#[derive(Debug)]
pub struct BlockInfo {
    pub slot: Slot,
    pub block_hash: B256,
}

/// transfer from solana account to mpt storage account.
pub fn account_from_solana_native(account: &AccountSharedData) -> Account {
    Account {
        lamports: account.lamports(),
        data: keccak256(account.data()),
        owner: TrieSolanaPubkey::from(B256::from_slice(account.owner().as_ref())),
        executable: account.executable(),
        rent_epoch: account.rent_epoch(),
    }
}

/// A function to decide whether we should store raw data on mpt
/// Account should not be very large, and currently we only store
/// some specific accounts.
///
/// * SPL Token - All mint Address
pub fn store_raw_account(account: &AccountSharedData) -> bool {
    // allow spl token -> mint token
    if *account.owner() == spl_token::id() {
        // mint size = 82, ATA = 165
        if account.data().len() == 82 {
            return true;
        }
    }
    false
}

/// Just used for implement `Encodable` && `Decodable`
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct WrappedSolanaAccount(pub AccountSharedData);

#[derive(Debug, Default, Clone, RlpEncodable, RlpDecodable)]
#[rlp(trailing)]
pub struct AccountWithTrie {
    pub block_number: u64,
    pub proofs: Vec<Vec<u8>>,
    pub withdrawal_proofs: Vec<Vec<u8>>,
    pub account: Option<WrappedSolanaAccount>,
}

impl Encodable for WrappedSolanaAccount {
    fn encode(&self, out: &mut dyn BufMut) {
        let tx_bytes = bincode::serialize(&self.0).unwrap_or_default();
        tx_bytes.encode(out);
    }
}

impl Decodable for WrappedSolanaAccount {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let tx_bytes = Vec::<u8>::decode(buf)?;
        let account = bincode::deserialize(&tx_bytes)
            .map_err(|_| alloy_rlp::Error::Custom("Failed to deserialize VersionedTransaction"))?;
        Ok(WrappedSolanaAccount(account))
    }
}

pub use self::encoder::{sol_account_encoder, withdrawal_account_encoder};

pub fn output_root(state_root: B256, withdrawal_root: B256, block_hash: B256) -> B256 {
    let version = U256::ZERO;
    let mut hasher = solana_program::keccak::Hasher::default();
    hasher.hash(version.as_le_slice());
    hasher.hash(state_root.as_slice());
    hasher.hash(withdrawal_root.as_slice());
    hasher.hash(block_hash.as_slice());
    B256::from(hasher.result().0)
}
