use crate::error::BridgeError;
use crate::instruction::{
    change_bridge_admin, create_spl, create_user_withdrawal_counter_account, deposit_erc20,
    deposit_eth, withdraw_eth, withdraw_spl,
};
use crate::pda::{spl_token_mint_pubkey, spl_token_owner_pubkey};
use crate::processor::{Processor, MINIMAL_GAS_LIMIT, SPL_SHARE_DECIMAL};
use crate::state::{BridgeConfig, BridgeOwner, WithdrawalCounter};
use ethabi::Address;
use serial_test::serial;
use solana_program::account_info::AccountInfo;
use solana_program::entrypoint::ProgramResult;
use solana_program::instruction::{AccountMeta, Instruction};
use solana_program::program_error::ProgramError;
use solana_program::program_memory::sol_memset;
use solana_program::program_option::COption;
use solana_program::program_pack::Pack;
use solana_program::pubkey::Pubkey;
use solana_program::system_instruction::{SystemInstruction, MAX_PERMITTED_DATA_LENGTH};
use solana_program::{msg, system_program};
use solana_sdk::account::{create_account_for_test, ReadableAccount};
use solana_sdk::account::{Account as SolanaAccount, WritableAccount};
use solana_sdk::native_loader::create_loadable_account_for_test;
use solana_sdk::rent::Rent;
use spl_token::state::Account as SplTokenAccount;
use spl_token::state::Mint;
use std::slice::from_raw_parts_mut;
use std::sync::RwLock;

const VAULT_INIT_LAMPORTS: u64 = 1_000_000_000_000_000;
const PAYER_INIT_LAMPORTS: u64 = 1_000_000_000_000;
const ADMIN_INIT_LAMPORTS: u64 = 1_000_000_000_000;
const PAYER_INIT_SPL: u64 = 1_000_000_000;

fn realloc(account_info: &AccountInfo, new_len: usize) -> Result<(), ProgramError> {
    let mut data = account_info.try_borrow_mut_data()?;
    assert_eq!(data.len(), MAX_PERMITTED_DATA_LENGTH as usize);

    // realloc
    unsafe { *data = from_raw_parts_mut(data.as_mut_ptr(), new_len) }
    sol_memset(&mut data, 0, new_len);

    Ok(())
}

fn program_process(
    program_id: &Pubkey,
    account_infos: &[AccountInfo],
    data: &[u8],
    allow_self: bool,
) -> ProgramResult {
    if program_id == &system_program::id() {
        let instruction: SystemInstruction =
            bincode::deserialize(data).map_err(|_| ProgramError::InvalidInstructionData)?;
        match instruction {
            SystemInstruction::Transfer { lamports } => {
                if account_infos.len() < 2 {
                    return Err(ProgramError::NotEnoughAccountKeys);
                }
                let source_info = &account_infos[0];
                if !source_info.is_signer {
                    return Err(ProgramError::MissingRequiredSignature);
                }
                if !source_info.data_is_empty() {
                    return Err(ProgramError::InvalidAccountData);
                }
                let dest_info = &account_infos[1];
                let mut source = source_info.try_borrow_mut_lamports()?;
                let mut dest = dest_info.try_borrow_mut_lamports()?;
                **source = source.checked_sub(lamports).ok_or(ProgramError::InsufficientFunds)?;
                **dest = dest.checked_add(lamports).ok_or(ProgramError::ArithmeticOverflow)?;
                Ok(())
            }
            SystemInstruction::Allocate { space } => {
                if account_infos.is_empty() {
                    return Err(ProgramError::NotEnoughAccountKeys);
                }
                let account_info = &account_infos[0];
                if !account_info.is_signer {
                    return Err(ProgramError::MissingRequiredSignature);
                }
                if account_info.data_len() != MAX_PERMITTED_DATA_LENGTH as usize
                    || account_info.owner != program_id
                {
                    return Err(ProgramError::AccountAlreadyInitialized);
                }
                realloc(account_info, space as usize)?;
                Ok(())
            }
            SystemInstruction::Assign { owner } => {
                if account_infos.is_empty() {
                    return Err(ProgramError::NotEnoughAccountKeys);
                }
                let account_info = &account_infos[0];
                if !account_info.is_signer {
                    return Err(ProgramError::MissingRequiredSignature);
                }
                account_info.assign(&owner);
                Ok(())
            }
            SystemInstruction::CreateAccount { lamports, space, owner } => {
                if account_infos.len() < 2 {
                    return Err(ProgramError::NotEnoughAccountKeys);
                }
                let source_info = &account_infos[0];
                if !source_info.is_signer {
                    return Err(ProgramError::MissingRequiredSignature);
                }
                let new_info = &account_infos[1];
                if new_info.data_len() != MAX_PERMITTED_DATA_LENGTH as usize
                    || new_info.owner != program_id
                {
                    return Err(ProgramError::AccountAlreadyInitialized);
                }
                realloc(new_info, space as usize)?;
                let mut source = source_info.try_borrow_mut_lamports()?;
                let mut new = new_info.try_borrow_mut_lamports()?;
                **source = source.checked_sub(lamports).ok_or(ProgramError::InsufficientFunds)?;
                **new = lamports;
                new_info.assign(&owner);
                Ok(())
            }
            _ => Err(ProgramError::InvalidInstructionData),
        }
    } else if program_id == &crate::id() {
        if allow_self {
            Processor::process(program_id, account_infos, data)
        } else {
            Err(ProgramError::IncorrectProgramId)
        }
    } else if program_id == &spl_token::id() {
        spl_token::processor::Processor::process(program_id, account_infos, data)
    } else if program_id == &spl_associated_token_account::id() {
        spl_associated_token_account::processor::process_instruction(
            program_id,
            account_infos,
            data,
        )
    } else if program_id == &mpl_token_metadata::ID {
        Ok(()) // Mock mpl token metadata program for temporary
    } else {
        Err(ProgramError::IncorrectProgramId)
    }
}

