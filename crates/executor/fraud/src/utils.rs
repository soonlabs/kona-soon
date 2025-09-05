use std::collections::HashSet;

use crate::accounts::SoonAccounts;
use crate::error::{Error, Result};
use alloy_primitives::{B256, keccak256};
use litesvm::LiteSVM;
use litesvm::accounts_callback::AccountsCallback;
use litesvm::types::TransactionMetadata;
use solana_sdk::account::{AccountSharedData, ReadableAccount};
use solana_sdk::pubkey::Pubkey;
use soon_primitives::mpt::account::TrieSolanaAccount as MptAccount;
use soon_primitives::blocks::RawBlock;
use soon_primitives::mpt::account_from_solana_native;

// pub fn init_litesvm_with_accounts<CB: AccountsCallback>(accounts: &SoonAccounts) -> Result<LiteSVM<CB>> {
//     let mut litesvm = LiteSVM::default().with_builtins().with_precompiles();
//     litesvm_import_accounts(&mut litesvm, accounts)?;
//     Ok(litesvm)
// }
//
// pub fn litesvm_import_accounts(litesvm: &mut LiteSVM<impl AccountsCallback>, accounts: &SoonAccounts) -> Result<()> {
//     litesvm.import_accounts(accounts.accounts.clone())?;
//     Ok(())
// }
//
// pub fn litesvm_import_accounts_one_by_one(
//     litesvm: &mut LiteSVM<impl AccountsCallback>,
//     accounts: &SoonAccounts,
// ) -> Result<String> {
//     let mut output = String::new();
//
//     output.push_str("=== LiteSVM Account Analysis ===\n");
//     output.push_str(&format!("Accounts to import: {}\n", accounts.accounts.len()));
//
//     output.push_str("\n--- Importing Soon Storage Accounts One by One ---\n");
//
//     let mut successfully_imported = 0;
//     let mut conflicts_resolved = 0;
//     let mut skipped_accounts = Vec::new();
//
//     for (pubkey, account) in &accounts.accounts {
//         let account_type = classify_account(pubkey, account);
//
//         // use panic catching to handle possible crashes
//         let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
//             if let Some(existing_account) = litesvm.get_account(pubkey) {
//                 // if account exists but differs, use the Soon storage version (overwrite)
//                 let existing_shared_data: AccountSharedData = existing_account.into();
//                 if existing_shared_data != *account {
//                     litesvm.set_account(*pubkey, (*account).clone().into()).map(|_| "overwritten")
//                 } else {
//                     Ok("identical")
//                 }
//             } else {
//                 // new account, add normally
//                 litesvm.set_account(*pubkey, (*account).clone().into()).map(|_| "added")
//             }
//         }));
//
//         match result {
//             Ok(Ok(status)) => match status {
//                 "overwritten" => {
//                     conflicts_resolved += 1;
//                     output.push_str(&format!(
//                         "  🔄 Overwriting {}: {} (was different)\n",
//                         pubkey, account_type
//                     ));
//                 }
//                 "identical" => {
//                     output.push_str(&format!(
//                         "  ✅ {}: {} (already identical)\n",
//                         pubkey, account_type
//                     ));
//                 }
//                 "added" => {
//                     successfully_imported += 1;
//                     output.push_str(&format!("  ➕ {}: {} (added new)\n", pubkey, account_type));
//                 }
//                 _ => {}
//             },
//             Ok(Err(e)) => {
//                 output.push_str(&format!(
//                     "  ❌ {}: {} - Error: {} (SKIPPED)\n",
//                     pubkey, account_type, e
//                 ));
//                 skipped_accounts.push((*pubkey, format!("Error: {}", e)));
//             }
//             Err(_) => {
//                 output.push_str(&format!(
//                     "  ❌ {}: {} - Panic occurred (SKIPPED)\n",
//                     pubkey, account_type
//                 ));
//                 skipped_accounts.push((*pubkey, "Panic occurred".to_string()));
//             }
//         }
//     }
//
//     output.push_str("\n--- Import Summary ---\n");
//     output.push_str(&format!("Successfully imported new: {}\n", successfully_imported));
//     output.push_str(&format!("Conflicts resolved (overwritten): {}\n", conflicts_resolved));
//     output.push_str(&format!("Skipped (unsupported): {}\n", skipped_accounts.len()));
//
//     if !skipped_accounts.is_empty() {
//         output.push_str("\nSkipped accounts:\n");
//         for (pubkey, error) in &skipped_accounts {
//             output.push_str(&format!("  {}: {}\n", pubkey, error));
//         }
//     }
//
//     // analyze the final account distribution
//     let exported_accounts = litesvm.export_accounts();
//     let mut sysvars = 0;
//     let mut builtins = 0;
//     let mut user_accounts = 0;
//
//     output.push_str("\n--- Final Account Summary ---\n");
//     for (pubkey, account) in &exported_accounts {
//         let account_type = classify_account(pubkey, account);
//         if account_type.starts_with("Sysvar") {
//             sysvars += 1;
//         } else if account_type.starts_with("Builtin") {
//             builtins += 1;
//         } else {
//             user_accounts += 1;
//         }
//     }
//
//     output.push_str(&format!("  Sysvars: {}\n", sysvars));
//     output.push_str(&format!("  Builtins: {}\n", builtins));
//     output.push_str(&format!("  User accounts: {}\n", user_accounts));
//
//     Ok(output)
// }

