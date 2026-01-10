# rro-lib

**Embedded RRO Vector Database for Rust Applications**

A modernized fork that runs RRO as an **in-process embedded library**, eliminating the need for a separate server. Aligned to RRO v1.16.3.

[![Rust](https://img.shields.io/badge/rust-1.89%2B-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)

---

## 🎯 What This Is

| Standard RRO | This Library (rro-lib) |
|-----------------|---------------------------|
| Separate server process | **Embedded in your app** |
| gRPC/REST communication | **Direct function calls** |
| Network latency | **Zero network overhead** |
| Manage server lifecycle | **Automatic lifecycle** |

```rust
// Start embedded RRO with one line
let client = RROInstance::start(None)?;

// Use it directly - no network, no gRPC
client.create_collection("my_vectors", vectors_config).await?;
client.upsert_points("my_vectors", points).await?;
let results = client.search_points("my_vectors", search_request).await?;
```

---

## 📁 Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                         Your Application                         │
├─────────────────────────────────────────────────────────────────┤
│                          RroClient                            │
│                    (mpsc channel sender)                         │
├─────────────────────────────────────────────────────────────────┤
│                         rro thread                            │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │                    RROInstance                        │    │
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

## 🗂️ Source File Map

### Core Files

| File | Lines | Purpose |
|------|-------|---------|
| `src/lib.rs` | 80 | Entry point, exports `RroClient`, `RROInstance`, `Settings` |
| `src/instance.rs` | 190 | **Tokio runtime setup**, spawns rro thread, message loop |
| `src/client.rs` | 430 | **30+ API methods** for collections, points, search, payloads |
| `src/config.rs` | 78 | YAML configuration loading via `config` crate |
| `src/helpers.rs` | 69 | Runtime builders (search, update, general) |
| `src/error.rs` | 15 | Error types wrapping `CollectionError`, `StorageError` |
| `src/snapshots.rs` | 143 | Snapshot recovery (currently unused) |

### Operations Module (`src/ops/`)

| File | Lines | Purpose |
|------|-------|---------|
| `mod.rs` | 20 | Exports + shard selector helper |
| `collections.rs` | 243 | Collection/Alias CRUD with RBAC |
| `points.rs` | ~600 | Point operations (upsert, delete, update vectors) |
| `query.rs` | ~300 | Search, recommend, scroll operations |

---

## 🔧 Custom Code Sections

### 1. **Three-Runtime Tokio Architecture** (`helpers.rs`)

```rust
// Search Runtime: CPU-bound, (N-1) threads
create_search_runtime(max_search_threads)

// Update Runtime: Optimizers, configurable threads
create_update_runtime(max_optimization_threads)

// General Runtime: I/O + async tasks
create_general_purpose_runtime()
```

> **Why 3 Runtimes?** Prevents search latency spikes during heavy indexing.

### 2. **Message-Passing Architecture** (`instance.rs`, `client.rs`)

```rust
// Client sends requests via mpsc channel
let (tx, rx) = mpsc::channel::<RROMsg>(1024);

// RRO thread receives and dispatches
while let Some((msg, resp_sender)) = rx.recv().await {
    tokio::spawn(async move {
        let res = msg.handle(&toc).await;
        resp_sender.send(res);
    });
}
```

> **Clean shutdown**: Dropping `RroClient` closes channel → thread exits → ToC cleanup.

### 3. **Direct Storage Access** (`ops/*.rs`)

No gRPC serialization. Direct calls to RRO internal crates:
```rust
// Direct TableOfContent access
toc.perform_collection_meta_op(op).await?;
collection.info(&shard).await?;
```

### 4. **Multi-Tenant Support** (`ops/collections.rs`)

```rust
CollectionRequest::GetWithShard((name, shard_key))
// → ShardSelectorInternal for tenant isolation
```

---

## ⚙️ Configuration

### Default Config (`config/config.yaml`)

```yaml
storage:
  storage_path: ./.storage
  snapshots_path: ./.snapshots
  on_disk_payload: true          # RAM optimization
  
  performance:
    max_search_threads: 0        # auto = CPU-1
    max_optimization_threads: 1
    
  hnsw_index:
    m: 16                        # graph connectivity
    ef_construct: 100            # build-time neighbors
    on_disk: false               # RAM for speed

telemetry_disabled: false
```

### Environment Override

```bash
export RRO__STORAGE__STORAGE_PATH=/custom/path
export RRO__STORAGE__ON_DISK_PAYLOAD=false
```

---

## 🛡️ Hardening Recommendations

### High Priority

| Issue | Current State | Recommendation |
|-------|---------------|----------------|
| **Panic on unexpected response** | `panic!("Unexpected response")` in client.rs | Return `Err(RROError::UnexpectedResponse)` |
| **No request timeout** | Unbounded channel waits | Add `tokio::time::timeout` wrapper |
| **Error context** | Basic error types | Add `anyhow` context or custom Display |
| **Graceful shutdown** | Spin-wait loop | Use `tokio::select!` with timeout |

### Medium Priority

| Issue | Recommendation |
|-------|----------------|
| **No metrics** | Add `prometheus` or `metrics` crate for observability |
| **No health check** | Expose `is_healthy()` method checking ToC state |
| **Channel backpressure** | Add bounded channel with timeout on send |
| **Memory limits** | Expose memory usage tracking from storage crate |

### Low Priority (Nice to Have)

| Feature | Notes |
|---------|-------|
| **Connection pooling** | Not needed (embedded), but useful for future multi-instance |
| **Request tracing** | Add `tracing::instrument` to client methods |
| **Snapshot to S3** | Extend `snapshots.rs` for cloud backup |

---

## 📊 API Coverage

### ✅ Implemented

**Collections**: create, list, get, update, delete  
**Aliases**: create, list, get, delete, rename  
**Points**: upsert, get, delete, count, update_vectors, delete_vectors  
**Payloads**: set, delete, clear  
**Search**: search, search_batch, search_group_by  
**Recommend**: recommend, recommend_batch, recommend_group_by  

### ❌ Not Yet Implemented

- **Scroll with offset** (partial - no pagination cursor)
- **Query API** (new universal query endpoint)
- **Cluster operations** (N/A for embedded)
- **Web Dashboard** (🎯 next milestone)

---

## 🚀 Next Milestones

### 1. Web Dashboard Integration

Port RRO's web UI to work with embedded mode:
```rust
// Proposed API
let client = RROInstance::start(None)?;
let dashboard = client.start_dashboard(8080)?; // Serves UI at localhost:8080
```

### 2. Enhanced Error Handling

Replace panics with proper error types:
```rust
#[derive(Error, Debug)]
pub enum RROError {
    #[error("Collection error: {0}")]
    Collection(#[from] CollectionError),
    #[error("Unexpected response type")]
    UnexpectedResponse,
    #[error("Request timeout")]
    Timeout,
}
```

### 3. Observability

```rust
// Metrics endpoint
client.metrics() -> PrometheusMetrics

// Health check
client.health_check() -> HealthStatus
```

---

## 🔗 Dependencies

This library uses RRO's internal crates via git submodule:

```
.modules/rro/lib/
├── api/
├── collection/
├── common/
├── segment/
├── shard/
└── storage/
```

**Update RRO version:**
```bash
cd .modules/rro
git fetch origin
git checkout v1.17.0  # or desired version
cd ../..
cargo build
```

---

## 📜 License

Apache 2.0 - Same as upstream RRO.

---

## 🙏 Credits

- [RRO Team](https://devpulse.app/) - Original vector database
- This fork maintained by [@EonsofStupid](https://github.com/EonsofStupid)
