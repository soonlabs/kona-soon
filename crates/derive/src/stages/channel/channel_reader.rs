//! This module contains the `ChannelReader` struct.

use crate::{
    batch::Batch,
    errors::PipelineError,
    stages::BatchStreamProvider,
    traits::{OriginAdvancer, OriginProvider, SignalReceiver},
    types::{PipelineResult, Signal},
};
use alloc::{boxed::Box, sync::Arc, vec::Vec};
use alloy_primitives::Bytes;
use async_trait::async_trait;
use core::fmt::Debug;
use soon_miniz_oxide::inflate::decompress_to_vec_zlib;
use soon_primitives::{blocks::BlockInfo, rollup_config::SoonRollupConfig};
use tracing::{debug, error, info, warn};

/// ZLIB Deflate Compression Method.
pub(crate) const ZLIB_DEFLATE_COMPRESSION_METHOD: u8 = 8;

/// ZLIB Reserved Compression Info.
pub(crate) const ZLIB_RESERVED_COMPRESSION_METHOD: u8 = 15;

/// Brotili Compression Channel Version.
#[allow(dead_code)]
pub(crate) const CHANNEL_VERSION_BROTLI: u8 = 1;

/// The [ChannelReader] provider trait.
#[async_trait]
pub trait ChannelReaderProvider {
    /// Pulls the next piece of data from the channel bank. Note that it attempts to pull data out
    /// of the channel bank prior to loading data in (unlike most other stages). This is to
    /// ensure maintain consistency around channel bank pruning which depends upon the order
    /// of operations.
    async fn next_data(&mut self) -> PipelineResult<Option<Bytes>>;
}

/// [ChannelReader] is a stateful stage that reads [Batch]es from `Channel`s.
///
/// The [ChannelReader] pulls `Channel`s from the channel bank as raw data
/// and pipes it into a `BatchReader`. Since the raw data is compressed,
/// the `BatchReader` first decompresses the data using the first bytes as
/// a compression algorithm identifier.
///
/// Once the data is decompressed, it is decoded into a `Batch` and passed
/// to the next stage in the pipeline.
#[derive(Debug)]
pub struct ChannelReader<P>
where
    P: ChannelReaderProvider + OriginAdvancer + OriginProvider + SignalReceiver + Debug,
{
    /// The previous stage of the derivation pipeline.
    prev: P,
    /// The batch reader.
    next_batch: Option<BatchReader>,
    /// The rollup configuration.
    cfg: Arc<SoonRollupConfig>,
}

impl<P> ChannelReader<P>
where
    P: ChannelReaderProvider + OriginAdvancer + OriginProvider + SignalReceiver + Debug,
{
    /// Create a new [ChannelReader] stage.
    pub fn new(prev: P, cfg: Arc<SoonRollupConfig>) -> Self {
        Self { prev, next_batch: None, cfg }
    }

    /// Creates the batch reader from available channel data.
    async fn set_batch_reader(&mut self) -> PipelineResult<()> {
        if self.next_batch.is_none() {
            let channel =
                self.prev.next_data().await?.ok_or(PipelineError::ChannelReaderEmpty.temp())?;

            // let origin = self
            //     .prev
            //     .origin()
            //     .ok_or(PipelineError::MissingOrigin.crit())?;

            self.next_batch = Some(BatchReader::new(&channel[..], self.cfg.channel_size as usize));
        }
        Ok(())
    }

    /// Forces the read to continue with the next channel, resetting any
    /// decoding / decompression state to a fresh start.
    pub fn next_channel(&mut self) {
        self.next_batch = None;
    }
}

#[async_trait]
impl<P> OriginAdvancer for ChannelReader<P>
where
    P: ChannelReaderProvider + OriginAdvancer + OriginProvider + SignalReceiver + Send + Debug,
{
    async fn advance_origin(&mut self) -> PipelineResult<()> {
        self.prev.advance_origin().await
    }
}

#[async_trait]
impl<P> BatchStreamProvider for ChannelReader<P>
where
    P: ChannelReaderProvider + OriginAdvancer + OriginProvider + SignalReceiver + Send + Debug,
{
    /// This method is called by the BatchStream if an invalid span batch is found.
    /// In the case of an invalid span batch, the associated channel must be flushed.
    ///
    /// See: <https://specs.optimism.io/protocol/holocene/derivation.html#span-batches>
    ///
    /// SAFETY: Only called post-holocene activation.
    fn flush(&mut self) {
        debug!(target: "channel-reader", "[POST-HOLOCENE] Flushing channel");
        self.next_channel();
    }

    async fn next_batch(&mut self) -> PipelineResult<Batch> {
        if let Err(e) = self.set_batch_reader().await {
            debug!(target: "channel-reader", "Failed to set batch reader: {:?}", e);
            self.next_channel();
            return Err(e);
        }
        match self
            .next_batch
            .as_mut()
            .expect("Cannot be None")
            .next_batch(self.cfg.as_ref())
            .ok_or(PipelineError::NotEnoughData.temp())
        {
            Ok(batch) => Ok(batch),
            Err(e) => {
                self.next_channel();
                Err(e)
            }
        }
    }
}

