use crate::error::DAProxyError;
use async_trait::async_trait;
use reqwest::{Client, Response};
use soon_derive::traits::DAProvider;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct DAProxyImpl {
    end_point: String,
}

impl DAProxyImpl {
    pub fn new_with_url(end_point: &str) -> Self {
        DAProxyImpl { end_point: end_point.to_string() }
    }

    pub async fn upload_preimage(&self, data: Vec<u8>) -> Result<Vec<u8>, DAProxyError> {
        let res = Client::new()
            .post(format!("{}put/", self.end_point))
            .header("Content-Type", "application/octet-stream")
            .body(data)
            .timeout(Duration::from_secs(900))
            .send()
            .await
            .map_err(|err| DAProxyError::ErrDAProxyUpload(err.to_string()))?;

        Ok(self.parse_response(res).await?)
    }

    pub async fn download_preimage(&self, commitment: Vec<u8>) -> Result<Vec<u8>, DAProxyError> {
        let res = Client::new()
            .get(format!("{}get/0x{}", self.end_point, hex::encode(commitment)))
            .timeout(Duration::from_secs(60))
            .send()
            .await
            .map_err(|err| DAProxyError::ErrDAProxyDownload(err.to_string()))?;

        Ok(self.parse_response(res).await?)
    }

    async fn parse_response(&self, res: Response) -> Result<Vec<u8>, DAProxyError> {
        if res.status().is_success() {
            Ok(Vec::from(
                res.bytes()
                    .await
                    .map_err(|err| DAProxyError::ErrDAProxyResponse(err.to_string()))?,
            ))
        } else {
            Err(DAProxyError::ErrDAProxyResponse(format!("receive status code:{}", res.status())))
        }
    }
}

#[async_trait]
impl DAProvider for DAProxyImpl {
    type Error = DAProxyError;
    async fn set_input(&self, data: Vec<u8>) -> Result<Vec<u8>, Self::Error> {
        self.upload_preimage(data).await
    }

    async fn get_input(&self, commitment: Vec<u8>) -> Result<Vec<u8>, Self::Error> {
        self.download_preimage(commitment).await
    }
}