pub fn classify_account(pubkey: &Pubkey, account: &AccountSharedData) -> String {
    let pubkey_str = pubkey.to_string();

    // first check if it's a known sysvar account
    if pubkey_str.starts_with("SysVar")
        || is_known_sysvar(&pubkey_str)
        || *account.owner() == solana_sdk::sysvar::id()
    {
        return format!("Sysvar ({})", pubkey_str);
    }

    // check special config accounts (also a sysvar)
    if pubkey_str == "StakeConfig11111111111111111111111111111111"
        || *account.owner() == solana_sdk::config::program::id()
    {
        return format!("Sysvar ({})", pubkey_str);
    }

    // check if it's a builtin program (executable account)
    if account.executable() {
        return format!("Builtin Program ({})", account.owner());
    }

    // check if it's a user account owned by the system program
    if *account.owner() == solana_sdk::system_program::id() {
        return "User Account".to_string();
    }

    // other accounts
    format!("Other Account (owner: {})", account.owner())
}

fn is_known_sysvar(pubkey_str: &str) -> bool {
    let known_sysvars = [
        "SysvarC1ock11111111111111111111111111111111",
        "SysvarEpochSchedu1e111111111111111111111111",
        "SysvarFees111111111111111111111111111111111",
        "SysvarRecentB1ockHashes11111111111111111111",
        "SysvarRent111111111111111111111111111111111",
        "SysvarRewards111111111111111111111111111111",
        "SysvarS1otHashes111111111111111111111111111",
        "SysvarS1otHistory11111111111111111111111111",
        "SysvarStakeHistory1111111111111111111111111",
        "SysvarEpochRewards1111111111111111111111111",
        "SysvarLastRestartS1ot1111111111111111111111",
        "Vote111111111111111111111111111111111111111", // Vote program but often treated as sysvar
        // Special config accounts that act like sysvars
        "StakeConfig11111111111111111111111111111111",
    ];

    known_sysvars.iter().any(|&sysvar| pubkey_str == sysvar)
}

pub fn add_trie_account(
    target: &mut Vec<(B256, MptAccount)>,
    pubkey: &Pubkey,
    account: &AccountSharedData,
) {
    let hashed_pubkey = keccak256(pubkey);
    let mpt_account = account_from_solana_native(account);
    target.push((hashed_pubkey, mpt_account));
}

