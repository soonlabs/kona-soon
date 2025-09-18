//! The [`AttributesBuilder`] and it's default implementation.

use crate::{
    errors::{BuilderError, PipelineEncodingError, PipelineError, PipelineErrorKind},
    traits::{AttributesBuilder, ChainProvider, L2ChainProvider},
    types::PipelineResult,
};
use alloc::{fmt::Debug, string::ToString, sync::Arc, vec, vec::Vec};
use alloy_consensus::Receipt;
use alloy_eips::BlockNumHash;
use alloy_primitives::{Address, B64, B256, Bytes, address};
use alloy_rpc_types_engine::PayloadAttributes;
use async_trait::async_trait;
use op_alloy_rpc_types_engine::OpPayloadAttributes;
use solana_sdk::transaction::SanitizedTransaction;
use soon_primitives::{
    blocks::L2BlockInfo,
    deposit::{AttributesDeposited, derive_deposits},
    derive::address_to_hash,
    l2blocks::L2Transaction,
    rollup_config::SoonRollupConfig,
    system::SystemConfig,
};

/// The sequencer fee vault address.
pub const SEQUENCER_FEE_VAULT_ADDRESS: Address =
    address!("4200000000000000000000000000000000000011");

/// A stateful implementation of the [AttributesBuilder].
#[derive(Debug, Default)]
pub struct StatefulAttributesBuilder<L1P, L2P>
where
    L1P: ChainProvider + Debug,
    L2P: L2ChainProvider + Debug,
{
    /// The rollup config.
    rollup_cfg: Arc<SoonRollupConfig>,
    /// The system config fetcher.
    config_fetcher: L2P,
    /// The L1 receipts fetcher.
    receipts_fetcher: L1P,
}

impl<L1P, L2P> StatefulAttributesBuilder<L1P, L2P>
where
    L1P: ChainProvider + Debug,
    L2P: L2ChainProvider + Debug,
{
    /// Create a new [StatefulAttributesBuilder] with the given epoch.
    pub const fn new(rcfg: Arc<SoonRollupConfig>, sys_cfg_fetcher: L2P, receipts: L1P) -> Self {
        Self { rollup_cfg: rcfg, config_fetcher: sys_cfg_fetcher, receipts_fetcher: receipts }
    }
}

