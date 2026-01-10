# Dashboard Port: Complete Structure

## Project Layout

```
c:\localdev\rro-rs\
├── rrors-port/                    # Embedded RRO library (EXISTING)
│   ├── src/
│   │   ├── client.rs                 # Add scroll_points, query_points, etc.
│   │   ├── ops/
│   │   │   └── points.rs             # Add Scroll, Query variants
│   │   └── ...
│   ├── dashboard-upstream/           # Reference React code (EXISTING, cloned)
│   └── docs/
│       ├── tokio-architecture.md
│       ├── ipc-contract.md
│       └── api-gap-analysis.md
│
└── rro-dashboard/                 # NEW - SvelteKit dashboard
    ├── src/
    │   ├── routes/                   # SvelteKit pages
    │   │   ├── +layout.svelte
    │   │   ├── +page.svelte          # Homepage
    │   │   ├── collections/
    │   │   │   ├── +page.svelte      # List collections
    │   │   │   └── [name]/
    │   │   │       └── +page.svelte  # Collection detail
    │   │   ├── console/
    │   │   │   └── +page.svelte      # Query console
    │   │   ├── visualize/
    │   │   │   └── +page.svelte      # 2D visualization
    │   │   ├── graph/
    │   │   │   └── +page.svelte      # Graph visualization
    │   │   └── datasets/
    │   │       └── +page.svelte      # Sample datasets
    │   │
    │   ├── lib/
    │   │   ├── rro/               # Tauri IPC bridge
    │   │   │   ├── client.ts         # invoke() wrappers
    │   │   │   ├── types.ts          # TypeScript contracts
    │   │   │   └── index.ts
    │   │   │
    │   │   ├── components/           # Svelte components
    │   │   │   ├── collections/
    │   │   │   │   ├── CollectionCard.svelte
    │   │   │   │   ├── CollectionList.svelte
    │   │   │   │   ├── CreateCollection.svelte
    │   │   │   │   └── DeleteDialog.svelte
    │   │   │   ├── points/
    │   │   │   │   ├── PointsTable.svelte
    │   │   │   │   ├── PointDetail.svelte
    │   │   │   │   ├── PayloadEditor.svelte
    │   │   │   │   └── Pagination.svelte
    │   │   │   ├── search/
    │   │   │   │   ├── VectorInput.svelte
    │   │   │   │   ├── FilterBuilder.svelte
    │   │   │   │   └── SearchResults.svelte
    │   │   │   ├── console/
    │   │   │   │   ├── CodeEditor.svelte
    │   │   │   │   └── ResultViewer.svelte
    │   │   │   ├── visualize/
    │   │   │   │   ├── Chart2D.svelte
    │   │   │   │   └── GraphView.svelte
    │   │   │   ├── common/
    │   │   │   │   ├── Sidebar.svelte
    │   │   │   │   ├── Header.svelte
    │   │   │   │   ├── Modal.svelte
    │   │   │   │   ├── Toast.svelte
    │   │   │   │   └── Loading.svelte
    │   │   │   └── snapshots/
    │   │   │       ├── SnapshotList.svelte
    │   │   │       └── CreateSnapshot.svelte
    │   │   │
    │   │   └── stores/               # Svelte stores
    │   │       ├── collections.ts
    │   │       ├── points.ts
    │   │       └── search.ts
    │   │
    │   ├── app.html
    │   └── app.css
    │
    ├── src-tauri/                    # Tauri Rust backend
    │   ├── Cargo.toml                # Depends on rro-lib
    │   ├── src/
    │   │   ├── main.rs
    │   │   ├── state.rs              # RroClient state
    │   │   └── commands/
    │   │       ├── mod.rs
    │   │       ├── collections.rs    # Collection commands
    │   │       ├── points.rs         # Points commands
    │   │       └── search.rs         # Search commands
    │   └── tauri.conf.json
    │
    ├── package.json
    ├── svelte.config.js
    ├── tsconfig.json
    └── vite.config.ts
```

---

## React → Svelte Component Mapping

| React Component | Svelte Component | Priority |
|-----------------|------------------|----------|
| `pages/Collections.jsx` | `routes/collections/+page.svelte` | P0 |
| `pages/Collection.jsx` | `routes/collections/[name]/+page.svelte` | P0 |
| `pages/Console.jsx` | `routes/console/+page.svelte` | P0 |
| `pages/Homepage.jsx` | `routes/+page.svelte` | P0 |
| `pages/Visualize.jsx` | `routes/visualize/+page.svelte` | P1 |
| `pages/Graph.jsx` | `routes/graph/+page.svelte` | P1 |
| `pages/Datasets.jsx` | `routes/datasets/+page.svelte` | P2 |
| `pages/Jwt.jsx` | SKIP (no auth in embedded) | - |
| `pages/Tutorial.jsx` | SKIP for now | P3 |

