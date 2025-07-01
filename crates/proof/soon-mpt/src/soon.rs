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
        // Use kona-mpt's TrieNode::insert with custom keys to match alloy_trie exactly
        use kona_mpt::{TrieNode, NoopTrieProvider};
        
        // Build kona-mpt trie using TrieNode::insert with custom keys
        let mut kona_root_node = TrieNode::Empty;
        let noop_provider = NoopTrieProvider;
        
        // Use the same sorted order as alloy_trie to ensure identical results
        for (key_hash, _addr, account) in sorted_data.iter() {
            let key_nibbles = Nibbles::unpack(key_hash);
            
            // Encode account using same sol_account_encoder
            let encoded_value = encoder(account);
            
            // Insert using TrieNode::insert with the exact same key as alloy_trie
            kona_root_node.insert(&key_nibbles, encoded_value.into(), &noop_provider)
                .expect("Failed to insert into kona trie");
        }
        
        let kona_root = kona_root_node.blind();
        println!("Kona MPT root:   {:?}", kona_root);
        
        // ===== COMPARISON =====
        assert_eq!(alloy_root, kona_root, "MPT roots should match between alloy_trie and kona-mpt");
        
        Ok(())
    }

    #[test]
    fn test_trie_provider_examples() -> Result<(), Box<dyn Error>> {
        use kona_mpt::{TrieNode, NoopTrieProvider, TrieProvider, Nibbles};
        use std::collections::HashMap;
        use alloy_primitives::{keccak256, B256, Bytes};
        use alloy_rlp::{Decodable, Encodable};

        println!("\n=== TrieProvider Usage Examples ===");

        // Example 1: NoopTrieProvider - suitable for building new trie from scratch
        println!("\n1. NoopTrieProvider Example:");
        let mut new_trie = TrieNode::Empty;
        let noop_provider = NoopTrieProvider;
        
        // When inserting into a fresh trie, no hash references are encountered,
        // so NoopTrieProvider is sufficient
        let key1 = Nibbles::unpack(&keccak256("key1"));
        let value1: Bytes = b"value1".to_vec().into();
        new_trie.insert(&key1, value1, &noop_provider)?;
        
        println!("  ✅ NoopTrieProvider works for fresh trie construction");
        
        // Example 2: Real TrieProvider - suitable for existing trie operations
        println!("\n2. Real TrieProvider Example:");
        
        // Simulate a simple in-memory database TrieProvider
        struct MemoryTrieProvider {
            preimages: HashMap<B256, Bytes>,
        }
        
        impl TrieProvider for MemoryTrieProvider {
            type Error = String;
            
            fn trie_node_by_hash(&self, key: B256) -> Result<TrieNode, Self::Error> {
                match self.preimages.get(&key) {
                    Some(rlp_encoded) => {
                        TrieNode::decode(&mut rlp_encoded.as_ref())
                            .map_err(|e| format!("Decode failed: {}", e))
                    }
                    None => Err(format!("Node not found for hash: {:?}", key))
                }
            }
        }
        
        // Build a provider with pre-stored node data
        let mut preimages = HashMap::new();
        
        // Create a leaf node and store its preimage
        let leaf_node = TrieNode::Leaf {
            prefix: Nibbles::unpack(&keccak256("existing_key")),
            value: b"existing_value".to_vec().into(),
        };
        
        let mut encoded = Vec::new();
        leaf_node.encode(&mut encoded);
        let leaf_hash = keccak256(&encoded);
        preimages.insert(leaf_hash, encoded.into());
        
        let memory_provider = MemoryTrieProvider { preimages };
        
        println!("  ✅ MemoryTrieProvider contains {} preimages", memory_provider.preimages.len());
        
        // Example 3: Why TrieProvider is needed for insert operations
        println!("\n3. Why insert operations need TrieProvider:");
        println!("  - MPT nodes may be stored as hashes to save memory");
        println!("  - When traversing to a hash reference, TrieProvider resolves the actual node");
        println!("  - In fresh trie construction, all nodes are in memory, no external resolution needed");
        println!("  - In existing large tries, real TrieProvider is needed to access stored nodes");
        
        // Example 4: When NoopTrieProvider fails
        println!("\n4. NoopTrieProvider limitations:");
        println!("  ⚠️  If hash node resolution is needed, NoopTrieProvider returns Empty");
        println!("  ⚠️  This may cause data loss or logic errors");
        println!("  ✅ Only use when certain no hash references will be encountered (fresh trie)");
        
        Ok(())
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
