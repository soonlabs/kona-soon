use crate::blocks::{BlockInfo, L2BlockInfo};
use std::sync::{Arc, RwLock};

#[derive(Default, Clone, Debug)]
pub struct SharedState {
    pub l2: Arc<RwLock<L2State>>,
    pub l1: Arc<RwLock<L1State>>,
}

#[derive(Default, Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct L2State {
    // UnsafeL2 is the absolute tip of the L2 chain,
    // pointing to block data that has not been submitted to L1 yet.
    // The sequencer is building this, and verifiers may also be ahead of the
    // SafeL2 block if they sync blocks via p2p or other offchain sources.
    pub unsafe_l2: L2BlockInfo,
    // SafeL2 points to the L2 block that was derived from the L1 chain.
    // This point may still reorg if the L1 chain reorgs.
    pub safe_l2: L2BlockInfo,
    // FinalizedL2 points to the L2 block that was derived fully from
    // finalized L1 information, thus irreversible.
    pub finalized_l2: L2BlockInfo,
    // PendingSafeL2 points to the L2 block processed from the batch,
    // but not consolidated to the safe block yet.
    pub pending_safe_l2: L2BlockInfo,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct L1State {
    // CurrentL1 is the L1 block that the derivation process is last idled at.
    // This may not be fully derived into L2 data yet.
    // The safe L2 blocks were produced/included fully from the L1 chain up to and including this L1 block.
    // If the node is synced, this matches the HeadL1, minus the verifier confirmation distance.
    pub current_l1: BlockInfo,
    // HeadL1 is the perceived head of the L1 chain, no confirmation distance.
    // The head is not guaranteed to build on the other L1 sync status fields,
    // as the node may be in progress of resetting to adapt to a L1 reorg.
    pub head_l1: BlockInfo,
    pub safe_l1: BlockInfo,
    pub finalized_l1: BlockInfo,
}

impl SharedState {
    pub fn new(
        unsafe_l2: L2BlockInfo,
        safe_l2: L2BlockInfo,
        finalized_l2: L2BlockInfo,
        head_l1: BlockInfo,
        safe_l1: BlockInfo,
        finalized_l1: BlockInfo,
    ) -> Self {
        Self {
            l2: Arc::new(RwLock::new(L2State {
                pending_safe_l2: safe_l2,
                unsafe_l2,
                safe_l2,
                finalized_l2,
            })),
            l1: Arc::new(RwLock::new(L1State {
                head_l1,
                safe_l1,
                finalized_l1,
                current_l1: Default::default(), // current l1 only set when derived
            })),
        }
    }

    pub fn unsafe_l2_height(&self) -> u64 {
        self.unsafe_l2().block_info.number
    }

    pub fn unsafe_l2(&self) -> L2BlockInfo {
        self.l2.read().unwrap().unsafe_l2
    }

    pub fn safe_l2_height(&self) -> u64 {
        self.safe_l2().block_info.number
    }

    pub fn safe_l2(&self) -> L2BlockInfo {
        self.l2.read().unwrap().safe_l2
    }

    pub fn finalized_l2_height(&self) -> u64 {
        self.finalized_l2().block_info.number
    }

    pub fn finalized_l2(&self) -> L2BlockInfo {
        self.l2.read().unwrap().finalized_l2
    }

    pub fn pending_safe_l2_height(&self) -> u64 {
        self.pending_safe_l2().block_info.number
    }

    pub fn pending_safe_l2(&self) -> L2BlockInfo {
        self.l2.read().unwrap().pending_safe_l2
    }

    pub fn set_unsafe_l2(&self, current: L2BlockInfo) {
        self.l2.write().unwrap().unsafe_l2 = current;
    }

    pub fn set_safe_l2(&self, safe: L2BlockInfo) {
        self.l2.write().unwrap().safe_l2 = safe;
    }

    pub fn set_finalized_l2(&self, finalized: L2BlockInfo) {
        self.l2.write().unwrap().finalized_l2 = finalized;
    }

    pub fn set_pending_safe_l2(&self, pending: L2BlockInfo) {
        self.l2.write().unwrap().pending_safe_l2 = pending;
    }

    pub fn reset_safe_l2(&self) {
        let finalized_l2 = self.l2.read().unwrap().finalized_l2;
        self.l2.write().unwrap().pending_safe_l2 = finalized_l2;
        self.l2.write().unwrap().safe_l2 = finalized_l2;
    }

    pub fn current_l1(&self) -> BlockInfo {
        self.l1.read().unwrap().current_l1
    }

    pub fn head_l1(&self) -> BlockInfo {
        self.l1.read().unwrap().head_l1
    }

    pub fn safe_l1(&self) -> BlockInfo {
        self.l1.read().unwrap().safe_l1
    }

    pub fn finalized_l1(&self) -> BlockInfo {
        self.l1.read().unwrap().finalized_l1
    }

    pub fn set_current_l1(&self, current: BlockInfo) {
        self.l1.write().unwrap().current_l1 = current;
    }

    pub fn set_head_l1(&self, head: BlockInfo) {
        self.l1.write().unwrap().head_l1 = head;
    }

    pub fn set_safe_l1(&self, safe: BlockInfo) {
        self.l1.write().unwrap().safe_l1 = safe;
    }

    pub fn set_finalized_l1(&self, finalized: BlockInfo) {
        self.l1.write().unwrap().finalized_l1 = finalized;
    }

    pub fn l1_state(&self) -> L1State {
        self.l1.read().unwrap().clone()
    }

    pub fn l2_state(&self) -> L2State {
        self.l2.read().unwrap().clone()
    }
}
