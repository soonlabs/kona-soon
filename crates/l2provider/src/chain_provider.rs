use crate::error::L2ChainProviderError;
use alloy_eips::BlockNumHash;
use alloy_primitives::{Address, B256, U160, hex::FromHex};
use async_trait::async_trait;
use hex;
use jsonrpsee::{
    core::client::ClientT,
    http_client::{HttpClient, HttpClientBuilder},
    rpc_params,
};
use l1_block_info::{instruction::L1BlockInfoInstruction, solana_program::clock::UnixTimestamp};
use serde_json::Value;
use solana_client::rpc_config::RpcBlockConfig;
use solana_sdk::{
    account::AccountSharedData,
    commitment_config::{CommitmentConfig, CommitmentLevel},
};
use solana_transaction_status::UiTransactionEncoding;
use soon_derive::prelude::L2ChainProvider;
use soon_primitives::{
    blocks::{BlockInfo, L2BlockInfo, str_block_hash_to},
    l2blocks::L2Block,
    mpt::{AccountWithTrie, WrappedSolanaAccount},
    output_root::OutputRoot,
    rpc::{OutputAtBlockResp, SoonGetAccountProofResp},
    system::SystemConfig,
    ui::UiConfirmedBlockWithEntries,
};
use std::fmt;
use std::time::{Duration, Instant};
use tracing::info;

#[derive(Clone)]
pub struct L2BlockFetcher {
    rpc_url: String,
    soon: HttpClient,
    request_timeout: Duration,
}

impl fmt::Debug for L2BlockFetcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("L2BlockFetcher").field("client", &self.rpc_url).finish()
    }
}

impl L2BlockFetcher {
    pub fn new_with_url(rpc_url: &str) -> Self {
        let request_timeout = Duration::from_secs(150);
        let soon = HttpClientBuilder::default()
            .request_timeout(request_timeout)
            .build(rpc_url)
            .expect("Failed to build HTTP client");

        info!(
            "Created HttpClient with configured timeout: {:?} seconds",
            request_timeout.as_secs()
        );
        L2BlockFetcher { soon, rpc_url: rpc_url.to_string(), request_timeout }
    }

    pub async fn get_block_by_number(&self, number: u64) -> Result<L2Block, L2ChainProviderError> {
        let params = rpc_params![
            number,
            RpcBlockConfig {
                encoding: Some(UiTransactionEncoding::Base58),
                transaction_details: None,
                rewards: Some(false),
                commitment: Some(CommitmentConfig { commitment: CommitmentLevel::Confirmed }),
                max_supported_transaction_version: Some(0),
            }
        ];
        let block: UiConfirmedBlockWithEntries =
            self.soon.request("getBlockWithEntries", params).await.map_err(|e| {
                L2ChainProviderError::FetchBlockWithEntriesFailed(format!(
                    "Error fetching block with entries {}: {:?}",
                    number, e
                ))
            })?;
        let block: L2Block = block.try_into().map_err(|e| {
            L2ChainProviderError::FetchBlockWithEntriesFailed(format!(
                "Error converting block with entries to L2Block: {:?}",
                e,
            ))
        })?;
        Ok(block)
    }

    pub async fn output_at_block(&self, num: u64) -> Result<OutputRoot, L2ChainProviderError> {
        let params = rpc_params![num];
        let res: OutputAtBlockResp =
            self.soon.request("outputAtBlock", params).await.map_err(|e| {
                L2ChainProviderError::FetchOutputAtBlockFailed(format!(
                    "Error fetching output at block: {:?}",
                    e
                ))
            })?;
        if res.query_slot != num {
            return Err(L2ChainProviderError::FetchOutputAtBlockFailed(format!(
                "query slot outdated, query slot:{}, res slot: {}",
                num, res.query_slot
            )));
        }

        Ok(OutputRoot {
            state_root: B256::from_hex(&res.state_root).unwrap(),
            bridge_storage_root: B256::from_hex(&res.withdrawal_root).unwrap(),
            block_hash: B256::from_hex(&res.block_hash).unwrap(),
        })
    }

    pub async fn sync_status(&self) -> Result<Value, L2ChainProviderError> {
        let sync_status = self.soon.request("getSyncStatus", rpc_params![]).await.map_err(|e| {
            L2ChainProviderError::FetchOutputAtBlockFailed(format!(
                "Error fetching sync status: {:?}",
                e
            ))
        })?;
        Ok(sync_status)
    }

    pub async fn rollup_config(&self) -> Result<Value, L2ChainProviderError> {
        let rollup_config =
            self.soon.request("getRollupConfig", rpc_params![]).await.map_err(|e| {
                L2ChainProviderError::FetchOutputAtBlockFailed(format!(
                    "Error fetching Rollup config: {:?}",
                    e
                ))
            })?;

        Ok(rollup_config)
    }

    pub async fn get_bank_hash(
        &self,
        block_number: u64,
    ) -> Result<Option<String>, L2ChainProviderError> {
        Ok(self.soon.request("getBankHash", rpc_params![block_number]).await.map_err(|e| {
            L2ChainProviderError::GetSlotBankHashFailed(format!(
                "Error fetching band hash: {:?}",
                e
            ))
        })?)
    }

    pub async fn get_block_time(
        &self,
        block_number: u64,
    ) -> Result<Option<UnixTimestamp>, L2ChainProviderError> {
        Ok(self.soon.request("getBlockTime", rpc_params![block_number]).await.map_err(|e| {
            L2ChainProviderError::GetSlotBlockTimeFailed(format!(
                "Error fetching block time: {:?}",
                e
            ))
        })?)
    }

    pub async fn get_trie_node(&self, _: B256) -> Result<Vec<u8>, L2ChainProviderError> {
        unimplemented!("soon-svm does not need trie node hint");
    }