#[async_trait]
impl<L1P, L2P> AttributesBuilder for StatefulAttributesBuilder<L1P, L2P>
where
    L1P: ChainProvider + Debug + Send,
    L2P: L2ChainProvider + Debug + Send,
{
    async fn prepare_payload_attributes(
        &mut self,
        l2_parent: L2BlockInfo,
        epoch: BlockNumHash,
    ) -> PipelineResult<OpPayloadAttributes> {
        let l1_origin;
        let deposit_transactions: Vec<Bytes>;

        let mut sys_config = self
            .config_fetcher
            .system_config_by_number(l2_parent.block_info.number)
            .await
            .map_err(|e| PipelineError::Provider(e.to_string()).temp())?;

        // If the L1 origin changed in this block, then we are in the first block of the epoch.
        // In this case we need to fetch all transaction receipts from the L1 origin block so
        // we can scan for user deposits.
        let sequence_number = if l2_parent.l1_origin.number != epoch.number {
            let epoch_info = self
                .receipts_fetcher
                .block_info_by_hash(epoch.hash)
                .await
                .map_err(|e| PipelineError::Provider(e.to_string()).temp())?;
            if l2_parent.l1_origin.hash != epoch_info.parent_hash {
                return Err(PipelineErrorKind::Reset(
                    BuilderError::BlockMismatchEpochReset(
                        epoch,
                        l2_parent.l1_origin,
                        epoch_info.parent_hash,
                    )
                    .into(),
                ));
            }

            let block_receipts = self
                .receipts_fetcher
                .receipts_by_hash(epoch.hash)
                .await
                .map_err(|e| PipelineError::Provider(e.to_string()).temp())?;
            let deposits = derive_sanitized_deposit_tx(
                epoch.number,
                epoch.hash,
                block_receipts.as_slice(),
                self.rollup_cfg.optimism_portal_address,
                self.rollup_cfg.l1_cross_domain_messenger,
                self.rollup_cfg.l1_standard_bridge,
            )
            .await
            .map_err(|e| PipelineError::BadEncoding(e).crit())?;

            sys_config
                .update_with_receipts(&block_receipts, self.rollup_cfg.l1_system_config_address)
                .map_err(|e| PipelineError::SystemConfigUpdate(e).crit())?;

            l1_origin = epoch_info;
            deposit_transactions = deposits;
            0
        } else {
            #[allow(clippy::collapsible_else_if)]
            if l2_parent.l1_origin.hash != epoch.hash {
                return Err(PipelineErrorKind::Reset(
                    BuilderError::BlockMismatch(epoch, l2_parent.l1_origin).into(),
                ));
            }

            let epoch_info = self
                .receipts_fetcher
                .block_info_by_hash(epoch.hash)
                .await
                .map_err(|e| PipelineError::Provider(e.to_string()).temp())?;
            l1_origin = epoch_info;
            deposit_transactions = vec![];
            l2_parent.seq_num + 1
        };

        // Sanity check the L1 origin was correctly selected to maintain the time invariant
        // between L1 and L2.
        let next_l2_time = l2_parent.block_info.timestamp + self.rollup_cfg.block_time;
        // if l2_parent.block_info.number != 0 && next_l2_time < l1_header.timestamp {
        //     return Err(PipelineErrorKind::Reset(
        //         BuilderError::BrokenTimeInvariant(
        //             l2_parent.l1_origin,
        //             next_l2_time,
        //             BlockNumHash {
        //                 hash: l1_header.hash_slow(),
        //                 number: l1_header.number,
        //             },
        //             l1_header.timestamp,
        //         )
        //         .into(),
        //     ));
        // }

        let upgrade_transactions: Vec<Bytes> = vec![];
        // if self.rollup_cfg.is_ecotone_active(next_l2_time)
        //     && !self
        //         .rollup_cfg
        //         .is_ecotone_active(l2_parent.block_info.timestamp)
        // {
        //     upgrade_transactions = Hardforks::ecotone_txs();
        // }
        // if self.rollup_cfg.is_fjord_active(next_l2_time)
        //     && !self
        //         .rollup_cfg
        //         .is_fjord_active(l2_parent.block_info.timestamp)
        // {
        //     upgrade_transactions.append(&mut Hardforks::fjord_txs());
        // }

        // Build and encode the L1 info transaction for the current payload.
        let l1_info_tx = AttributesDeposited::try_from(
            &l1_origin,
            sequence_number,
            address_to_hash(sys_config.batcher_address),
        )
        .map_err(|err| PipelineError::ParseL1BlockInfoTxErr(err.to_string()).temp())?;
        let encoded_l1_info_tx = l1_info_tx
            .to_sanitized_transaction(&Default::default())
            .map_err(|err| PipelineError::ParseL1BlockInfoTxErr(err.to_string()).temp())?;

        let mut txs =
            Vec::with_capacity(1 + deposit_transactions.len() + upgrade_transactions.len());
        txs.push(
            sanitized_derived_tx_to_bytes(encoded_l1_info_tx)
                .map_err(|e| PipelineError::BadEncoding(e).crit())?,
        );
        txs.extend(deposit_transactions);
        txs.extend(upgrade_transactions);

        let withdrawals = None;
        // if self.rollup_cfg.is_canyon_active(next_l2_time) {
        //     withdrawals = Some(Vec::default());
        // }

        let parent_beacon_root = None;
        // if self.rollup_cfg.is_ecotone_active(next_l2_time) {
        //     // if the parent beacon root is not available, default to zero hash
        //     parent_beacon_root = Some(l1_header.parent_beacon_block_root.unwrap_or_default());
        // }

        Ok(OpPayloadAttributes {
            payload_attributes: PayloadAttributes {
                timestamp: next_l2_time,
                prev_randao: B256::default(),
                suggested_fee_recipient: SEQUENCER_FEE_VAULT_ADDRESS,
                parent_beacon_block_root: parent_beacon_root,
                withdrawals,
            },
            transactions: Some(txs),
            no_tx_pool: Some(true),
            gas_limit: Some(u64::from_be_bytes(
                alloy_primitives::U64::from(sys_config.gas_limit).to_be_bytes(),
            )),
            eip_1559_params: eip_1559_params_from_system_config(
                &self.rollup_cfg,
                l2_parent.block_info.timestamp,
                0,
                &sys_config,
            ),
        })
    }
}

