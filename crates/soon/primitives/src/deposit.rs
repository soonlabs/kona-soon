use crate::blocks::BlockInfo;
use crate::error::DepositError;
use crate::native_tx::new_native_transaction;
use alloy_consensus::{Eip658Value, Receipt};
use alloy_primitives::private::alloy_rlp::Encodable;
use alloy_primitives::{Address, B256, BlockHash, Log, U256, keccak256};
use bridge::instruction::{deposit_erc20, deposit_eth};
use hex_literal::hex;
use l1_block_info::instruction::update_l1_block_info;
use num_bigint::BigUint;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::transaction::SanitizedTransaction;
use std::array::TryFromSliceError;
use std::collections::HashSet;
use std::ops::{Add, Sub};

/// Deposit log event abi signature.
pub const SOON_DEPOSIT_EVENT_ABI: &str = "TransactionDeposited(address,bytes32,uint256,bytes)";

const ETH_DEPOSIT_SELECTOR: [u8; 4] = hex!("596a37c5");
const ERC20_DEPOSIT_SELECTOR: [u8; 4] = hex!("f73fb39c");
const CROSS_DOMAIN_MESSENGER_RELAY_SELECTOR: [u8; 4] = hex!("E5D0A3D7");
const SOON_BRIDGE_PUBKEY_HEX: &str =
    "0x02c806312cb859f1bc25448e39f87aa09857d83ccb4a837df55648e000000000";

pub const USER_DEPOSIT_SOURCE_DOMAIN: u32 = 0;
pub const L1_INFO_DEPOSIT_SOURCE_DOMAIN: u32 = 1;

#[derive(Debug, Clone)]
pub struct ExternalData {
    /// l1 sender address, should l1 standard bridge for ETH/ERC20
    pub from: Address,
    /// l2 receiver address, should be hex string of bridge pubkey
    pub to: String,
    /// ETH value to mint on L2
    pub mint: U256,
    /// ETH value to send to the recipient
    pub value: U256,
    /// Gas limit for the L2 transaction
    pub gas: u64,
    /// If this is a contract creation
    pub is_creation: bool,
    pub extra_data: Vec<u8>,
}

impl ExternalData {
    pub fn extract(opaque_data: &[u8]) -> Result<(Self, Vec<u8>), DepositError> {
        if opaque_data.length() < 305 {
            return Err(DepositError::InvalidDataLength);
        }
        let mint = U256::from_be_slice(&opaque_data[0..32]);
        let value = U256::from_be_slice(&opaque_data[32..64]);
        let gas = u64::from_be_bytes(opaque_data[64..72].try_into().map_err(
            |e: TryFromSliceError| {
                DepositError::ParseDepositTxError(format!("convert gas limit failed: {e}"))
            },
        )?);
        let is_creation = opaque_data[72] != 0;

        //decode cross domain message
        let relay_selector = &opaque_data[73..77];
        if relay_selector != CROSS_DOMAIN_MESSENGER_RELAY_SELECTOR {
            return Err(DepositError::InvalidRelaySelector(hex::encode(relay_selector)));
        }
        let _relay_nonce = &opaque_data[77..109];
        let from = Address::from_slice(&opaque_data[121..141]);
        let to = B256::from_slice(&opaque_data[141..173]).to_string();
        let _value = &opaque_data[173..205];
        let _gas_limit = &opaque_data[205..237];
        let _bridge_data_offset = &opaque_data[237..269];
        let _bridge_data_length = &opaque_data[269..301];

        //decode standard bridge data
        let deposit_selector = &opaque_data[301..305];
        if deposit_selector != ETH_DEPOSIT_SELECTOR && deposit_selector != ERC20_DEPOSIT_SELECTOR {
            return Err(DepositError::InvalidDepositSelector(hex::encode(deposit_selector)));
        }

        let extra_data = opaque_data[73..301].to_vec();
        let rest = opaque_data[301..].to_vec();

        Ok((Self { from, to, mint, value, gas, is_creation, extra_data }, rest))
    }
}

