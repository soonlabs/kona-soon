use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    instruction::Instruction,
    program::{invoke, invoke_signed},
    program_pack::Pack,
    pubkey::Pubkey,
    rent::Rent,
    system_instruction,
};

#[allow(clippy::too_many_arguments)]
pub(crate) fn create_account<'a, P: Pack>(
    rent: &Rent,
    payer_account_info: &AccountInfo<'a>,
    target_account_info: &AccountInfo<'a>,
    system_program_info: &AccountInfo<'a>,
    owner: &Pubkey,
    payer_signer_seeds: &[&[u8]],
    target_signer_seeds: &[&[u8]],
) -> ProgramResult {
    let required_lamports =
        rent.minimum_balance(P::LEN).saturating_sub(target_account_info.lamports());
    if required_lamports > 0 {
        invoke_optionally_signed(
            &system_instruction::transfer(
                payer_account_info.key,
                target_account_info.key,
                required_lamports,
            ),
            &[payer_account_info.clone(), target_account_info.clone(), system_program_info.clone()],
            payer_signer_seeds,
        )?;
    }

    invoke_optionally_signed(
        &system_instruction::allocate(target_account_info.key, P::LEN as u64),
        &[target_account_info.clone(), system_program_info.clone()],
        target_signer_seeds,
    )?;

    invoke_optionally_signed(
        &system_instruction::assign(target_account_info.key, owner),
        &[target_account_info.clone(), system_program_info.clone()],
        target_signer_seeds,
    )
}

#[inline]
fn invoke_optionally_signed(
    instruction: &Instruction,
    account_infos: &[AccountInfo],
    signer_seeds: &[&[u8]],
) -> ProgramResult {
    if signer_seeds.is_empty() {
        invoke(instruction, account_infos)
    } else {
        invoke_signed(instruction, account_infos, &[signer_seeds])
    }
}
