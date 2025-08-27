use alloy_primitives::Keccak256;
use soon_primitives::mpt::TrieSolanaAccount as MptAccount;

pub(crate) fn sol_account_encoder(account: &MptAccount) -> Vec<u8> {
    if account.lamports == 0 {
        return vec![0u8; 32];
    }
    let mut hasher = Keccak256::new();
    // allocate 128 bytes buffer on the stack
    const BUF_SIZE: usize = 128;
    const TOTAL_FIELD_SIZE: usize = 8 /* lamports */ + 8 /* slot */ + 8 /* rent_epoch */ + 1 /* exec_flag */ + 32 /* owner_key */;
    const DATA_SIZE_CAN_FIT: usize = BUF_SIZE - TOTAL_FIELD_SIZE;
    let mut buffer = Vec::with_capacity(BUF_SIZE);
    // collect lamports, rent_epoch into buffer to hash
    // change little endian to big endian to follow solidity
    buffer.extend(account.lamports.to_be_bytes());
    buffer.extend(account.rent_epoch.to_be_bytes());
    if account.data.len() > DATA_SIZE_CAN_FIT {
        // For larger accounts whose data can't fit into the buffer, update the hash now.
        hasher.update(&buffer);
        buffer.clear();
        // hash account's data
        hasher.update(account.data.as_slice());
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
    hasher.update(&buffer);
    hasher.finalize().0.to_vec()
}
