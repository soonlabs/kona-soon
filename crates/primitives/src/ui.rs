use serde::{Deserialize, Serialize};
use solana_program::hash::ParseHashError;
use solana_transaction_status::{EntrySummary, UiConfirmedBlock};

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UiEntrySummary {
    pub num_hashes: u64,
    pub hash: String,
    pub num_transactions: u64,
    pub starting_transaction_index: usize,
}

impl From<EntrySummary> for UiEntrySummary {
    fn from(value: EntrySummary) -> Self {
        Self {
            num_hashes: value.num_hashes,
            hash: value.hash.to_string(),
            num_transactions: value.num_transactions,
            starting_transaction_index: value.starting_transaction_index,
        }
    }
}

impl TryFrom<UiEntrySummary> for EntrySummary {
    type Error = ParseHashError;

    fn try_from(value: UiEntrySummary) -> Result<Self, Self::Error> {
        Ok(Self {
            num_hashes: value.num_hashes,
            hash: value.hash.parse()?,
            num_transactions: value.num_transactions,
            starting_transaction_index: value.starting_transaction_index,
        })
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UiConfirmedBlockWithEntries {
    pub block: UiConfirmedBlock,
    pub entries: Vec<UiEntrySummary>,
}