/// Derive deposits as `Vec<Bytes>` for transaction receipts.
///
/// Successful deposits must be emitted by the deposit contract and have the correct event
/// signature. So the receipt address must equal the specified deposit contract and the first topic
/// must be the [DEPOSIT_EVENT_ABI_HASH].
async fn derive_sanitized_deposit_tx(
    l1_block_num: u64,
    l1_block_hash: B256,
    receipts: &[Receipt],
    deposit_contract: Address,
    l1_cross_domain_messenger: Address,
    l1_standard_bridge: Address,
) -> Result<Vec<Bytes>, PipelineEncodingError> {
    let mut res = Vec::new();
    let deposit_txs = derive_deposits(
        l1_block_num,
        l1_block_hash,
        receipts,
        deposit_contract,
        l1_cross_domain_messenger,
        l1_standard_bridge,
    );
    for user_deposit in deposit_txs {
        let sanitized_tx = user_deposit.to_sanitized_transaction(&Default::default())?;
        res.push(sanitized_derived_tx_to_bytes(sanitized_tx)?);
    }
    Ok(res)
}

fn sanitized_derived_tx_to_bytes(
    sanitized_tx: SanitizedTransaction,
) -> Result<Bytes, PipelineEncodingError> {
    let version_tx = sanitized_tx.to_versioned_transaction();
    let tx = L2Transaction::new_from_derived_tx(version_tx);
    bincode::serialize(&tx)
        .map(Into::into)
        .map_err(|e| PipelineEncodingError::L2TxSerializeErr(e.to_string()))
}

