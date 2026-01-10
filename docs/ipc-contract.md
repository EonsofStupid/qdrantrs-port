# rro-lib API → Tauri IPC Command Mapping

Complete contract for Tauri integration. Each Rust method maps 1:1 to a Tauri command.

---

## Collections (6 commands)

| Rust Method | Tauri Command | Parameters | Return Type |
|-------------|---------------|------------|-------------|
| `is_healthy()` | `is_healthy` | none | `bool` |
| `health_check()` | `health_check` | none | `Result<(), String>` |
| `create_collection(name, config)` | `create_collection` | `{ name: string, vectorSize: number, distance: "Cosine" \| "Euclid" \| "Dot" }` | `Result<bool, String>` |
| `list_collections()` | `list_collections` | none | `Result<string[], String>` |
| `get_collection(name)` | `get_collection` | `{ name: string }` | `Result<CollectionInfo \| null, String>` |
| `update_collection(name, data)` | `update_collection` | `{ name: string, data: UpdateCollection }` | `Result<bool, String>` |
| `delete_collection(name)` | `delete_collection` | `{ name: string }` | `Result<bool, String>` |

---

## Aliases (5 commands)

| Rust Method | Tauri Command | Parameters | Return Type |
|-------------|---------------|------------|-------------|
| `create_alias(collection, alias)` | `create_alias` | `{ collectionName: string, aliasName: string }` | `Result<bool, String>` |
| `list_aliases()` | `list_aliases` | none | `Result<[string, string][], String>` |
| `get_aliases(collection)` | `get_aliases` | `{ collectionName: string }` | `Result<[string, string][], String>` |
| `delete_alias(alias)` | `delete_alias` | `{ aliasName: string }` | `Result<bool, String>` |
| `rename_alias(old, new)` | `rename_alias` | `{ oldAliasName: string, newAliasName: string }` | `Result<bool, String>` |

---

## Points (8 commands)

| Rust Method | Tauri Command | Parameters | Return Type |
|-------------|---------------|------------|-------------|
| `get_points(collection, data)` | `get_points` | `{ collection: string, ids: PointId[], withPayload?: bool, withVector?: bool }` | `Result<Record[], String>` |
| `upsert_points(collection, points)` | `upsert_points` | `{ collection: string, points: PointStruct[] }` | `Result<UpdateResult, String>` |
| `delete_points(collection, selector)` | `delete_points` | `{ collection: string, points: PointId[] \| Filter }` | `Result<UpdateResult, String>` |
| `count_points(collection, filter, exact)` | `count_points` | `{ collection: string, filter?: Filter, exact?: bool }` | `Result<number, String>` |
| `update_vectors(collection, points)` | `update_vectors` | `{ collection: string, points: PointVectors[] }` | `Result<UpdateResult, String>` |
| `delete_vectors(collection, data)` | `delete_vectors` | `{ collection: string, data: DeleteVectors }` | `Result<UpdateResult, String>` |
| `set_payload(collection, data)` | `set_payload` | `{ collection: string, points: PointId[], payload: object }` | `Result<UpdateResult, String>` |
| `delete_payload(collection, data)` | `delete_payload` | `{ collection: string, points: PointId[], keys: string[] }` | `Result<UpdateResult, String>` |
| `clear_payload(collection, points)` | `clear_payload` | `{ collection: string, points: PointId[] }` | `Result<UpdateResult, String>` |

---

## Search & Query (6 commands)

| Rust Method | Tauri Command | Parameters | Return Type |
|-------------|---------------|------------|-------------|
| `search_points(collection, data)` | `search_points` | `{ collection: string, vector: number[], limit: number, filter?: Filter, withPayload?: bool }` | `Result<ScoredPoint[], String>` |
| `search_points_batch(collection, data)` | `search_points_batch` | `{ collection: string, searches: SearchRequest[] }` | `Result<ScoredPoint[][], String>` |
| `search_points_group_by(collection, data)` | `search_points_group_by` | `{ collection: string, ...SearchGroupsRequest }` | `Result<PointGroup[], String>` |
| `recommend_points(collection, data)` | `recommend_points` | `{ collection: string, positive: PointId[], negative?: PointId[], limit: number }` | `Result<ScoredPoint[], String>` |
| `recommend_points_batch(collection, data)` | `recommend_points_batch` | `{ collection: string, requests: RecommendRequest[] }` | `Result<ScoredPoint[][], String>` |
| `recommend_points_group_by(collection, data)` | `recommend_points_group_by` | `{ collection: string, ...RecommendGroupsRequest }` | `Result<PointGroup[], String>` |