| React Component Dir | Svelte Component Dir | Files |
|---------------------|----------------------|-------|
| `components/Collections/` (29 files) | `lib/components/collections/` | 4-6 |
| `components/Points/` (8 files) | `lib/components/points/` | 4 |
| `components/CodeEditorWindow/` (18 files) | `lib/components/console/` | 2 |
| `components/VisualizeChart/` (4 files) | `lib/components/visualize/` | 2 |
| `components/GraphVisualisation/` (1 file) | `lib/components/visualize/` | 1 |
| `components/Snapshots/` (4 files) | `lib/components/snapshots/` | 2 |
| `components/Sidebar/` (2 files) | `lib/components/common/` | 1 |
| `components/Common/` (31 files) | `lib/components/common/` | 5 |

---

## TypeScript Contracts (lib/rro/types.ts)

```typescript
// Core types
export type PointId = string | number;
export type Distance = "Cosine" | "Euclid" | "Dot" | "Manhattan";

// Collection
export interface CollectionInfo {
  status: "green" | "yellow" | "grey" | "red";
  indexed_vectors_count?: number;
  points_count?: number;
  segments_count: number;
  config: CollectionConfig;
}

export interface CollectionConfig {
  params: {
    vectors: VectorsConfig;
  };
}

export interface VectorsConfig {
  size: number;
  distance: Distance;
}

// Points
export interface Record {
  id: PointId;
  payload?: Record<string, unknown>;
  vector?: number[] | Record<string, number[]>;
  shard_key?: string;
}

export interface ScrollResult {
  points: Record[];
  next_page_offset?: PointId;
}

export interface ScoredPoint extends Record {
  score: number;
}

// Operations
export interface UpdateResult {
  operation_id?: number;
  status: "acknowledged" | "completed";
}
```

---

## Tauri Client (lib/rro/client.ts)

```typescript
import { invoke } from '@tauri-apps/api/core';
import type { 
  CollectionInfo, 
  ScrollResult, 
  ScoredPoint,
  UpdateResult 
} from './types';

export const rro = {
  // Collections
  listCollections: () => 
    invoke<string[]>('list_collections'),
  
  getCollection: (name: string) => 
    invoke<CollectionInfo | null>('get_collection', { name }),
  
  createCollection: (name: string, vectorSize: number, distance: string) =>
    invoke<boolean>('create_collection', { name, vectorSize, distance }),
  
  deleteCollection: (name: string) => 
    invoke<boolean>('delete_collection', { name }),
  
  collectionExists: (name: string) =>
    invoke<boolean>('collection_exists', { name }),

  // Points
  scrollPoints: (collection: string, limit?: number, offset?: string) =>
    invoke<ScrollResult>('scroll_points', { collection, limit, offset }),
  
  getPoints: (collection: string, ids: (string | number)[]) =>
    invoke<Record[]>('get_points', { collection, ids }),
  
  upsertPoints: (collection: string, points: unknown[]) =>
    invoke<UpdateResult>('upsert_points', { collection, points }),
  
  deletePoints: (collection: string, ids: (string | number)[]) =>
    invoke<UpdateResult>('delete_points', { collection, ids }),

  // Search
  searchPoints: (collection: string, vector: number[], limit: number) =>
    invoke<ScoredPoint[]>('search_points', { collection, vector, limit }),
  
  // Health
  isHealthy: () => invoke<boolean>('is_healthy'),
};
```

---

## Implementation Phases

### Phase 1: Core Infrastructure (8-10 hrs)

1. **rro-lib gaps** (4 hrs)
   - Add scroll_points to client.rs
   - Add query_points to client.rs
   - Add collection_exists to client.rs
   - cargo check passes

2. **Tauri project setup** (2 hrs)
   - Create rro-dashboard folder
   - npm create tauri-app@latest (SvelteKit template)
   - Add rro-lib dependency
   - Verify builds

3. **Tauri commands** (2 hrs)
   - Implement P0 commands (list/get/create collection, scroll, search)
   - Test from Tauri dev console

### Phase 2: Core UI (10-12 hrs)

4. **Layout & Navigation** (2 hrs)
   - Sidebar.svelte
   - +layout.svelte
   - Basic routing

5. **Collections Page** (4 hrs)
   - CollectionList.svelte
   - CollectionCard.svelte
   - CreateCollection.svelte
   - DeleteDialog.svelte

6. **Collection Detail** (4 hrs)
   - routes/collections/[name]/+page.svelte
   - PointsTable.svelte
   - Pagination.svelte
   - PayloadEditor.svelte

### Phase 3: Advanced Features (8-10 hrs)

7. **Console** (3 hrs)
   - CodeEditor.svelte (Monaco or CodeMirror)
   - ResultViewer.svelte

8. **Search** (3 hrs)
   - VectorInput.svelte
   - SearchResults.svelte

9. **Visualization** (2-4 hrs optional)
   - Chart2D.svelte
   - GraphView.svelte

---

## Estimated Total: 26-32 hours

| Phase | Hours |
|-------|-------|
| Phase 1: Infrastructure | 8-10 |
| Phase 2: Core UI | 10-12 |
| Phase 3: Advanced | 8-10 |

---

## Next Action

Create `c:\localdev\rro-rs\rro-dashboard\` folder structure.
