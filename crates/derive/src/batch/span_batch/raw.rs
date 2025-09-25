//! Raw Span Batch

use super::{SpanBatch, SpanBatchElement, SpanBatchError, SpanBatchPayload, SpanBatchPrefix};
use crate::batch::{BatchType, SpanDecodingError};
use alloc::{vec, vec::Vec};
use alloy_primitives::bytes;

/// Raw Span Batch
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawSpanBatch {
    /// The span batch prefix
    pub prefix: SpanBatchPrefix,
    /// The span batch payload
    pub payload: SpanBatchPayload,
}

impl TryFrom<SpanBatch> for RawSpanBatch {
    type Error = SpanBatchError;

    fn try_from(value: SpanBatch) -> Result<Self, Self::Error> {
        if value.batches.is_empty() {
            return Err(SpanBatchError::EmptySpanBatch);
        }

        // These should never error since we check for an empty batch above.
        let span_start = value.batches.first().ok_or(SpanBatchError::EmptySpanBatch)?;
        let span_end = value.batches.last().ok_or(SpanBatchError::EmptySpanBatch)?;

        Ok(Self {
            prefix: SpanBatchPrefix {
                rel_timestamp: span_start.timestamp - value.genesis_timestamp,
                l1_origin_num: span_end.epoch_num,
                parent_check: value.parent_check,
                l1_origin_check: value.l1_origin_check,
            },
            payload: SpanBatchPayload {
                block_count: value.batches.len() as u64,
                origin_bits: value.origin_bits.clone(),
                block_tx_counts: value.block_tx_counts.clone(),
                txs: value.txs.clone(),
            },
        })
    }
}

impl RawSpanBatch {
    /// Returns the batch type
    pub const fn get_batch_type(&self) -> BatchType {
        BatchType::Span
    }

    /// Encodes the [RawSpanBatch] into a writer.
    pub fn encode(&self, w: &mut dyn bytes::BufMut) -> Result<(), SpanBatchError> {
        self.prefix.encode_prefix(w);
        self.payload.encode_payload(w)
    }

    /// Decodes the [RawSpanBatch] from a reader.]
    pub fn decode(r: &mut &[u8]) -> Result<Self, SpanBatchError> {
        let prefix = SpanBatchPrefix::decode_prefix(r)?;
        let payload = SpanBatchPayload::decode_payload(r)?;
        Ok(Self { prefix, payload })
    }

    /// Converts a [RawSpanBatch] into a [SpanBatch], which has a list of [SpanBatchElement]s. Thos
    /// function does not populate the [SpanBatch] with chain configuration data, which is
    /// required for making payload attributes.
    pub fn derive(
        &mut self,
        block_time: u64,
        genesis_time: u64,
        chain_id: u64,
    ) -> Result<SpanBatch, SpanBatchError> {
        if self.payload.block_count == 0 {
            return Err(SpanBatchError::EmptySpanBatch);
        }

        let mut block_origin_nums = vec![0u64; self.payload.block_count as usize];
        let mut l1_origin_number = self.prefix.l1_origin_num;
        for i in (0..self.payload.block_count).rev() {
            block_origin_nums[i as usize] = l1_origin_number;
            if self
                .payload
                .origin_bits
                .get_bit(i as usize)
                .ok_or(SpanBatchError::Decoding(SpanDecodingError::L1OriginCheck))?
                == 1
                && i > 0
            {
                l1_origin_number -= 1;
            }
        }

        // Get all transactions in the batch.
        let enveloped_txs = self.payload.txs.full_txs(chain_id)?;

        let mut tx_idx = 0;
        let batches = (0..self.payload.block_count).fold(Vec::new(), |mut acc, i| {
            let transactions =
                (0..self.payload.block_tx_counts[i as usize]).fold(Vec::new(), |mut acc, _| {
                    acc.push(enveloped_txs[tx_idx].clone());
                    tx_idx += 1;
                    acc
                });
            acc.push(SpanBatchElement {
                epoch_num: block_origin_nums[i as usize],
                timestamp: genesis_time + self.prefix.rel_timestamp + block_time * i,
                transactions: transactions.into_iter().map(|v| v.into()).collect(),
            });
            acc
        });

        Ok(SpanBatch {
            parent_check: self.prefix.parent_check,
            l1_origin_check: self.prefix.l1_origin_check,
            batches,
            ..Default::default()
        })
    }
}
