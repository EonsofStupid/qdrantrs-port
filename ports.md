# Qdrant Dashboard Port - Embedded Edition

## Goal

Port the Qdrant Dashboard (React app) from HTTP/REST calls to **direct embedded calls** via Tauri IPC.

**NO HTTP. NO REST. DIRECT FUNCTION CALLS.**

---

## Current Architecture (What We're Replacing)

```
┌─────────────────────────────────────────────────────────────────┐
│                    qdrant-web-ui (React)                         │
│                         Axios HTTP                               │
│                            ↓                                     │
│                    HTTP/REST over network                        │
│                            ↓                                     │
│                    Qdrant Server (actix)                         │
│                            ↓                                     │
│                       TableOfContent                             │
└─────────────────────────────────────────────────────────────────┘
```

**We are eliminating the middle layers.**

---

## Target Architecture (Embedded)

```
┌─────────────────────────────────────────────────────────────────┐
│                      Tauri Application                           │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │                  React Frontend                              ││
│  │              (qdrant-web-ui, modified)                       ││
│  │                        ↓                                     ││
│  │              invoke("list_collections")    ← Tauri IPC       ││
│  └──────────────────────────┬──────────────────────────────────┘│
│                             │ (no network, process-internal)    │
│  ┌──────────────────────────▼──────────────────────────────────┐│
│  │              Tauri Rust Backend                              ││
│  │                                                              ││
│  │   #[tauri::command]                                          ││
│  │   async fn list_collections(                                 ││
│  │       state: State<QdrantClient>                             ││
│  │   ) -> Vec<CollectionInfo> {                                 ││
│  │       state.list_collections().await                         ││
│  │   }                                                          ││
│  │                        ↓                                     ││
│  │              QdrantClient (our library)                      ││
│  │                        ↓                                     ││
│  │                  QdrantInstance                              ││
│  │              (embedded, in-process)                          ││
│  └─────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────┘
```

**Zero network overhead. Direct function calls via Tauri IPC.**

---

## Why Tauri

| Feature | Tauri | Electron |
|---------|-------|----------|
| Backend language | **Rust** ✅ | JavaScript/Node |
| Can use our QdrantClient | **Direct** ✅ | Need napi-rs binding |
| Binary size | ~3MB | ~150MB |
| Memory usage | Low | High |
| Security | Sandboxed | Less secure |

Tauri is Rust-native. We can directly use `qdrant-lib` in the Tauri backend.

---

## Implementation Plan

### Phase 1: Tauri Project Setup

**Goal:** Create Tauri app that embeds our qdrant-lib

```bash
# Create new Tauri project adjacent to qdrantrs-port
cargo install create-tauri-app
cargo create-tauri-app qdrant-dashboard --template react-ts
```

**Structure:**
```
qdrant-dashboard/
├── src/                    # React frontend
│   ├── App.tsx
│   └── ...
├── src-tauri/
│   ├── Cargo.toml          # Depends on qdrant-lib
│   ├── src/
│   │   ├── main.rs         # Tauri entry point
│   │   ├── commands/       # Tauri commands
│   │   │   ├── mod.rs
│   │   │   ├── collections.rs
│   │   │   ├── points.rs
│   │   │   └── search.rs
│   │   └── state.rs        # QdrantClient state
│   └── tauri.conf.json
└── package.json
```

**src-tauri/Cargo.toml:**
```toml
[dependencies]
tauri = { version = "2", features = ["shell-open"] }
qdrant-lib = { path = "../qdrantrs-port" }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

---

### Phase 2: Tauri Commands (Rust Backend)

**Goal:** Expose QdrantClient methods as Tauri commands

**src-tauri/src/state.rs:**
```rust
use qdrant_lib::{QdrantClient, QdrantInstance};
use std::sync::Arc;

pub struct AppState {
    pub client: Arc<QdrantClient>,
}

impl AppState {
    pub fn new() -> Self {
        let instance = QdrantInstance::start(None)
            .expect("Failed to start embedded Qdrant");
        Self {
            client: Arc::new(instance),
        }
    }
}
```

**src-tauri/src/commands/collections.rs:**
```rust
use tauri::State;
use crate::state::AppState;

#[tauri::command]
pub async fn list_collections(
    state: State<'_, AppState>
) -> Result<Vec<String>, String> {
    state.client
        .list_collections()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_collection(
    state: State<'_, AppState>,
    name: String
) -> Result<CollectionInfo, String> {
    state.client
        .get_collection(&name)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_collection(
    state: State<'_, AppState>,
    name: String,
    vector_size: usize
) -> Result<bool, String> {
    // Create vectors config and call client
    state.client
        .create_collection(&name, vectors_config)
        .await
        .map_err(|e| e.to_string())
}
```

**src-tauri/src/main.rs:**
```rust
mod commands;
mod state;

use state::AppState;

fn main() {
    tauri::Builder::default()
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::collections::list_collections,
            commands::collections::get_collection,
            commands::collections::create_collection,
            commands::points::get_points,
            commands::points::upsert_points,
            commands::search::search_points,
        ])
        .run(tauri::generate_context!())
        .expect("error running tauri application");
}
```

---

### Phase 3: Port React Frontend

**Goal:** Replace Axios HTTP calls with Tauri invoke()

**Original (HTTP):**
```typescript
// qdrant-web-ui uses Axios
import axios from 'axios';

