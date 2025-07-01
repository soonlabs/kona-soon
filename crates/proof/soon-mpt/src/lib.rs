#![doc = "Soon-specific MPT utilities"]
#![cfg_attr(not(test), no_std)]

#[cfg(not(test))]
#[allow(unused_extern_crates)]
extern crate alloc;

/// Hello world function for soon-mpt module
pub fn hello_world() -> &'static str {
    "Hello from soon-mpt!"
}

mod soon;
mod traits;
mod encoder;

pub use soon::{TrieSolanaAccount, TrieSolanaPubkey};
pub type Account = TrieSolanaAccount;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hello_world() {
        assert_eq!(hello_world(), "Hello from soon-mpt!");
    }
}
