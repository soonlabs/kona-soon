use solana_sdk::{
    account::{Account, AccountSharedData},
    message::SanitizedMessage,
    sysvar::{self, instructions::construct_instructions_data},
};

pub(crate) mod inner_instructions;
pub(crate) mod rent;

pub(crate) fn construct_instructions_account(message: &SanitizedMessage) -> AccountSharedData {
    AccountSharedData::from(Account {
        data: construct_instructions_data(&message.decompile_instructions()),
        owner: sysvar::id(),
        ..Account::default()
    })
}