#[derive(Debug, Clone)]
pub struct EthDepositData {
    pub from: Address,
    pub to: Pubkey,
    pub amount: U256,
    pub extra_data: Vec<u8>,
}

impl TryFrom<Vec<u8>> for EthDepositData {
    type Error = DepositError;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        let from = Address::from_slice(&value[12..32]);
        let to = Pubkey::try_from(&value[32..64]).map_err(|_| DepositError::InvalidPubKey)?;
        let amount = U256::from_be_slice(&value[64..96]);
        // TODO: current extra data needs to be fixed size of 32 bytes, need to update this later
        let mut extra_data = value[96..].to_vec();
        extra_data.resize(32, 0);
        Ok(Self { from, to, amount, extra_data })
    }
}

#[derive(Debug, Clone)]
pub struct Erc20DepositData {
    pub l2_contract: Pubkey,
    pub l1_contract: Address,
    pub from: Address,
    pub to: Pubkey,
    pub amount: U256,
    pub extra_data: Vec<u8>,
}

impl TryFrom<Vec<u8>> for Erc20DepositData {
    type Error = DepositError;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        let l2_contract =
            Pubkey::try_from(&value[0..32]).map_err(|_| DepositError::InvalidPubKey)?;
        let l1_contract = Address::from_slice(&value[44..64]);
        let from = Address::from_slice(&value[76..96]);
        let to = Pubkey::try_from(&value[96..128]).map_err(|_| DepositError::InvalidPubKey)?;
        let amount = U256::from_be_slice(&value[128..160]);
        let extra_data = value[160..].to_vec();

        Ok(Self { l2_contract, l1_contract, from, to, amount, extra_data })
    }
}

#[derive(Debug, Clone)]
pub enum DepositType {
    ETH(EthDepositData),
    ERC20(Erc20DepositData),
}

impl TryFrom<Vec<u8>> for DepositType {
    type Error = DepositError;

    /// Converts the emitted L1 deposit event log into [UserDeposited]
    fn try_from(mut data: Vec<u8>) -> Result<Self, DepositError> {
        let selector: [u8; 4] = data[0..4].try_into().map_err(|e: TryFromSliceError| {
            DepositError::ParseDepositTxError(format!("convert selector failed: {}", e))
        })?;
        data.drain(0..4);
        match selector {
            ETH_DEPOSIT_SELECTOR => Ok(Self::ETH(data.try_into()?)),
            ERC20_DEPOSIT_SELECTOR => Ok(Self::ERC20(data.try_into()?)),
            _ => Err(DepositError::ParseDepositTxError("unknown deposit type".to_string())),
        }
    }
}

/// Represents a user deposited transaction.
#[derive(Debug, Clone)]
pub struct UserDeposited {
    /// Address of the sender
    pub bridge_sender: Address,
    /// Address of the l2 bridge, should be hex string of bridge pubkey
    pub bridge_target: String,
    /// ETH value to mint on L2
    pub mint: U256,
    /// ETH value to send to the recipient
    pub value: U256,
    /// Gas limit for the L2 transaction
    pub gas: u64,
    /// If this is a contract creation
    pub is_creation: bool,
    /// The L1 block number this was submitted in.
    pub l1_block_num: u64,
    /// The L1 block hash this was submitted in.
    pub l1_block_hash: BlockHash,
    /// The index of the emitted deposit event log in the L1 block.
    pub log_index: u64,
    /// The data decoded from opaque data.
    pub deposit_type: DepositType,
}

