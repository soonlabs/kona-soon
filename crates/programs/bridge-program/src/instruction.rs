//! Instruction types

use mpl_token_metadata::programs::MPL_TOKEN_METADATA_ID;
use num_traits::ToBytes;

use crate::error::BridgeError;
use ethabi::Address;
use solana_program::{
    instruction::{AccountMeta, Instruction},
    program_error::ProgramError,
    pubkey::Pubkey,
};
use spl_associated_token_account::get_associated_token_address;
use std::convert::TryInto;

use crate::pda::{
    bridge_owner_pubkey, config_pubkey, spl_token_metadata_pubkey, spl_token_mint_pubkey,
    spl_token_owner_pubkey, user_withdrawal_counter_pubkey, vault_pubkey,
    withdrawal_transaction_pubkey,
};

const U64_BYTES: usize = 8;
const U128_BYTES: usize = 16;

/// Instructions supported by the svm deposit bridge program.
#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub enum BridgeInstruction<'a> {
    /// Create a global vault account to transfer SOL to depositor.
    ///
    /// Accounts expected by this instruction:
    ///
    ///   0. `[]` System program.
    ///   1. `[]` Rent sysvar.
    ///   2. `[writable]` Vault counter account to be created.
    ///   3. `[signer]` Payer.
    #[deprecated(note = "This instruction is no longer used.")]
    CreateVaultAccount,

    /// Transfer SOL to depositor and store the deposit info.
    ///
    /// Accounts expected by this instruction:
    ///
    ///   0. `[writable]` Deposit info account.
    ///   1. `[]` Depositor account.
    ///   2. `[writable]` Vault account.
    DepositETH {
        /// Address of the sender
        from: Address,
        /// Address of the recipient, or None if the transaction is a contract creation
        to: Address,
        /// ETH value to mint on L2
        mint: u128,
        /// ETH value to send to the recipient
        value: u128,
        /// Gas limit for the L2 transaction
        gas: u64,
        /// If this is a contract creation
        is_creation: bool,
        /// The L1 block number this was submitted in.
        l1_block_num: u64,
        /// The L1 block hash this was submitted in.
        l1_block_hash: [u8; 32],
        /// The index of the emitted deposit event log in the L1 block.
        log_index: u128,
        /// The data decoded from opaque data, 0: ETH, 1: ERC20
        deposit_type: u8,
        /// The address of the depositor on L1
        deposit_from: Address,
        /// The address of the depositor on L2
        deposit_to: Pubkey,
        /// The deposit amount
        deposit_amount: u128,
        /// The deposit extra data
        deposit_extra_data: [u8; 32],
        /// The deposit l2 contract
        deposit_l2_contract: Pubkey,
        /// The deposit l1 contract
        deposit_l1_contract: Address,
    },

    /// Create a global `counter` as seed for the WithdrawalTransaction account.
    ///
    /// Accounts expected by this instruction:
    ///
    ///   0. `[]` System program.
    ///   1. `[]` Rent sysvar.
    ///   2. `[writable]` Withdrawal counter account to be created.
    ///   3. `[signer]` Payer.
    #[deprecated(note = "This instruction is no longer used")]
    CreateWithdrawalCounterAccount,

    /// Withdraw ETH & create a withdrawal transaction account to store the withdrawal transaction
    /// info.
    ///
    /// Accounts expected by this instruction:
    ///
    ///   0. `[]` System program.
    ///   1. `[]` Rent sysvar.
    ///   2. `[writable]` Withdrawal counter account.
    ///   3. `[writable]` Withdrawal transaction account to be created.
    ///   4. `[signer]` Payer.
    WithdrawETH {
        /// user address on L1
        target: Address,
        /// withdraw amount in lamports
        value: u128,
        /// gas limit of L1
        gas_limit: u128,
    },

    /// Create SPL token for ERC20 on L1 bridge to Soon.
    ///
    /// Accounts expected by this instruction:
    ///
    ///   0. `[]` System program.
    ///   1. `[]` Rent sysvar.
    ///   2. `[writable]` Withdrawal counter account.
    ///   3. `[writable]` Withdrawal transaction account to be created.
    ///   4. `[signer]` Payer.
    CreateSPL {
        /// remote ERC20 token address on L1
        remote_token: Address,
        /// name of SPL token
        name: &'a str,
        /// symbol of SPL token
        symbol: &'a str,
        /// uri of SPL token
        uri: &'a str,
        /// decimals of SPL token
        decimals: u8,
    },

    /// Create SPL token for ERC20 on L1 bridge to SOON.
    ///
    /// Accounts expected by this instruction:
    ///
    ///   0. `[]` System program.
    ///   1. `[]` Rent sysvar.
    ///   2. `[writable]` Withdrawal counter account.
    ///   3. `[writable]` Withdrawal transaction account to be created.
    ///   4. `[signer]` Payer.
    DepositERC20 {
        /// local SPL token pubkey on SOON
        local_token: Pubkey,
        /// remote ERC20 token address on L1
        remote_token: Address,
        /// user address on L1
        from: Address,
        /// user pubkey on SOON
        to: Pubkey,
        /// deposit amount
        amount: u128,
    },

    /// Create SPL token for ERC20 on L1 bridge to Soon.
    ///
    /// Accounts expected by this instruction:
    ///
    ///   0. `[]` System program.
    ///   1. `[]` Rent sysvar.
    ///   2. `[writable]` Withdrawal counter account.
    ///   3. `[writable]` Withdrawal transaction account to be created.
    ///   4. `[signer]` Payer.
    WithdrawSPL {
        /// remote ERC20 token address on L1
        remote_token: Address,
        /// user address on L1
        to: Address, // should add bridge address on L1
        /// withdrawal amount
        amount: u128,
        /// gas limit of L1
        gas_limit: u128,
    },

    /// Create bridge config account
    ///
    /// Accounts expected by this instruction:
    ///
    ///   0. `[]` System program.
    ///   1. `[]` Rent sysvar.
    ///   3. `[writable]` Bridge config account to be created.
    ///   4. `[signer]` Payer.
    #[deprecated(note = "This instruction is no longer used")]
    CreateBridgeConfigAccount {
        /// l1 cross domain messenger address on L1
        l1_cross_domain_messenger: Address,
        /// l1 standard bridge address on L1
        l1_standard_bridge: Address,
    },

    /// Change the admin pubkey of the bridge
    ///
    /// Accounts expected by this instruction:
    ChangeBridgeAdmin {
        /// New admin pubkey
        new_admin: Pubkey,
    },

    /// Create a `counter` + user_key as seed of the WithdrawalTransaction account for each user.
    ///
    /// Accounts expected by this instruction:
    ///
    ///   0. `[]` System program.
    ///   1. `[]` Rent sysvar.
    ///   2. `[writable]` User withdrawal counter account to be created.
    ///   3. `[signer]` Payer.
    CreateUserWithdrawalCounterAccount,

    /// Create SPL token for ERC20 on L1 bridge to Soon.
    ///
    /// Accounts expected by this instruction:
    ///
    ///   0. `[]` System program.
    ///   1. `[]` Rent sysvar.
    ///   2. `[writable]` Withdrawal counter account.
    ///   3. `[writable]` Withdrawal transaction account to be created.
    ///   4. `[signer]` Payer.
    UpdateSPLMetadata {
        /// remote ERC20 token address on L1
        remote_token: Address,
        /// name of SPL token
        name: &'a str,
        /// symbol of SPL token
        symbol: &'a str,
        /// uri of SPL token
        uri: &'a str,
    },
}

