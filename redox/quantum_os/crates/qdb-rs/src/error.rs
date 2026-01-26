//! Error types for QDB operations

use thiserror::Error;

/// Error type for QDB operations
#[derive(Error, Debug)]
pub enum QdbError {
    /// Key not found in database
    #[error("Key not found: {0}")]
    KeyNotFound(String),

    /// Key already exists (for insert operations)
    #[error("Key already exists: {0}")]
    KeyExists(String),

    /// Invalid key format
    #[error("Invalid key format: {0}")]
    InvalidKey(String),

    /// Value serialization/deserialization error
    #[error("Value error: {0}")]
    ValueError(String),

    /// QFT format error
    #[error("QFT error: {0}")]
    QftError(#[from] qft::QftError),

    /// I/O error
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Transaction error
    #[error("Transaction error: {0}")]
    TransactionError(String),

    /// Query error
    #[error("Query error: {0}")]
    QueryError(String),

    /// Backend error
    #[error("Backend error: {0}")]
    BackendError(String),

    /// Checksum mismatch
    #[error("Checksum mismatch for key: {0}")]
    ChecksumMismatch(String),

    /// Golay decode failed
    #[error("Golay error correction failed")]
    GolayDecodeFailed,

    /// Database is read-only
    #[error("Database is read-only")]
    ReadOnly,

    /// Database is closed
    #[error("Database is closed")]
    Closed,

    /// Concurrent modification detected
    #[error("Concurrent modification detected for key: {0}")]
    ConcurrentModification(String),

    /// Index error
    #[error("Index error: {0}")]
    IndexError(String),

    /// Capacity exceeded
    #[error("Capacity exceeded: {0}")]
    CapacityExceeded(String),
}

/// Result type alias for QDB operations
pub type Result<T> = std::result::Result<T, QdbError>;

impl From<serde_json::Error> for QdbError {
    fn from(e: serde_json::Error) -> Self {
        QdbError::SerializationError(e.to_string())
    }
}