impl UserDeposited {
    pub fn to_sanitized_transaction(
        self,
        reserved_account_keys: &HashSet<Pubkey>,
    ) -> Result<SanitizedTransaction, DepositError> {
        let instructions = match self.clone().deposit_type {
            DepositType::ETH(data) => vec![deposit_eth(
                self.bridge_sender.into_array().into(),
                Address::default().into_array().into(), //use default zero address
                self.mint
                    .try_into()
                    .map_err(|_| DepositError::ParseDepositTxError("Invalid mint".to_string()))?,
                data.amount
                    .try_into()
                    .map_err(|_| DepositError::ParseDepositTxError("Invalid amount".to_string()))?,
                self.gas,
                self.is_creation,
                self.l1_block_num,
                self.l1_block_hash.into(),
                self.log_index as u128,
                data.from.into_array().into(),
                data.to,
                data.amount
                    .try_into()
                    .map_err(|_| DepositError::ParseDepositTxError("Invalid amount".to_string()))?,
                data.extra_data.try_into().map_err(|_| {
                    DepositError::ParseDepositTxError("Invalid extra data".to_string())
                })?,
            )],
            DepositType::ERC20(data) => vec![deposit_erc20(
                data.l2_contract,
                data.l1_contract.into_array().into(),
                data.from.into_array().into(),
                data.to,
                data.amount
                    .try_into()
                    .map_err(|_| DepositError::ParseDepositTxError("Invalid amount".to_string()))?,
            )],
        };
        let tx = new_native_transaction(&instructions, self.source_hash())?;

        Ok(SanitizedTransaction::try_from_legacy_transaction(tx, reserved_account_keys)?)
    }

    // User-deposited: keccak256(bytes32(uint256(0)), keccak256(l1BlockHash, bytes32(uint256(l1LogIndex))))
    pub fn source_hash(&self) -> B256 {
        let mut input = [0u8; 64];
        input[0..32].copy_from_slice(self.l1_block_hash.as_slice());
        input[64 - 8..64].copy_from_slice(self.log_index.to_be_bytes().as_slice());
        let deposit_id_hash = keccak256(input);

        let mut domain_input = [0u8; 64];
        domain_input[32 - 4..32]
            .copy_from_slice(USER_DEPOSIT_SOURCE_DOMAIN.to_be_bytes().as_slice());
        domain_input[32..64].copy_from_slice(deposit_id_hash.as_slice());

        keccak256(domain_input)
    }

    /// Converts the emitted L1 deposit event log into [UserDeposited]
    pub fn parse_from_log(
        l1_block_num: u64,
        l1_block_hash: B256,
        log_index: u64,
        log: &Log,
        expected_bridge_sender: Address,
        expected_relay_sender: Address,
    ) -> Result<Self, DepositError> {
        let mut opaque_data = log.data.data.to_vec();
        //remove data offset and data length
        opaque_data.drain(..64);

        let bridge_sender = Address::from_word(log.topics()[1]);
        let bridge_sender = undo_l1_to_l2_alias(bridge_sender);
        if bridge_sender != expected_bridge_sender {
            return Err(DepositError::InvalidDomainMessenger(bridge_sender.to_string()));
        }
        let bridge_target = log.topics()[2].to_string().to_lowercase();
        if bridge_target != SOON_BRIDGE_PUBKEY_HEX {
            return Err(DepositError::InvalidL2CrossDomainMessenger(bridge_target));
        }
        let (ext, data) = ExternalData::extract(&opaque_data)?;
        if ext.from != expected_relay_sender {
            return Err(DepositError::InvalidL1StandardBridge(ext.from.to_string()));
        }
        if ext.to != SOON_BRIDGE_PUBKEY_HEX {
            return Err(DepositError::InvalidL2StandardBridge(ext.to));
        }
        let deposit_type = DepositType::try_from(data)?;

        let user_deposit = Self {
            bridge_sender,
            bridge_target,
            mint: ext.mint,
            value: ext.value,
            gas: ext.gas,
            is_creation: ext.is_creation,
            deposit_type,
            l1_block_num,
            l1_block_hash,
            log_index,
        };

        Ok(user_deposit)
    }
}

