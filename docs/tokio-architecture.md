# qdrant-lib Tokio/Async Architecture

## Overview

The embedded qdrant-lib uses Tokio for async runtime and inter-thread communication.

---

## File-by-File Tokio Usage

### 1. `src/lib.rs` (Lines 13, 41-43)

**Purpose:** Type definitions for message passing

```rust
use tokio::sync::{mpsc, oneshot};

type QdrantMsg = (QdrantRequest, QdrantResponder);
type QdrantResult = Result<QdrantResponse, StorageError>;
type QdrantResponder = oneshot::Sender<QdrantResult>;
```

**What it does:**
- `mpsc::Sender<QdrantMsg>` - Multi-producer channel for requests
- `oneshot::Sender<QdrantResult>` - One-shot channel for responses

---

### 2. `src/instance.rs` (Lines 15-18, 43-63)

**Purpose:** Spawns Qdrant thread and message loop

```rust
use tokio::{
    runtime::Handle,
    sync::{mpsc, oneshot},
};

// Channel creation
let (tx, mut rx) = mpsc::channel::<QdrantMsg>(QDRANT_CHANNEL_BUFFER);
let (terminated_tx, terminated_rx) = oneshot::channel::<()>();

// Message loop
rt.block_on(async move {
    while let Some((msg, resp_sender)) = rx.recv().await {
        let toc_clone = toc.clone();
        tokio::spawn(async move {
            let res = msg.handle(&toc_clone).await;
            resp_sender.send(res);
        });
    }
});
```

**What it does:**
- Creates mpsc channel for client → qdrant communication
- Creates oneshot channel for shutdown signaling
- Spawns new Tokio task for each request (concurrent handling)

---

### 3. `src/helpers.rs` (Lines 5, 7-68)

**Purpose:** Creates 3 Tokio runtimes

```rust
use tokio::runtime::{self, Runtime};

// Search Runtime - CPU-bound operations
pub fn create_search_runtime(max_search_threads: usize) -> io::Result<Runtime>

// Update Runtime - Optimizer/indexing operations
pub fn create_update_runtime(max_optimization_threads: usize) -> io::Result<Runtime>

// General Runtime - I/O and misc async tasks
pub fn create_general_purpose_runtime() -> io::Result<Runtime>
```

**What it does:**
- **Search Runtime:** N-1 CPU threads for vector search
- **Update Runtime:** Configurable threads for index optimization
- **General Runtime:** I/O, misc async operations

---

### 4. `src/client.rs` (Lines 20-23, 73-86, 479-489)

**Purpose:** Request/response handling with timeouts

```rust
use tokio::sync::{
    mpsc,
    oneshot::{self, error::TryRecvError},
};

// Health check with timeout
pub async fn health_check(&self) -> Result<(), QdrantError> {
    let timeout = Duration::from_secs(5);
    let (tx, rx) = oneshot::channel::<QdrantResult>();
    
    self.tx.send((msg, tx)).await.map_err(|_| QdrantError::ChannelClosed)?;
    
    match tokio::time::timeout(timeout, rx).await {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(_)) => Err(QdrantError::ChannelClosed),
        Err(_) => Err(QdrantError::Timeout(timeout)),
    }
}

// Send request with timeout (internal)
async fn send_request_with_timeout(&self, msg, timeout) -> Result<...> {
    match tokio::time::timeout(timeout, rx).await {
        // ... timeout handling
    }
}
```

**What it does:**
- `tokio::time::timeout` - All requests have 30s default timeout
- `oneshot::channel` - Per-request response channel
- `mpsc::Sender::send` - Async send to qdrant thread

---

### 5. `src/error.rs` (Line 4)

**Purpose:** Error types for channel failures

```rust
use tokio::sync::oneshot;

#[derive(Error, Debug)]
pub enum QdrantError {
    #[error("Response channel closed: {0}")]
    ResponseRecv(#[from] oneshot::error::RecvError),
    
    #[error("Request timed out after {0:?}")]
    Timeout(Duration),
    
    #[error("Qdrant instance is shutting down")]
    ChannelClosed,
}
```

---

## Communication Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                        Your Application                          │
│                                                                  │
│   async fn example() {                                           │
│       client.search_points(...).await?;  // Caller is async     │
│   }                                                              │
└───────────────────────────────┬─────────────────────────────────┘
                                │
                   mpsc::Sender │ (async send)
                                ▼
                    ┌───────────────────────┐
                    │    mpsc::channel      │
                    │  (buffered: 1024)     │
                    └───────────────────────┘
                                │
                   mpsc::Receiver (rx.recv().await)
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                      Qdrant Thread                               │
│                                                                  │
│   rt.block_on(async {                                           │
│       while let Some((msg, resp_sender)) = rx.recv().await {    │
│           tokio::spawn(async move {                             │
│               let res = msg.handle(&toc).await;                 │
│               resp_sender.send(res);  // oneshot response       │
│           });                                                   │
│       }                                                         │
│   });                                                           │
└───────────────────────────────┬─────────────────────────────────┘
                                │
                   oneshot::Sender (response)
                                ▼
                    ┌───────────────────────┐
                    │   oneshot::channel    │
                    │  (per-request)        │
                    └───────────────────────┘
                                │
                   oneshot::Receiver (rx.await)
                   + tokio::time::timeout
                                ▼
                    ┌───────────────────────┐
                    │     Result<T, E>      │
                    └───────────────────────┘
```

---

## Runtime Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                      Embedded Qdrant                             │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │               Qdrant Thread (std::thread)                │    │
│  │                                                          │    │
│  │  ┌────────────────┐ ┌────────────────┐ ┌──────────────┐ │    │
│  │  │ search_runtime │ │ update_runtime │ │general_runtime│ │    │
│  │  │                │ │                │ │              │ │    │
│  │  │ N-1 workers    │ │ Configurable   │ │ N workers    │ │    │
│  │  │ (CPU-bound)    │ │ (optimizers)   │ │ (I/O)        │ │    │
│  │  └────────────────┘ └────────────────┘ └──────────────┘ │    │
│  │                         │                                │    │
│  │                         ▼                                │    │
│  │                  TableOfContent                          │    │
│  │                  (storage layer)                         │    │
│  └──────────────────────────────────────────────────────────┘    │
│                              ▲                                   │
│                              │ mpsc channel                      │
│                              │                                   │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │                    QdrantClient                           │   │
│  │              (your async code calls here)                 │   │
│  └──────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

---

## Key Constants

| Constant | Location | Value | Purpose |
|----------|----------|-------|---------|
| `QDRANT_CHANNEL_BUFFER` | instance.rs:21 | 1024 | mpsc channel capacity |
| `SHUTDOWN_TIMEOUT` | client.rs:27 | 30 seconds | Max wait for graceful shutdown |
| Default request timeout | client.rs | 30 seconds | Per-request timeout |
| Health check timeout | client.rs:75 | 5 seconds | Quick health probe |

---

## Integration Points for Dashboard

When adding dashboard/Tauri integration, you'll interface through:

1. **QdrantClient** - Already async-ready for Tauri commands
2. **Existing methods** - All return `Result<T, QdrantError>`
3. **No additional Tokio setup** - Tauri has its own async runtime

```rust
// Tauri command example
#[tauri::command]
async fn list_collections(state: State<'_, Arc<QdrantClient>>) -> Result<Vec<String>, String> {
    state.list_collections()
        .await
        .map_err(|e| e.to_string())
}
```

The qdrant-lib is already async-first. Tauri integration is straightforward.
