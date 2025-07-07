use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Fraudproof error: {0}")]
    FraudproofError(String),

    #[error("LiteSVM error: {0}")]
    LiteSVMError(#[from] litesvm::error::LiteSVMError),
}
