# API Gap Analysis: Dashboard vs rro-lib

## CRITICAL FINDING

The rro-lib has **27 methods**. The dashboard uses **30+ distinct API calls**.

**There are GAPS that must be filled before the dashboard can work.**

---

## Dashboard API Calls (Extracted from source)

### Collections (7 calls)
| Dashboard Call | rro-lib Method | Status |
|----------------|-------------------|--------|
| `getCollections()` | `list_collections()` | ✅ EXISTS |
| `getCollection(name)` | `get_collection(name)` | ✅ EXISTS |
| `createCollection(name, params)` | `create_collection(name, config)` | ✅ EXISTS |
| `deleteCollection(name)` | `delete_collection(name)` | ✅ EXISTS |
| `collectionExists(name)` | ❌ | ❌ **MISSING** |
| `getCollectionAliases(name)` | `get_aliases(name)` | ✅ EXISTS (different name) |
| `updateCollectionCluster(name, ops)` | ❌ | ❌ **N/A EMBEDDED** |

### Aliases (2 calls)
| Dashboard Call | rro-lib Method | Status |
|----------------|-------------------|--------|
| `getAliases()` | `list_aliases()` | ✅ EXISTS |
| `updateCollectionAliases(actions)` | ❌ | ❌ **MISSING** (bulk alias ops) |

### Points (7 calls)
| Dashboard Call | rro-lib Method | Status |
|----------------|-------------------|--------|
| `scroll(collection, opts)` | ❌ | ❌ **MISSING** |
| `retrieve(collection, opts)` | `get_points(collection, data)` | ✅ EXISTS (different name) |
| `delete(collection, opts)` | `delete_points(collection, selector)` | ✅ EXISTS |
| `setPayload(collection, opts)` | `set_payload(collection, data)` | ✅ EXISTS |
| `overwritePayload(collection, opts)` | ❌ | ❌ **MISSING** |
| `recommend(collection, opts)` | `recommend_points(collection, data)` | ✅ EXISTS |
| `query(collection, opts)` | ❌ | ❌ **MISSING** (unified query API) |

### Search (2 calls)
| Dashboard Call | rro-lib Method | Status |
|----------------|-------------------|--------|
| `search(collection, opts)` | `search_points(collection, data)` | ✅ EXISTS (implied) |
| `searchMatrixPairs(collection, opts)` | ❌ | ❌ **MISSING** |

### Snapshots (3 calls)
| Dashboard Call | rro-lib Method | Status |
|----------------|-------------------|--------|
| `recoverSnapshot(name, opts)` | `recover_snapshots()` (in snapshots.rs) | ⚠️ EXISTS (not in client) |
| `getSnapshotUploadUrl(name)` | ❌ | ❌ **N/A EMBEDDED** |
| `abortDownload()` | ❌ | ❌ **N/A EMBEDDED** |

### Indexes (1 call)
| Dashboard Call | rro-lib Method | Status |
|----------------|-------------------|--------|
| `createPayloadIndex(name, params)` | ❌ | ❌ **MISSING** |

### Service (2 calls)
| Dashboard Call | rro-lib Method | Status |
|----------------|-------------------|--------|
| `api('service').telemetry()` | ❌ | ❌ **MISSING** |
| `getApiKey()` | ❌ | ❌ **N/A EMBEDDED** |

---

## Gap Summary

### ❌ MISSING Methods (Must Add to rro-lib)

| Method | Priority | Notes |
|--------|----------|-------|
| `scroll_points()` | **P0** | Core browsing functionality |
| `query_points()` | **P0** | Unified query API (new in RRO) |
| `collection_exists()` | P1 | Simple check |
| `overwrite_payload()` | P1 | Full payload replacement |
| `update_collection_aliases()` | P1 | Bulk alias operations |
| `create_payload_index()` | P1 | Index creation |
| `search_matrix_pairs()` | P2 | Graph visualization |
| `get_telemetry()` | P2 | Service info |

### ❌ N/A for Embedded (Skip or Stub)

| Method | Reason |
|--------|--------|
| `updateCollectionCluster()` | Single-node embedded, no cluster |
| `getSnapshotUploadUrl()` | HTTP URL for upload, not needed embedded |
| `abortDownload()` | Download controller, N/A embedded |
| `getApiKey()` | No auth needed embedded |

---

## rro-lib Methods NOT Used by Dashboard

These exist in rro-lib but dashboard doesn't call them:

