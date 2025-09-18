//! This module contains the batch types for the OP Stack derivation pipeline: [SpanBatch] &
//! [SingleBatch].

use crate::{errors::PipelineEncodingError, traits::L2ChainProvider};
use alloy_rlp::{Buf, Decodable};

mod batch_type;
pub use batch_type::BatchType;

mod validity;
pub use validity::BatchValidity;

mod span_batch;
pub use span_batch::{
    MAX_SPAN_BATCH_ELEMENTS, RawSpanBatch, SpanBatch, SpanBatchBits,
    SpanBatchEip1559TransactionData, SpanBatchEip2930TransactionData, SpanBatchElement,
    SpanBatchError, SpanBatchLegacyTransactionData, SpanBatchPayload, SpanBatchPrefix,
    SpanBatchSignature, SpanBatchTransactionData, SpanBatchTransactions, SpanDecodingError,
};

mod single_batch;
pub use single_batch::SingleBatch;
use soon_primitives::{
    blocks::{BlockInfo, L2BlockInfo},
    rollup_config::SoonRollupConfig,
};

/// A batch with its inclusion block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchWithInclusionBlock {
    /// The inclusion block
    pub inclusion_block: BlockInfo,
    /// The batch
    pub batch: Batch,
}

impl BatchWithInclusionBlock {
    /// Creates a new batch with inclusion block.
    pub const fn new(inclusion_block: BlockInfo, batch: Batch) -> Self {
        Self { inclusion_block, batch }
    }

    /// Validates the batch can be applied on top of the specified L2 safe head.
    /// The first entry of the l1_blocks should match the origin of the l2_safe_head.
    /// One or more consecutive l1_blocks should be provided.
    /// In case of only a single L1 block, the decision whether a batch is valid may have to stay
    /// undecided.
    pub async fn check_batch<BF: L2ChainProvider>(
        &self,
        cfg: &SoonRollupConfig,
        l1_blocks: &[BlockInfo],
        l2_safe_head: L2BlockInfo,
        fetcher: &mut BF,
    ) -> BatchValidity {
        match &self.batch {
            Batch::Single(single_batch) => {
                single_batch.check_batch(cfg, l1_blocks, l2_safe_head, &self.inclusion_block)
            }
            Batch::Span(span_batch) => {
                span_batch
                    .check_batch(cfg, l1_blocks, l2_safe_head, &self.inclusion_block, fetcher)
                    .await
            }
        }
    }
}

/// A Batch.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)]
pub enum Batch {
    /// A single batch
    Single(SingleBatch),
    /// Span Batches
    Span(SpanBatch),
}

impl Batch {
    /// Returns the timestamp for the batch.
    pub fn timestamp(&self) -> u64 {
        match self {
            Self::Single(sb) => sb.timestamp,
            Self::Span(sb) => sb.starting_timestamp(),
        }
    }

    /// Attempts to decode a batch from a reader.
    pub fn decode(r: &mut &[u8], _cfg: &SoonRollupConfig) -> Result<Self, PipelineEncodingError> {
        if r.is_empty() {
            return Err(PipelineEncodingError::EmptyBuffer);
        }

        // Read the batch type
        let batch_type = BatchType::from(r[0]);
        r.advance(1);

        match batch_type {
            BatchType::Single => {
                let single_batch =
                    SingleBatch::decode(r).map_err(PipelineEncodingError::AlloyRlpError)?;
                Ok(Self::Single(single_batch))
            }
            BatchType::Span => {
                let mut raw_span_batch = RawSpanBatch::decode(r)?;
                let span_batch = raw_span_batch
                    .derive(0, 0, 0)
                    .map_err(PipelineEncodingError::SpanBatchError)?;
                Ok(Self::Span(span_batch))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::hex;
    use soon_primitives::rollup_config::mock_roll_up_config;

    #[test]
    fn test_timestamp() {
        let single_batch = SingleBatch { timestamp: 123, ..Default::default() };
        let span_batch = SpanBatch {
            batches: vec![SpanBatchElement { timestamp: 456, ..Default::default() }],
            ..Default::default()
        };

        assert_eq!(Batch::Single(single_batch).timestamp(), 123);
        assert_eq!(Batch::Span(span_batch).timestamp(), 456);
    }

    #[test]
    fn test_decode_single_batch() {
        let batch_str = String::from(
            "00f84ca0e96307e235490ec6f3653f2a22add063d32a7d5928f1dbf99b32f2af2ae8d4e1836bb704a04102342dbfd9b5b25927bda585428d7ba5cb6480222a740bc5c94b47448c2f80846732c3b5c0",
        );
        let data_vec = hex::decode(batch_str.as_bytes()).unwrap();

        let rollup_config = mock_roll_up_config();
        let _ = Batch::decode(&mut data_vec.as_slice(), &rollup_config).unwrap();
    }
}