    pub async fn get_tried_account_proof(
        &self,
        account: B256,
        block_number: u64,
    ) -> Result<AccountWithTrie, L2ChainProviderError> {
        let params = rpc_params![account, block_number];

        info!(
            "Starting request with configured timeout: {:?} seconds",
            self.request_timeout.as_secs()
        );
        let start_time = Instant::now();

        let res: SoonGetAccountProofResp =
            self.soon.request("getSoonAccountProof", params).await.map_err(|e| {
                let elapsed = start_time.elapsed();
                info!("Request failed after {:?} seconds. Error: {:?}", elapsed.as_secs(), e);
                L2ChainProviderError::FetchTriedAccountFailed(format!(
                    "Error fetching output at block (elapsed: {:?}s, timeout: {:?}s): {:?}",
                    elapsed.as_secs(),
                    self.request_timeout.as_secs(),
                    e
                ))
            })?;

        let elapsed = start_time.elapsed();
        info!(
            "Request completed successfully in {:?} seconds (timeout was {:?} seconds)",
            elapsed.as_secs(),
            self.request_timeout.as_secs()
        );

        let shared_data: Option<AccountSharedData> = match res.account {
            Some(ui_account) => Some(ui_account.decode().ok_or(
                L2ChainProviderError::FetchTriedAccountFailed(format!(
                    "Can't decode preimage of account {} at block: {}",
                    account, block_number,
                )),
            )?),
            None => None,
        };
        let wrapped_account = shared_data.map(|account| WrappedSolanaAccount(account));

        let decode_proof = |proof: String| -> Result<Vec<u8>, L2ChainProviderError> {
            // Remove 0x prefix if present
            let proof_str = if proof.starts_with("0x") { &proof[2..] } else { &proof };
            hex::decode(proof_str).map_err(|e| {
                L2ChainProviderError::FetchTriedAccountFailed(format!(
                    "Failed to decode hex proof for account {} at block {}: {}",
                    account, block_number, e
                ))
            })
        };

        let proofs = res.proof.into_iter().map(decode_proof).collect::<Result<Vec<_>, _>>()?;

        let withdrawal_proofs = match res.withdrawal_proof {
            None => vec![],
            Some(proofs) => proofs.into_iter().map(decode_proof).collect::<Result<Vec<_>, _>>()?,
        };

        Ok(AccountWithTrie { block_number, account: wrapped_account, proofs, withdrawal_proofs })
    }

    pub async fn get_storage_node_proof(
        &self,
        _: B256,
        _: u64,
    ) -> Result<Vec<Vec<u8>>, L2ChainProviderError> {
        unimplemented!("soon-svm does not need storage node proof hint");
    }

    fn get_l1_block_info(
        &self,
        block: L2Block,
    ) -> Result<L1BlockInfoInstruction, L2ChainProviderError> {
        let l1_block_info_tx = block.transactions.first().ok_or(
            L2ChainProviderError::FetchBlockInfoFailed("No l1 block info tx found".to_string()),
        )?;
        let l1_block_info_tx_data =
            l1_block_info_tx.transaction().message.instructions().first().ok_or(
                L2ChainProviderError::FetchBlockInfoFailed("No instruction found".to_string()),
            )?;
        let l1_block_info_instruction =
            L1BlockInfoInstruction::unpack(l1_block_info_tx_data.data.as_slice())
                .map_err(|err| L2ChainProviderError::FetchBlockInfoFailed(err.to_string()))?;
        Ok(l1_block_info_instruction)
    }
}

#[async_trait]
impl L2ChainProvider for L2BlockFetcher {
    type Error = L2ChainProviderError;

    async fn l2_block_info_by_number(&mut self, number: u64) -> Result<L2BlockInfo, Self::Error> {
        let block = self.get_block_by_number(number).await?;
        let block_info = BlockInfo::new(
            str_block_hash_to(block.blockhash.as_str()),
            number,
            str_block_hash_to(block.previous_blockhash.as_str()),
            block.block_time,
        );
        let l1_block_info_instruction = self.get_l1_block_info(block)?;
        match l1_block_info_instruction {
            L1BlockInfoInstruction::UpdateL1BlockInfo {
                number,
                timestamp: _,
                base_fee: _,
                hash,
                sequence_number,
                batcher_hash: _,
                fee_overhead: _,
                fee_scalar: _,
                gas: _,
                is_system_tx: _,
            } => Ok(L2BlockInfo {
                block_info,
                l1_origin: BlockNumHash { number, hash: B256::from(hash) },
                seq_num: sequence_number,
            }),
            _ => Err(L2ChainProviderError::FetchBlockInfoFailed(
                "Invalid l1 block info instruction".to_string(),
            )),
        }
    }

    async fn block_by_number(&mut self, slot: u64) -> Result<L2Block, Self::Error> {
        self.get_block_by_number(slot).await
    }

    async fn system_config_by_number(&mut self, number: u64) -> Result<SystemConfig, Self::Error> {
        let block = self.get_block_by_number(number).await?;
        let l1_block_info_instruction = self.get_l1_block_info(block)?;
        match l1_block_info_instruction {
            L1BlockInfoInstruction::UpdateL1BlockInfo {
                number: _,
                timestamp: _,
                base_fee: _,
                hash: _,
                sequence_number: _,
                batcher_hash,
                fee_overhead: _,
                fee_scalar: _,
                gas: _,
                is_system_tx: _,
            } => {
                let address_value = U160::from_le_slice(&batcher_hash[..20]);
                Ok(SystemConfig {
                    batcher_address: Address::from(address_value),
                    ..Default::default()
                })
            }
            _ => Err(L2ChainProviderError::FetchBlockInfoFailed(
                "Invalid l1 block info instruction".to_string(),
            )),
        }
    }
}