struct TestSyscallStubs {
    caller_stack: RwLock<Vec<Pubkey>>,
    return_data: RwLock<Option<(Pubkey, Vec<u8>)>>,
}

impl solana_sdk::program_stubs::SyscallStubs for TestSyscallStubs {
    fn sol_invoke_signed(
        &self,
        instruction: &Instruction,
        account_infos: &[AccountInfo],
        signers_seeds: &[&[&[u8]]],
    ) -> ProgramResult {
        msg!(
            "TestSyscallStubs::sol_invoke_signed(), caller_stack: {:?}",
            self.caller_stack.read().unwrap()
        );

        let mut new_account_infos = vec![];

        // mimic check for token related program in accounts
        if !account_infos.iter().any(|x| {
            *x.key == system_program::id()
                || *x.key == spl_token::id()
                || *x.key == spl_associated_token_account::id()
                || *x.key == mpl_token_metadata::ID
        }) {
            return Err(ProgramError::InvalidAccountData);
        }

        for meta in instruction.accounts.iter() {
            for account_info in account_infos.iter() {
                if meta.pubkey == *account_info.key {
                    let mut new_account_info = account_info.clone();
                    let caller = self.caller_stack.read().unwrap().last().cloned().unwrap();
                    for seeds in signers_seeds.iter() {
                        let signer = Pubkey::create_program_address(seeds, &caller)
                            .map_err(|_| ProgramError::InvalidSeeds)?;
                        println!("instruction id: {}", instruction.program_id);
                        println!("account info owner: {}", account_info.owner);
                        if *account_info.key == signer {
                            new_account_info.is_signer = true;
                        }
                    }
                    new_account_infos.push(new_account_info);
                }
            }
        }

        self.caller_stack.write().unwrap().push(instruction.program_id);
        let res =
            program_process(&instruction.program_id, &new_account_infos, &instruction.data, false);
        self.caller_stack.write().unwrap().pop();
        res
    }

    fn sol_get_rent_sysvar(&self, var_addr: *mut u8) -> u64 {
        unsafe {
            *(var_addr as *mut _ as *mut Rent) = Rent::default();
        }
        solana_program::entrypoint::SUCCESS
    }

    fn sol_get_return_data(&self) -> Option<(Pubkey, Vec<u8>)> {
        let return_data = self.return_data.read().unwrap();
        return_data.clone()
    }

    fn sol_set_return_data(&self, data: &[u8]) {
        let caller = self.caller_stack.read().unwrap().last().cloned().unwrap();
        let mut return_data = self.return_data.write().unwrap();
        *return_data = Some((caller, data.to_vec()));
    }
}

fn test_syscall_stubs() {
    use std::sync::Once;
    static ONCE: Once = Once::new();

    ONCE.call_once(|| {
        solana_program::program_stubs::set_syscall_stubs(Box::new(TestSyscallStubs {
            caller_stack: RwLock::new(vec![crate::id()]),
            return_data: RwLock::new(None),
        }));
    });
}