---

## TypeScript Interfaces (Contracts)

```typescript
// Core types
type PointId = string | number;
type Distance = "Cosine" | "Euclid" | "Dot" | "Manhattan";

// Collection
interface CollectionInfo {
  name: string;
  status: "green" | "yellow" | "red";
  vectors_count?: number;
  points_count?: number;
  segments_count: number;
  config: CollectionConfig;
}

interface CollectionConfig {
  params: {
    vectors: VectorsConfig;
  };
}

interface VectorsConfig {
  size: number;
  distance: Distance;
}

// Points
interface PointStruct {
  id: PointId;
  vector: number[] | Record<string, number[]>;
  payload?: Record<string, unknown>;
}

interface Record {
  id: PointId;
  payload?: Record<string, unknown>;
  vector?: number[] | Record<string, number[]>;
}

interface ScoredPoint extends Record {
  score: number;
  version: number;
}

interface PointGroup {
  id: string;
  hits: ScoredPoint[];
}

// Operations
interface UpdateResult {
  operation_id: number;
  status: "acknowledged" | "completed";
}

// Filters
interface Filter {
  must?: Condition[];
  should?: Condition[];
  must_not?: Condition[];
}

interface Condition {
  key: string;
  match?: { value: unknown };
  range?: { gte?: number; lte?: number; gt?: number; lt?: number };
  // ... more condition types
}
```

---

## Rust Tauri Implementation Pattern

```rust
// src-tauri/src/commands/collections.rs

use tauri::State;
use std::sync::Arc;
use rro_lib::{RroClient, RROError};

#[tauri::command]
pub async fn list_collections(
    state: State<'_, Arc<RroClient>>
) -> Result<Vec<String>, String> {
    state.list_collections()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_collection(
    state: State<'_, Arc<RroClient>>,
    name: String
) -> Result<Option<serde_json::Value>, String> {
    state.get_collection(&name)
        .await
        .map(|opt| opt.map(|info| serde_json::to_value(info).unwrap()))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_collection(
    state: State<'_, Arc<RroClient>>,
    name: String,
    vector_size: usize,
    distance: String,
) -> Result<bool, String> {
    use rro_lib::{Distance, VectorParams};
    use collection::operations::types::VectorsConfig;
    
    let dist = match distance.as_str() {
        "Cosine" => Distance::Cosine,
        "Euclid" => Distance::Euclid,
        "Dot" => Distance::Dot,
        _ => return Err("Invalid distance".to_string()),
    };
    
    let config = VectorsConfig::Single(VectorParams {
        size: vector_size as u64,
        distance: dist,
        ..Default::default()
    });
    
    state.create_collection(&name, config)
        .await
        .map_err(|e| e.to_string())
}
```

---

## SvelteKit Integration Pattern

```typescript
// src/lib/rro.ts
import { invoke } from '@tauri-apps/api/core';

export const rro = {
  // Collections
  listCollections: () => invoke<string[]>('list_collections'),
  getCollection: (name: string) => invoke<CollectionInfo | null>('get_collection', { name }),
  createCollection: (name: string, vectorSize: number, distance: Distance) =>
    invoke<boolean>('create_collection', { name, vectorSize, distance }),
  deleteCollection: (name: string) => invoke<boolean>('delete_collection', { name }),
  
  // Points
  getPoints: (collection: string, ids: PointId[], opts?: { withPayload?: boolean; withVector?: boolean }) =>
    invoke<Record[]>('get_points', { collection, ids, ...opts }),
  upsertPoints: (collection: string, points: PointStruct[]) =>
    invoke<UpdateResult>('upsert_points', { collection, points }),
  deletePoints: (collection: string, points: PointId[]) =>
    invoke<UpdateResult>('delete_points', { collection, points }),
  countPoints: (collection: string, filter?: Filter, exact = false) =>
    invoke<number>('count_points', { collection, filter, exact }),
  
  // Search
  searchPoints: (collection: string, vector: number[], limit: number, opts?: { filter?: Filter; withPayload?: boolean }) =>
    invoke<ScoredPoint[]>('search_points', { collection, vector, limit, ...opts }),
  
  // Health
  isHealthy: () => invoke<boolean>('is_healthy'),
  healthCheck: () => invoke<void>('health_check'),
};
```

---

## Command Count Summary

| Category | Commands |
|----------|----------|
| Collections | 7 |
| Aliases | 5 |
| Points | 9 |
| Search/Query | 6 |
| **Total** | **27** |

All 27 methods from `RroClient` are mapped to Tauri IPC commands.
