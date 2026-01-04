# qdrant-lib

**Embedded Qdrant Vector Database for Rust Applications**

A modernized fork that runs Qdrant as an **in-process embedded library**, eliminating the need for a separate server. Aligned to Qdrant v1.16.3.

[![Rust](https://img.shields.io/badge/rust-1.89%2B-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![Build](https://img.shields.io/badge/build-passing-brightgreen.svg)]()

---

## 🎯 What This Is

| Standard Qdrant | This Library (qdrant-lib) |
|-----------------|---------------------------|
| Separate server process | **Embedded in your app** |
| gRPC/REST communication | **Direct function calls** |
| Network latency | **Zero network overhead** |
| Manage server lifecycle | **Automatic lifecycle** |

```rust
// Start embedded Qdrant with one line
let client = QdrantInstance::start(None)?;

// Use it directly - no network, no gRPC
client.create_collection("my_vectors", vectors_config).await?;
client.upsert_points("my_vectors", points).await?;
let results = client.search_points("my_vectors", search_request).await?;

// Health check
if client.is_healthy() {
    client.health_check().await?;
}
```

---

## 📋 Prerequisites

| Requirement | Version | Installation |
|-------------|---------|--------------|
| **Rust** | 1.89+ | [rustup.rs](https://rustup.rs) |
| **protoc** | 33.x+ | `winget install Google.Protobuf` |

### Quick Start

```powershell
# Clone with submodule
git clone --recurse-submodules https://github.com/EonsofStupid/qdrantrs-port.git
cd qdrantrs-port

# Or init submodule after clone
git submodule update --init --recursive

# Verify protoc
protoc --version  # Should show libprotoc 33.x

# Build
cargo build
```

---

## 📁 Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                         Your Application                         │
├─────────────────────────────────────────────────────────────────┤
│                          QdrantClient                            │
│                    (mpsc channel sender)                         │
├─────────────────────────────────────────────────────────────────┤
│                         qdrant thread                            │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │                    QdrantInstance                        │    │
│  │  ┌─────────────────┬─────────────────┬────────────────┐ │    │
│  │  │  Search Runtime │ Update Runtime  │ General Runtime│ │    │
│  │  │  (N-1 threads)  │ (optimizers)    │ (I/O + misc)   │ │    │
│  │  └─────────────────┴─────────────────┴────────────────┘ │    │
│  │                         ↓                                │    │
│  │              TableOfContent (ToC)                        │    │
│  │         ┌──────────┬──────────┬──────────┐              │    │
│  │         │Collection│Collection│Collection│              │    │
│  │         └──────────┴──────────┴──────────┘              │    │
│  └─────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────┘
```

---

## 🛡️ Hardening (Completed)

### ✅ Error Handling

All `panic!` calls replaced with proper error types:

```rust
#[derive(Error, Debug)]
pub enum QdrantError {
    #[error("Collection error: {0}")]
    Collection(#[from] CollectionError),
    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),
    #[error("Request timed out after {0:?}")]
    Timeout(Duration),
    #[error("Qdrant instance is shutting down")]
    ChannelClosed,
    #[error("Unexpected response type: expected {expected}, got {actual}")]
    UnexpectedResponse { expected: &'static str, actual: String },
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
```

### ✅ Request Timeout

30-second default timeout on all requests:

```rust
// Uses 30s default timeout
client.search_points("collection", request).await?;

// Internal: configurable via send_request_with_timeout()
```

### ✅ Graceful Shutdown

Proper shutdown with timeout instead of spin-wait:

```rust
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(30);

impl Drop for QdrantClient {
    fn drop(&mut self) {
        // Closes channel, waits up to 30s, logs result
    }
}
```

### ✅ Health Check

```rust
// Quick sync check - is channel open?
if client.is_healthy() {
    // Full async check - does instance respond?
    client.health_check().await?;
}
```

---

## 🗂️ Source File Map

| File | Lines | Purpose |
|------|-------|---------|
| `src/lib.rs` | 80 | Entry point, exports `QdrantClient`, `QdrantInstance` |
| `src/instance.rs` | 190 | Tokio runtime setup, spawns qdrant thread |
| `src/client.rs` | 493 | 30+ API methods with hardening |
| `src/error.rs` | 50 | Enhanced error types |
| `src/config.rs` | 78 | YAML configuration loading |
| `src/helpers.rs` | 69 | Runtime builders (search, update, general) |
| `src/ops/*.rs` | ~1100 | Collection/Points/Query operations |

---

## ⚙️ Configuration

### Default Config (`config/config.yaml`)

```yaml
storage:
  storage_path: ./.storage
  snapshots_path: ./.snapshots
  on_disk_payload: true
  
  performance:
    max_search_threads: 0        # auto = CPU-1
    max_optimization_threads: 1
    
  hnsw_index:
    m: 16
    ef_construct: 100
    on_disk: false

telemetry_disabled: false
```

### Environment Override

```bash
export QDRANT__STORAGE__STORAGE_PATH=/custom/path
export QDRANT__STORAGE__ON_DISK_PAYLOAD=false
```

---

## 📊 API Coverage

### ✅ Implemented

| Category | Methods |
|----------|---------|
| **Collections** | create, list, get, update, delete |
| **Aliases** | create, list, get, delete, rename |
| **Points** | upsert, get, delete, count, update_vectors, delete_vectors |
| **Payloads** | set, delete, clear |
| **Search** | search, search_batch, search_group_by |
| **Recommend** | recommend, recommend_batch, recommend_group_by |
| **Health** | is_healthy, health_check |

### 🔜 Roadmap

- Web Dashboard integration (embedded HTTP server)
- Metrics/observability endpoint
- Query API (universal query endpoint)

---

## 🔗 Dependencies

Qdrant internal crates via git submodule:

```
.modules/qdrant/lib/
├── api/         # REST/gRPC types
├── collection/  # Collection management
├── common/      # Shared utilities
├── segment/     # Vector segment storage
├── shard/       # Sharding logic
└── storage/     # Storage layer
```

### Update Qdrant Version

```bash
cd .modules/qdrant
git fetch origin
git checkout v1.17.0
cd ../..
cargo build
```

---

## 🧪 Development

### Build

```powershell
# Ensure PATH includes protoc and cargo
$env:Path = [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path","User")

cargo build
cargo test
```

### Workflow

See `.agent/workflows/cargo.md` for cargo PATH configuration.

---

## 📜 License

Apache 2.0 - Same as upstream Qdrant.

---

## 🙏 Credits

- [Qdrant Team](https://qdrant.tech/) - Original vector database
- Fork maintained by [@EonsofStupid](https://github.com/EonsofStupid)

---

## 📝 Changelog

### dev branch (2026-01-04)

**Hardening Release**
- ✅ Enhanced `QdrantError` with `Timeout`, `ChannelClosed`, `UnexpectedResponse`, `Io` variants
- ✅ Replaced all `panic!` calls with proper error returns
- ✅ Added 30s request timeout via `send_request_with_timeout()`
- ✅ Graceful shutdown with 30s timeout (replaces spin-wait)
- ✅ Added `is_healthy()` and `health_check()` methods
- ✅ Build verified with protoc 33.2