fn do_process_instruction(
    instruction: Instruction,
    accounts: Vec<&mut SolanaAccount>,
) -> ProgramResult {
    test_syscall_stubs();

    // approximate the logic in the actual runtime which runs the instruction
    // and only updates accounts if the instruction is successful
    let mut account_clones = accounts.iter().map(|x| (*x).clone()).collect::<Vec<_>>();
    let mut meta = instruction
        .accounts
        .iter()
        .zip(account_clones.iter_mut())
        .map(|(account_meta, account)| {
            (&account_meta.pubkey, account_meta.is_signer, account_meta.is_writable, account)
        })
        .collect::<Vec<_>>();
    let account_infos = meta
        .iter_mut()
        .map(|(key, is_signer, is_writable, account)| {
            AccountInfo::new(
                key,
                *is_signer,
                *is_writable,
                &mut account.lamports,
                &mut account.data,
                &account.owner,
                account.executable,
                account.rent_epoch,
            )
        })
        .collect::<Vec<_>>();
    let res = program_process(&instruction.program_id, &account_infos, &instruction.data, true);

    if res.is_ok() {
        let mut account_metas = instruction
            .accounts
            .iter()
            .zip(accounts)
            .map(|(account_meta, account)| (account_meta.pubkey, account))
            .collect::<Vec<_>>();
        for account_info in account_infos {
            for (key, account) in account_metas.iter_mut() {
                if account_info.key == key && account_info.is_writable {
                    account.owner = *account_info.owner;
                    account.lamports = **account_info.try_borrow_lamports()?;
                    account.data = account_info.try_borrow_data()?.to_vec();
                }
            }
        }
    }
    res
}

fn system_program_account() -> SolanaAccount {
    create_loadable_account_for_test("system_program").into()
}

fn rent_sysvar_account(rent: &Rent) -> SolanaAccount {
    create_account_for_test(rent)
}

fn spl_token_program_account() -> SolanaAccount {
    create_loadable_account_for_test("spl_token_program").into()
}

fn spl_associated_token_program_account() -> SolanaAccount {
    create_loadable_account_for_test("spl_associated_token_program").into()
}

fn mpl_token_metadata_program_account() -> SolanaAccount {
    create_loadable_account_for_test("mpl_token_metadata_program").into()
}

fn bridge_vault_account() -> SolanaAccount {
    SolanaAccount::new(VAULT_INIT_LAMPORTS, 0, &system_program::id())
}

fn bridge_config_account(rent: &Rent) -> SolanaAccount {
    let length = BridgeConfig::LEN;
    let mut data = vec![0; length];
    BridgeConfig::default().pack_into_slice(&mut data);
    SolanaAccount {
        lamports: rent.minimum_balance(length),
        data,
        owner: crate::id(),
        executable: false,
        rent_epoch: 0,
    }
}

fn bridge_owner_account(rent: &Rent, admin: Pubkey) -> SolanaAccount {
    let length = BridgeOwner::LEN;
    let mut data = vec![0; length];
    BridgeOwner { admin }.pack_into_slice(&mut data);
    SolanaAccount {
        lamports: rent.minimum_balance(length),
        data,
        owner: crate::id(),
        executable: false,
        rent_epoch: 0,
    }
}

fn uninitialized_account() -> SolanaAccount {
    SolanaAccount::new(0, MAX_PERMITTED_DATA_LENGTH as usize, &system_program::id())
}

fn payer_account() -> SolanaAccount {
    SolanaAccount::new(PAYER_INIT_LAMPORTS, 0, &system_program::id())
}

fn admin_account() -> SolanaAccount {
    SolanaAccount::new(ADMIN_INIT_LAMPORTS, 0, &system_program::id())
}

fn spl_token_owner_account() -> SolanaAccount {
    SolanaAccount::new(0, 0, &system_program::id())
}

fn initialized_user_withdrawal_counter_account(rent: &Rent, counter: u64) -> SolanaAccount {
    let length = WithdrawalCounter::LEN;
    let mut data = vec![0; length];
    WithdrawalCounter { counter }.pack_into_slice(&mut data);
    SolanaAccount {
        lamports: rent.minimum_balance(length),
        data,
        owner: crate::id(),
        executable: false,
        rent_epoch: 0,
    }
}

fn initialized_spl_token_mint_account(
    rent: &Rent,
    mint_authority: Pubkey,
    decimals: u8,
    supply: u64,
) -> SolanaAccount {
    let length = Mint::LEN;
    let mut data = vec![0; length];
    Mint {
        mint_authority: COption::Some(mint_authority),
        supply,
        decimals,
        is_initialized: true,
        freeze_authority: COption::None,
    }
    .pack_into_slice(&mut data);
    SolanaAccount {
        lamports: rent.minimum_balance(length),
        data,
        owner: spl_token::id(),
        executable: false,
        rent_epoch: 0,
    }
}

fn initialized_spl_associated_token_account(
    mint: &Pubkey,
    owner: &Pubkey,
    rent: &Rent,
) -> SolanaAccount {
    let length = SplTokenAccount::LEN;
    let mut data = vec![0; length];
    SplTokenAccount {
        mint: *mint,
        owner: *owner,
        amount: PAYER_INIT_SPL,
        delegate: COption::None,
        state: spl_token::state::AccountState::Initialized,
        is_native: COption::None,
        delegated_amount: 0,
        close_authority: COption::None,
    }
    .pack_into_slice(&mut data);
    SolanaAccount {
        lamports: rent.minimum_balance(length),
        data,
        owner: spl_token::id(),
        executable: false,
        rent_epoch: 0,
    }
}