impl<'a> BridgeInstruction<'a> {
    /// Returns true if the instruction is a derived instruction.
    pub fn is_derived(&self) -> bool {
        matches!(
            self,
            BridgeInstruction::DepositETH { .. } | BridgeInstruction::DepositERC20 { .. }
        )
    }

    /// Unpacks a byte buffer into a SvmWithdrawBridgeInstruction
    pub fn unpack(input: &'a [u8]) -> Result<Self, ProgramError> {
        use BridgeError::InvalidInstruction;

        let (&tag, rest) = input.split_first().ok_or(InvalidInstruction)?;
        Ok(match tag {
            #[allow(deprecated)]
            0 => Self::CreateVaultAccount,
            1 => {
                let (from, rest) = Self::unpack_address(rest)?;
                let (to, rest) = Self::unpack_address(rest)?;
                let (mint, rest) = Self::unpack_u128(rest)?;
                let (value, rest) = Self::unpack_u128(rest)?;
                let (gas, rest) = Self::unpack_u64(rest)?;
                let (is_creation, rest) = Self::unpack_bool(rest)?;
                let (l1_block_num, rest) = Self::unpack_u64(rest)?;
                let (l1_block_hash, rest) = Self::unpack_bytes32(rest)?;
                let (log_index, rest) = Self::unpack_u128(rest)?;
                let (deposit_type, rest) = Self::unpack_u8(rest)?;
                let (deposit_from, rest) = Self::unpack_address(rest)?;
                let (deposit_to, rest) = Self::unpack_pubkey(rest)?;
                let (deposit_amount, rest) = Self::unpack_u128(rest)?;
                let (deposit_extra_data, rest) = Self::unpack_bytes32(rest)?;
                let (deposit_l2_contract, rest) = Self::unpack_pubkey(rest)?;
                let (deposit_l1_contract, _rest) = Self::unpack_address(rest)?;
                Self::DepositETH {
                    from,
                    to,
                    mint,
                    value,
                    gas,
                    is_creation,
                    l1_block_num,
                    l1_block_hash,
                    log_index,
                    deposit_type,
                    deposit_from,
                    deposit_to,
                    deposit_amount,
                    deposit_extra_data,
                    deposit_l2_contract,
                    deposit_l1_contract,
                }
            }
            #[allow(deprecated)]
            2 => Self::CreateWithdrawalCounterAccount,
            3 => {
                let (target, rest) = Self::unpack_address(rest)?;
                let (value, rest) = Self::unpack_u128(rest)?;
                let (gas_limit, _rest) = Self::unpack_u128(rest)?;
                Self::WithdrawETH { target, value, gas_limit }
            }
            4 => {
                let (remote_token, rest) = Self::unpack_address(rest)?;
                let (name, rest) = Self::unpack_str(rest)?;
                let (symbol, rest) = Self::unpack_str(rest)?;
                let (uri, rest) = Self::unpack_str(rest)?;
                let (decimals, _rest) = Self::unpack_u8(rest)?;
                Self::CreateSPL { remote_token, name, symbol, uri, decimals }
            }
            5 => {
                let (local_token, rest) = Self::unpack_pubkey(rest)?;
                let (remote_token, rest) = Self::unpack_address(rest)?;
                let (from, rest) = Self::unpack_address(rest)?;
                let (to, rest) = Self::unpack_pubkey(rest)?;
                let (amount, _rest) = Self::unpack_u128(rest)?;
                Self::DepositERC20 { local_token, remote_token, from, to, amount }
            }
            6 => {
                let (remote_token, rest) = Self::unpack_address(rest)?;
                let (to, rest) = Self::unpack_address(rest)?;
                let (amount, rest) = Self::unpack_u128(rest)?;
                let (gas_limit, _rest) = Self::unpack_u128(rest)?;
                Self::WithdrawSPL { remote_token, to, amount, gas_limit }
            }
            7 => {
                let (l1_cross_domain_messenger, rest) = Self::unpack_address(rest)?;
                let (l1_standard_bridge, _) = Self::unpack_address(rest)?;
                #[allow(deprecated)]
                Self::CreateBridgeConfigAccount { l1_cross_domain_messenger, l1_standard_bridge }
            }
            8 => {
                let (new_admin, _) = Self::unpack_pubkey(rest)?;
                Self::ChangeBridgeAdmin { new_admin }
            }
            9 => Self::CreateUserWithdrawalCounterAccount,
            10 => {
                let (remote_token, rest) = Self::unpack_address(rest)?;
                let (name, rest) = Self::unpack_str(rest)?;
                let (symbol, rest) = Self::unpack_str(rest)?;
                let (uri, _) = Self::unpack_str(rest)?;
                Self::UpdateSPLMetadata { remote_token, name, symbol, uri }
            }
            _ => return Err(InvalidInstruction.into()),
        })
    }

