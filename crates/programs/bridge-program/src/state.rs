//! State transition types

use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use ethabi::Address;
use ethnum::U256;
use serde::{Serialize, Serializer};
use solana_program::{
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack, Sealed},
    pubkey::Pubkey,
};

/// WithdrawalCounter data.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WithdrawalCounter {
    /// counter
    pub counter: u64,
}
impl Sealed for WithdrawalCounter {}
impl IsInitialized for WithdrawalCounter {
    fn is_initialized(&self) -> bool {
        true
    }
}
impl Pack for WithdrawalCounter {
    const LEN: usize = 8;

    fn pack_into_slice(&self, dst: &mut [u8]) {
        let dst = array_mut_ref![dst, 0, 8];
        let (counter_dst,) = mut_array_refs![dst, 8];
        let &WithdrawalCounter { counter } = self;
        *counter_dst = counter.to_le_bytes();
    }

    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        let src = array_ref![src, 0, 8];
        let (counter,) = array_refs![src, 8];
        let counter = u64::from_le_bytes(*counter);

        Ok(WithdrawalCounter { counter })
    }
}

/// ETH withdrawal transaction calldata length
pub const ETH_WITHDRAWAL_TRANSACTION_CALLDATA_LEN: usize = 420;

/// ETHWithdrawalL1Calldata data.
#[derive(Copy, Clone, Debug, Serialize, PartialEq)]
pub struct ETHWithdrawalCalldata {
    /// data
    #[serde(serialize_with = "serialize_eth_withdrawal_calldata_array")]
    pub data: [u8; ETH_WITHDRAWAL_TRANSACTION_CALLDATA_LEN],
}

fn serialize_eth_withdrawal_calldata_array<S>(
    array: &[u8; ETH_WITHDRAWAL_TRANSACTION_CALLDATA_LEN],
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_bytes(array)
}

/// ETHWithdrawalTransaction data.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ETHWithdrawalTransaction {
    /// withdrawal counter
    pub nonce: U256,
    /// user who wants to withdraw
    pub sender: Pubkey,
    /// user address in L1
    pub target: Address,
    /// withdraw amount in lamports
    pub value: U256,
    /// gas limit in L1
    pub gas_limit: U256,
    /// calldata used by smart contract in L1
    pub data: ETHWithdrawalCalldata,
}
impl Sealed for ETHWithdrawalTransaction {}
impl IsInitialized for ETHWithdrawalTransaction {
    fn is_initialized(&self) -> bool {
        true
    }
}
impl Pack for ETHWithdrawalTransaction {
    const LEN: usize = 148 + ETH_WITHDRAWAL_TRANSACTION_CALLDATA_LEN;

    fn pack_into_slice(&self, dst: &mut [u8]) {
        dst[0..16].copy_from_slice(&self.nonce.high().to_be_bytes());
        dst[16..32].copy_from_slice(&self.nonce.low().to_be_bytes());
        dst[32..64].copy_from_slice(&self.sender.to_bytes());
        dst[64..84].copy_from_slice(&self.target.to_fixed_bytes());
        dst[84..100].copy_from_slice(&self.value.high().to_be_bytes());
        dst[100..116].copy_from_slice(&self.value.low().to_be_bytes());
        dst[116..132].copy_from_slice(&self.gas_limit.high().to_be_bytes());
        dst[132..148].copy_from_slice(&self.gas_limit.low().to_be_bytes());
        dst[148..(148 + ETH_WITHDRAWAL_TRANSACTION_CALLDATA_LEN)].copy_from_slice(&self.data.data);
    }

    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        let nonce_high = u128::from_be_bytes(
            src[..16].try_into().map_err(|_| ProgramError::InvalidAccountData)?,
        );
        let nonce_low = u128::from_be_bytes(
            src[16..32].try_into().map_err(|_| ProgramError::InvalidAccountData)?,
        );
        let nonce = U256::from_words(nonce_high, nonce_low);

        let sender =
            Pubkey::try_from(&src[32..64]).map_err(|_| ProgramError::InvalidAccountData)?;
        let target: Address = Address::from_slice(&src[64..84]);

        let value_high = u128::from_be_bytes(
            src[84..100].try_into().map_err(|_| ProgramError::InvalidAccountData)?,
        );
        let value_low = u128::from_be_bytes(
            src[100..116].try_into().map_err(|_| ProgramError::InvalidAccountData)?,
        );
        let value = U256::from_words(value_high, value_low);

        let gas_limit_high = u128::from_be_bytes(
            src[116..132].try_into().map_err(|_| ProgramError::InvalidAccountData)?,
        );
        let gas_limit_low = u128::from_be_bytes(
            src[132..148].try_into().map_err(|_| ProgramError::InvalidAccountData)?,
        );
        let gas_limit = U256::from_words(gas_limit_high, gas_limit_low);

        let data_raw: [u8; ETH_WITHDRAWAL_TRANSACTION_CALLDATA_LEN] = src
            [148..(148 + ETH_WITHDRAWAL_TRANSACTION_CALLDATA_LEN)]
            .try_into()
            .map_err(|_| ProgramError::InvalidAccountData)?;
        let data = ETHWithdrawalCalldata { data: data_raw };

        Ok(Self { nonce, sender, target, value, gas_limit, data })
    }
}

/// SPL withdrawal transaction calldata length
pub const SPL_WITHDRAWAL_TRANSACTION_CALLDATA_LEN: usize = 484;

