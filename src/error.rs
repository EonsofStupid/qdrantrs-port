use collection::operations::types::CollectionError;
use storage::content_manager::errors::StorageError;
use thiserror::Error;
use tokio::sync::oneshot;
use std::time::Duration;

/// RRO embedded library errors
#[derive(Error, Debug)]
pub enum RROError {
    /// Error from collection operations
    #[error("Collection error: {0}")]
    Collection(#[from] CollectionError),

    /// Error from storage layer
    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),

    /// Failed to receive response from rro thread
    #[error("Response channel closed: {0}")]
    ResponseRecv(#[from] oneshot::error::RecvError),

    /// Request timed out
    #[error("Request timed out after {0:?}")]
    Timeout(Duration),

    /// Channel to rro thread is closed (instance shutting down)
    #[error("RRO instance is shutting down")]
    ChannelClosed,

    /// Received unexpected response type (internal error)
    #[error("Unexpected response type: expected {expected}, got {actual}")]
    UnexpectedResponse {
        expected: &'static str,
        actual: String,
    },

    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

impl RROError {
    /// Create an unexpected response error
    pub fn unexpected<T: std::fmt::Debug>(expected: &'static str, actual: T) -> Self {
        RROError::UnexpectedResponse {
            expected,
            actual: format!("{:?}", actual),
        }
    }
}