    fn unpack_address(input: &[u8]) -> Result<(Address, &[u8]), ProgramError> {
        if input.len() >= 20 {
            let (data, rest) = input.split_at(20);
            let address: Address = Address::from_slice(data);
            Ok((address, rest))
        } else {
            Err(BridgeError::InvalidInstruction.into())
        }
    }

    fn unpack_pubkey(input: &[u8]) -> Result<(Pubkey, &[u8]), ProgramError> {
        if input.len() >= 32 {
            let (key, rest) = input.split_at(32);
            let pk = Pubkey::try_from(key).map_err(|_| BridgeError::InvalidInstruction)?;
            Ok((pk, rest))
        } else {
            Err(BridgeError::InvalidInstruction.into())
        }
    }

    fn unpack_u128(input: &[u8]) -> Result<(u128, &[u8]), ProgramError> {
        let value = input
            .get(..U128_BYTES)
            .and_then(|slice| slice.try_into().ok())
            .map(u128::from_le_bytes)
            .ok_or(BridgeError::InvalidInstruction)?;
        Ok((value, &input[U128_BYTES..]))
    }

    fn unpack_u64(input: &[u8]) -> Result<(u64, &[u8]), ProgramError> {
        let value = input
            .get(..U64_BYTES)
            .and_then(|slice| slice.try_into().ok())
            .map(u64::from_le_bytes)
            .ok_or(BridgeError::InvalidInstruction)?;
        Ok((value, &input[U64_BYTES..]))
    }

