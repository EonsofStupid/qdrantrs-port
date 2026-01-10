mod client;
mod config;
mod error;
mod helpers;
mod instance;
mod ops;

use std::backtrace::Backtrace;
use std::mem::ManuallyDrop;
use std::panic;
use std::thread::JoinHandle;
use storage::content_manager::toc::TableOfContent;
use tokio::sync::{mpsc, oneshot};
use tracing::error;

// Public types from api crate (REST schema)
pub use api::rest::schema::PointStruct;

// Vector params from collection
pub use collection::operations::types::VectorParams;

// Collection types
pub use collection::operations::types::{PointRequest, ScrollRequest, ScrollResult, SearchRequest};
pub use api::rest::schema::QueryRequest as CollectionQueryRequest; // Alias it for compatibility or clarity? Or just QueryRequestAPI?
// Actually client.rs uses it. Let's just use QueryRequest from api.
pub use api::rest::schema::QueryRequest as QueryPointsRequest;

pub use config::Settings;
pub use error::RROError;
pub use instance::RROInstance;
pub use instance::{RRORequest, RROResponse};
pub use ops::*;
pub use segment::types::{Distance, Payload, WithPayloadInterface};
pub use storage::content_manager::errors::StorageError;

// Re-exports for full access
pub use api;
pub use collection;
pub use common;
pub use segment;
pub use shard;
pub use storage;

type RROMsg = (RRORequest, RROResponder);
type RROResult = Result<RROResponse, StorageError>;
type RROResponder = oneshot::Sender<RROResult>;

#[derive(Debug)]
pub struct RroClient {
    tx: ManuallyDrop<mpsc::Sender<RROMsg>>,
    terminated_rx: oneshot::Receiver<()>,
    #[allow(dead_code)]
    handle: JoinHandle<Result<(), RROError>>,
}

#[async_trait::async_trait]
trait Handler {
    type Response;
    type Error;
    async fn handle(self, toc: &TableOfContent) -> Result<Self::Response, Self::Error>;
}

pub fn setup_panic_hook() {
    panic::set_hook(Box::new(move |panic_info| {
        let backtrace = Backtrace::force_capture().to_string();
        let loc = if let Some(loc) = panic_info.location() {
            format!(" in file {} at line {}", loc.file(), loc.line())
        } else {
            String::new()
        };
        let message = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            s
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s
        } else {
            "Payload not captured as it is not a string."
        };

        error!("Panic backtrace: \n{}", backtrace);
        error!("Panic occurred{loc}: {message}");
    }));
}
