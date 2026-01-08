# Migration Refactoring Plan: Qdrant Dashboard

**Audit Date:** 2026-01-05
**Source:** `c:\localdev\qdrant-rs\qdrantrs-port\dashboard-upstream\`
**Target:** SvelteKit + Tauri (`src-tauri`)

## 1. Executive Summary
The initial "MVP" assumption (Collection Cards, simple Textarea Console) conflicts with the upstream React architecture. The upstream dashboard is a sophisticated tool featuring:
1.  **Collections:** A detailed **Data Table** (not Cards) with complex columns (Aliases, Shards, Segments) and client-side pagination.
2.  **Console:** A custom **IDE-lite** using Monaco Editor, a custom DSL parser (`GET /...`), and Code Lens ("Run" buttons).
3.  **Data Fetching:** A "Lazy Load" pattern (Fetch List -> Slice -> Fetch Details) to handle large datasets.

This plan re-scopes the migration to achieve **True Parity**.

---

## 2. Component Architecture (Strict Mapping)

### Phase 1: Global Foundation (`src/lib/components/common/`)
| Component | Upstream Source | Complexity | Status |
|-----------|-----------------|------------|--------|
| `Sidebar.svelte` | `components/Common/LeftDrawer.jsx` | Low | ✅ MVP exists |
| `Header.svelte` | `components/Common/Header.jsx` | Low | ❌ Missing |
| `CodeEditor.svelte` | `components/EditorCommon/index.jsx` | High (Monaco) | ❌ Missing |
| `ActionsMenu.svelte` | `components/Common/ActionsMenu.jsx` | Low (MUI Popover) | ❌ Missing |

### Phase 2: Collections Feature (`src/routes/collections/`)
**Critical Change:** Switch from Grid/Card layout to **Table Layout**.

| Component | Svelte Implementation | Upstream Logic |
|-----------|-----------------------|----------------|
| `CollectionTable.svelte` | `CollectionsList.jsx` | Renders MUI Table. Needs columns: Name, Status, Points, Segments, Config. |
| `CollectionRow.svelte` | `CollectionTableRow` | Handles row state + "Delete" dialog trigger. |
| `VectorsConfigChips.svelte` | `VectorsConfigChips.jsx` | Renders badges for vector params (e.g. `768 x Cosine`). |
| `AliasList.svelte` | `CollectionAliases.jsx` | Fetches/displays aliases in the name column. |

**Data Fetching Strategy (Refactor):**
- **Current:** `Promise.all(getCollection)` for ALL collections (Performance risk).
- **Target:** 
    1. `list_collections()` (Get all names).
    2. Client-side pagination (Slice names).
    3. `get_collection(name)` ONLY for the visible slice.

### Phase 3: Console Feature (`src/routes/console/`)
**Critical Change:** Implement Custom DSL Parser + Monaco Editor.

| Component | Logic to Port | Source File |
|-----------|---------------|-------------|
| `Console.svelte` | Split-pane layout, LocalStorage persistence (`qwuiConsoleCode`) | `pages/Console.jsx` |
| `RequestParser.ts` | **Port `requestFromCode`**: Parse `METHOD ENDPOINT` headers. | `CodeEditorWindow/config/RequesFromCode.js` |
| `BlockDetector.ts` | **Port `getCodeBlocks`**: Detect start/end lines of requests. | `EditorCommon/config/Rules.js` |
| `MonacoWrapper.svelte` | Register Code Lens Provider (Run Buttons) & Syntax Highlighting. | `CodeEditorWindow/index.jsx` |

### Phase 4: Points Browser (`src/routes/collections/[name]/`)
**Critical Change:** Implement Infinite Scroll Table.

| Component | Logic to Port | Source File |
|-----------|---------------|-------------|
| `PointsTable.svelte` | `Points/PointsTable.jsx` | Displays Payload + Vector preview. |
| `PayloadViewer.svelte` | `JsonViewerCustom.jsx` | Syntax highlighted, collapsible JSON viewer. |
| `FilterBuilder.svelte` | `Collections/SearchBar.jsx` | Filter construction UI. |

---

## 3. Detailed Execution Steps

### Step 1: Foundation Clean-up
1.  **Clean State:** Remove `CollectionCard.svelte` (incorrect architecture).
2.  **Strict Types:** Finalize `types.ts` to include `AliasDescription`, `CleanupStatus`.
3.  **Dependencies:** Add `svelte-monaco` (or configure Monaco loader manually) and `flexsearch` (if upstream uses it, otherwise simple filter).

### Step 2: Collection Table Implementation
1.  Create `CollectionTable.svelte` structure matching `CollectionsList.jsx`.
2.  Implement `VectorsConfigChips.svelte`.
3.  Refactor `routes/collections/+page.svelte` to implement the **Paginated Fetch Pattern**.

### Step 3: Console Logic Port
1.  Create `src/lib/console/parser.ts`: Port `codeParse` and `requestFromCode` logic (pure TS).
2.  Create `src/lib/console/blocks.ts`: Port `getCodeBlocks` logic (pure TS).
3.  Test parser with unit tests (ensure `GET collections` -> `{ method: 'GET', endpoint: 'collections' }`).

### Step 4: Console UI Implementation
1.  Implement `Console.svelte` with a split layout.
2.  Integrate Monaco Editor.
3.  Wire up `Ctrl+Enter` to `parser.ts` execution.

---

## 4. Verification Checklist
- [ ] Collections page handles >100 collections without lagging (Pagination check).
- [ ] Console correctly parses multi-line JSON bodies.
- [ ] Console supports Comments (`//`) rejection.
- [ ] "Run" button appears above loop blocks in Monaco.