    fn unpack_u8(input: &[u8]) -> Result<(u8, &[u8]), ProgramError> {
        let (&value, rest) = input.split_first().ok_or(BridgeError::InvalidInstruction)?;
        Ok((value, rest))
    }

    fn unpack_bool(input: &[u8]) -> Result<(bool, &[u8]), ProgramError> {
        let byte_value = input.first().ok_or(BridgeError::InvalidInstruction)?;
        let value = match *byte_value {
            0 => false,
            1 => true,
            _ => return Err(BridgeError::InvalidInstruction.into()),
        };
        Ok((value, &input[1..]))
    }

    fn unpack_bytes32(input: &[u8]) -> Result<([u8; 32], &[u8]), ProgramError> {
        if input.len() >= 32 {
            let (data, rest) = input.split_at(32);
            let address: [u8; 32] =
                <[u8; 32]>::try_from(data).map_err(|_| BridgeError::InvalidInstruction)?;
            Ok((address, rest))
        } else {
            Err(BridgeError::InvalidInstruction.into())
        }
    }

    fn unpack_str(input: &[u8]) -> Result<(&str, &[u8]), ProgramError> {
        let length = *input.first().ok_or(BridgeError::InvalidInstruction)? as usize;
        if input.len() < 1 + length {
            return Err(BridgeError::InvalidInstruction.into());
        }

        let (str_bytes, rest) = input[1..].split_at(length);
        let result = std::str::from_utf8(str_bytes).map_err(|_| BridgeError::InvalidInstruction)?;

        Ok((result, rest))
    }

    fn pack_str(s: &str) -> Result<Vec<u8>, BridgeError> {
        if s.len() > u8::MAX as usize {
            return Err(BridgeError::StringLengthTooLong);
        }

        let mut buf = Vec::with_capacity(s.len() + 1);
        buf.push(s.len() as u8);
        buf.extend_from_slice(s.as_bytes());
        Ok(buf)
    }

