//! Contains the concrete implementation of the [L2ChainProvider] trait for the client program.

use crate::alloc::string::ToString;
use crate::{HintType, errors::OracleProviderError};
use alloc::{boxed::Box, sync::Arc, vec::Vec};
use alloy_eips::BlockNumHash;
use alloy_primitives::{Address, B256, Bytes, U160, keccak256};
use alloy_rlp::Decodable;
use async_trait::async_trait;
use kona_driver::PipelineCursor;
use kona_executor::TrieDBProvider;
use kona_mpt::{TrieHinter, TrieNode, TrieProvider};
use kona_preimage::{CommsClient, PreimageKey, PreimageKeyType};
use l1_block_info::instruction::L1BlockInfoInstruction;
use soon_derive::traits::L2ChainProvider;
use soon_primitives::blocks::{BlockInfo, L2BlockInfo, str_block_hash_to};
use soon_primitives::l2blocks::L2Block;
use soon_primitives::rollup_config::SoonRollupConfig;
use soon_primitives::system::SystemConfig;
use spin::RwLock;

/// Trait for setting a pipeline cursor.
pub trait CursorSetter {
    /// Sets the derivation pipeline cursor.
    fn set_cursor(&mut self, cursor: Arc<RwLock<PipelineCursor>>);
}

/// The oracle-backed L2 chain provider for the client program.
#[derive(Debug, Clone)]
pub struct OracleL2ChainProvider<T: CommsClient> {
    /// The L2 safe head block hash.
    l2_head: B256,
    /// The rollup configuration.
    #[allow(dead_code)]
    rollup_config: Arc<SoonRollupConfig>,
    /// The preimage oracle client.
    oracle: Arc<T>,
    /// The derivation pipeline cursor
    cursor: Option<Arc<RwLock<PipelineCursor>>>,
    /// The L2 chain ID to use for the provider's hints.
    chain_id: Option<u64>,
}

impl<T: CommsClient> OracleL2ChainProvider<T> {
    /// Creates a new [OracleL2ChainProvider] with the given boot information and oracle client.
    pub const fn new(l2_head: B256, rollup_config: Arc<SoonRollupConfig>, oracle: Arc<T>) -> Self {
        Self { l2_head, rollup_config, oracle, cursor: None, chain_id: None }
    }

    /// Sets the L2 chain ID to use for the provider's hints.
    pub fn set_chain_id(&mut self, chain_id: Option<u64>) {
        self.chain_id = chain_id;
    }

    /// Updates the derivation pipeline cursor
    pub fn set_cursor(&mut self, cursor: Arc<RwLock<PipelineCursor>>) {
        self.cursor = Some(cursor);
    }

    /// Fetches the latest known safe head block hash according to the derivation pipeline cursor
    /// or uses the initial l2_head value if no cursor is set.
    pub async fn l2_safe_head(&self) -> Result<B256, OracleProviderError> {
        self.cursor
            .as_ref()
            .map_or(Ok(self.l2_head), |cursor| Ok(cursor.read().l2_safe_head().block_info.hash))
    }
}

impl<T: CommsClient> CursorSetter for OracleL2ChainProvider<T> {
    fn set_cursor(&mut self, cursor: Arc<RwLock<PipelineCursor>>) {
        self.cursor = Some(cursor);
    }
}

impl<T: CommsClient> OracleL2ChainProvider<T> {
    pub async fn get_l2_block_info_by_number(
        &self,
        number: u64,
    ) -> Result<L2BlockInfo, OracleProviderError> {
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
            _ => Err(OracleProviderError::FetchBlockInfoFailed(
                "Invalid l1 block info instruction".to_string(),
            )),
        }
    }

    pub async fn get_block_by_number(&self, number: u64) -> Result<L2Block, OracleProviderError> {
        let number_bytes = number.to_be_bytes();
        HintType::L2BlockData
            .with_data(&[number_bytes.as_ref()])
            .send(self.oracle.as_ref())
            .await?;
        let number_hash = keccak256(number_bytes.as_ref());
        let block_bytes = self.oracle.get(PreimageKey::new_keccak256(*number_hash)).await?;

        Decodable::decode(&mut block_bytes.as_slice()).map_err(OracleProviderError::Rlp)
    }

    fn get_l1_block_info(
        &self,
        block: L2Block,
    ) -> Result<L1BlockInfoInstruction, OracleProviderError> {
        let l1_block_info_tx = block.transactions.first().ok_or(
            OracleProviderError::FetchBlockInfoFailed("No l1 block info tx found".to_string()),
        )?;
        let l1_block_info_tx_data =
            l1_block_info_tx.0.message.instructions().first().ok_or(
                OracleProviderError::FetchBlockInfoFailed("No instruction found".to_string()),
            )?;
        let l1_block_info_instruction =
            L1BlockInfoInstruction::unpack(l1_block_info_tx_data.data.as_slice())
                .map_err(|err| OracleProviderError::FetchBlockInfoFailed(err.to_string()))?;
        Ok(l1_block_info_instruction)
    }
}

