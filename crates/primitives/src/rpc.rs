use serde::{Deserialize, Serialize};
use solana_account_decoder::UiAccount;
use solana_sdk::clock::Slot;

fn fn_never<T>(_: &T) -> bool {
    false
}
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct OutputAtBlockResp {
    pub version: String,
    pub output_root: String,
    pub state_root: String,
    pub withdrawal_root: String,
    pub block_hash: String,
    // the slot of response status.
    pub query_slot: Slot,
}

/// SoonGetWithdrawalProofResp follows the format of eth_getProof response, but with specific soon
/// info. It contains follow fields:
///     * the merkle root of all solana account state.
///     * merkle proof for withdrawal native program address (calling WNP below).
///     * When one raise a withdrawal request, the WNP will derive a new pda account. This api also
///       generates root and proof of this pda in storage-hash way. So api also contains:
///         * merkle root of WNP storage hash
///         * proof for pda address in WNP's storage root
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct SoonGetWithdrawalProofResp {
    pub state_root: String,            // state root of all address
    pub withdrawal_root: String,       // state root of WNP storage_root
    pub state_proof: Vec<String>,      // proof for WNP on root_hash
    pub withdrawal_proof: Vec<String>, // proof for pda on storage_hash
}

/// SoonGetAccountProofResp generates any account proof on global state root.
/// It contains follow fields:
///     * the merkle root of all solana account state.
///     * merkle proof for given address.
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct SoonGetAccountProofResp {
    pub state_root: String, // state root of all address
    pub proof: Vec<String>, // proof for given address
    pub query_slot: Slot,   // queried height of root and proof
    pub exist: bool,        // the account is existed or not
    /// raw account status on query_slot height.
    /// only store some specific accounts on mpt store.
    /// so don't promise this field will have value.
    #[serde(skip_serializing_if = "fn_never")]
    pub account: Option<UiAccount>,
    /// If the account is withdrawal related, will extend an withdrawal proof field.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub withdrawal_proof: Option<Vec<String>>,
}