const getCollections = async () => {
  const response = await axios.get('/collections');
  return response.data.result;
};
```

**Ported (Tauri IPC):**
```typescript
import { invoke } from '@tauri-apps/api/core';

const getCollections = async () => {
  return await invoke('list_collections');
};
```

**Create abstraction layer:**
```typescript
// src/api/qdrant.ts
import { invoke } from '@tauri-apps/api/core';

export const qdrantApi = {
  listCollections: () => invoke('list_collections'),
  getCollection: (name: string) => invoke('get_collection', { name }),
  createCollection: (name: string, vectorSize: number) => 
    invoke('create_collection', { name, vectorSize }),
  
  getPoints: (collection: string, ids: string[]) =>
    invoke('get_points', { collection, ids }),
  
  searchPoints: (collection: string, vector: number[], limit: number) =>
    invoke('search_points', { collection, vector, limit }),
};
```

---

### Phase 4: Fork and Modify qdrant-web-ui

**Steps:**
1. Fork `qdrant/qdrant-web-ui` to our repo
2. Remove Axios dependency
3. Replace all API calls with `invoke()` calls
4. Update package.json for Tauri compatibility
5. Test each feature

**Files to modify in qdrant-web-ui:**
- `src/api/` - Replace HTTP client with Tauri invoke
- `src/context/Client.jsx` - Remove URL configuration
- Any file importing axios

---

## Commands to Implement

| Tauri Command | QdrantClient Method | Dashboard Usage |
|---------------|---------------------|-----------------|
| `list_collections` | `list_collections()` | Homepage |
| `get_collection` | `get_collection(name)` | Collection detail |
| `create_collection` | `create_collection(...)` | Create dialog |
| `delete_collection` | `delete_collection(name)` | Delete button |
| `get_points` | `get_points(...)` | Point browser |
| `scroll_points` | `scroll_points(...)` | Point list |
| `search_points` | `search_points(...)` | Search tab |
| `upsert_points` | `upsert_points(...)` | Insert dialog |
| `delete_points` | `delete_points(...)` | Delete button |
| `health_check` | `health_check()` | Status bar |

---

## Project Structure (Final)

```
c:\localdev\qdrant-rs\
├── qdrantrs-port/              # Our embedded Qdrant library
│   ├── src/
│   ├── Cargo.toml
│   └── ...
│
└── qdrant-dashboard/           # NEW - Tauri app
    ├── src/                    # React frontend (forked qdrant-web-ui)
    │   ├── App.tsx
    │   ├── api/
    │   │   └── qdrant.ts       # Tauri invoke wrappers
    │   └── ...
    ├── src-tauri/
    │   ├── Cargo.toml          # depends on qdrant-lib
    │   ├── src/
    │   │   ├── main.rs
    │   │   ├── state.rs
    │   │   └── commands/
    │   │       ├── mod.rs
    │   │       ├── collections.rs
    │   │       ├── points.rs
    │   │       └── search.rs
    │   └── tauri.conf.json
    ├── package.json
    └── vite.config.ts
```

---

## Dependencies

**Tauri (Rust side):**
```toml
[dependencies]
tauri = { version = "2", features = [] }
qdrant-lib = { path = "../qdrantrs-port" }
tokio = { version = "1", features = ["rt-multi-thread"] }
serde = { version = "1", features = ["derive"] }
```

**Frontend (package.json):**
```json
{
  "dependencies": {
    "@tauri-apps/api": "^2",
    "react": "^18",
    "@mui/material": "^5"
  }
}
```

---

## Testing Strategy

1. **Unit tests:** Each Tauri command works correctly
2. **Integration:** Start app, create collection, insert points, search
3. **Manual:** Full dashboard walkthrough

---

## Timeline Estimate

| Phase | Effort |
|-------|--------|
| Phase 1: Tauri setup | 2-3 hours |
| Phase 2: Rust commands | 3-4 hours |
| Phase 3: Port React frontend | 4-6 hours |
| Phase 4: Testing & polish | 2-3 hours |

**Total: ~12-16 hours**

---

## Next Steps

1. [ ] Create Tauri project in `c:\localdev\qdrant-rs\qdrant-dashboard`
2. [ ] Add qdrant-lib as dependency
3. [ ] Implement AppState with embedded Qdrant
4. [ ] Implement `list_collections` command
5. [ ] Fork qdrant-web-ui
6. [ ] Replace one API call with invoke() and test
7. [ ] Port remaining API calls
8. [ ] Full integration testing

---

## Key Difference from Wrong Plan

| Wrong Plan | Correct Plan |
|------------|--------------|
| Add actix-web HTTP server | **NO HTTP** |
| Dashboard calls REST API | Dashboard calls **Tauri IPC** |
| Network overhead | **Zero network overhead** |
| Defeats embedded purpose | **Preserves embedded architecture** |
