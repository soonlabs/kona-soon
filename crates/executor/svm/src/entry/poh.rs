use solana_program::hash::{Hash, hash, hashv};

const LOW_POWER_MODE: u64 = u64::MAX;

pub(super) struct Poh {
    pub hash: Hash,
    num_hashes: u64,
    hashes_per_tick: u64,
    remaining_hashes: u64,
    tick_number: u64,
}

#[derive(Debug)]
pub(super) struct PohEntry {
    pub hash: Hash,
}

impl Poh {
    pub(super) fn new(hash: Hash, hashes_per_tick: Option<u64>) -> Self {
        Self::new_with_slot_info(hash, hashes_per_tick, 0)
    }

    pub(super) fn new_with_slot_info(
        hash: Hash,
        hashes_per_tick: Option<u64>,
        tick_number: u64,
    ) -> Self {
        let hashes_per_tick = hashes_per_tick.unwrap_or(LOW_POWER_MODE);
        assert!(hashes_per_tick > 1);
        Poh { hash, num_hashes: 0, hashes_per_tick, remaining_hashes: hashes_per_tick, tick_number }
    }

    pub(super) fn hash(&mut self, max_num_hashes: u64) -> bool {
        let num_hashes = std::cmp::min(self.remaining_hashes - 1, max_num_hashes);

        for _ in 0..num_hashes {
            self.hash = hash(self.hash.as_ref());
        }
        self.num_hashes += num_hashes;
        self.remaining_hashes -= num_hashes;

        assert!(self.remaining_hashes > 0);
        self.remaining_hashes == 1 // Return `true` if caller needs to `tick()` next
    }

    pub(super) fn record(&mut self, mixin: Hash) -> Option<PohEntry> {
        if self.remaining_hashes == 1 {
            return None; // Caller needs to `tick()` first
        }

        self.hash = hashv(&[self.hash.as_ref(), mixin.as_ref()]);
        self.num_hashes = 0;
        self.remaining_hashes -= 1;

        Some(PohEntry { hash: self.hash })
    }

    pub(super) fn tick(&mut self) -> Option<PohEntry> {
        self.hash = hash(self.hash.as_ref());
        self.num_hashes += 1;
        self.remaining_hashes -= 1;

        // If we are in low power mode then always generate a tick.
        // Otherwise only tick if there are no remaining hashes
        if self.hashes_per_tick != LOW_POWER_MODE && self.remaining_hashes != 0 {
            return None;
        }

        self.remaining_hashes = self.hashes_per_tick;
        self.num_hashes = 0;
        self.tick_number += 1;
        Some(PohEntry { hash: self.hash })
    }
}
