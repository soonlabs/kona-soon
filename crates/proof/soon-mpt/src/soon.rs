/// Some primitives cloned from soon.
/// TODO Just take reference
use alloy_primitives::B256;

use core::ops::Deref;
use core::fmt::{Debug, Formatter};
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;

#[derive(Default, Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct TrieSolanaPubkey(pub Pubkey);

impl Deref for TrieSolanaPubkey {
    type Target = Pubkey;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Default, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct TrieSolanaAccount {
    /// lamports in the account
    pub lamports: u64,
    /// data held in this account
    pub data: B256,
    /// this account's data contains a loaded program (and is now read-only)
    pub executable: bool,
    /// the epoch at which this account will next owe rent
    pub rent_epoch: u64,
    /// the program that owns this account. If executable, the program that loads this account.
    pub owner: TrieSolanaPubkey,
}

impl Debug for TrieSolanaAccount {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TrieSolanaAccount")
            .field("lamports", &self.lamports)
            .field("data", &self.data)
            .field("executable", &self.executable)
            .field("rent_epoch", &self.rent_epoch)
            .field("owner", &self.owner)
            .finish()
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::soon::TrieSolanaAccount;
    use alloy_primitives::Address;
    use alloy_primitives::hex::FromHex;
    use std::boxed::Box;
    use std::error::Error;
    use std::vec::Vec;

    pub(crate) static TEST_DATA_FILE: &str = "testdata/test.json";

    #[test]
    fn test_mpt_same_kona_alloy() -> Result<(), Box<dyn Error>> {
        use alloy_trie::{HashBuilder, Nibbles};
        use crate::encoder::sol_account_encoder;
        use alloy_primitives::keccak256;
        
        let data = read_test_data();
        println!("Data: {:?}", data);
        
        let encoder = sol_account_encoder();
        
        // ===== ALLOY_TRIE IMPLEMENTATION (existing) =====
        let mut hb = HashBuilder::default();
        
        // Sort by key hash to ensure correct order for HashBuilder
        let mut sorted_data: Vec<_> = data.iter()
            .map(|(addr, account)| {
                let key_hash = keccak256(addr.as_slice());
                (key_hash, addr, account)
            })
            .collect();
        sorted_data.sort_by(|a, b| a.0.cmp(&b.0));
        
        for (key_hash, _addr, account) in sorted_data.iter() {
            let key_nibbles = Nibbles::unpack(key_hash);
            
            // Encode account using sol_account_encoder
            let encoded_value = encoder(account);
            
            hb.add_leaf(key_nibbles, &encoded_value);
        }
        
        let alloy_root = hb.root();
        println!("Alloy trie root: {:?}", alloy_root);
        
        // ===== KONA-MPT IMPLEMENTATION (new) =====
        // Use kona-mpt's TrieNode to manually build the trie structure
        use kona_mpt::TrieNode;
        
        // Build kona-mpt trie using TrieNode structure
        let mut kona_root_node = TrieNode::Empty;
        
        // Use the same sorted order as alloy_trie
        for (key_hash, _addr, account) in sorted_data.iter() {
            let key_nibbles = Nibbles::unpack(key_hash);
            
            // Encode account using same sol_account_encoder
            let encoded_value = encoder(account);
            
            // Insert this key-value pair into the kona trie
            kona_root_node = insert_into_kona_trie(kona_root_node, key_nibbles, encoded_value.into());
        }
        
        let kona_root = kona_root_node.blind();
        println!("Kona MPT root:   {:?}", kona_root);
        
        // ===== COMPARISON =====
        assert_eq!(alloy_root, kona_root, "MPT roots should match between alloy_trie and kona-mpt");
        
        Ok(())
    }

