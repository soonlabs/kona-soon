use risc0_zkvm::guest::env;
use solana_sdk::account::AccountSharedData;
use solana_sdk::pubkey::Pubkey;
use litesvm::{L2Block, LiteSVM};
use litesvm::accounts_callback::MemoryAccountsCallback;
use serde::{Deserialize, Serialize};
use solana_sdk::clock::Slot;
use solana_sdk::hash::Hash;

#[derive(Debug, Serialize, Deserialize)]
struct WitnessData {
    soon_accounts: Vec<(Pubkey, AccountSharedData)>,
    parent_slot: Slot,
    parent_bank_hash: Hash,
    clock_timestamp: i64,
    leader: Pubkey,
}

fn main() {
    // TODO: Implement your guest code here

    // read the input
    let data = env::read_frame();
    let witness: WitnessData = bincode::deserialize(&data).expect("deserialize witness data failed");

    let data = env::read_frame();
    let l2_block: L2Block = bincode::deserialize(&data).expect("deserialize l2 block failed");
    
    // create svm
    let mut svm: LiteSVM<MemoryAccountsCallback> = LiteSVM::new_soon()
        .with_parent_slot(witness.parent_slot)
        .with_parent_bank_hash(witness.parent_bank_hash)
        .with_leader_schedule(witness.leader.into())
        .with_sig_verify(false)
        .with_blockhash_verify(true)
        .with_accounts_callback(witness.soon_accounts.into())
        .with_clock_timestamp(witness.clock_timestamp);
    svm.finish_init().expect("svm finish init failed");

    // execute block
    let results = svm.execute_block(l2_block.into()).expect("svm execute block failed");
    env::log(&format!("results: {:#?}", results));

    let diff_accounts = svm.export_diff_accounts();

    // write public output to the journal
    env::commit(&diff_accounts);
}