| Method | Notes |
|--------|-------|
| `update_collection()` | Config updates |
| `create_alias()` | Individual alias (dashboard uses bulk) |
| `delete_alias()` | Individual alias |
| `rename_alias()` | Individual alias |
| `count_points()` | Point counting |
| `update_vectors()` | Vector updates |
| `delete_vectors()` | Vector deletion |
| `delete_payload()` | Payload key deletion |
| `clear_payload()` | Full payload clear |
| `search_points_batch()` | Batch search |
| `search_points_group_by()` | Grouped search |
| `recommend_points_batch()` | Batch recommend |
| `recommend_points_group_by()` | Grouped recommend |

---

## Methods to Add to rro-lib

### P0 - Critical for Dashboard

```rust
// 1. scroll_points - Browse points with cursor
pub async fn scroll_points(
    &self,
    collection_name: impl Into<String>,
    limit: Option<usize>,
    offset: Option<PointId>,
    filter: Option<Filter>,
    with_payload: bool,
    with_vector: bool,
) -> Result<(Vec<LocalRecord>, Option<PointId>), RROError>

// 2. query_points - Unified query API
pub async fn query_points(
    &self,
    collection_name: impl Into<String>,
    query: QueryRequest,
) -> Result<Vec<LocalScoredPoint>, RROError>

// 3. collection_exists
pub async fn collection_exists(
    &self,
    name: impl Into<String>,
) -> Result<bool, RROError>
```

### P1 - Important for Full Feature Parity

```rust
// 4. overwrite_payload
pub async fn overwrite_payload(
    &self,
    collection_name: impl Into<String>,
    data: SetPayload,
) -> Result<UpdateResult, RROError>

// 5. update_collection_aliases (bulk)
pub async fn update_collection_aliases(
    &self,
    actions: Vec<AliasAction>,
) -> Result<bool, RROError>

// 6. create_payload_index
pub async fn create_payload_index(
    &self,
    collection_name: impl Into<String>,
    field_name: impl Into<String>,
    field_type: PayloadFieldType,
) -> Result<UpdateResult, RROError>
```

---

## SvelteKit Component Mapping

This is NOT done yet. Need to map React components to Svelte equivalents.

### React Pages → Svelte Routes

| React Page | Svelte Route | Status |
|------------|--------------|--------|
| `pages/Collections.jsx` | `/collections/+page.svelte` | ❌ TODO |
| `pages/Collection.jsx` | `/collections/[name]/+page.svelte` | ❌ TODO |
| `pages/Visualize.jsx` | `/visualize/+page.svelte` | ❌ TODO |
| `pages/Graph.jsx` | `/graph/+page.svelte` | ❌ TODO |
| `pages/Datasets.jsx` | `/datasets/+page.svelte` | ❌ TODO |
| `pages/Jwt.jsx` | N/A (no auth in embedded) | SKIP |
| `pages/Tutorial.jsx` | `/tutorial/+page.svelte` | ❌ TODO |
| `pages/Homepage.jsx` | `/+page.svelte` | ❌ TODO |

### React Components → Svelte Components

| React Component Path | Purpose | Svelte Path (proposed) |
|---------------------|---------|------------------------|
| `components/Points/PointsTabs.jsx` | Point browsing | `lib/components/points/PointsTabs.svelte` |
| `components/Points/PayloadEditor.jsx` | Edit payload | `lib/components/points/PayloadEditor.svelte` |
| `components/Collections/CollectionsList.jsx` | List view | `lib/components/collections/CollectionsList.svelte` |
| `components/Collections/DeleteDialog.jsx` | Delete confirm | `lib/components/collections/DeleteDialog.svelte` |
| `components/Collections/CreateCollection/*` | Create wizard | `lib/components/collections/CreateCollection.svelte` |
| `components/Snapshots/*` | Snapshot mgmt | `lib/components/snapshots/*.svelte` |
| `components/CodeEditorWindow/*` | Console | `lib/components/console/*.svelte` |
| `components/GraphVisualisation/*` | Graph viz | `lib/components/graph/*.svelte` |
| `components/VisualizeChart/*` | 2D viz | `lib/components/visualize/*.svelte` |

---

## Next Steps

1. **Add missing P0 methods to rro-lib** (scroll_points, query_points, collection_exists)
2. **Add P1 methods** (overwrite_payload, update_collection_aliases, create_payload_index)
3. **Create complete SvelteKit route structure**
4. **Create TypeScript contracts for all methods**
5. **Implement Tauri commands**
6. **Build Svelte components**