fn no_sig_tx_payer_account() -> SolanaAccount {
    SolanaAccount::new(1, 0, &system_program::id())
}

#[test]
#[serial]
fn test_deposit_eth() -> anyhow::Result<()> {
    let rent = Rent::default();

    let mut system_program_account = system_program_account();
    let mut deposit_to_account = uninitialized_account();
    let mut vault_account = bridge_vault_account();
    let mut no_sig_tx_payer_account = no_sig_tx_payer_account();

    let deposit_amount = 1_000_000_000u64;
    let receiver = Pubkey::new_unique();
    let instruction = deposit_eth(
        Default::default(),
        Default::default(),
        Default::default(),
        deposit_amount as u128,
        Default::default(),
        Default::default(),
        Default::default(),
        Default::default(),
        Default::default(),
        Default::default(),
        receiver,
        Default::default(),
        Default::default(),
    );

    // deposit successfully
    let instruction1 = instruction.clone();
    do_process_instruction(
        instruction1,
        vec![
            &mut system_program_account,
            &mut deposit_to_account,
            &mut vault_account,
            &mut no_sig_tx_payer_account,
        ],
    )?;
    assert_eq!(deposit_to_account.lamports(), deposit_amount);
    assert_eq!(vault_account.lamports(), VAULT_INIT_LAMPORTS - deposit_amount);

    // deposit amount less than rent_min_balance
    deposit_to_account.set_lamports(0);
    vault_account.set_lamports(VAULT_INIT_LAMPORTS);
    let rent_min_balance = rent.minimum_balance(0);
    assert_eq!(
        do_process_instruction(
            deposit_eth(
                Default::default(),
                Default::default(),
                Default::default(),
                (rent_min_balance - 1) as u128,
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                receiver,
                Default::default(),
                Default::default(),
            ),
            vec![
                &mut system_program_account,
                &mut deposit_to_account,
                &mut vault_account,
                &mut no_sig_tx_payer_account,
            ],
        ),
        Err(BridgeError::InvalidDepositAmount.into()),
    );

    // invalid vault account
    let mut instruction2 = instruction.clone();
    instruction2.accounts[2] = AccountMeta::new(Pubkey::new_unique(), false);
    assert_eq!(
        Err(BridgeError::InvalidVaultAccount.into()),
        do_process_instruction(
            instruction2,
            vec![
                &mut system_program_account,
                &mut deposit_to_account,
                &mut vault_account,
                &mut no_sig_tx_payer_account,
            ],
        )
    );

    // invalid depositor account
    let mut instruction3 = instruction.clone();
    instruction3.accounts[1] = AccountMeta::new(Pubkey::new_unique(), false);
    assert_eq!(
        Err(BridgeError::InvalidDepositorAccount.into()),
        do_process_instruction(
            instruction3,
            vec![
                &mut system_program_account,
                &mut deposit_to_account,
                &mut vault_account,
                &mut no_sig_tx_payer_account,
            ],
        )
    );

    // invalid no_sig_tx_payer_account
    let mut instruction4 = instruction.clone();
    instruction4.accounts[3] = AccountMeta::new_readonly(Pubkey::new_unique(), true);
    assert_eq!(
        Err(BridgeError::InvalidNoSigTxPayer.into()),
        do_process_instruction(
            instruction4,
            vec![
                &mut system_program_account,
                &mut deposit_to_account,
                &mut vault_account,
                &mut no_sig_tx_payer_account,
            ],
        )
    );

    Ok(())
}

#[test]
#[serial]
fn test_create_user_withdrawal_counter() -> anyhow::Result<()> {
    let rent = Rent::default();

    let mut system_program_account = system_program_account();
    let mut rent_sysvar_account = rent_sysvar_account(&rent);
    let mut user_withdrawal_counter_account = uninitialized_account();
    let mut payer_account = payer_account();

    let payer = Pubkey::new_unique();
    let instruction = create_user_withdrawal_counter_account(payer);
    do_process_instruction(
        instruction,
        vec![
            &mut system_program_account,
            &mut rent_sysvar_account,
            &mut user_withdrawal_counter_account,
            &mut payer_account,
        ],
    )?;
    // create withdrawal counter successfully
    assert_eq!(
        user_withdrawal_counter_account.lamports(),
        rent.minimum_balance(WithdrawalCounter::LEN),
    );
    let counter = WithdrawalCounter::unpack(&user_withdrawal_counter_account.data)?;
    assert_eq!(counter.counter, 0);

    Ok(())
}

