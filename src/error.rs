use thiserror::Error;

#[derive(Error, Debug)]
pub enum SuperbaseError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Data corruption error: CRC mismatch at offset {0}")]
    ChecksumMismatch(u64),

    #[error("Document not found with ID: {0}")]
    NotFound(String),

    #[error("Collection '{0}' already exists")]
    CollectionAlreadyExists(String),

    #[error("Collection '{0}' not found")]
    CollectionNotFound(String),

    #[error("Invalid query syntax: {0}")]
    InvalidQuery(String),

    #[error("Index error: {0}")]
    IndexError(String),

    #[error("Lock error: {0}")]
    LockError(String),

    #[error("Transaction/Operation aborted: {0}")]
    Aborted(String),

    #[error("Generic error: {0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, SuperbaseError>;
