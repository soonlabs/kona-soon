use crate::error::AlloyChainProviderError;
use crate::l1_provider::{create_l1_provider, L1Client};
use alloy_primitives::B256;
use alloy::transports::http::reqwest::Url;
use alloy_consensus::Receipt;
use alloy_eips::BlockNumberOrTag;
use async_trait::async_trait;
use chrono::Utc;
use lru::LruCache;
use soon_derive::traits::ChainProvider;
use soon_primitives::blocks::{BlockInfo, L1Header, L1Transaction};
use std::cmp::min;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};
use tracing::{error, info, warn};

const CACHE_SIZE: usize = 2048;
const BACKGROUND_TASK_STEP_INTERVAL: Duration = Duration::from_secs(1);
const DERIVE_HEAD_BLOCK_UPDATE_INTERVAL: i64 = 5;
const DERIVE_FINALIZE_BLOCK_UPDATE_INTERVAL: i64 = 100;

#[derive(Debug, Clone)]
pub struct L1ChainProvider {
    derive_delay_l1_block_num: u64,
    /// L1 Chain Id
    l1_chain_id: u64,
    /// L1 client rpc url
    l1_url: Url,
    /// The inner Ethereum JSON-RPC provider.
    inner: Arc<RwLock<Box<dyn L1Client>>>,
    /// Next fetch block receipt height
    next_fetch_block_receipt_height: Arc<AtomicU64>,
    /// `header_by_hash` LRU cache.
    header_by_hash_cache: Arc<Mutex<LruCache<B256, L1Header>>>,
    /// `block_info_by_hash` LRU cache.
    receipts_by_hash_cache: Arc<Mutex<LruCache<B256, Vec<Receipt>>>>,
    /// `block_info_by_number` LRU cache.
    /// NOTE: only cache finalized block info
    block_info_by_number_cache: Arc<Mutex<LruCache<u64, BlockInfo>>>,
    /// `block_info_and_transactions_by_hash` LRU cache.
    latest_finalized_block_info: Arc<Mutex<Option<BlockInfo>>>,
    latest_safe_block_info: Arc<Mutex<Option<BlockInfo>>>,
    latest_head_block_info: Arc<Mutex<Option<BlockInfo>>>,
    is_background_task_running: Arc<AtomicBool>,
}

