#![allow(deprecated)]

use serde::{Deserialize, Serialize};
use solana_program::{
    clock::MAX_PROCESSING_AGE,
    declare_id,
    pubkey::Pubkey,
    sysvar::{
        Sysvar, SysvarId,
        recent_blockhashes::{Entry, IterItem},
    },
};
use std::ops::Deref;

declare_id!("SoonSysvarRecentB1ockHashes1111111111111111");

pub const MAX_ENTRIES: usize = MAX_PROCESSING_AGE;

#[repr(C)]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct SoonRecentBlockhashes(Vec<Entry>);

impl SysvarId for SoonRecentBlockhashes {
    fn id() -> Pubkey {
        id()
    }

    fn check_id(pubkey: &Pubkey) -> bool {
        check_id(pubkey)
    }
}

impl Default for SoonRecentBlockhashes {
    fn default() -> Self {
        Self(Vec::with_capacity(MAX_ENTRIES))
    }
}

impl<'a> FromIterator<IterItem<'a>> for SoonRecentBlockhashes {
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = IterItem<'a>>,
    {
        let mut new = Self::default();
        for i in iter {
            new.0.push(Entry::new(i.1, i.2))
        }
        new
    }
}

impl Sysvar for SoonRecentBlockhashes {
    fn size_of() -> usize {
        // hard-coded so that we don't have to construct an empty
        40 * MAX_ENTRIES + 8 // 40 bytes for each Entry and 8 bytes for the length
    }
}

impl Deref for SoonRecentBlockhashes {
    type Target = Vec<Entry>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