#[test]
#[serial]
fn test_withdraw_eth() -> anyhow::Result<()> {
    let rent = Rent::default();

    let mut system_program_account = system_program_account();
    let mut rent_sysvar_account = rent_sysvar_account(&rent);
    let mut user_withdrawal_counter = initialized_user_withdrawal_counter_account(&rent, 0);
    let mut withdrawal_transaction_account = uninitialized_account();
    let mut vault_account = bridge_vault_account();
    let mut bridge_config_account = bridge_config_account(&rent);
    let mut payer = payer_account();

    let payer_key = Pubkey::new_unique();
    // withdrawl successfully
    let withdrawal_amount = 1_000_000_000u64;
    let instruction = withdraw_eth(
        Default::default(),
        payer_key,
        withdrawal_amount as u128,
        MINIMAL_GAS_LIMIT,
        0u64,
    );
    do_process_instruction(
        instruction.clone(),
        vec![
            &mut system_program_account,
            &mut rent_sysvar_account,
            &mut user_withdrawal_counter,
            &mut withdrawal_transaction_account,
            &mut vault_account,
            &mut bridge_config_account,
            &mut payer,
        ],
    )?;
    let counter = WithdrawalCounter::unpack(&user_withdrawal_counter.data)?;
    assert_eq!(counter.counter, 1);
    assert_eq!(vault_account.lamports(), VAULT_INIT_LAMPORTS + withdrawal_amount);

    // withdraw transaction account has lamports
    payer = payer_account();
    withdrawal_transaction_account = uninitialized_account();
    withdrawal_transaction_account.lamports += 1;
    vault_account = bridge_vault_account();
    do_process_instruction(
        withdraw_eth(
            Default::default(),
            payer_key,
            withdrawal_amount as u128,
            MINIMAL_GAS_LIMIT,
            1u64,
        ),
        vec![
            &mut system_program_account,
            &mut rent_sysvar_account,
            &mut user_withdrawal_counter,
            &mut withdrawal_transaction_account,
            &mut vault_account,
            &mut bridge_config_account,
            &mut payer,
        ],
    )?;
    let counter = WithdrawalCounter::unpack(&user_withdrawal_counter.data)?;
    assert_eq!(counter.counter, 2);
    assert_eq!(vault_account.lamports(), VAULT_INIT_LAMPORTS + withdrawal_amount);

    // withdrawal zero value should fail
    payer = payer_account();
    withdrawal_transaction_account = uninitialized_account();
    vault_account = bridge_vault_account();
    assert_eq!(
        do_process_instruction(
            withdraw_eth(Default::default(), payer_key, 0u128, MINIMAL_GAS_LIMIT, 2u64,),
            vec![
                &mut system_program_account,
                &mut rent_sysvar_account,
                &mut user_withdrawal_counter,
                &mut withdrawal_transaction_account,
                &mut vault_account,
                &mut bridge_config_account,
                &mut payer,
            ],
        ),
        Err(BridgeError::InvalidWithdrawAmount.into())
    );

    // withdrawal amount > u64::MAX should fail
    assert_eq!(
        do_process_instruction(
            withdraw_eth(
                Default::default(),
                payer_key,
                u64::MAX as u128 + 1,
                MINIMAL_GAS_LIMIT,
                2u64,
            ),
            vec![
                &mut system_program_account,
                &mut rent_sysvar_account,
                &mut user_withdrawal_counter,
                &mut withdrawal_transaction_account,
                &mut vault_account,
                &mut bridge_config_account,
                &mut payer,
            ],
        ),
        Err(BridgeError::InvalidWithdrawAmount.into())
    );

    Ok(())
}

#[test]
#[serial]
fn test_change_bridge_admin() -> anyhow::Result<()> {
    let rent = Rent::default();

    let admin_key = Pubkey::new_unique();
    let new_admin_key = Pubkey::new_unique();
    let mut bridge_owner = bridge_owner_account(&rent, admin_key);
    let mut admin = admin_account();
    let mut new_admin = admin_account();

    let instruction = change_bridge_admin(admin_key, new_admin_key);

    // change admin successfully
    let instruction1 = instruction.clone();
    do_process_instruction(instruction1, vec![&mut bridge_owner, &mut admin, &mut new_admin])?;
    let owner = BridgeOwner::unpack(&bridge_owner.data)?;
    assert_eq!(owner.admin, new_admin_key);

    // invalid bridge owner account
    let mut instruction2 = instruction.clone();
    instruction2.accounts[0] = AccountMeta::new(Pubkey::new_unique(), false);
    assert_eq!(
        Err(BridgeError::InvalidAdmin.into()),
        do_process_instruction(
            change_bridge_admin(admin_key, new_admin_key),
            vec![&mut bridge_owner, &mut admin, &mut new_admin],
        )
    );

    // invalid admin account
    let fake_admin = Pubkey::new_unique();
    assert_eq!(
        Err(BridgeError::InvalidAdmin.into()),
        do_process_instruction(
            change_bridge_admin(fake_admin, new_admin_key),
            vec![&mut bridge_owner, &mut admin, &mut new_admin],
        )
    );

    Ok(())
}

