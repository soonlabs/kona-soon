//! This module contains the [SpanBatchSignature] type, which represents the ECDSA signature of a
//! transaction within a span batch.

use super::SpanBatchError;
use alloy_primitives::{B256, Signature, U256};

/// The ECDSA signature of a transaction within a span batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpanBatchSignature {
    pub(crate) v: bool,
    pub(crate) r: U256,
    pub(crate) s: U256,
}

impl From<Signature> for SpanBatchSignature {
    fn from(value: Signature) -> Self {
        Self { v: value.v(), r: value.r(), s: value.s() }
    }
}

impl TryFrom<SpanBatchSignature> for Signature {
    type Error = SpanBatchError;

    fn try_from(value: SpanBatchSignature) -> Result<Self, Self::Error> {
        Ok(Self::from_scalars_and_parity(B256::from(value.r), B256::from(value.s), value.v))
    }
}
