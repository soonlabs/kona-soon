use alloy_primitives::bytes::BufMut;
use alloy_primitives::{B256, keccak256};
use alloy_rlp::{Decodable, Encodable, RlpDecodable, RlpEncodable};
use serde::{Deserialize, Serialize};
use solana_sdk::account::{AccountSharedData, ReadableAccount};
use solana_sdk::pubkey::Pubkey;
use std::ops::Deref;

/// A solana adaptive account to use mpt.
/// we derive a new fork version instead of impl `Compact` for:
/// 1. bool, u64, need a flag to record length, which is derived by Compact automatically.
/// 2. we need implement `Compact` for `PubKey`.
#[derive(
    Debug, Default, Serialize, Deserialize, PartialEq, Eq, Clone, RlpEncodable, RlpDecodable,
)]
pub struct TrieSolanaAccount {
    /// lamports in the account
    pub lamports: u64,
    /// data held in this account
    pub data: B256,
    /// this account's data contains a loaded program (and is now read-only)
    pub executable: bool,
    /// the epoch at which this account will next owe rent
    pub rent_epoch: u64,
    /// the program that owns this account. If executable, the program that loads this account.
    pub owner: TrieSolanaPubkey,
}

impl From<AccountSharedData> for TrieSolanaAccount {
    fn from(account: AccountSharedData) -> Self {
        TrieSolanaAccount {
            lamports: account.lamports(),
            data: keccak256(account.data()),
            owner: TrieSolanaPubkey::from(B256::from_slice(account.owner().as_ref())),
            executable: account.executable(),
            rent_epoch: account.rent_epoch(),
        }
    }
}

/// Use to fight with compiler to transfer `Pubkey([u8;32])` to `B256`(which is also a [u8;32] inner)
#[derive(Default, Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct TrieSolanaPubkey(pub Pubkey);

impl From<B256> for TrieSolanaPubkey {
    fn from(value: B256) -> Self {
        Self(Pubkey::from(value.0))
    }
}

impl From<TrieSolanaPubkey> for B256 {
    fn from(value: TrieSolanaPubkey) -> Self {
        B256::new(value.0.to_bytes())
    }
}

impl Encodable for TrieSolanaPubkey {
    fn encode(&self, out: &mut dyn BufMut) {
        self.0.to_bytes().encode(out)
    }
}

impl Decodable for TrieSolanaPubkey {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let decoded: [u8; 32] = Decodable::decode(buf)?;
        Ok(Self(Pubkey::from(decoded)))
    }
}

impl Deref for TrieSolanaPubkey {
    type Target = Pubkey;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Default, Clone, RlpEncodable, RlpDecodable)]
pub struct AccountWithTrie {
    pub block_number: u64,
    pub account: WrappedSolanaAccount,
    pub proofs: Vec<Vec<u8>>,
}

/// Just used for implement `Encodable` && `Decodable`
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct WrappedSolanaAccount(pub AccountSharedData);

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