#[test]
#[serial]
fn test_create_spl() -> anyhow::Result<()> {
    let rent = Rent::default();
    let admin = Pubkey::new_unique();

    let mut system_program_account = system_program_account();
    let mut rent_sysvar_account = rent_sysvar_account(&rent);
    let mut spl_token_program_account = spl_token_program_account();
    let mut spl_token_owner_account = spl_token_owner_account();
    let mut spl_token_mint_account = uninitialized_account();
    let mut vault_account = bridge_vault_account();
    let mut bridge_owner_account = bridge_owner_account(&rent, admin);
    let mut admin_account = admin_account();
    let mut mpl_token_metadata_program_account = mpl_token_metadata_program_account();
    let mut metadata_account = uninitialized_account();

    let instruction = create_spl(
        Address::random(),
        admin,
        "Test Token",
        "TEST",
        "https://ipfs.io/ipfs/QmXRVXSRbH9nKYPgVfakXRhDhEaXWs6QYu3rToadXhtHPr",
        6,
    )?;
    // create spl successfully
    do_process_instruction(
        instruction.clone(),
        vec![
            &mut system_program_account,
            &mut rent_sysvar_account,
            &mut spl_token_program_account,
            &mut spl_token_owner_account,
            &mut spl_token_mint_account,
            &mut vault_account,
            &mut bridge_owner_account,
            &mut admin_account,
            &mut mpl_token_metadata_program_account,
            &mut metadata_account,
        ],
    )?;

    // invalid spl token program account
    let mut instruction2 = instruction.clone();
    instruction2.accounts[2] = AccountMeta::new_readonly(Pubkey::new_unique(), false);
    assert_eq!(
        Err(BridgeError::InvalidSPLTokenProgramId.into()),
        do_process_instruction(
            instruction2,
            vec![
                &mut system_program_account,
                &mut rent_sysvar_account,
                &mut spl_token_program_account,
                &mut spl_token_owner_account,
                &mut spl_token_mint_account,
                &mut vault_account,
                &mut bridge_owner_account,
                &mut admin_account,
                &mut mpl_token_metadata_program_account,
                &mut metadata_account,
            ],
        )
    );

    // invalid vault account
    let mut instruction3 = instruction.clone();
    instruction3.accounts[5] = AccountMeta::new(Pubkey::new_unique(), false);
    assert_eq!(
        Err(BridgeError::InvalidVaultAccount.into()),
        do_process_instruction(
            instruction3,
            vec![
                &mut system_program_account,
                &mut rent_sysvar_account,
                &mut spl_token_program_account,
                &mut spl_token_owner_account,
                &mut spl_token_mint_account,
                &mut vault_account,
                &mut bridge_owner_account,
                &mut admin_account,
                &mut mpl_token_metadata_program_account,
                &mut metadata_account,
            ],
        )
    );

    // invalid bridge owner account
    let mut instruction4 = instruction.clone();
    instruction4.accounts[6] = AccountMeta::new_readonly(Pubkey::new_unique(), false);
    assert_eq!(
        Err(BridgeError::InvalidBridgeOwnerAccount.into()),
        do_process_instruction(
            instruction4,
            vec![
                &mut system_program_account,
                &mut rent_sysvar_account,
                &mut spl_token_program_account,
                &mut spl_token_owner_account,
                &mut spl_token_mint_account,
                &mut vault_account,
                &mut bridge_owner_account,
                &mut admin_account,
                &mut mpl_token_metadata_program_account,
                &mut metadata_account,
            ],
        )
    );

    // invalid admin account
    let mut instruction5 = instruction.clone();
    instruction5.accounts[7] = AccountMeta::new(Pubkey::new_unique(), true);
    assert_eq!(
        Err(BridgeError::InvalidAdmin.into()),
        do_process_instruction(
            instruction5,
            vec![
                &mut system_program_account,
                &mut rent_sysvar_account,
                &mut spl_token_program_account,
                &mut spl_token_owner_account,
                &mut spl_token_mint_account,
                &mut vault_account,
                &mut bridge_owner_account,
                &mut admin_account,
                &mut mpl_token_metadata_program_account,
                &mut metadata_account,
            ],
        )
    );

    // invalid token owner account
    let mut instruction6 = instruction.clone();
    instruction6.accounts[3] = AccountMeta::new_readonly(Pubkey::new_unique(), false);
    assert_eq!(
        Err(BridgeError::InvalidSPLTokenOwnerAccount.into()),
        do_process_instruction(
            instruction6,
            vec![
                &mut system_program_account,
                &mut rent_sysvar_account,
                &mut spl_token_program_account,
                &mut spl_token_owner_account,
                &mut spl_token_mint_account,
                &mut vault_account,
                &mut bridge_owner_account,
                &mut admin_account,
                &mut mpl_token_metadata_program_account,
                &mut metadata_account,
            ],
        )
    );

    // invalid token mint account
    let mut instruction7 = instruction.clone();
    instruction7.accounts[4] = AccountMeta::new(Pubkey::new_unique(), false);
    assert_eq!(
        Err(BridgeError::InvalidSPLTokenMintAccount.into()),
        do_process_instruction(
            instruction7,
            vec![
                &mut system_program_account,
                &mut rent_sysvar_account,
                &mut spl_token_program_account,
                &mut spl_token_owner_account,
                &mut spl_token_mint_account,
                &mut vault_account,
                &mut bridge_owner_account,
                &mut admin_account,
                &mut mpl_token_metadata_program_account,
                &mut metadata_account,
            ],
        )
    );

    // invalid mpl token metadata account
    let mut instruction8 = instruction.clone();
    instruction8.accounts[8] = AccountMeta::new(Pubkey::new_unique(), false);
    assert_eq!(
        Err(BridgeError::InvalidMetadataProgramId.into()),
        do_process_instruction(
            instruction8,
            vec![
                &mut system_program_account,
                &mut rent_sysvar_account,
                &mut spl_token_program_account,
                &mut spl_token_owner_account,
                &mut spl_token_mint_account,
                &mut vault_account,
                &mut bridge_owner_account,
                &mut admin_account,
                &mut mpl_token_metadata_program_account,
                &mut metadata_account,
            ],
        )
    );

    // invalid metadata account
    let mut instruction9 = instruction.clone();
    instruction9.accounts[9] = AccountMeta::new(Pubkey::new_unique(), false);
    assert_eq!(
        Err(BridgeError::InvalidMetadataAccount.into()),
        do_process_instruction(
            instruction9,
            vec![
                &mut system_program_account,
                &mut rent_sysvar_account,
                &mut spl_token_program_account,
                &mut spl_token_owner_account,
                &mut spl_token_mint_account,
                &mut vault_account,
                &mut bridge_owner_account,
                &mut admin_account,
                &mut mpl_token_metadata_program_account,
                &mut metadata_account,
            ],
        )
    );

    Ok(())
}

