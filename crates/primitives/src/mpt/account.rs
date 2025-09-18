use alloy_primitives::B256;
use alloy_rlp::{BufMut, Decodable, Encodable, RlpDecodable, RlpEncodable};
use derive_more::Deref;
use serde::{Deserialize, Serialize};
use solana_program::pubkey::Pubkey;

/// Use to fight with compiler to transfer `Pubkey([u8;32])` to `B256`(which is also a [u8;32] inner)
#[derive(Default, Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Deref)]
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

#[derive(
    Default, Serialize, Deserialize, PartialEq, Eq, Clone, RlpEncodable, RlpDecodable, Debug,
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
