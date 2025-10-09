#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(any(test, feature = "test-utils")), warn(unused_crate_dependencies))]

extern crate alloc;

/// Required types and traits for derivation pipeline.
#[allow(ambiguous_glob_reexports)]
pub mod prelude {
    pub use crate::{
        attributes::*, batch::*, errors::*, pipeline::*, sources::*, stages::*, traits::*, types::*,
    };
}

pub mod attributes;
pub mod batch;
pub mod errors;
pub mod pipeline;
pub mod sources;
pub mod stages;
pub mod traits;
pub mod types;

#[cfg(any(test, feature = "test-utils"))]
pub mod test_utils;
