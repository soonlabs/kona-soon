use crate::{HintType, errors::OracleProviderError};
use alloc::{boxed::Box, sync::Arc, vec::Vec};
use alloy_primitives::keccak256;
use async_trait::async_trait;
use kona_preimage::{CommsClient, PreimageKey};
use soon_derive::traits::DAProvider;

/// An oracle-backed blob provider.
#[derive(Debug, Clone)]
pub struct OracleDaProvider<T: CommsClient> {
    oracle: Arc<T>,
}

impl<T: CommsClient> OracleDaProvider<T> {
    /// Constructs a new `OracleBlobProvider`.
    pub const fn new(oracle: Arc<T>) -> Self {
        Self { oracle }
    }
}

#[async_trait]
impl<T: CommsClient + Sync + Send> DAProvider for OracleDaProvider<T> {
    type Error = OracleProviderError;

    async fn set_input(&self, data: Vec<u8>) -> Result<Vec<u8>, Self::Error> {
        unimplemented!()
    }
    /// fetch data by key from DA provider
    async fn get_input(&self, key: Vec<u8>) -> Result<Vec<u8>, Self::Error> {
        HintType::DAProxyBlob.with_data(&[key.as_ref()]).send(self.oracle.as_ref()).await?;
        let key_hash = keccak256::<&[u8]>(key.as_ref());
        Ok(self.oracle.get(PreimageKey::new_keccak256(*key_hash)).await?)
    }
}