pub fn compare_common_accounts(
    soon_accounts: &SoonAccounts,
    litesvm_accounts: &SoonAccounts,
) -> Result<String> {
    let mut output = String::new();
    output.push_str("\n=== Detailed Account Data Comparison ===\n");

    let soon_map: std::collections::HashMap<_, _> =
        soon_accounts.accounts.iter().map(|(pk, acc)| (*pk, acc)).collect();

    let litesvm_map: std::collections::HashMap<_, _> =
        litesvm_accounts.accounts.iter().map(|(pk, acc)| (*pk, acc)).collect();

    let soon_pubkeys: HashSet<_> = soon_map.keys().collect();
    let litesvm_pubkeys: HashSet<_> = litesvm_map.keys().collect();

    let common_accounts: Vec<_> = soon_pubkeys.intersection(&litesvm_pubkeys).collect();

    output.push_str(&format!("Comparing {} common accounts...\n", common_accounts.len()));

    let mut matches = 0;
    let mut differences = 0;

    for pubkey in &common_accounts {
        let soon_account = soon_map.get(pubkey).unwrap();
        let litesvm_account = litesvm_map.get(pubkey).unwrap();

        let mut account_differs = false;
        let mut diff_details = Vec::new();

        // Compare lamports
        if soon_account.lamports() != litesvm_account.lamports() {
            account_differs = true;
            diff_details.push(format!(
                "  📊 Lamports: Soon={}, LiteSVM={}",
                soon_account.lamports(),
                litesvm_account.lamports()
            ));
        }

        // Compare data
        if soon_account.data() != litesvm_account.data() {
            account_differs = true;
            diff_details.push(format!(
                "  📄 Data length: Soon={}, LiteSVM={}",
                soon_account.data().len(),
                litesvm_account.data().len()
            ));

            // Show first few bytes that differ if lengths are different or data differs
            let soon_data = soon_account.data();
            let litesvm_data = litesvm_account.data();
            let min_len = std::cmp::min(soon_data.len(), litesvm_data.len());

            if min_len > 0 {
                let mut first_diff = None;
                for i in 0..min_len {
                    if soon_data[i] != litesvm_data[i] {
                        first_diff = Some(i);
                        break;
                    }
                }

                if let Some(diff_pos) = first_diff {
                    let start = diff_pos.saturating_sub(2);
                    let end = std::cmp::min(diff_pos + 10, min_len);
                    diff_details.push(format!(
                        "  🔍 Data differs at byte {}: Soon[{}..{}]={:?}, LiteSVM[{}..{}]={:?}",
                        diff_pos,
                        start,
                        end,
                        &soon_data[start..end],
                        start,
                        end,
                        &litesvm_data[start..end]
                    ));
                } else if soon_data.len() != litesvm_data.len() {
                    diff_details.push("  📏 Data content identical but lengths differ".to_string());
                }
            }
        }

        // Compare owner
        if soon_account.owner() != litesvm_account.owner() {
            account_differs = true;
            diff_details.push(format!(
                "  👤 Owner: Soon={}, LiteSVM={}",
                soon_account.owner(),
                litesvm_account.owner()
            ));
        }

        // Compare executable
        if soon_account.executable() != litesvm_account.executable() {
            account_differs = true;
            diff_details.push(format!(
                "  ⚙️ Executable: Soon={}, LiteSVM={}",
                soon_account.executable(),
                litesvm_account.executable()
            ));
        }

        // Compare rent epoch
        if soon_account.rent_epoch() != litesvm_account.rent_epoch() {
            account_differs = true;
            diff_details.push(format!(
                "  🏠 Rent epoch: Soon={}, LiteSVM={}",
                soon_account.rent_epoch(),
                litesvm_account.rent_epoch()
            ));
        }

        if account_differs {
            differences += 1;
            let account_type = classify_account(pubkey, litesvm_account);
            output.push_str(&format!("❌ {} ({}):\n", pubkey, account_type));
            for detail in diff_details {
                output.push_str(&format!("{}\n", detail));
            }
        } else {
            matches += 1;
            if matches <= 3 {
                // Only show first few matches to avoid spam
                let account_type = classify_account(pubkey, litesvm_account);
                output.push_str(&format!("✅ {} ({}): All fields match\n", pubkey, account_type));
            }
        }
    }

    output.push_str("\n📈 Account comparison summary:\n");
    output.push_str(&format!("  ✅ Matching accounts: {}\n", matches));
    output.push_str(&format!("  ❌ Differing accounts: {}\n", differences));
    output.push_str(&format!("  📊 Total compared: {}\n", common_accounts.len()));

    if differences > 0 {
        output.push_str(&format!("⚠️ Found {} accounts with data differences!\n", differences));
    } else {
        output.push_str("🎉 All common accounts have identical data!\n");
    }

    Ok(output)
}

