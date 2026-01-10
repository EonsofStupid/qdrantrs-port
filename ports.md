# RRO Dashboard Port - Embedded Edition

## Goal

Port the RRO Dashboard (React app) from HTTP/REST calls to **direct embedded calls** via Tauri IPC.

**NO HTTP. NO REST. DIRECT FUNCTION CALLS.**

---

## Current Architecture (What We're Replacing)

```
┌─────────────────────────────────────────────────────────────────┐
│                    rro-web-ui (React)                         │
│                         Axios HTTP                               │
│                            ↓                                     │
│                    HTTP/REST over network                        │
│                            ↓                                     │
│                    RRO Server (actix)                         │
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
│  │              (rro-web-ui, modified)                       ││
│  │                        ↓                                     ││
│  │              invoke("list_collections")    ← Tauri IPC       ││
│  └──────────────────────────┬──────────────────────────────────┘│
│                             │ (no network, process-internal)    │
│  ┌──────────────────────────▼──────────────────────────────────┐│
│  │              Tauri Rust Backend                              ││
│  │                                                              ││
│  │   #[tauri::command]                                          ││
│  │   async fn list_collections(                                 ││
│  │       state: State<RroClient>                             ││
│  │   ) -> Vec<CollectionInfo> {                                 ││
│  │       state.list_collections().await                         ││
│  │   }                                                          ││
│  │                        ↓                                     ││
│  │              RroClient (our library)                      ││
│  │                        ↓                                     ││
│  │                  RROInstance                              ││
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
| Can use our RroClient | **Direct** ✅ | Need napi-rs binding |
| Binary size | ~3MB | ~150MB |
| Memory usage | Low | High |
| Security | Sandboxed | Less secure |

Tauri is Rust-native. We can directly use `rro-lib` in the Tauri backend.

---

## Implementation Plan

### Phase 1: Tauri Project Setup

**Goal:** Create Tauri app that embeds our rro-lib

```bash
# Create new Tauri project adjacent to rrors-port
cargo install create-tauri-app
cargo create-tauri-app rro-dashboard --template react-ts
```

**Structure:**
```
rro-dashboard/
├── src/                    # React frontend
│   ├── App.tsx
│   └── ...
├── src-tauri/
│   ├── Cargo.toml          # Depends on rro-lib
│   ├── src/
│   │   ├── main.rs         # Tauri entry point
│   │   ├── commands/       # Tauri commands
│   │   │   ├── mod.rs
│   │   │   ├── collections.rs
│   │   │   ├── points.rs
│   │   │   └── search.rs
│   │   └── state.rs        # RroClient state
│   └── tauri.conf.json
└── package.json
```

**src-tauri/Cargo.toml:**
```toml
[dependencies]
tauri = { version = "2", features = ["shell-open"] }
rro-lib = { path = "../rrors-port" }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

---

### Phase 2: Tauri Commands (Rust Backend)

**Goal:** Expose RroClient methods as Tauri commands

**src-tauri/src/state.rs:**
```rust
use rro_lib::{RroClient, RROInstance};
use std::sync::Arc;

pub struct AppState {
    pub client: Arc<RroClient>,
}

impl AppState {
    pub fn new() -> Self {
        let instance = RROInstance::start(None)
            .expect("Failed to start embedded RRO");
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
// rro-web-ui uses Axios
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
// src/api/rro.ts
import { invoke } from '@tauri-apps/api/core';

export const rroApi = {
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

### Phase 4: Fork and Modify rro-web-ui

**Steps:**
1. Fork `rro/rro-web-ui` to our repo
2. Remove Axios dependency
3. Replace all API calls with `invoke()` calls
4. Update package.json for Tauri compatibility
5. Test each feature

**Files to modify in rro-web-ui:**
- `src/api/` - Replace HTTP client with Tauri invoke
- `src/context/Client.jsx` - Remove URL configuration
- Any file importing axios

---

## Commands to Implement

| Tauri Command | RroClient Method | Dashboard Usage |
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
c:\localdev\rro-rs\
├── rrors-port/              # Our embedded RRO library
│   ├── src/
│   ├── Cargo.toml
│   └── ...
│
└── rro-dashboard/           # NEW - Tauri app
    ├── src/                    # React frontend (forked rro-web-ui)
    │   ├── App.tsx
    │   ├── api/
    │   │   └── rro.ts       # Tauri invoke wrappers
    │   └── ...
    ├── src-tauri/
    │   ├── Cargo.toml          # depends on rro-lib
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
rro-lib = { path = "../rrors-port" }
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

1. [ ] Create Tauri project in `c:\localdev\rro-rs\rro-dashboard`
2. [ ] Add rro-lib as dependency
3. [ ] Implement AppState with embedded RRO
4. [ ] Implement `list_collections` command
5. [ ] Fork rro-web-ui
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