#[test]
#[serial]
fn test_deposit_erc20() -> anyhow::Result<()> {
    let rent = Rent::default();

    const DECIMALS: u8 = 6;
    let remote_token = Address::random();
    let local_token = spl_token_mint_pubkey(&remote_token);
    let spl_token_owner = spl_token_owner_pubkey(&remote_token);
    let to = Pubkey::new_unique();

    let mut system_program_account = system_program_account();
    let mut spl_token_program_account = spl_token_program_account();
    let mut spl_associated_token_program_account = spl_associated_token_program_account();
    let mut spl_token_owner_account = spl_token_owner_account();
    let mut spl_token_mint_account =
        initialized_spl_token_mint_account(&rent, spl_token_owner, DECIMALS, 0);
    let mut depositor_account = payer_account();
    let mut spl_associated_token_account = uninitialized_account();
    let mut vault_account = bridge_vault_account();
    let mut no_sig_tx_payer_account = no_sig_tx_payer_account();

    let from = Address::random();
    let deposit_amount = 1_000_000u64;
    let actual_amount =
        deposit_amount * 10u64.pow(DECIMALS as u32) / 10u64.pow(SPL_SHARE_DECIMAL as u32);
    let instruction = deposit_erc20(local_token, remote_token, from, to, deposit_amount as u128);

    // deposit erc20 successfully
    let instruction1 = instruction.clone();
    do_process_instruction(
        instruction1,
        vec![
            &mut system_program_account,
            &mut spl_token_program_account,
            &mut spl_associated_token_program_account,
            &mut spl_token_owner_account,
            &mut spl_token_mint_account,
            &mut depositor_account,
            &mut spl_associated_token_account,
            &mut vault_account,
            &mut no_sig_tx_payer_account,
        ],
    )?;
    let spl_associated_token = SplTokenAccount::unpack(&spl_associated_token_account.data)?;
    assert_eq!(spl_associated_token.amount, actual_amount);
    assert_eq!(spl_associated_token.close_authority.unwrap(), spl_token_owner);

    // deposit erc20 to an initialized account successfully
    spl_associated_token_account =
        initialized_spl_associated_token_account(&local_token, &to, &rent);
    do_process_instruction(
        instruction.clone(),
        vec![
            &mut system_program_account,
            &mut spl_token_program_account,
            &mut spl_associated_token_program_account,
            &mut spl_token_owner_account,
            &mut spl_token_mint_account,
            &mut depositor_account,
            &mut spl_associated_token_account,
            &mut vault_account,
            &mut no_sig_tx_payer_account,
        ],
    )?;
    let spl_associated_token = SplTokenAccount::unpack(&spl_associated_token_account.data)?;
    assert_eq!(spl_associated_token.amount, PAYER_INIT_SPL + actual_amount);
    assert!(spl_associated_token.close_authority.is_none());

    // `NO_SIG_TX_PAYER` is not signer
    let mut instruction2 = instruction.clone();
    instruction2.accounts[8].is_signer = false;
    assert_eq!(
        Err(BridgeError::InvalidNoSigTxPayer.into()),
        do_process_instruction(
            instruction2,
            vec![
                &mut system_program_account,
                &mut spl_token_program_account,
                &mut spl_associated_token_program_account,
                &mut spl_token_owner_account,
                &mut spl_token_mint_account,
                &mut depositor_account,
                &mut spl_associated_token_account,
                &mut vault_account,
                &mut no_sig_tx_payer_account,
            ],
        )
    );

    // `to` is not a signer
    let mut instruction3 = instruction.clone();
    instruction3.accounts[5].is_signer = false;
    assert_eq!(
        Err(BridgeError::InvalidDepositorAccount.into()),
        do_process_instruction(
            instruction3,
            vec![
                &mut system_program_account,
                &mut spl_token_program_account,
                &mut spl_associated_token_program_account,
                &mut spl_token_owner_account,
                &mut spl_token_mint_account,
                &mut depositor_account,
                &mut spl_associated_token_account,
                &mut vault_account,
                &mut no_sig_tx_payer_account,
            ],
        )
    );

    Ok(())
}