/// Represents the attributes provided as calldata in an attributes deposited transaction.
#[derive(Debug, Clone, Default)]
pub struct AttributesDeposited {
    /// The L1 epoch block number
    pub l1_block_num: u64,
    /// The L1 epoch block timestamp
    pub timestamp: u64,
    /// The L1 epoch base fee
    pub base_fee: u128,
    /// The L1 epoch block hash
    pub l1_block_hash: BlockHash,
    /// The L2 block's position in the epoch
    pub sequence_number: u64,
    /// A versioned hash of the current authorized batcher sender.
    pub batcher_hash: U256,
    /// The current L1 fee overhead to apply to L2 transactions cost computation. Unused after Ecotone hard fork.
    pub fee_overhead: U256,
    /// The current L1 fee scalar to apply to L2 transactions cost computation. Unused after Ecotone hard fork.
    pub fee_scalar: U256,
    /// Gas limit: 1_000_000 if post-Regolith, otherwise 150_000_000
    pub gas: u64,
    /// False if post-Regolith, otherwise true
    pub is_system_tx: bool,
}

impl AttributesDeposited {
    pub fn new(block: &BlockInfo, sequence_number: u64, batcher_hash: U256) -> Self {
        Self {
            l1_block_num: block.number,
            timestamp: block.timestamp,
            base_fee: 0,
            l1_block_hash: block.hash,
            batcher_hash,
            sequence_number,
            is_system_tx: true,
            // TODO: update fee if needed later
            ..Default::default()
        }
    }

    pub fn to_sanitized_transaction(
        self,
        reserved_account_keys: &HashSet<Pubkey>,
    ) -> Result<SanitizedTransaction, DepositError> {
        let instructions = vec![update_l1_block_info(
            self.l1_block_num,
            self.timestamp,
            self.base_fee,
            self.l1_block_hash.into(),
            self.sequence_number,
            self.batcher_hash.to_le_bytes(),
            self.fee_overhead.try_into().map_err(|_| {
                DepositError::ParseL1BlockInfoTxError("Invalid fee overhead".to_string())
            })?,
            self.fee_scalar.try_into().map_err(|_| {
                DepositError::ParseL1BlockInfoTxError("Invalid fee scalar".to_string())
            })?,
            self.gas,
            self.is_system_tx,
        )];

        let tx = new_native_transaction(&instructions, self.source_hash())?;
        let tx = SanitizedTransaction::try_from_legacy_transaction(tx, reserved_account_keys)?;

        Ok(tx)
    }

    // Deposit source hash computation follows Op Spec: https://specs.optimism.io/protocol/deposits.html#source-hash-computation
    // L1 attributes deposited: keccak256(bytes32(uint256(1)), keccak256(l1BlockHash, bytes32(uint256(seqNumber))))
    pub fn source_hash(&self) -> B256 {
        let mut input = [0u8; 64];
        input[0..32].copy_from_slice(self.l1_block_hash.as_slice());
        input[64 - 8..64].copy_from_slice(self.sequence_number.to_be_bytes().as_slice());
        let deposit_id_hash = keccak256(input);

        let mut domain_input = [0u8; 64];
        domain_input[32 - 4..32]
            .copy_from_slice(L1_INFO_DEPOSIT_SOURCE_DOMAIN.to_be_bytes().as_slice());
        domain_input[32..64].copy_from_slice(deposit_id_hash.as_slice());

        keccak256(domain_input)
    }
}

/// Derive deposits as `Vec<Bytes>` for transaction receipts.
///
/// Successful deposits must be emitted by the deposit contract and have the correct event
/// signature. So the receipt address must equal the specified deposit contract and the first topic
/// must be the [DEPOSIT_EVENT_ABI_HASH].
pub fn derive_deposits(
    l1_block_num: u64,
    l1_block_hash: B256,
    receipts: &[Receipt],
    deposit_contract: Address,
    l1_cross_domain_messenger: Address,
    l1_standard_bridge: Address,
) -> Vec<UserDeposited> {
    let mut global_index = 0;
    let mut res = Vec::new();
    let deposit_event_signature = keccak256(SOON_DEPOSIT_EVENT_ABI.as_bytes());
    for r in receipts.iter() {
        if Eip658Value::Eip658(false) == r.status {
            continue;
        }
        for l in r.logs.iter() {
            let curr_index = global_index;
            global_index += 1;
            if l.data.topics().first().is_none_or(|i| *i != deposit_event_signature) {
                continue;
            }
            if l.address != deposit_contract {
                continue;
            }
            match UserDeposited::parse_from_log(
                l1_block_num,
                l1_block_hash,
                curr_index,
                l,
                l1_cross_domain_messenger,
                l1_standard_bridge,
            ) {
                Ok(user_deposit) => {
                    res.push(user_deposit);
                }
                Err(err) => {
                    warn!("Derive deposit tx failed: {}", err);
                }
            }
        }
    }
    res
}