    /// Packs a BridgeInstruction into a byte buffer.
    pub fn pack(&self) -> Result<Vec<u8>, BridgeError> {
        let mut buf = Vec::with_capacity(std::mem::size_of::<Self>());
        match self {
            #[allow(deprecated)]
            BridgeInstruction::CreateVaultAccount => {
                buf.push(0);
            }
            BridgeInstruction::DepositETH {
                from,
                to,
                mint,
                value,
                gas,
                is_creation,
                l1_block_num,
                l1_block_hash,
                log_index,
                deposit_type,
                deposit_from,
                deposit_to,
                deposit_amount,
                deposit_extra_data,
                deposit_l2_contract,
                deposit_l1_contract,
            } => {
                buf.push(1);
                buf.extend_from_slice(from.as_bytes());
                buf.extend_from_slice(to.as_bytes());
                buf.extend_from_slice(&mint.to_le_bytes());
                buf.extend_from_slice(&value.to_le_bytes());
                buf.extend_from_slice(&gas.to_le_bytes());
                buf.push(*is_creation as u8);
                buf.extend_from_slice(&l1_block_num.to_le_bytes());
                buf.extend_from_slice(l1_block_hash);
                buf.extend_from_slice(&log_index.to_le_bytes());
                buf.push(*deposit_type);
                buf.extend_from_slice(deposit_from.as_bytes());
                buf.extend_from_slice(deposit_to.as_ref());
                buf.extend_from_slice(&deposit_amount.to_le_bytes());
                buf.extend_from_slice(deposit_extra_data);
                buf.extend_from_slice(deposit_l2_contract.as_ref());
                buf.extend_from_slice(deposit_l1_contract.as_bytes());
            }
            #[allow(deprecated)]
            BridgeInstruction::CreateWithdrawalCounterAccount => {
                buf.push(2);
            }
            BridgeInstruction::WithdrawETH { target, value, gas_limit } => {
                buf.push(3);
                buf.extend_from_slice(target.as_bytes());
                buf.extend_from_slice(&value.to_le_bytes());
                buf.extend_from_slice(&gas_limit.to_le_bytes());
            }
            BridgeInstruction::CreateSPL { remote_token, name, symbol, uri, decimals } => {
                buf.push(4);
                buf.extend_from_slice(remote_token.as_bytes());
                buf.extend_from_slice(Self::pack_str(name)?.as_ref());
                buf.extend_from_slice(Self::pack_str(symbol)?.as_ref());
                buf.extend_from_slice(Self::pack_str(uri)?.as_ref());
                buf.push(*decimals);
            }
            BridgeInstruction::DepositERC20 { local_token, remote_token, from, to, amount } => {
                buf.push(5);
                buf.extend_from_slice(local_token.as_ref());
                buf.extend_from_slice(remote_token.as_bytes());
                buf.extend_from_slice(from.as_bytes());
                buf.extend_from_slice(to.as_ref());
                buf.extend_from_slice(&amount.to_le_bytes());
            }
            BridgeInstruction::WithdrawSPL { remote_token, to, amount, gas_limit } => {
                buf.push(6);
                buf.extend_from_slice(remote_token.as_bytes());
                buf.extend_from_slice(to.as_bytes());
                buf.extend_from_slice(&amount.to_le_bytes());
                buf.extend_from_slice(&gas_limit.to_le_bytes());
            }
            #[allow(deprecated)]
            BridgeInstruction::CreateBridgeConfigAccount {
                l1_cross_domain_messenger,
                l1_standard_bridge,
            } => {
                buf.push(7);
                buf.extend_from_slice(l1_cross_domain_messenger.as_bytes());
                buf.extend_from_slice(l1_standard_bridge.as_bytes());
            }
            BridgeInstruction::ChangeBridgeAdmin { new_admin } => {
                buf.push(8);
                buf.extend_from_slice(new_admin.as_ref());
            }
            BridgeInstruction::CreateUserWithdrawalCounterAccount => {
                buf.push(9);
            }
            BridgeInstruction::UpdateSPLMetadata { remote_token, name, symbol, uri } => {
                buf.push(10);
                buf.extend_from_slice(remote_token.as_bytes());
                buf.extend_from_slice(Self::pack_str(name)?.as_ref());
                buf.extend_from_slice(Self::pack_str(symbol)?.as_ref());
                buf.extend_from_slice(Self::pack_str(uri)?.as_ref());
            }
        }
        Ok(buf)
    }
}

/// Creates a `CreateVaultAccount` instruction.
#[deprecated(note = "This instruction is no longer used.")]
pub fn create_vault_account(payer: Pubkey) -> Instruction {
    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new_readonly(solana_program::system_program::ID, false),
            AccountMeta::new_readonly(solana_program::sysvar::rent::ID, false),
            AccountMeta::new(vault_pubkey(), false),
            AccountMeta::new_readonly(payer, true),
        ],
        #[allow(deprecated)]
        data: BridgeInstruction::CreateVaultAccount.pack().unwrap(),
    }
}