#[async_trait]
impl<T: CommsClient + Send + Sync> L2ChainProvider for OracleL2ChainProvider<T> {
    type Error = OracleProviderError;

    async fn l2_block_info_by_number(&mut self, number: u64) -> Result<L2BlockInfo, Self::Error> {
        self.get_l2_block_info_by_number(number).await
    }

    async fn block_by_number(&mut self, number: u64) -> Result<L2Block, Self::Error> {
        self.get_block_by_number(number).await
    }

    async fn system_config_by_number(&mut self, number: u64) -> Result<SystemConfig, Self::Error> {
        let block = self.block_by_number(number).await?;
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
            _ => Err(OracleProviderError::FetchBlockInfoFailed(
                "Invalid l1 block info instruction".to_string(),
            )),
        }
    }
}

impl<T: CommsClient> TrieProvider for OracleL2ChainProvider<T> {
    type Error = OracleProviderError;

    fn trie_node_by_hash(&self, key: B256) -> Result<TrieNode, OracleProviderError> {
        // On L2, trie node preimages are stored as keccak preimage types in the oracle. We assume
        // that a hint for these preimages has already been sent, prior to this call.
        crate::block_on(async move {
            TrieNode::decode(
                &mut self
                    .oracle
                    .get(PreimageKey::new(*key, PreimageKeyType::Keccak256))
                    .await
                    .map_err(OracleProviderError::Preimage)?
                    .as_ref(),
            )
            .map_err(OracleProviderError::Rlp)
        })
    }
}

impl<T: CommsClient> TrieDBProvider for OracleL2ChainProvider<T> {
    fn bytecode_by_hash(&self, hash: B256) -> Result<Bytes, OracleProviderError> {
        crate::block_on(async move {
            self.oracle
                .get(PreimageKey::new(*hash, PreimageKeyType::Keccak256))
                .await
                .map(Bytes::from)
                .map_err(OracleProviderError::Preimage)
        })
    }
}

impl<T: CommsClient> TrieHinter for OracleL2ChainProvider<T> {
    type Error = OracleProviderError;

    fn hint_trie_node(&self, hash: B256) -> Result<(), Self::Error> {
        crate::block_on(async move {
            HintType::L2StateNode
                .with_data(&[hash.as_slice()])
                .with_data(self.chain_id.map_or_else(Vec::new, |id| id.to_be_bytes().to_vec()))
                .send(self.oracle.as_ref())
                .await
        })
    }

    fn hint_account_proof(&self, address: Address, block_number: u64) -> Result<(), Self::Error> {
        crate::block_on(async move {
            HintType::L2AccountProof
                .with_data(&[block_number.to_be_bytes().as_ref(), address.as_slice()])
                .with_data(self.chain_id.map_or_else(Vec::new, |id| id.to_be_bytes().to_vec()))
                .send(self.oracle.as_ref())
                .await
        })
    }

    fn hint_storage_proof(
        &self,
        address: alloy_primitives::Address,
        slot: alloy_primitives::U256,
        block_number: u64,
    ) -> Result<(), Self::Error> {
        crate::block_on(async move {
            HintType::L2AccountStorageProof
                .with_data(&[
                    block_number.to_be_bytes().as_ref(),
                    address.as_slice(),
                    slot.to_be_bytes::<32>().as_ref(),
                ])
                .with_data(self.chain_id.map_or_else(Vec::new, |id| id.to_be_bytes().to_vec()))
                .send(self.oracle.as_ref())
                .await
        })
    }
}
