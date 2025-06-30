#![doc = "Soon-specific MPT utilities"]
#![cfg_attr(not(test), no_std)]

/// Hello world function for soon-mpt module
pub fn hello_world() -> &'static str {
    "Hello from soon-mpt!"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hello_world() {
        assert_eq!(hello_world(), "Hello from soon-mpt!");
    }
}