/// Creates a `DepositETH` instruction.
#[allow(clippy::too_many_arguments)]
pub fn deposit_eth(
    from: Address,
    to: Address,
    mint: u128,
    value: u128,
    gas: u64,
    is_creation: bool,
    l1_block_num: u64,
    l1_block_hash: [u8; 32],
    log_index: u128,
    deposit_from: Address,
    deposit_to: Pubkey,
    deposit_amount: u128,
    deposit_extra_data: [u8; 32],
) -> Instruction {
    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new_readonly(solana_program::system_program::id(), false),
            AccountMeta::new(deposit_to, false),
            AccountMeta::new(vault_pubkey(), false),
            AccountMeta::new_readonly(crate::NO_SIG_TX_PAYER, true),
        ],
        data: BridgeInstruction::DepositETH {
            from,
            to,
            mint,
            value,
            gas,
            is_creation,
            l1_block_num,
            l1_block_hash,
            log_index,
            deposit_type: 0,
            deposit_from,
            deposit_to,
            deposit_amount,
            deposit_extra_data,
            deposit_l2_contract: Pubkey::default(),
            deposit_l1_contract: Address::zero(),
        }
        .pack()
        .unwrap(),
    }
}

/// Creates a `DepositERC20` instruction.
pub fn deposit_erc20(
    local_token: Pubkey,
    remote_token: Address,
    from: Address,
    to: Pubkey,
    amount: u128,
) -> Instruction {
    let spl_token_mint_key = spl_token_mint_pubkey(&remote_token);
    let depositor_associated_token_key = get_associated_token_address(&to, &spl_token_mint_key);

    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new_readonly(solana_program::system_program::id(), false),
            AccountMeta::new_readonly(spl_token::id(), false),
            AccountMeta::new_readonly(spl_associated_token_account::id(), false),
            AccountMeta::new_readonly(spl_token_owner_pubkey(&remote_token), false),
            AccountMeta::new(spl_token_mint_key, false),
            AccountMeta::new_readonly(to, true),
            AccountMeta::new(depositor_associated_token_key, false),
            AccountMeta::new(vault_pubkey(), false),
            AccountMeta::new_readonly(crate::NO_SIG_TX_PAYER, true),
        ],
        data: BridgeInstruction::DepositERC20 { local_token, remote_token, from, to, amount }
            .pack()
            .unwrap(),
    }
}

/// Creates a `CreateSPL` instruction.
pub fn create_spl(
    remote_token: Address,
    admin: Pubkey,
    name: &str,
    symbol: &str,
    uri: &str,
    decimals: u8,
) -> Result<Instruction, BridgeError> {
    Ok(Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new_readonly(solana_program::system_program::id(), false),
            AccountMeta::new_readonly(solana_program::sysvar::rent::id(), false),
            AccountMeta::new_readonly(spl_token::id(), false),
            AccountMeta::new_readonly(spl_token_owner_pubkey(&remote_token), false),
            AccountMeta::new(spl_token_mint_pubkey(&remote_token), false),
            AccountMeta::new(vault_pubkey(), false),
            AccountMeta::new_readonly(bridge_owner_pubkey(), false),
            AccountMeta::new_readonly(admin, true),
            AccountMeta::new_readonly(MPL_TOKEN_METADATA_ID, false),
            AccountMeta::new(spl_token_metadata_pubkey(&remote_token), false),
        ],
        data: BridgeInstruction::CreateSPL { remote_token, name, symbol, uri, decimals }.pack()?,
    })
}

/// Creates a `WithdrawETH` instruction.
pub fn withdraw_eth(
    target: Address,
    payer: Pubkey,
    value: u128,
    gas_limit: u128,
    counter: u64,
) -> Instruction {
    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new_readonly(solana_program::system_program::id(), false),
            AccountMeta::new_readonly(solana_program::sysvar::rent::id(), false),
            AccountMeta::new(user_withdrawal_counter_pubkey(&payer), false),
            AccountMeta::new(withdrawal_transaction_pubkey(&payer, counter), false),
            AccountMeta::new(vault_pubkey(), false),
            AccountMeta::new_readonly(config_pubkey(), false),
            AccountMeta::new(payer, true),
        ],
        data: BridgeInstruction::WithdrawETH { target, value, gas_limit }.pack().unwrap(),
    }
}