impl<P> OriginProvider for ChannelReader<P>
where
    P: ChannelReaderProvider + OriginAdvancer + OriginProvider + SignalReceiver + Debug,
{
    fn origin(&self) -> Option<BlockInfo> {
        self.prev.origin()
    }
}

#[async_trait]
impl<P> SignalReceiver for ChannelReader<P>
where
    P: ChannelReaderProvider + OriginAdvancer + OriginProvider + SignalReceiver + Debug + Send,
{
    async fn signal(&mut self, signal: Signal) -> PipelineResult<()> {
        match signal {
            Signal::FlushChannel => {
                // Drop the current in-progress channel.
                warn!(target: "channel-reader", "Flushed channel");
                self.next_batch = None;
            }
            s => {
                self.prev.signal(s).await?;
                self.next_channel();
            }
        }
        Ok(())
    }
}

/// Batch Reader provides a function that iteratively consumes batches from the reader.
/// The L1Inclusion block is also provided at creation time.
/// Warning: the batch reader can read every batch-type.
/// The caller of the batch-reader should filter the results.
#[derive(Debug)]
pub(crate) struct BatchReader {
    /// The raw data to decode.
    data: Option<Vec<u8>>,
    /// The current cursor in the `decompressed` data.
    cursor: usize,
    /// The maximum RLP bytes per channel.
    max_rlp_bytes_per_channel: usize,
}

impl BatchReader {
    /// Creates a new [BatchReader] from the given data and max decompressed RLP bytes per channel.
    pub(crate) fn new<T>(data: T, max_rlp_bytes_per_channel: usize) -> Self
    where
        T: Into<Vec<u8>>,
    {
        Self { data: Some(data.into()), cursor: 0, max_rlp_bytes_per_channel }
    }