impl L1ChainProvider {
    /// Creates a new [AlloyChainProvider] with the given alloy provider.
    pub fn new(chain_id: u64, rpc_url: &str) -> Self {
        let l1_url: Url = rpc_url
            .parse()
            .unwrap_or_else(|_| panic!("invalid rpc url for l1:{}", rpc_url));
        let provider = create_l1_provider(chain_id, l1_url.clone());
        Self {
            derive_delay_l1_block_num: 0,
            l1_chain_id: chain_id,
            l1_url,
            inner: Arc::new(RwLock::new(provider)),
            next_fetch_block_receipt_height: Arc::new(AtomicU64::new(0)),
            header_by_hash_cache: Arc::new(Mutex::new(LruCache::new(
                NonZeroUsize::new(CACHE_SIZE).unwrap(),
            ))), // only used in DA
            receipts_by_hash_cache: Arc::new(Mutex::new(LruCache::new(
                NonZeroUsize::new(CACHE_SIZE).unwrap(),
            ))),
            block_info_by_number_cache: Arc::new(Mutex::new(LruCache::new(
                NonZeroUsize::new(CACHE_SIZE).unwrap(),
            ))),
            latest_finalized_block_info: Arc::new(Mutex::new(None)),
            latest_safe_block_info: Arc::new(Mutex::new(None)),
            latest_head_block_info: Arc::new(Mutex::new(None)),
            is_background_task_running: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn is_background_task_running(&self) -> bool {
        self.is_background_task_running.load(Ordering::Relaxed)
    }

    pub fn start_background_task(
        mut self,
        next_fetch_block_receipt_height: u64,
        derive_delay_l1_block_num: u64,
        exit: Arc<AtomicBool>,
    ) {
        if self.is_background_task_running() {
            return;
        }

        self.derive_delay_l1_block_num = derive_delay_l1_block_num;
        std::thread::Builder::new()
            .name("l1-provider-background-task".to_string())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();

                let mut last_head_update_timestamp = 0;
                let mut last_finalize_update_timestamp = 0;
                let mut next_fetch_height = next_fetch_block_receipt_height;
                let max_concurrency_level = self.get_concurrency_level();
                loop {
                    let now = Utc::now().timestamp();
                    let step_result: Result<bool, AlloyChainProviderError> =
                        runtime.block_on(async {
                            let latest_number = self.latest_block_number().await?;

                            if now - last_head_update_timestamp > DERIVE_HEAD_BLOCK_UPDATE_INTERVAL
                            {
                                let head = self
                                    .block_info_by_number(BlockNumberOrTag::Number(
                                        latest_number
                                            .saturating_sub(self.derive_delay_l1_block_num),
                                    ))
                                    .await?;
                                let mut head_block_gurad = self.latest_head_block_info.lock().await;
                                *head_block_gurad = Some(head);
                                drop(head_block_gurad);
                                last_head_update_timestamp = now;
                            }

                            if now - last_finalize_update_timestamp
                                > DERIVE_FINALIZE_BLOCK_UPDATE_INTERVAL
                            {
                                let (safe, finalized) = tokio::try_join!(
                                    self.block_info_by_number(BlockNumberOrTag::Safe),
                                    self.block_info_by_number(BlockNumberOrTag::Finalized),
                                )?;

                                *self.latest_safe_block_info.lock().await = Some(safe);
                                *self.latest_finalized_block_info.lock().await = Some(finalized);
                                last_finalize_update_timestamp = now;
                            }

                            let instant = Instant::now();
                            let concurrency: u64 = min(
                                max_concurrency_level as u64,
                                latest_number.saturating_sub(next_fetch_height),
                            );
                            if concurrency == 0 {
                                return Ok(false);
                            }
                            let tasks = (0..concurrency)
                                .map(|i| self.pre_fetch_receipts(next_fetch_height + i))
                                .collect::<Vec<_>>();
                            futures::future::try_join_all(tasks).await?;

                            let fetch_duration = instant.elapsed().as_millis();
                            if fetch_duration > 2000 {
                                warn!("heavy step,  duration:{}", fetch_duration);
                            }

                            next_fetch_height += concurrency;

                            self.next_fetch_block_receipt_height
                                .store(next_fetch_height, Ordering::Relaxed);

                            let in_catchup = next_fetch_height
                                < latest_number.saturating_sub(self.derive_delay_l1_block_num);

                            Ok(in_catchup)
                        });

                    let idle_duration = match step_result {
                        Err(err) => {
                            error!(
                                "l1-provider-background-task stepped err:{}",
                                err.to_string()
                            );
                            BACKGROUND_TASK_STEP_INTERVAL
                        }
                        Ok(in_catchup) => {
                            if in_catchup {
                                Duration::from_millis(300)
                            } else {
                                BACKGROUND_TASK_STEP_INTERVAL
                            }
                        }
                    };

                    if exit.load(Ordering::Relaxed) {
                        info!("l1-provider-background-task exited");
                        break;
                    }

                    std::thread::sleep(idle_duration);
                }
            })
            .unwrap();
    }

    pub fn next_fetch_block_receipt_height(&self) -> u64 {
        self.next_fetch_block_receipt_height.load(Ordering::Relaxed)
    }

    pub async fn reset_provider(&self) {
        let provider = create_l1_provider(self.l1_chain_id, self.l1_url.clone());
        *self.inner.write().await = provider;
    }

    /// Returns latest block number.
    pub async fn latest_block_number(&self) -> Result<u64, AlloyChainProviderError> {
        self.inner.read().await.get_block_number().await
    }