fn padding_0_for_address_data(data: Vec<u8>) -> Vec<u8> {
    let mut ret_data = data.clone();
    let padding_len = 20_usize.saturating_sub(ret_data.len());
    ret_data.splice(0..0, std::iter::repeat(0).take(padding_len));
    ret_data
}

fn l1_to_l2_offset() -> BigUint {
    BigUint::from_bytes_be(
        hex::decode("0x1111000000000000000000000000000000001111".as_bytes()).unwrap().as_slice(),
    )
}

fn max_address_to_uint() -> BigUint {
    BigUint::from_bytes_be(
        hex::decode("0xffffffffffffffffffffffffffffffffffffffff".as_bytes()).unwrap().as_slice(),
    )
}

#[allow(dead_code)]
fn apply_l1_to_l2_alias(l1_address: Address) -> Address {
    let offset = l1_to_l2_offset();
    let max = max_address_to_uint();
    let l1_num = BigUint::from_bytes_be(l1_address.as_slice());
    let mut l2_alias_num = l1_num + offset;
    if l2_alias_num.gt(&max) {
        l2_alias_num = l2_alias_num.sub(max).sub(BigUint::from(1_u8))
    }
    Address::from_slice(padding_0_for_address_data(l2_alias_num.to_bytes_be()).as_slice())
}