/// Returns the eip1559 parameters from a [SystemConfig] encoded as a [B64].
fn eip_1559_params_from_system_config(
    _rollup_config: &SoonRollupConfig,
    _parent_timestamp: u64,
    _next_timestamp: u64,
    _sys_config: &SystemConfig,
) -> Option<B64> {
    Some(B64::ZERO)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        errors::ResetError,
        test_utils::{TestChainProvider, TestSystemConfigL2Fetcher},
    };
    use alloc::vec;
    use alloy_consensus::{Eip658Value, Receipt};
    use alloy_primitives::{B256, Log, LogData, U64, U256, keccak256};
    use soon_primitives::{
        blocks::{BlockInfo, L2BlockInfo},
        deposit::SOON_DEPOSIT_EVENT_ABI,
        system::SystemConfig,
    };

    #[allow(dead_code)]
    fn generate_valid_log() -> Log {
        let deposit_contract = address!("1111111111111111111111111111111111111111");
        let mut data = vec![0u8; 192];
        let offset: [u8; 8] = U64::from(32).to_be_bytes();
        data[24..32].copy_from_slice(&offset);
        let len: [u8; 8] = U64::from(128).to_be_bytes();
        data[56..64].copy_from_slice(&len);
        // Copy the u128 mint value
        let mint: [u8; 16] = 10_u128.to_be_bytes();
        data[80..96].copy_from_slice(&mint);
        // Copy the tx value
        let value: [u8; 32] = U256::from(100).to_be_bytes();
        data[96..128].copy_from_slice(&value);
        // Copy the gas limit
        let gas: [u8; 8] = 1000_u64.to_be_bytes();
        data[128..136].copy_from_slice(&gas);
        // Copy the isCreation flag
        data[136] = 1;
        let from = address!("2222222222222222222222222222222222222222");
        let mut from_bytes = vec![0u8; 32];
        from_bytes[12..32].copy_from_slice(from.as_slice());
        let to = address!("3333333333333333333333333333333333333333");
        let mut to_bytes = vec![0u8; 32];
        to_bytes[12..32].copy_from_slice(to.as_slice());
        Log {
            address: deposit_contract,
            data: LogData::new_unchecked(
                vec![
                    keccak256(SOON_DEPOSIT_EVENT_ABI.as_bytes()),
                    B256::from_slice(&from_bytes),
                    B256::from_slice(&to_bytes),
                    B256::default(),
                ],
                Bytes::from(data),
            ),
        }
    }

    #[allow(dead_code)]
    fn generate_valid_receipt() -> Receipt {
        let mut bad_dest_log = generate_valid_log();
        bad_dest_log.data.topics_mut()[1] = B256::default();
        let mut invalid_topic_log = generate_valid_log();
        invalid_topic_log.data.topics_mut()[0] = B256::default();
        Receipt {
            status: Eip658Value::Eip658(true),
            logs: vec![generate_valid_log(), bad_dest_log, invalid_topic_log],
            ..Default::default()
        }
    }

    #[test]
    fn test_default_eip_1559_params_from_system_config() {
        let rollup_config = SoonRollupConfig::default();
        let sys_config = SystemConfig {
            eip1559_denominator: None,
            eip1559_elasticity: None,
            ..Default::default()
        };
        let expected = Some(B64::ZERO);
        assert_eq!(eip_1559_params_from_system_config(&rollup_config, 0, 0, &sys_config), expected);
    }

    #[test]
    fn test_default_eip_1559_params_first_block_holocene() {
        let rollup_config = SoonRollupConfig::default();
        let sys_config = SystemConfig {
            eip1559_denominator: Some(1),
            eip1559_elasticity: Some(2),
            ..Default::default()
        };
        assert_eq!(
            eip_1559_params_from_system_config(&rollup_config, 0, 2, &sys_config),
            Some(B64::ZERO)
        );
    }

    #[tokio::test]
    async fn test_prepare_payload_block_mismatch_epoch_reset() {
        let cfg = Arc::new(SoonRollupConfig::default());
        let l2_number = 1;
        let mut fetcher = TestSystemConfigL2Fetcher::default();
        fetcher.insert(l2_number, SystemConfig::default());
        let mut provider = TestChainProvider::default();
        let header = alloy_consensus::Header::default();
        let hash = header.hash_slow();
        provider.insert_header(hash, header);
        let mut builder = StatefulAttributesBuilder::new(cfg, fetcher, provider);
        let epoch = BlockNumHash { hash, number: l2_number };
        let l2_parent = L2BlockInfo {
            block_info: BlockInfo { hash: B256::ZERO, number: l2_number, ..Default::default() },
            l1_origin: BlockNumHash { hash: B256::left_padding_from(&[0xFF]), number: 2 },
            seq_num: 0,
        };
        // This should error because the l2 parent's l1_origin.hash should equal the epoch header
        // hash. Here we use the default header whose hash will not equal the custom `l2_hash`.
        let expected =
            BuilderError::BlockMismatchEpochReset(epoch, l2_parent.l1_origin, B256::default());
        let err = builder.prepare_payload_attributes(l2_parent, epoch).await.unwrap_err();
        assert_eq!(err, PipelineErrorKind::Reset(expected.into()));
    }

    #[tokio::test]
    async fn test_prepare_payload_block_mismatch() {
        let cfg = Arc::new(SoonRollupConfig::default());
        let l2_number = 1;
        let mut fetcher = TestSystemConfigL2Fetcher::default();
        fetcher.insert(l2_number, SystemConfig::default());
        let mut provider = TestChainProvider::default();
        let header = alloy_consensus::Header::default();
        let hash = header.hash_slow();
        provider.insert_header(hash, header);
        let mut builder = StatefulAttributesBuilder::new(cfg, fetcher, provider);
        let epoch = BlockNumHash { hash, number: l2_number };
        let l2_parent = L2BlockInfo {
            block_info: BlockInfo { hash: B256::ZERO, number: l2_number, ..Default::default() },
            l1_origin: BlockNumHash { hash: B256::ZERO, number: l2_number },
            seq_num: 0,
        };
        // This should error because the l2 parent's l1_origin.hash should equal the epoch hash
        // Here the default header is used whose hash will not equal the custom `l2_hash` above.
        let expected = BuilderError::BlockMismatch(epoch, l2_parent.l1_origin);
        let err = builder.prepare_payload_attributes(l2_parent, epoch).await.unwrap_err();
        assert_eq!(err, PipelineErrorKind::Reset(ResetError::AttributesBuilder(expected)));
    }

    #[tokio::test]
    #[ignore] //remove block timestamp check
    async fn test_prepare_payload_broken_time_invariant() {
        let block_time = 10;
        let timestamp = 100;
        let cfg = Arc::new(SoonRollupConfig { block_time, ..Default::default() });
        let l2_number = 1;
        let mut fetcher = TestSystemConfigL2Fetcher::default();
        fetcher.insert(l2_number, SystemConfig::default());
        let mut provider = TestChainProvider::default();
        let header = alloy_consensus::Header { timestamp, ..Default::default() };
        let hash = header.hash_slow();
        provider.insert_header(hash, header);
        let mut builder = StatefulAttributesBuilder::new(cfg, fetcher, provider);
        let epoch = BlockNumHash { hash, number: l2_number };
        let l2_parent = L2BlockInfo {
            block_info: BlockInfo { hash: B256::ZERO, number: l2_number, ..Default::default() },
            l1_origin: BlockNumHash { hash, number: l2_number },
            seq_num: 0,
        };
        let next_l2_time = l2_parent.block_info.timestamp + block_time;
        let block_id = BlockNumHash { hash, number: 0 };
        let expected = BuilderError::BrokenTimeInvariant(
            l2_parent.l1_origin,
            next_l2_time,
            block_id,
            timestamp,
        );
        let err = builder.prepare_payload_attributes(l2_parent, epoch).await.unwrap_err();
        assert_eq!(err, PipelineErrorKind::Reset(ResetError::AttributesBuilder(expected)));
    }

    #[tokio::test]
    async fn test_prepare_payload_without_forks() {
        let block_time = 10;
        let timestamp = 100;
        let cfg = Arc::new(SoonRollupConfig { block_time, ..Default::default() });
        let l2_number = 1;
        let mut fetcher = TestSystemConfigL2Fetcher::default();
        fetcher.insert(l2_number, SystemConfig::default());
        let mut provider = TestChainProvider::default();
        let header = alloy_consensus::Header { timestamp, ..Default::default() };
        let prev_randao = header.mix_hash;
        let hash = header.hash_slow();
        provider.insert_header(hash, header);
        let mut builder = StatefulAttributesBuilder::new(cfg, fetcher, provider);
        let epoch = BlockNumHash { hash, number: l2_number };
        let l2_parent = L2BlockInfo {
            block_info: BlockInfo {
                hash: B256::ZERO,
                number: l2_number,
                timestamp,
                parent_hash: hash,
            },
            l1_origin: BlockNumHash { hash, number: l2_number },
            seq_num: 0,
        };
        let next_l2_time = l2_parent.block_info.timestamp + block_time;
        let payload = builder.prepare_payload_attributes(l2_parent, epoch).await.unwrap();
        let expected = OpPayloadAttributes {
            payload_attributes: PayloadAttributes {
                timestamp: next_l2_time,
                prev_randao,
                suggested_fee_recipient: SEQUENCER_FEE_VAULT_ADDRESS,
                parent_beacon_block_root: None,
                withdrawals: None,
            },
            transactions: payload.transactions.clone(),
            no_tx_pool: Some(true),
            gas_limit: Some(u64::from_be_bytes(
                alloy_primitives::U64::from(SystemConfig::default().gas_limit).to_be_bytes(),
            )),
            eip_1559_params: Some(B64::ZERO),
        };
        assert_eq!(payload, expected);
        assert_eq!(payload.transactions.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_prepare_payload_with_canyon() {
        let block_time = 0;
        let timestamp = 100;
        let cfg = Arc::new(SoonRollupConfig { block_time, ..Default::default() });
        let l2_number = 1;
        let mut fetcher = TestSystemConfigL2Fetcher::default();
        fetcher.insert(l2_number, SystemConfig::default());
        let mut provider = TestChainProvider::default();
        let header = alloy_consensus::Header { timestamp, ..Default::default() };
        let prev_randao = header.mix_hash;
        let hash = header.hash_slow();
        provider.insert_header(hash, header);
        let mut builder = StatefulAttributesBuilder::new(cfg, fetcher, provider);
        let epoch = BlockNumHash { hash, number: l2_number };
        let l2_parent = L2BlockInfo {
            block_info: BlockInfo {
                hash: B256::ZERO,
                number: l2_number,
                timestamp,
                parent_hash: hash,
            },
            l1_origin: BlockNumHash { hash, number: l2_number },
            seq_num: 0,
        };
        let next_l2_time = l2_parent.block_info.timestamp + block_time;
        let payload = builder.prepare_payload_attributes(l2_parent, epoch).await.unwrap();
        let expected = OpPayloadAttributes {
            payload_attributes: PayloadAttributes {
                timestamp: next_l2_time,
                prev_randao,
                suggested_fee_recipient: SEQUENCER_FEE_VAULT_ADDRESS,
                parent_beacon_block_root: None,
                withdrawals: None,
            },
            transactions: payload.transactions.clone(),
            no_tx_pool: Some(true),
            gas_limit: Some(u64::from_be_bytes(
                alloy_primitives::U64::from(SystemConfig::default().gas_limit).to_be_bytes(),
            )),
            eip_1559_params: Some(B64::ZERO),
        };
        assert_eq!(payload, expected);
        assert_eq!(payload.transactions.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_prepare_payload_with_ecotone() {
        let block_time = 2;
        let timestamp = 100;
        let cfg = Arc::new(SoonRollupConfig { block_time, ..Default::default() });
        let l2_number = 1;
        let mut fetcher = TestSystemConfigL2Fetcher::default();
        fetcher.insert(l2_number, SystemConfig::default());
        let mut provider = TestChainProvider::default();
        let header = alloy_consensus::Header { timestamp, ..Default::default() };
        let prev_randao = header.mix_hash;
        let hash = header.hash_slow();
        provider.insert_header(hash, header);
        let mut builder = StatefulAttributesBuilder::new(cfg, fetcher, provider);
        let epoch = BlockNumHash { hash, number: l2_number };
        let l2_parent = L2BlockInfo {
            block_info: BlockInfo {
                hash: B256::ZERO,
                number: l2_number,
                timestamp,
                parent_hash: hash,
            },
            l1_origin: BlockNumHash { hash, number: l2_number },
            seq_num: 0,
        };
        let next_l2_time = l2_parent.block_info.timestamp + block_time;
        let payload = builder.prepare_payload_attributes(l2_parent, epoch).await.unwrap();
        let expected = OpPayloadAttributes {
            payload_attributes: PayloadAttributes {
                timestamp: next_l2_time,
                prev_randao,
                suggested_fee_recipient: SEQUENCER_FEE_VAULT_ADDRESS,
                parent_beacon_block_root: None,
                withdrawals: None,
            },
            transactions: payload.transactions.clone(),
            no_tx_pool: Some(true),
            gas_limit: Some(u64::from_be_bytes(
                alloy_primitives::U64::from(SystemConfig::default().gas_limit).to_be_bytes(),
            )),
            eip_1559_params: Some(B64::ZERO),
        };
        assert_eq!(payload, expected);
        assert_eq!(payload.transactions.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_prepare_payload_with_fjord() {
        let block_time = 0;
        let timestamp = 100;
        let cfg = Arc::new(SoonRollupConfig { block_time, ..Default::default() });
        let l2_number = 1;
        let mut fetcher = TestSystemConfigL2Fetcher::default();
        fetcher.insert(l2_number, SystemConfig::default());
        let mut provider = TestChainProvider::default();
        let header = alloy_consensus::Header { timestamp, ..Default::default() };
        let prev_randao = header.mix_hash;
        let hash = header.hash_slow();
        provider.insert_header(hash, header);
        let mut builder = StatefulAttributesBuilder::new(cfg, fetcher, provider);
        let epoch = BlockNumHash { hash, number: l2_number };
        let l2_parent = L2BlockInfo {
            block_info: BlockInfo {
                hash: B256::ZERO,
                number: l2_number,
                timestamp,
                parent_hash: hash,
            },
            l1_origin: BlockNumHash { hash, number: l2_number },
            seq_num: 0,
        };
        let next_l2_time = l2_parent.block_info.timestamp + block_time;
        let payload = builder.prepare_payload_attributes(l2_parent, epoch).await.unwrap();
        let expected = OpPayloadAttributes {
            payload_attributes: PayloadAttributes {
                timestamp: next_l2_time,
                prev_randao,
                suggested_fee_recipient: SEQUENCER_FEE_VAULT_ADDRESS,
                parent_beacon_block_root: None,
                withdrawals: None,
            },
            transactions: payload.transactions.clone(),
            no_tx_pool: Some(true),
            gas_limit: Some(u64::from_be_bytes(
                alloy_primitives::U64::from(SystemConfig::default().gas_limit).to_be_bytes(),
            )),
            eip_1559_params: Some(B64::ZERO),
        };
        assert_eq!(payload.transactions.as_ref().unwrap().len(), 1);
        assert_eq!(payload, expected);
    }
}