/// Creates a `CreateBridgeConfigAccount` instruction.
pub fn change_bridge_admin(admin: Pubkey, new_admin: Pubkey) -> Instruction {
    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new(bridge_owner_pubkey(), false),
            AccountMeta::new_readonly(admin, true),
        ],
        data: BridgeInstruction::ChangeBridgeAdmin { new_admin }.pack().unwrap(),
    }
}

/// Creates a `WithdrawSPL` instruction.
pub fn withdraw_spl(
    remote_token: Address,
    payer: Pubkey,
    to: Address,
    amount: u128,
    gas_limit: u128,
    counter: u64,
) -> Instruction {
    let spl_token_mint_key = spl_token_mint_pubkey(&remote_token);
    let from_associated_token_key = get_associated_token_address(&payer, &spl_token_mint_key);

    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new_readonly(solana_program::system_program::id(), false),
            AccountMeta::new_readonly(solana_program::sysvar::rent::id(), false),
            AccountMeta::new_readonly(spl_token::id(), false),
            AccountMeta::new(user_withdrawal_counter_pubkey(&payer), false),
            AccountMeta::new(withdrawal_transaction_pubkey(&payer, counter), false),
            AccountMeta::new_readonly(spl_token_owner_pubkey(&remote_token), false),
            AccountMeta::new(spl_token_mint_key, false),
            AccountMeta::new(from_associated_token_key, false),
            AccountMeta::new_readonly(config_pubkey(), false),
            AccountMeta::new(payer, true),
        ],
        data: BridgeInstruction::WithdrawSPL { remote_token, to, amount, gas_limit }
            .pack()
            .unwrap(),
    }
}

/// Creates a `CreateUserWithdrawalCounterAccount` instruction.
pub fn create_user_withdrawal_counter_account(payer: Pubkey) -> Instruction {
    Instruction {
        program_id: crate::id(),
        accounts: vec![
            AccountMeta::new_readonly(solana_program::system_program::id(), false),
            AccountMeta::new_readonly(solana_program::sysvar::rent::id(), false),
            AccountMeta::new(user_withdrawal_counter_pubkey(&payer), false),
            AccountMeta::new(payer, true),
        ],
        data: BridgeInstruction::CreateUserWithdrawalCounterAccount {}.pack().unwrap(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_str_encoding() -> Result<(), ProgramError> {
        let s = "hello";
        let packed = BridgeInstruction::pack_str(s)?;
        let (unpacked, _) = BridgeInstruction::unpack_str(&packed)?;
        assert_eq!(s, unpacked);

        let s = "abcdefghijklmnopqrstuvwxyz1234567890";
        let packed = BridgeInstruction::pack_str(s)?;
        let (unpacked, _) = BridgeInstruction::unpack_str(&packed)?;
        assert_eq!(s, unpacked);

        // Test string at max length
        let mut long_str = String::new();
        for _ in 0..=256 {
            long_str.push('a');
        }
        let packed = BridgeInstruction::pack_str(s)?;
        let (unpacked, _) = BridgeInstruction::unpack_str(&packed)?;
        assert_eq!(s, unpacked);

        // Test string too long
        long_str = String::new();
        for _ in 0..=257 {
            long_str.push('a');
        }
        assert_eq!(BridgeInstruction::pack_str(&long_str), Err(BridgeError::StringLengthTooLong));
        Ok(())
    }

    #[test]
    fn test_create_spl_encoding() -> Result<(), ProgramError> {
        let remote_token = [0u8; 20];
        let name = "name";
        let symbol = "symbol";
        let uri = "uri";
        let decimals = 18;

        let instruction = BridgeInstruction::CreateSPL {
            remote_token: remote_token.into(),
            name,
            symbol,
            uri,
            decimals,
        };

        let packed_data = instruction.pack()?;
        let unpacked_instruction = BridgeInstruction::unpack(&packed_data)?;
        assert_eq!(
            instruction, unpacked_instruction,
            "Unpacked instruction did not match original"
        );
        Ok(())
    }
}
