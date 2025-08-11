//! Contains the tip for the derivation driver.

use alloy_primitives::B256;
use soon_primitives::blocks::L2BlockInfo;

/// A cursor that keeps track of the L2 tip block.
#[derive(Debug, Clone)]
pub struct TipCursor {
    /// The current L2 safe head.
    pub l2_safe_head: L2BlockInfo,
    /// The state root of the L2 safe head.
    pub l2_safe_head_state_root: B256,
}

impl TipCursor {
    /// Instantiates a new `SyncCursor`.
    pub fn new(l2_safe_head: L2BlockInfo, l2_safe_head_state_root: B256) -> Self {
        Self { l2_safe_head, l2_safe_head_state_root }
    }

    /// Returns the current L2 safe head.
    pub const fn l2_safe_head(&self) -> &L2BlockInfo {
        &self.l2_safe_head
    }

    /// Returns the state root of the L2 safe head.
    pub const fn l2_safe_head_state_root(&self) -> &B256 {
        &self.l2_safe_head_state_root
    }
}