/// SPLWithdrawalL1Calldata data.
#[derive(Copy, Clone, Debug, Serialize, PartialEq)]
pub struct SPLWithdrawalCalldata {
    /// data
    #[serde(serialize_with = "serialize_spl_withdrawal_calldata_array")]
    pub data: [u8; SPL_WITHDRAWAL_TRANSACTION_CALLDATA_LEN],
}

fn serialize_spl_withdrawal_calldata_array<S>(
    array: &[u8; SPL_WITHDRAWAL_TRANSACTION_CALLDATA_LEN],
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_bytes(array)
}

/// SPLWithdrawalTransaction data.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SPLWithdrawalTransaction {
    /// withdrawal counter
    pub nonce: U256,
    /// user who wants to withdraw
    pub sender: Pubkey,
    /// user address in L1
    pub target: Address,
    /// withdraw amount in lamports
    pub value: U256,
    /// gas limit in L1
    pub gas_limit: U256,
    /// calldata used by smart contract in L1
    pub data: SPLWithdrawalCalldata,
}
impl Sealed for SPLWithdrawalTransaction {}
impl IsInitialized for SPLWithdrawalTransaction {
    fn is_initialized(&self) -> bool {
        true
    }
}
impl Pack for SPLWithdrawalTransaction {
    const LEN: usize = 148 + SPL_WITHDRAWAL_TRANSACTION_CALLDATA_LEN;

    fn pack_into_slice(&self, dst: &mut [u8]) {
        dst[0..16].copy_from_slice(&self.nonce.high().to_be_bytes());
        dst[16..32].copy_from_slice(&self.nonce.low().to_be_bytes());
        dst[32..64].copy_from_slice(&self.sender.to_bytes());
        dst[64..84].copy_from_slice(&self.target.to_fixed_bytes());
        dst[84..100].copy_from_slice(&self.value.high().to_be_bytes());
        dst[100..116].copy_from_slice(&self.value.low().to_be_bytes());
        dst[116..132].copy_from_slice(&self.gas_limit.high().to_be_bytes());
        dst[132..148].copy_from_slice(&self.gas_limit.low().to_be_bytes());
        dst[148..(148 + SPL_WITHDRAWAL_TRANSACTION_CALLDATA_LEN)].copy_from_slice(&self.data.data);
    }

    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        let nonce_high = u128::from_be_bytes(
            src[..16].try_into().map_err(|_| ProgramError::InvalidAccountData)?,
        );
        let nonce_low = u128::from_be_bytes(
            src[16..32].try_into().map_err(|_| ProgramError::InvalidAccountData)?,
        );
        let nonce = U256::from_words(nonce_high, nonce_low);

        let sender =
            Pubkey::try_from(&src[32..64]).map_err(|_| ProgramError::InvalidAccountData)?;
        let target: Address = Address::from_slice(&src[64..84]);

        let value_high = u128::from_be_bytes(
            src[84..100].try_into().map_err(|_| ProgramError::InvalidAccountData)?,
        );
        let value_low = u128::from_be_bytes(
            src[100..116].try_into().map_err(|_| ProgramError::InvalidAccountData)?,
        );
        let value = U256::from_words(value_high, value_low);

        let gas_limit_high = u128::from_be_bytes(
            src[116..132].try_into().map_err(|_| ProgramError::InvalidAccountData)?,
        );
        let gas_limit_low = u128::from_be_bytes(
            src[132..148].try_into().map_err(|_| ProgramError::InvalidAccountData)?,
        );
        let gas_limit = U256::from_words(gas_limit_high, gas_limit_low);

        let data_raw: [u8; SPL_WITHDRAWAL_TRANSACTION_CALLDATA_LEN] = src
            [148..(148 + SPL_WITHDRAWAL_TRANSACTION_CALLDATA_LEN)]
            .try_into()
            .map_err(|_| ProgramError::InvalidAccountData)?;
        let data = SPLWithdrawalCalldata { data: data_raw };

        Ok(Self { nonce, sender, target, value, gas_limit, data })
    }
}

/// BridgeConfig data.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BridgeConfig {
    /// l1 cross domain messenger address on L1
    pub l1_cross_domain_messenger: Address,
    /// l1 standard bridge address on L1
    pub l1_standard_bridge: Address,
}
impl Sealed for BridgeConfig {}
impl IsInitialized for BridgeConfig {
    fn is_initialized(&self) -> bool {
        true
    }
}
impl Pack for BridgeConfig {
    const LEN: usize = 40;

    fn pack_into_slice(&self, dst: &mut [u8]) {
        dst[..20].copy_from_slice(&self.l1_cross_domain_messenger.to_fixed_bytes());
        dst[20..40].copy_from_slice(&self.l1_standard_bridge.to_fixed_bytes());
    }

    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        let l1_cross_domain_messenger: Address = Address::from_slice(&src[..20]);
        let l1_standard_bridge: Address = Address::from_slice(&src[20..40]);

        Ok(Self { l1_cross_domain_messenger, l1_standard_bridge })
    }
}

/// BridgeOwner data.
pub struct BridgeOwner {
    /// admin pubkey
    pub admin: Pubkey,
}
impl Sealed for BridgeOwner {}
impl IsInitialized for BridgeOwner {
    fn is_initialized(&self) -> bool {
        true
    }
}
impl Pack for BridgeOwner {
    const LEN: usize = 32;

    fn pack_into_slice(&self, dst: &mut [u8]) {
        dst.copy_from_slice(&self.admin.to_bytes());
    }

    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        let admin = Pubkey::try_from(src).map_err(|_| ProgramError::InvalidAccountData)?;
        Ok(Self { admin })
    }
}
