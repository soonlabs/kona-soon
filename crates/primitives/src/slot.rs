use solana_sdk::hash::Hash;

#[derive(Debug, Clone, Default)]
pub struct SlotHead {
    pub slot: u64,
    pub hash: Option<Hash>,
    pub timestamp: Option<i64>,
}

#[derive(Debug, Clone, Default)]
pub struct SlotInfo {
    pub head: SlotHead,
    pub parent: SlotHead,
    pub store_height: Option<u64>,
}
