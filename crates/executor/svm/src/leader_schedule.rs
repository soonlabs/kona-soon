use itertools::Itertools;
use serde::{Deserialize, Serialize};
use solana_program::clock::Slot;
use solana_program::pubkey::Pubkey;

#[derive(Debug, Default, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct LeaderSchedule {
    leaders: Vec<(Slot, Pubkey)>,
}

impl LeaderSchedule {
    pub fn new(leaders: Vec<(Slot, Pubkey)>) -> Self {
        Self::from_iter(leaders)
    }
    
    pub fn one(leader: Pubkey) -> Self {
        Self {
            leaders: vec![(0, leader)],
        }
    }

    pub fn leader_at_slot(&self, slot: Slot) -> Option<Pubkey> {
        match self.leaders.binary_search_by_key(&slot, |(s, _)| *s) {
            Ok(i) => Some(self.leaders[i].1),
            Err(i) => {
                if i == 0 {
                    None // No leader for this slot
                } else {
                    Some(self.leaders[i - 1].1) // Return the last known leader before this slot
                }
            }
        }
    }
}

impl FromIterator<(Slot, Pubkey)> for LeaderSchedule {
    fn from_iter<T: IntoIterator<Item=(Slot, Pubkey)>>(iter: T) -> Self {
        let mut leaders = iter.into_iter().unique_by(|(slot, _)| *slot).collect::<Vec<_>>();
        leaders.sort_unstable_by_key(|(s, _)| *s);
        Self { leaders }
    }
}