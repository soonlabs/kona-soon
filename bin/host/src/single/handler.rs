//! [HintHandler] for the [SingleChainHost].

use crate::{
    HintHandler, OnlineHostBackendCfg, backend::util::store_ordered_trie, kv::SharedKeyValueStore,
    single::cfg::SingleChainHost,
};
use alloy_eips::eip2718::Encodable2718;
use alloy_primitives::{Address, B256, Bytes, keccak256};
use alloy_provider::Provider;
use alloy_rlp::{BytesMut, Encodable};
use alloy_rpc_types::Block;
use anyhow::{Result, anyhow, ensure};
use async_trait::async_trait;
use bs58;
use kona_preimage::{PreimageKey, PreimageKeyType};
use kona_proof::{Hint, HintType};
use soon_primitives::output_root::OutputRoot;
use tracing::info;

/// The [HintHandler] for the [SingleChainHost].
#[derive(Debug, Clone, Copy)]
pub struct SingleChainHintHandler;

#[async_trait]
impl HintHandler for SingleChainHintHandler {
    type Cfg = SingleChainHost;

    async fn fetch_hint(
        hint: Hint<<Self::Cfg as OnlineHostBackendCfg>::HintType>,
        cfg: &Self::Cfg,
        providers: &<Self::Cfg as OnlineHostBackendCfg>::Providers,
        kv: SharedKeyValueStore,
    ) -> Result<()> {
        match hint.ty {
            HintType::L1BlockHeader => {
                ensure!(hint.data.len() == 32, "Invalid hint data length for l1 block header");

                let hash: B256 = hint.data.as_ref().try_into()?;
                let raw_header: Bytes =
                    providers.l1.client().request("debug_getRawHeader", [hash]).await?;

                let mut kv_lock = kv.write().await;
                kv_lock.set(PreimageKey::new_keccak256(*hash).into(), raw_header.into())?;
            }
            HintType::L1Transactions => {
                ensure!(hint.data.len() == 32, "Invalid hint data length for l1 transaction");

                let hash: B256 = hint.data.as_ref().try_into()?;
                let Block { transactions, .. } = providers
                    .l1
                    .get_block_by_hash(hash)
                    .full()
                    .await?
                    .ok_or(anyhow!("Block not found"))?;
                let encoded_transactions = transactions
                    .into_transactions()
                    .map(|tx| tx.inner.encoded_2718())
                    .collect::<Vec<_>>();

                store_ordered_trie(kv.as_ref(), encoded_transactions.as_slice()).await?;
            }
            HintType::L1Receipts => {
                ensure!(hint.data.len() == 32, "Invalid hint data length for l1 receipts");

                let hash: B256 = hint.data.as_ref().try_into()?;
                let raw_receipts: Vec<Bytes> =
                    providers.l1.client().request("debug_getRawReceipts", [hash]).await?;

                store_ordered_trie(kv.as_ref(), raw_receipts.as_slice()).await?;
            }
            HintType::L1Blob => {}
            HintType::DAProxyBlob => {
                // ensure!(hint.data.len() == 513, "Invalid hint data length");
                let data = providers.da.download_preimage(hint.data.as_ref().to_vec()).await?;
                let mut kv_lock = kv.write().await;
                kv_lock
                    .set(PreimageKey::new_da_proxy_blob(hint.data.as_ref()).into(), data.into())?;
            }
            HintType::L1Precompile => {
                ensure!(hint.data.len() >= 28, "Invalid hint data length for l1 precompile");

                let address = Address::from_slice(&hint.data.as_ref()[..20]);
                let gas = u64::from_be_bytes(hint.data.as_ref()[20..28].try_into()?);
                let input = hint.data[28..].to_vec();
                let input_hash = keccak256(hint.data.as_ref());

                let result = crate::eth::execute(address, input, gas).map_or_else(
                    |_| vec![0u8; 1],
                    |raw_res| {
                        let mut res = Vec::with_capacity(1 + raw_res.len());
                        res.push(0x01);
                        res.extend_from_slice(&raw_res);
                        res
                    },
                );

                let mut kv_lock = kv.write().await;
                kv_lock.set(PreimageKey::new_keccak256(*input_hash).into(), hint.data.into())?;
                kv_lock.set(
                    PreimageKey::new(*input_hash, PreimageKeyType::Precompile).into(),
                    result,
                )?;
            }
            HintType::StartingL2Output => {
                ensure!(hint.data.len() == 32, "Invalid hint data length for starting l2 output");

                let output_res: OutputRoot =
                    providers.l2.output_at_block(cfg.agreed_l2_block_number).await?;
                info!("output_res:{}", output_res);
                let output_root_hash = output_res.hash();

                info!("cfg.agreed_l2_block_number:{}", cfg.agreed_l2_block_number);
                info!("output_root_hash:{}", output_root_hash);
                info!("cfg.agreed_l2_output_root:{}", cfg.agreed_l2_output_root);

                ensure!(
                    output_root_hash == cfg.agreed_l2_output_root,
                    "Output root does not match L2 head."
                );

                let mut kv_write_lock = kv.write().await;
                kv_write_lock.set(
                    PreimageKey::new_keccak256(*output_root_hash).into(),
                    output_res.encode().into(),
                )?;
            }
            HintType::L2StateNode => {
                // all trie node is loaded into oracel with account proof hint.
                // we can just skip hint this type.
                return Ok(());
                // ensure!(hint.data.len() == 32, "Invalid hint data length");
                // let hash: B256 = hint.data.as_ref().try_into()?;
                // warn!(target: "single_hint_handler", "L2StateNode hint was sent for node hash: {}", hash);
                // warn!(
                //     target: "single_hint_handler",
                //     "`debug_executePayload` failed to return a complete witness."
                // );
                // let kv_read_lock = kv.read().await?;
                // let preimage =
                // // Fetch the preimage from the L2 chain provider.
                // let preimage = providers.l2.get_trie_node(hash).await?;
                // let mut kv_write_lock = kv.write().await;
                // kv_write_lock.set(PreimageKey::new_keccak256(*hash).into(), preimage)?;
            }
            HintType::L2AccountProof => {
                // block number + hashed address<b256>
                ensure!(hint.data.len() == 8 + 32, "Invalid hint req for L2AccountProof");
                let block_number = u64::from_be_bytes(hint.data.as_ref()[..8].try_into()?);
                let hashed_address = B256::from_slice(&hint.data.as_ref()[8..40]);

                let tried_account =
                    providers.l2.get_tried_account_proof(hashed_address, block_number).await?;
                // need to write account + trie proof node into kv.
                let mut out_buf = BytesMut::default();
                if let Some(account) = tried_account.account {
                    Encodable::encode(&account, &mut out_buf);
                }
                let mut kv_lock = kv.write().await;
                kv_lock.set(
                    PreimageKey::new_l2_account_proof(hashed_address.into()).into(),
                    out_buf.into(),
                )?;
                tried_account.proofs.into_iter().try_for_each(|node| {
                    let node_hash = keccak256::<&[u8]>(node.as_ref());
                    let key = PreimageKey::new_keccak256(*node_hash);
                    kv_lock.set(key.into(), node.into())?;
                    Ok::<(), anyhow::Error>(())
                })?;
                tried_account.withdrawal_proofs.into_iter().try_for_each(|node| {
                    let node_hash = keccak256::<&[u8]>(node.as_ref());
                    let key = PreimageKey::new_keccak256(*node_hash);
                    kv_lock.set(key.into(), node.into())?;
                    Ok::<(), anyhow::Error>(())
                })?;
            }
            // HintType::L2AccountStorageProof => {
            //     ensure!(hint.data.len() == 8 + 32, "Invalid hint data length");

            //     let block_number = u64::from_be_bytes(hint.data.as_ref()[..8].try_into()?);
            //     let account = B256::from_slice(&hint.data.as_ref()[8..40]);

            //     let proof_response =
            //         providers.l2.get_storage_node_proof(account, block_number).await?;

            //     // Write the account proof nodes to the key-value store.
            //     let mut kv_lock = kv.write().await;
            //     proof_response.into_iter().try_for_each(|node| {
            //         let node_hash = keccak256::<&[u8]>(node.as_ref());
            //         let key = PreimageKey::new_keccak256(*node_hash);
            //         kv_lock.set(key.into(), node.into())?;
            //         Ok::<(), anyhow::Error>(())
            //     })?;
            // }
            HintType::L2BlockData => {
                ensure!(hint.data.len() == 8, "Invalid hint data length for l2 block data");

                let block_number = u64::from_be_bytes(hint.data.as_ref()[..8].try_into()?);

                let block = providers.l2.get_block_by_number(block_number).await?;
                let mut out_buf = BytesMut::default();
                Encodable::encode(&block, &mut out_buf);
                let mut kv_lock = kv.write().await;
                kv_lock.set(PreimageKey::new_block_slot(block_number).into(), out_buf.into())?;
            }
            HintType::L2BankHash => {
                ensure!(hint.data.len() == 8, "Invalid hint data length for l2 bank hash");
                info!("handle L2BankHash request.");
                let block_number = u64::from_be_bytes(hint.data.as_ref()[..8].try_into()?);
                let bank_hash = providers.l2.get_bank_hash(block_number).await?;
                info!("bank_hash:{:?}", bank_hash);
                let bank_hash_bytes = match bank_hash {
                    Some(hash) => bs58::decode(hash.as_str()).into_vec().unwrap(),
                    None => vec![],
                };
                info!("bank_hash_bytes len:{}", bank_hash_bytes.len());
                let mut kv_lock = kv.write().await;
                kv_lock.set(PreimageKey::new_l2_bank_hash(block_number).into(), bank_hash_bytes)?;
            }
            HintType::L2BlockTime => {
                ensure!(hint.data.len() == 8, "Invalid hint data length for l2 block time");
                let block_number = u64::from_be_bytes(hint.data.as_ref()[..8].try_into()?);
                let block_time = providers.l2.get_block_time(block_number).await?;
                let block_time_bytes = match block_time {
                    Some(time) => time.to_be_bytes().to_vec(),
                    None => vec![],
                };
                let mut kv_lock = kv.write().await;
                kv_lock
                    .set(PreimageKey::new_l2_block_time(block_number).into(), block_time_bytes)?;
            }
        }

        Ok(())
    }
}