pub fn analyze_account_sets(
    soon_accounts: &SoonAccounts,
    litesvm_accounts: &SoonAccounts,
) -> (
    HashSet<solana_sdk::pubkey::Pubkey>,
    HashSet<solana_sdk::pubkey::Pubkey>,
    HashSet<solana_sdk::pubkey::Pubkey>,
    String,
) {
    let mut output = String::new();
    output.push_str("\n=== Account Set Analysis ===\n");

    let soon_pubkeys: HashSet<_> = soon_accounts.accounts.iter().map(|(pk, _)| *pk).collect();
    let litesvm_pubkeys: HashSet<_> = litesvm_accounts.accounts.iter().map(|(pk, _)| *pk).collect();

    output.push_str(&format!("Soon storage accounts: {}\n", soon_pubkeys.len()));
    output.push_str(&format!("LiteSVM accounts: {}\n", litesvm_pubkeys.len()));

    // Find accounts only in LiteSVM (not in original Soon storage)
    let only_in_litesvm: HashSet<_> = litesvm_pubkeys.difference(&soon_pubkeys).cloned().collect();
    if !only_in_litesvm.is_empty() {
        output.push_str(&format!("\n🔍 Accounts only in LiteSVM ({}):\n", only_in_litesvm.len()));
        for pubkey in &only_in_litesvm {
            if let Some((_, account)) =
                litesvm_accounts.accounts.iter().find(|(pk, _)| pk == pubkey)
            {
                let account_type = classify_account(pubkey, account);
                output.push_str(&format!(
                    "  {}: {} (lamports: {}, data_len: {}, owner: {})\n",
                    pubkey,
                    account_type,
                    account.lamports(),
                    account.data().len(),
                    account.owner()
                ));
            }
        }
    } else {
        output.push_str("\n✅ No extra accounts in LiteSVM\n");
    }

    // Find accounts only in Soon storage (not imported to LiteSVM)
    let only_in_soon: HashSet<_> = soon_pubkeys.difference(&litesvm_pubkeys).cloned().collect();
    if !only_in_soon.is_empty() {
        output.push_str(&format!(
            "\n❌ Accounts in Soon storage but not in LiteSVM ({}):\n",
            only_in_soon.len()
        ));
        for pubkey in &only_in_soon {
            if let Some((_, account)) = soon_accounts.accounts.iter().find(|(pk, _)| pk == pubkey) {
                let account_type = classify_account(pubkey, account);
                output.push_str(&format!(
                    "  {}: {} (lamports: {}, data_len: {}, owner: {})\n",
                    pubkey,
                    account_type,
                    account.lamports(),
                    account.data().len(),
                    account.owner()
                ));
            }
        }
    } else {
        output.push_str("\n✅ All Soon storage accounts were imported to LiteSVM\n");
    }

    // Common accounts (intersection)
    let common_accounts: HashSet<_> =
        soon_pubkeys.intersection(&litesvm_pubkeys).cloned().collect();

    let mut mismatched = 0;
    let soon_map: std::collections::HashMap<_, _> =
        soon_accounts.accounts.iter().map(|(pk, acc)| (*pk, acc)).collect();
    let litesvm_map: std::collections::HashMap<_, _> =
        litesvm_accounts.accounts.iter().map(|(pk, acc)| (*pk, acc)).collect();
    let mut mismatched_accounts = Vec::new();
    for pubkey in &common_accounts {
        let soon_account = soon_map[pubkey];
        let litesvm_account = litesvm_map[pubkey];
        if soon_account.lamports() != litesvm_account.lamports()
            || soon_account.data() != litesvm_account.data()
            || soon_account.owner() != litesvm_account.owner()
            || soon_account.executable() != litesvm_account.executable()
            || soon_account.rent_epoch() != litesvm_account.rent_epoch()
        {
            mismatched += 1;
            mismatched_accounts.push((pubkey, soon_account, litesvm_account));
        }
    }
    output.push_str(&format!(
        "\n🤝 Common accounts in both systems: {} ({} mismatched)\n",
        common_accounts.len(),
        mismatched
    ));
    for (pubkey, soon_account, litesvm_account) in mismatched_accounts {
        if soon_account.data() == litesvm_account.data() {
            output.push_str(&format!(
                "❌ mismatched ({:?}), soon: {:?}, litesvm: {:?}\n",
                pubkey, soon_account, litesvm_account
            ));
        } else {
            output.push_str(&format!(
                "❌ mismatched ({:?}), soon: {:?}, litesvm: {:?}\nsoon data: {}\nlitesvm data: {}\n",
                pubkey,
                soon_account,
                litesvm_account,
                hex::encode(soon_account.data()),
                hex::encode(litesvm_account.data())
            ));
        }
    }

    (only_in_litesvm, only_in_soon, common_accounts, output)
}

pub fn assert_accounts_equal(soon_accounts: &SoonAccounts, litesvm_accounts: &SoonAccounts) {
    assert_eq!(soon_accounts.state_root(), litesvm_accounts.state_root());
}

pub fn litesvm_new_block(
    litesvm: &mut LiteSVM<impl AccountsCallback>,
    block: &RawBlock,
) -> Result<Vec<TransactionMetadata>> {
    let mut tx_results = Vec::new();
    for tx in &block.transactions {
        let tx_result = litesvm
            .send_transaction(tx.to_versioned_transaction())
            .map_err(|e| Error::FraudproofError(format!("{:?}, tx details: {:?}", e, tx)))?;
        tx_results.push(tx_result);
    }
    Ok(tx_results)
}
