use crate::blocks::L2BlockInfo;
use alloy_primitives::{Address, U256};
use op_alloy_rpc_types_engine::OpPayloadAttributes;

/// Payload Attributes with parent block reference.
#[derive(Debug, Clone, PartialEq)]
pub struct OpAttributesWithParent {
    /// The payload attributes.
    pub attributes: OpPayloadAttributes,
    /// The parent block reference.
    pub parent: L2BlockInfo,
    /// Whether the current batch is the last in its span.
    pub is_last_in_span: bool,
}

impl OpAttributesWithParent {
    /// Create a new [OpAttributesWithParent] instance.
    pub const fn new(
        attributes: OpPayloadAttributes,
        parent: L2BlockInfo,
        is_last_in_span: bool,
    ) -> Self {
        Self { attributes, parent, is_last_in_span }
    }

    /// Returns the payload attributes.
    pub const fn attributes(&self) -> &OpPayloadAttributes {
        &self.attributes
    }

    /// Returns the parent block reference.
    pub const fn parent(&self) -> &L2BlockInfo {
        &self.parent
    }

    /// Returns whether the current batch is the last in its span.
    pub const fn is_last_in_span(&self) -> bool {
        self.is_last_in_span
    }
}

pub fn address_to_hash(address: Address) -> U256 {
    let mut batcher_hash: Vec<u8> = vec![0; 12];
    let mut batcher_addr_vec = address.to_vec();
    batcher_hash.append(&mut batcher_addr_vec);
    U256::from_be_slice(batcher_hash.as_slice())
}
