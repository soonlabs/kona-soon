use soon_derive::errors::{PipelineError, PipelineErrorKind};

#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum DAProxyError {
    #[error("da proxy upload image error: {0}")]
    ErrDAProxyUpload(String),

    #[error("da proxy download image error: {0}")]
    ErrDAProxyDownload(String),

    #[error("da proxy response data format error: {0}")]
    ErrDAProxyResponse(String),
}

impl From<DAProxyError> for PipelineErrorKind {
    fn from(e: DAProxyError) -> Self {
        match e {
            DAProxyError::ErrDAProxyDownload(e) => PipelineErrorKind::Temporary(
                PipelineError::Provider(format!("Da provider error: {e}")),
            ),
            _ => PipelineErrorKind::Temporary(PipelineError::Provider(
                "other da provider error".to_string(),
            )),
        }
    }
}
