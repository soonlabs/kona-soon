use crate::mpt::account::TrieSolanaAccount as Account;
use alloy_rlp::Encodable;
use smallvec::SmallVec;

/// Handle the value encoding into the leaf node.
/// In Ethereum, the default encode method is rlp(account),
/// use this type to customize the encoding method.
///
/// When Ethereum verifies the proof, what actually happened is that the proof program
/// will decode the leaf node from the proof, and just check whether this node is equal
/// to given encoded leaf node, so the encoding method itself can be anyone, only the
/// bytes array after encoding matters.
pub type TrieNodeValueEncoder = Box<dyn Fn(&Account) -> Vec<u8>>;

pub fn eth_rlp_encoder() -> TrieNodeValueEncoder {
    Box::new(|account: &Account| {
        // if data is empty, account field is nearly 50 * len(u8), so we apply a 64 bytes buffer.
        // if data is not empty(contract address), the length should be dominated by data field.
        let mut account_rlp = Vec::with_capacity(account.data.len().next_power_of_two().min(64));
        account.encode(&mut account_rlp);
        account_rlp
    })
}

/// Default hashing method of Solana Account.
/// This function will align the data format of the account in the contract verification.
///
/// Overall, in comparison.
/// In geth, the value node format of trie is rlp(account).
/// In solana, the value node format is keccak(account).
pub fn sol_account_encoder() -> TrieNodeValueEncoder {
    Box::new(|account: &Account| {
        if account.lamports == 0 {
            return vec![0u8; 32];
        }
        let mut hasher = solana_program::keccak::Hasher::default();
        // allocate 128 bytes buffer on the stack
        const BUF_SIZE: usize = 128;
        const TOTAL_FIELD_SIZE: usize = 8 /* lamports */ + 8 /* slot */ + 8 /* rent_epoch */ + 1 /* exec_flag */ + 32 /* owner_key */;
        const DATA_SIZE_CAN_FIT: usize = BUF_SIZE - TOTAL_FIELD_SIZE;
        let mut buffer = SmallVec::<[u8; BUF_SIZE]>::new();
        // collect lamports, rent_epoch into buffer to hash
        // change little endian to big endian to follow solidity
        buffer.extend_from_slice(&account.lamports.to_be_bytes());
        buffer.extend_from_slice(&account.rent_epoch.to_be_bytes());
        if account.data.len() > DATA_SIZE_CAN_FIT {
            // For larger accounts whose data can't fit into the buffer, update the hash now.
            hasher.hash(&buffer);
            buffer.clear();

            // hash account's data
            hasher.hash(account.data.as_slice());
        } else {
            // For small accounts whose data can fit into the buffer, append it to the buffer.
            buffer.extend_from_slice(account.data.as_slice());
        }
        // collect exec_flag, owner, pubkey into buffer to hash
        if account.executable {
            buffer.push(1_u8);
        } else {
            buffer.push(0_u8);
        }
        buffer.extend_from_slice(account.owner.as_ref());
        hasher.hash(&buffer);
        hasher.result().0.to_vec()
    })
}

/// Same format with sol_account_encoder, but only encode data field.
/// It's only used for withdrawal merkle root calculation.
/// Reason of this function is that we only involve those field with determinacy.
/// Some field like lamports, rent_epoch, can be maliciously modified by anyone to attack user withdraw.
pub fn withdrawal_account_encoder() -> TrieNodeValueEncoder {
    Box::new(|account: &Account| account.data.as_slice().to_owned())
}