    /// Returns latest finalized block info.
    pub async fn get_latest_finalized_block_info(
        &self,
    ) -> Result<Option<BlockInfo>, AlloyChainProviderError> {
        Ok(*self.latest_finalized_block_info.lock().await)
    }

    /// Returns latest safe block info.
    pub async fn get_latest_safe_block_info(
        &self,
    ) -> Result<Option<BlockInfo>, AlloyChainProviderError> {
        Ok(*self.latest_safe_block_info.lock().await)
    }

    /// Returns latest head block info.
    pub async fn get_latest_head_block_info(&self) -> Result<BlockInfo, AlloyChainProviderError> {
        Ok(self.latest_head_block_info.lock().await.unwrap_or_default())
    }

    async fn pre_fetch_receipts(&self, number: u64) -> Result<(), AlloyChainProviderError> {
        let block_info = self
            .block_info_by_number(BlockNumberOrTag::Number(number))
            .await?;
        self.receipts_by_hash(block_info.hash).await?;
        Ok(())
    }

    fn get_concurrency_level(&self) -> u8 {
        if self.l1_chain_id == 1 || self.l1_chain_id == 11155111 {
            3
        } else {
            6
        }
    }
}

#[async_trait]
impl ChainProvider for L1ChainProvider {
    type Error = AlloyChainProviderError;

    async fn header_by_hash(&self, hash: B256) -> Result<L1Header, Self::Error> {
        let mut cache_guard = self.header_by_hash_cache.lock().await;
        if let Some(header) = cache_guard.get(&hash) {
            return Ok(header.clone());
        }
        drop(cache_guard);

        let provider_guard = self.inner.read().await;
        let header = provider_guard.get_header_by_hash(hash).await?;
        drop(provider_guard);

        self.header_by_hash_cache.lock().await.put(hash, header);

        Ok(header)
    }

    async fn block_info_by_hash(&self, hash: B256) -> Result<BlockInfo, Self::Error> {
        Ok(self.header_by_hash(hash).await?.into())
    }

    async fn block_info_by_number(
        &self,
        number: BlockNumberOrTag,
    ) -> Result<BlockInfo, Self::Error> {
        if let BlockNumberOrTag::Number(n) = number {
            let mut cache_guard = self.block_info_by_number_cache.lock().await;
            if let Some(block_info) = cache_guard.get(&n) {
                return Ok(*block_info);
            }
        }

        let provider_guard = self.inner.read().await;
        let header = provider_guard.get_header_by_number(number).await?;
        drop(provider_guard);

        // only cache the block info if it is finalized
        let finalized_block_info = self
            .latest_finalized_block_info
            .lock()
            .await
            .unwrap_or_default();
        if finalized_block_info.number >= header.number {
            self.block_info_by_number_cache
                .lock()
                .await
                .put(header.number, header.into());
        }
        self.header_by_hash_cache
            .lock()
            .await
            .put(header.hash, header);
        Ok(header.into())
    }

    async fn receipts_by_hash(&self, hash: B256) -> Result<Vec<Receipt>, Self::Error> {
        let mut cache_guard = self.receipts_by_hash_cache.lock().await;
        if let Some(receipts) = cache_guard.get(&hash) {
            return Ok(receipts.to_vec());
        } else {
            drop(cache_guard);
            let res_receipts = self
                .inner
                .read()
                .await
                .get_block_receipts_by_hash(hash)
                .await?;
            self.receipts_by_hash_cache
                .lock()
                .await
                .put(hash, res_receipts.clone());
            Ok(res_receipts)
        }
    }

    async fn get_block_transactions_by_hash(
        &self,
        hash: B256,
    ) -> Result<Vec<L1Transaction>, Self::Error> {
        let (_, txs) = self.inner.read().await.get_full_block_by_hash(hash).await?;
        Ok(txs)
    }
}