    /// Pulls out the next batch from the reader.
    pub(crate) fn next_batch(&mut self, cfg: &SoonRollupConfig) -> Option<Batch> {
        match self.data.clone() {
            Some(channel_data) => {
                let compressed_data = channel_data.as_slice()[self.cursor..].to_vec();
                // Peek at the data to determine the compression type.
                if compressed_data.is_empty() {
                    return None;
                }

                let compression_type = compressed_data[0];
                if (compression_type & 0x0F) == ZLIB_DEFLATE_COMPRESSION_METHOD ||
                    (compression_type & 0x0F) == ZLIB_RESERVED_COMPRESSION_METHOD
                {
                    let (decompressed, consumed) = decompress_to_vec_zlib(&compressed_data).ok()?;

                    // Check the size of the decompressed channel RLP.
                    if decompressed.len() > self.max_rlp_bytes_per_channel {
                        return None;
                    }

                    let decompressed_reader = &mut decompressed.as_slice();
                    let Ok(batch) = Batch::decode(decompressed_reader, cfg) else {
                        error!(target: "batch-reader", "Failed to decode batch, skipping batch");
                        return None;
                    };

                    // Print batch details
                    match &batch {
                        Batch::Single(single_batch) => {
                            info!(
                                target: "channel-reader",
                                "Decoded SingleBatch: parent_hash={}, epoch_num={}, epoch_hash={}, timestamp={}, tx_count={}",
                                single_batch.parent_hash,
                                single_batch.epoch_num,
                                single_batch.epoch_hash,
                                single_batch.timestamp,
                                single_batch.transactions.len()
                            );
                        }
                        Batch::Span(_) => {
                            unimplemented!("span batch is not supported yet")
                        }
                    }

                    // Advance the cursor on the reader.
                    self.cursor += consumed;
                    Some(batch)
                } else {
                    error!(target: "batch-reader", "Unsupported compression type: {:x}, skipping batch", compression_type);
                    None
                }
            }
            None => None,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        errors::PipelineErrorKind, test_utils::TestChannelReaderProvider, types::ResetSignal,
    };
    use alloc::vec;
    use soon_primitives::da::channel::{
        FJORD_MAX_RLP_BYTES_PER_CHANNEL, MAX_RLP_BYTES_PER_CHANNEL,
    };

    fn new_compressed_batch_data() -> Bytes {
        let file_contents =
            alloc::string::String::from_utf8_lossy(include_bytes!("../../../testdata/batch.hex"));
        let file_contents = &(&*file_contents)[..file_contents.len()];
        let data = alloy_primitives::hex::decode(file_contents).unwrap();
        data.into()
    }

    #[tokio::test]
    async fn test_flush_channel_reader() {
        let mock = TestChannelReaderProvider::new(vec![Ok(Some(new_compressed_batch_data()))]);
        let mut reader = ChannelReader::new(mock, Arc::new(SoonRollupConfig::default()));
        reader.next_batch = Some(BatchReader::new(
            new_compressed_batch_data(),
            FJORD_MAX_RLP_BYTES_PER_CHANNEL as usize,
        ));
        reader.signal(Signal::FlushChannel).await.unwrap();
        assert!(reader.next_batch.is_none());
    }

    #[tokio::test]
    async fn test_reset_channel_reader() {
        let mock = TestChannelReaderProvider::new(vec![Ok(None)]);
        let mut reader = ChannelReader::new(mock, Arc::new(SoonRollupConfig::default()));
        reader.next_batch = Some(BatchReader::new(
            vec![0x00, 0x01, 0x02],
            FJORD_MAX_RLP_BYTES_PER_CHANNEL as usize,
        ));
        assert!(!reader.prev.reset);
        reader.signal(ResetSignal::default().signal()).await.unwrap();
        assert!(reader.next_batch.is_none());
        assert!(reader.prev.reset);
    }

    #[tokio::test]
    async fn test_next_batch_batch_reader_set_fails() {
        let mock = TestChannelReaderProvider::new(vec![Err(PipelineError::Eof.temp())]);
        let mut reader = ChannelReader::new(mock, Arc::new(SoonRollupConfig::default()));
        assert_eq!(reader.next_batch().await, Err(PipelineError::Eof.temp()));
        assert!(reader.next_batch.is_none());
    }

    #[tokio::test]
    async fn test_next_batch_batch_reader_no_data() {
        let mock = TestChannelReaderProvider::new(vec![Ok(None)]);
        let mut reader = ChannelReader::new(mock, Arc::new(SoonRollupConfig::default()));
        assert!(matches!(
            reader.next_batch().await.unwrap_err(),
            PipelineErrorKind::Temporary(PipelineError::ChannelReaderEmpty)
        ));
        assert!(reader.next_batch.is_none());
    }

    #[tokio::test]
    async fn test_next_batch_batch_reader_not_enough_data() {
        let mut first = new_compressed_batch_data();
        let decompressed_len = decompress_to_vec_zlib(&first).unwrap().1;
        let second = first.split_to(decompressed_len / 2);
        let mock = TestChannelReaderProvider::new(vec![Ok(Some(first)), Ok(Some(second))]);
        let mut reader = ChannelReader::new(mock, Arc::new(SoonRollupConfig::default()));
        assert_eq!(reader.next_batch().await, Err(PipelineError::NotEnoughData.temp()));
        assert!(reader.next_batch.is_none());
    }

    #[tokio::test]
    async fn test_next_batch_succeeds() {
        let raw = new_compressed_batch_data();
        let mock = TestChannelReaderProvider::new(vec![Ok(Some(raw))]);
        let mut reader = ChannelReader::new(
            mock,
            Arc::new(SoonRollupConfig { channel_size: 10_000_000, ..Default::default() }),
        );
        let res = reader.next_batch().await.unwrap();
        matches!(res, Batch::Span(_));
        assert!(reader.next_batch.is_some());
    }

    #[test]
    fn test_batch_reader() {
        let raw = new_compressed_batch_data();
        let decompressed_len = decompress_to_vec_zlib(&raw).unwrap().1;
        let mut reader = BatchReader::new(raw, MAX_RLP_BYTES_PER_CHANNEL as usize);
        reader.next_batch(&SoonRollupConfig::default()).unwrap();
        assert_eq!(reader.cursor, decompressed_len);
    }

    #[test]
    fn test_batch_reader_fjord() {
        let raw = new_compressed_batch_data();
        let decompressed_len = decompress_to_vec_zlib(&raw).unwrap().1;
        let mut reader = BatchReader::new(raw, FJORD_MAX_RLP_BYTES_PER_CHANNEL as usize);
        reader.next_batch(&SoonRollupConfig { ..Default::default() }).unwrap();
        assert_eq!(reader.cursor, decompressed_len);
    }

    #[tokio::test]
    async fn test_flush_post_holocene() {
        let raw = new_compressed_batch_data();
        let config = Arc::new(SoonRollupConfig { channel_size: 10_000_000, ..Default::default() });
        let mock = TestChannelReaderProvider::new(vec![Ok(Some(raw))]);
        let mut reader = ChannelReader::new(mock, config);
        let res = reader.next_batch().await.unwrap();
        matches!(res, Batch::Span(_));
        assert!(reader.next_batch.is_some());
        reader.flush();
        assert!(reader.next_batch.is_none());
    }
}