fn undo_l1_to_l2_alias(l2_alias: Address) -> Address {
    let offset = l1_to_l2_offset();
    let max = max_address_to_uint();
    let mut l2_alias_num = BigUint::from_bytes_be(l2_alias.as_slice());
    if l2_alias_num.lt(&offset) {
        l2_alias_num = l2_alias_num.add(max).add(BigUint::from(1_u8));
    }
    let l1_num = l2_alias_num - offset;
    Address::from_slice(padding_0_for_address_data(l1_num.to_bytes_be()).as_slice())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::address;

    #[test]
    fn parse_deposit_to_portal_should_fail() -> anyhow::Result<()> {
        let log_str = include_str!("./test_data/deposit_to_portal.json");
        let log: Log = serde_json::from_str(log_str).unwrap();

        //use dev1 contract address
        let ret = UserDeposited::parse_from_log(
            0,
            B256::ZERO,
            0,
            &log,
            "0x2f4318d893c96bf52666Bf8Fe8199690a5f56eFf".parse().unwrap(),
            "0xDC8D9CB62E4AD001531EedFdFB30ad581dc6147a".parse().unwrap(),
        );

        let err = ret.unwrap_err();
        assert!(matches!(err, DepositError::InvalidDomainMessenger(..)));

        Ok(())
    }

    #[test]
    fn parse_deposit_to_portal_with_empty_data_should_failed() -> anyhow::Result<()> {
        let log_str = include_str!("./test_data/deposit_to_portal_with_empty_data.json");
        let log: Log = serde_json::from_str(log_str).unwrap();

        //use dev1 contract address
        let ret = UserDeposited::parse_from_log(
            0,
            B256::ZERO,
            0,
            &log,
            "0x2f4318d893c96bf52666Bf8Fe8199690a5f56eFf".parse().unwrap(),
            "0xDC8D9CB62E4AD001531EedFdFB30ad581dc6147a".parse().unwrap(),
        );

        let err = ret.unwrap_err();
        assert!(matches!(err, DepositError::InvalidDomainMessenger(..)));

        Ok(())
    }

    #[test]
    fn parse_deposit_to_messenger_should_fail() -> anyhow::Result<()> {
        let log_str = include_str!("./test_data/deposit_to_messenger.json");
        let log: Log = serde_json::from_str(log_str).unwrap();

        //use dev1 contract address
        let ret = UserDeposited::parse_from_log(
            0,
            B256::ZERO,
            0,
            &log,
            "0x2f4318d893c96bf52666Bf8Fe8199690a5f56eFf".parse().unwrap(),
            "0xDC8D9CB62E4AD001531EedFdFB30ad581dc6147a".parse().unwrap(),
        );

        let err = ret.unwrap_err();
        assert!(matches!(err, DepositError::InvalidL1StandardBridge(..)));

        Ok(())
    }

    #[test]
    fn parse_deposit_to_messenger_with_empty_data_should_fail() -> anyhow::Result<()> {
        let log_str = include_str!("./test_data/deposit_to_messenger_with_empty_data.json");
        let log: Log = serde_json::from_str(log_str).unwrap();

        //use dev1 contract address
        let ret = UserDeposited::parse_from_log(
            0,
            B256::ZERO,
            0,
            &log,
            "0x2f4318d893c96bf52666Bf8Fe8199690a5f56eFf".parse().unwrap(),
            "0xDC8D9CB62E4AD001531EedFdFB30ad581dc6147a".parse().unwrap(),
        );

        let err = ret.unwrap_err();
        assert!(matches!(err, DepositError::InvalidDepositSelector(..)));

        Ok(())
    }

    #[test]
    fn parse_deposit_to_invalid_l2_cross_domain_messenger_should_failed() -> anyhow::Result<()> {
        let log_str = include_str!("./test_data/deposit_to_invalid_l2_cross_domain_messenger.json");
        let log: Log = serde_json::from_str(log_str).unwrap();

        //use dev1 contract address
        let ret = UserDeposited::parse_from_log(
            0,
            B256::ZERO,
            0,
            &log,
            "0x2f4318d893c96bf52666Bf8Fe8199690a5f56eFf".parse().unwrap(),
            "0xDC8D9CB62E4AD001531EedFdFB30ad581dc6147a".parse().unwrap(),
        );

        let err = ret.unwrap_err();
        assert!(matches!(err, DepositError::InvalidDomainMessenger(..)));

        Ok(())
    }

    #[test]
    fn parse_deposit_to_invalid_l2_standard_bridge_should_failed() -> anyhow::Result<()> {
        let log_str = include_str!("./test_data/deposit_to_invalid_l2_standard_bridge.json");
        let log: Log = serde_json::from_str(log_str).unwrap();

        //use dev1 contract address
        let ret = UserDeposited::parse_from_log(
            0,
            B256::ZERO,
            0,
            &log,
            "0x2f4318d893c96bf52666Bf8Fe8199690a5f56eFf".parse().unwrap(),
            "0xDC8D9CB62E4AD001531EedFdFB30ad581dc6147a".parse().unwrap(),
        );

        let err = ret.unwrap_err();
        assert!(matches!(err, DepositError::InvalidL1StandardBridge(..)));

        Ok(())
    }

    #[test]
    fn parse_eth_deposit_log_works() -> anyhow::Result<()> {
        let raw_hex = hex!(
            "000000000000000000000000000000000000000000000000000000000000162e000000000000000000000000000000000000000000000000000000000000162e000000000004686d00e5d0a3d7000100000000000000000000000000000000000000000000000000000000000000000000000000000000000084d986cd0dad64ada30b07e07eb3f172e5bd9d340000000000000000000000004200000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000162e00000000000000000000000000000000000000000000000000000000000004d200000000000000000000000000000000000000000000000000000000000000c000000000000000000000000000000000000000000000000000000000000000a4596a37c50000000000000000000000008b9aa4d8c5a890b41cd10126cbf160d9150f7c5c1111111111111111111111111111111111111111111111111111111111111111000000000000000000000000000000000000000000000000000000000000162e0000000000000000000000000000000000000000000000000000000000000080000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"
        );

        let (external, inner) = ExternalData::extract(&raw_hex)?;
        assert_eq!(external.value.to::<u64>(), 5678);
        assert_eq!(external.mint, external.value);
        assert_eq!(external.gas, 288877);
        assert!(!external.is_creation);
        assert_eq!(external.extra_data.len(), 228); // TODO: check the content if needed later

        let deposit_type = DepositType::try_from(inner)?;
        if let DepositType::ETH(deposit) = deposit_type {
            assert_eq!(deposit.amount.to::<u64>(), 5678);
            assert_eq!(deposit.from, address!("8b9AA4d8c5a890B41CD10126CBF160d9150f7c5C"));
            assert_eq!(
                deposit.to,
                hex!("1111111111111111111111111111111111111111111111111111111111111111")
                    .try_into()?
            );
            // assert_eq!(deposit.extra_data.len(), 92); // TODO: check the content if needed later
            assert_eq!(deposit.extra_data.len(), 32);
        } else {
            panic!("unexpected deposit type");
        }

        Ok(())
    }

    #[test]
    fn parse_erc20_deposit_log_works() -> anyhow::Result<()> {
        let raw_hex = hex!(
            "00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000023680a00e5d0a3d7000100000000000000000000000000000000000000000000000000000000000000000000000000000000000084d986cd0dad64ada30b07e07eb3f172e5bd9d340000000000000000000000004200000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001e848000000000000000000000000000000000000000000000000000000000000000c000000000000000000000000000000000000000000000000000000000000000e4f73fb39c222222222222222222222222222222222222222222222222222222222222222200000000000000000000000033333333333333333333333333333333333333330000000000000000000000008b9aa4d8c5a890b41cd10126cbf160d9150f7c5c111111111111111111111111111111111111111111111111111111111111111100000000000000000000000000000000000000000000000000000000001e848000000000000000000000000000000000000000000000000000000000000000c0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"
        );
        let (external, inner) = ExternalData::extract(&raw_hex)?;
        assert_eq!(external.value.to::<u64>(), 0);
        assert_eq!(external.mint, external.value);
        assert_eq!(external.gas, 2320394);
        assert!(!external.is_creation);
        assert_eq!(external.extra_data.len(), 228); // TODO: check the content if needed later

        let deposit_type = DepositType::try_from(inner)?;
        if let DepositType::ERC20(deposit) = deposit_type {
            assert_eq!(deposit.amount.to::<u64>(), 2000000);
            assert_eq!(deposit.from, address!("8b9AA4d8c5a890B41CD10126CBF160d9150f7c5C"));
            assert_eq!(
                deposit.to,
                hex!("1111111111111111111111111111111111111111111111111111111111111111")
                    .try_into()?
            );
            assert_eq!(
                deposit.l2_contract,
                hex!("2222222222222222222222222222222222222222222222222222222222222222")
                    .try_into()?
            );
            assert_eq!(deposit.l1_contract, address!("3333333333333333333333333333333333333333"));
            assert_eq!(deposit.extra_data.len(), 92); // TODO: check the content if needed later
        } else {
            panic!("unexpected deposit type");
        }
        Ok(())
    }

    // cast keccak $(cast concat-hex 0x0000000000000000000000000000000000000000000000000000000000000001 $(cast keccak $(cast concat-hex 0xc00e5d67c2755389aded7d8b151cbd5bcdf7ed275ad5e028b664880fc7581c77 0x0000000000000000000000000000000000000000000000000000000000000004)))
    // # 0x0586c503340591999b8b38bc9834bb16aec7d5bc00eb5587ab139c9ddab81977
    #[test]
    fn l1_attribute_deposited_source_hash_works() -> anyhow::Result<()> {
        let l1_attribute = AttributesDeposited {
            l1_block_num: 0,
            timestamp: 0,
            base_fee: 0,
            l1_block_hash: hex!("c00e5d67c2755389aded7d8b151cbd5bcdf7ed275ad5e028b664880fc7581c77")
                .into(),
            sequence_number: 4,
            batcher_hash: Default::default(),
            fee_overhead: Default::default(),
            fee_scalar: Default::default(),
            gas: 0,
            is_system_tx: false,
        };

        assert_eq!(
            l1_attribute.source_hash(),
            B256::from(hex!("0586c503340591999b8b38bc9834bb16aec7d5bc00eb5587ab139c9ddab81977"))
        );

        Ok(())
    }

    // cast keccak $(cast concat-hex 0x0000000000000000000000000000000000000000000000000000000000000000 $(cast keccak $(cast concat-hex 0xc00e5d67c2755389aded7d8b151cbd5bcdf7ed275ad5e028b664880fc7581c77 0x0000000000000000000000000000000000000000000000000000000000000005)))
    // # 0x4c75c1d40d73ad30002aa6a91e24bb68c7bac1e723c2e1b073f889e227df4071
    #[test]
    fn user_deposited_source_hash_works() -> anyhow::Result<()> {
        let l1_attribute = UserDeposited {
            bridge_sender: Default::default(),
            bridge_target: Default::default(),
            mint: Default::default(),
            value: Default::default(),
            gas: 0,
            is_creation: false,
            l1_block_num: 0,
            l1_block_hash: hex!("c00e5d67c2755389aded7d8b151cbd5bcdf7ed275ad5e028b664880fc7581c77")
                .into(),
            log_index: 5,
            deposit_type: DepositType::ETH(EthDepositData {
                from: Default::default(),
                to: Default::default(),
                amount: Default::default(),
                extra_data: vec![],
            }),
        };

        assert_eq!(
            l1_attribute.source_hash(),
            B256::from(hex!("4c75c1d40d73ad30002aa6a91e24bb68c7bac1e723c2e1b073f889e227df4071"))
        );

        Ok(())
    }

    #[test]
    fn test_undo_l1_to_l2_alias_0() -> anyhow::Result<()> {
        let address0: Address = "0xCc248cE37870443D5B2B02a36619d3478738F207".parse().unwrap();
        let target0: Address = "0xbB138cE37870443d5b2B02a36619D3478738E0f6".parse().unwrap();
        assert_eq!(undo_l1_to_l2_alias(address0), target0);
        Ok(())
    }

    #[test]
    fn test_undo_l1_to_l2_alias_1() -> anyhow::Result<()> {
        let address0: Address = "0x0374D6C1706601dC9a69735275bb5e7f87B37986".parse().unwrap();
        let target0: Address = "0xF263d6c1706601DC9A69735275Bb5e7f87B36875".parse().unwrap();
        assert_eq!(undo_l1_to_l2_alias(address0), target0);
        Ok(())
    }

    #[test]
    fn test_apply_and_undo_alias_0() -> anyhow::Result<()> {
        let source_addr = "0x0000000000000000000000000000000000000001".parse().unwrap();
        let dest_addr = apply_l1_to_l2_alias(undo_l1_to_l2_alias(source_addr));
        assert_eq!(source_addr, dest_addr);
        Ok(())
    }

    #[test]
    fn test_apply_and_undo_alias_1() -> anyhow::Result<()> {
        let source_addr = "0xfF000000000000000000000000000000000000FF".parse().unwrap();
        let dest_addr = undo_l1_to_l2_alias(apply_l1_to_l2_alias(source_addr));
        assert_eq!(source_addr, dest_addr);
        Ok(())
    }

    #[test]
    fn test_apply_and_undo_alias_2() -> anyhow::Result<()> {
        let source_addr = "0xF263d6c1706601DC9A69735275Bb5e7f87B36875".parse().unwrap();
        let dest_addr = undo_l1_to_l2_alias(apply_l1_to_l2_alias(source_addr));
        assert_eq!(source_addr, dest_addr);
        Ok(())
    }
}