#[test]
#[serial]
fn test_withdraw_spl() -> anyhow::Result<()> {
    let rent = Rent::default();

    const DECIMALS: u8 = 6;
    let remote_token = Address::random();
    let local_token = spl_token_mint_pubkey(&remote_token);
    let payer = Pubkey::new_unique();
    let spl_token_owner_key = spl_token_owner_pubkey(&remote_token);
    let counter = 0u64;

    let mut system_program_account = system_program_account();
    let mut rent_sysvar_account = rent_sysvar_account(&rent);
    let mut spl_token_program_account = spl_token_program_account();
    let mut user_withdrawal_counter_account =
        initialized_user_withdrawal_counter_account(&rent, counter);
    let mut withdrawal_transaction_account = uninitialized_account();
    let mut spl_token_owner_account = uninitialized_account();
    let mut spl_token_mint_account =
        initialized_spl_token_mint_account(&rent, spl_token_owner_key, DECIMALS, 1_000_000_000);
    let mut payer_account = payer_account();
    let mut payer_spl_associated_token_account =
        initialized_spl_associated_token_account(&local_token, &payer, &rent);
    let mut bridge_config_account = bridge_config_account(&rent);

    let to = Address::random();
    let withdrawal_amount = 1_000_000u64;
    let gas_limit = 1_000_000u128;
    let instruction =
        withdraw_spl(remote_token, payer, to, withdrawal_amount as u128, gas_limit, counter);

    // withdraw spl successfully
    let instruction1 = instruction.clone();
    do_process_instruction(
        instruction1,
        vec![
            &mut system_program_account,
            &mut rent_sysvar_account,
            &mut spl_token_program_account,
            &mut user_withdrawal_counter_account,
            &mut withdrawal_transaction_account,
            &mut spl_token_owner_account,
            &mut spl_token_mint_account,
            &mut payer_spl_associated_token_account,
            &mut bridge_config_account,
            &mut payer_account,
        ],
    )?;
    let payer_spl_associated_token =
        SplTokenAccount::unpack(&payer_spl_associated_token_account.data)?;
    assert_eq!(payer_spl_associated_token.amount, PAYER_INIT_SPL - withdrawal_amount);

    // withdrawal zero value should fail
    let instruction2 = withdraw_spl(remote_token, payer, to, 0u128, gas_limit, counter + 1);
    assert_eq!(
        do_process_instruction(
            instruction2,
            vec![
                &mut system_program_account,
                &mut rent_sysvar_account,
                &mut spl_token_program_account,
                &mut user_withdrawal_counter_account,
                &mut withdrawal_transaction_account,
                &mut spl_token_owner_account,
                &mut spl_token_mint_account,
                &mut payer_spl_associated_token_account,
                &mut bridge_config_account,
                &mut payer_account,
            ],
        ),
        Err(BridgeError::InvalidWithdrawAmount.into())
    );

    // withdraw value > u64::MAX should fail
    let instruction3 =
        withdraw_spl(remote_token, payer, to, u64::MAX as u128 + 1, gas_limit, counter + 1);
    assert_eq!(
        do_process_instruction(
            instruction3,
            vec![
                &mut system_program_account,
                &mut rent_sysvar_account,
                &mut spl_token_program_account,
                &mut user_withdrawal_counter_account,
                &mut withdrawal_transaction_account,
                &mut spl_token_owner_account,
                &mut spl_token_mint_account,
                &mut payer_spl_associated_token_account,
                &mut bridge_config_account,
                &mut payer_account,
            ],
        ),
        Err(BridgeError::InvalidWithdrawAmount.into())
    );

    Ok(())
}
