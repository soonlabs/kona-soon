//! This module contains the eip7702 transaction data type for a span batch.

use crate::batch::SpanBatchError;
use alloc::vec::Vec;
use alloy_consensus::{SignableTransaction, Signed, TxEip7702};
use alloy_eips::{eip2930::AccessList, eip7702::SignedAuthorization};
use alloy_primitives::{Address, Signature, U256};
use alloy_rlp::{Bytes, RlpDecodable, RlpEncodable};

/// The transaction data for an EIP-7702 transaction within a span batch.
#[derive(Debug, Clone, PartialEq, Eq, RlpEncodable, RlpDecodable)]
pub struct SpanBatchEip7702TransactionData {
    /// The ETH value of the transaction.
    pub value: U256,
    /// Maximum priority fee per gas.
    pub max_priority_fee_per_gas: U256,
    /// Maximum fee per gas.
    pub max_fee_per_gas: U256,
    /// Transaction calldata.
    pub data: Bytes,
    /// Access list, used to pre-warm storage slots through static declaration.
    pub access_list: AccessList,
    /// Authorization list, used to allow a signer to delegate code to a contract
    pub authorization_list: Vec<SignedAuthorization>,
}

impl SpanBatchEip7702TransactionData {
    /// Converts [SpanBatchEip7702TransactionData] into a signed [`TxEip7702`].
    pub fn to_signed_tx(
        &self,
        nonce: u64,
        gas: u64,
        to: Address,
        chain_id: u64,
        signature: Signature,
    ) -> Result<Signed<TxEip7702>, SpanBatchError> {
        // SAFETY: A U256 as be bytes is always 32 bytes long.
        let mut max_fee_per_gas = [0u8; 16];
        max_fee_per_gas.copy_from_slice(&self.max_fee_per_gas.to_be_bytes::<32>()[16..]);
        let max_fee_per_gas = u128::from_be_bytes(max_fee_per_gas);

        // SAFETY: A U256 as be bytes is always 32 bytes long.
        let mut max_priority_fee_per_gas = [0u8; 16];
        max_priority_fee_per_gas
            .copy_from_slice(&self.max_priority_fee_per_gas.to_be_bytes::<32>()[16..]);
        let max_priority_fee_per_gas = u128::from_be_bytes(max_priority_fee_per_gas);

        let eip7702_tx = TxEip7702 {
            chain_id,
            nonce,
            max_fee_per_gas,
            max_priority_fee_per_gas,
            gas_limit: gas,
            to,
            value: self.value,
            input: self.data.clone().into(),
            access_list: self.access_list.clone(),
            authorization_list: self.authorization_list.clone(),
        };
        let signature_hash = eip7702_tx.signature_hash();
        Ok(Signed::new_unchecked(eip7702_tx, signature, signature_hash))
    }
}