    /// Manually insert a key-value pair into a kona-mpt TrieNode structure
    fn insert_into_kona_trie(node: kona_mpt::TrieNode, key: alloy_trie::Nibbles, value: alloy_primitives::Bytes) -> kona_mpt::TrieNode {
        use kona_mpt::TrieNode;
        use alloy_trie::Nibbles;
        
        match node {
            TrieNode::Empty => {
                // For empty node, create a leaf
                TrieNode::Leaf { prefix: key, value }
            }
            TrieNode::Leaf { prefix: existing_key, value: existing_value } => {
                // Find common prefix between existing key and new key
                let common_len = find_common_prefix_length(&existing_key, &key);
                
                if common_len == existing_key.len() && common_len == key.len() {
                    // Keys are identical, replace value
                    TrieNode::Leaf { prefix: key, value }
                } else if common_len == existing_key.len() {
                    // New key extends existing key, need to create extension + branch
                    let remaining_key = key.slice(common_len..);
                    let mut branch_stack = vec![TrieNode::Empty; 17];
                    if !remaining_key.is_empty() {
                        let first_nibble = remaining_key[0] as usize;
                        let rest_key = remaining_key.slice(1..);
                        branch_stack[first_nibble] = if rest_key.is_empty() {
                            TrieNode::Leaf { prefix: Nibbles::default(), value }
                        } else {
                            TrieNode::Leaf { prefix: rest_key, value }
                        };
                    }
                    // Put existing value in branch value position (index 16)
                    branch_stack[16] = TrieNode::Leaf { prefix: alloy_trie::Nibbles::default(), value: existing_value };
                    
                    if common_len > 0 {
                        TrieNode::Extension {
                            prefix: existing_key.slice(..common_len),
                            node: Box::new(TrieNode::Branch { stack: branch_stack })
                        }
                    } else {
                        TrieNode::Branch { stack: branch_stack }
                    }
                } else if common_len == key.len() {
                    // Existing key extends new key
                    let remaining_existing = existing_key.slice(common_len..);
                    let mut branch_stack = vec![TrieNode::Empty; 17];
                    let first_nibble = remaining_existing[0] as usize;
                    let rest_existing = remaining_existing.slice(1..);
                    branch_stack[first_nibble] = if rest_existing.is_empty() {
                        TrieNode::Leaf { prefix: alloy_trie::Nibbles::default(), value: existing_value }
                    } else {
                        TrieNode::Leaf { prefix: rest_existing, value: existing_value }
                    };
                    // Put new value in branch value position (index 16)
                    branch_stack[16] = TrieNode::Leaf { prefix: alloy_trie::Nibbles::default(), value };
                    
                    if common_len > 0 {
                        TrieNode::Extension {
                            prefix: key.slice(..common_len),
                            node: Box::new(TrieNode::Branch { stack: branch_stack })
                        }
                    } else {
                        TrieNode::Branch { stack: branch_stack }
                    }
                } else {
                    // Neither key is prefix of the other, create branch
                    let mut branch_stack = vec![TrieNode::Empty; 17];
                    
                    // Insert existing key-value
                    let remaining_existing = existing_key.slice(common_len..);
                    if !remaining_existing.is_empty() {
                        let first_nibble = remaining_existing[0] as usize;
                        let rest_existing = remaining_existing.slice(1..);
                        branch_stack[first_nibble] = if rest_existing.is_empty() {
                            TrieNode::Leaf { prefix: alloy_trie::Nibbles::default(), value: existing_value }
                        } else {
                            TrieNode::Leaf { prefix: rest_existing, value: existing_value }
                        };
                    }
                    
                    // Insert new key-value  
                    let remaining_new = key.slice(common_len..);
                    if !remaining_new.is_empty() {
                        let first_nibble = remaining_new[0] as usize;
                        let rest_new = remaining_new.slice(1..);
                        branch_stack[first_nibble] = if rest_new.is_empty() {
                            TrieNode::Leaf { prefix: alloy_trie::Nibbles::default(), value }
                        } else {
                            TrieNode::Leaf { prefix: rest_new, value }
                        };
                    }
                    
                    if common_len > 0 {
                        TrieNode::Extension {
                            prefix: key.slice(..common_len),
                            node: Box::new(TrieNode::Branch { stack: branch_stack })
                        }
                    } else {
                        TrieNode::Branch { stack: branch_stack }
                    }
                }
            }
            TrieNode::Extension { prefix, mut node } => {
                let common_len = find_common_prefix_length(&prefix, &key);
                
                if common_len == prefix.len() {
                    // New key extends beyond extension, recurse into child
                    let remaining_key = key.slice(common_len..);
                    *node = insert_into_kona_trie(*node, remaining_key, value);
                    TrieNode::Extension { prefix, node }
                } else {
                    // Extension needs to be split
                    let mut branch_stack = vec![TrieNode::Empty; 17];
                    
                    // Handle remaining extension
                    let remaining_extension = prefix.slice(common_len..);
                    if !remaining_extension.is_empty() {
                        let first_nibble = remaining_extension[0] as usize;
                        let rest_extension = remaining_extension.slice(1..);
                        branch_stack[first_nibble] = if rest_extension.is_empty() {
                            *node
                        } else {
                            TrieNode::Extension { prefix: rest_extension, node }
                        };
                    }
                    
                    // Handle new key
                    let remaining_new = key.slice(common_len..);
                    if !remaining_new.is_empty() {
                        let first_nibble = remaining_new[0] as usize;
                        let rest_new = remaining_new.slice(1..);
                        branch_stack[first_nibble] = if rest_new.is_empty() {
                            TrieNode::Leaf { prefix: alloy_trie::Nibbles::default(), value }
                        } else {
                            TrieNode::Leaf { prefix: rest_new, value }
                        };
                    }
                    
                    if common_len > 0 {
                        TrieNode::Extension {
                            prefix: prefix.slice(..common_len),
                            node: Box::new(TrieNode::Branch { stack: branch_stack })
                        }
                    } else {
                        TrieNode::Branch { stack: branch_stack }
                    }
                }
            }
            TrieNode::Branch { mut stack } => {
                if key.is_empty() {
                    // Insert into branch value position (index 16)
                    stack[16] = TrieNode::Leaf { prefix: alloy_trie::Nibbles::default(), value };
                } else {
                    // Insert into appropriate child
                    let first_nibble = key[0] as usize;
                    let remaining_key = key.slice(1..);
                    stack[first_nibble] = insert_into_kona_trie(stack[first_nibble].clone(), remaining_key, value);
                }
                TrieNode::Branch { stack }
            }
            _ => {
                // For other node types, return as-is (this shouldn't happen in normal operation)
                node
            }
        }
    }

    /// Find the length of common prefix between two Nibbles
    fn find_common_prefix_length(a: &alloy_trie::Nibbles, b: &alloy_trie::Nibbles) -> usize {
        let min_len = a.len().min(b.len());
        for i in 0..min_len {
            if a[i] != b[i] {
                return i;
            }
        }
        min_len
    }

    fn read_test_data() -> Vec<(Address, TrieSolanaAccount)> {
        use std::collections::HashMap;
        use std::fs;

        let content = fs::read_to_string(TEST_DATA_FILE).expect("Failed to read test data file");

        let json: HashMap<String, serde_json::Value> =
            serde_json::from_str(&content).expect("Failed to parse JSON");

        let mut data = vec![];
        let one = B256::left_padding_from(&[1u8]);

        for (addr, balance) in json {
            let addr = Address::from_hex(&addr).unwrap();
            let balance = balance.get("balance").unwrap().as_str().unwrap();
            let balance =
                balance.trim_start_matches("0x").chars().rev().take(16).collect::<String>();
            let lamports = u64::from_str_radix(&balance, 16).unwrap();
            let account = TrieSolanaAccount { data: one, lamports, ..Default::default() };
            data.push((addr, account));
        }
        data
    }
}
