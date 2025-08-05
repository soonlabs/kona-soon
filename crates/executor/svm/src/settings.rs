use solana_program::clock::MAX_PROCESSING_AGE;

pub struct Settings {
    pub max_age: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            max_age: MAX_PROCESSING_AGE,
        }
    }
}