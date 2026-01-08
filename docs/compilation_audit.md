# Compilation Error Audit

**Timestamp:** 2026-01-05
**Status:** Analyzing 23 compilation errors in `src-tauri`.

## 1. Missing Imports (`state` type)
**Error:** `cannot find type 'State' in this scope`
**Location:** `src/commands/collections.rs`
**Cause:** Removed `use tauri::State;` during previous refactor.
**Fix:** Restore `use tauri::State;` in `src/commands/collections.rs`.

## 2. Unresolved Imports (`PointStruct`)
**Error:** `unresolved import 'qdrant_lib::collection::operations::types::PointStruct'`
**Location:** `src/commands/points.rs`
**Cause:** `PointStruct` is located in `api::rest::schema`, not `collection::operations::types`.
**Fix:** Import from `qdrant_lib::PointStruct` (re-exported) or `qdrant_lib::api::rest::schema::PointStruct`.

## 3. Structural Mismatches (`VectorParams`)
**Error:** `missing field 'datatype' in initializer of 'VectorParams'`
**Location:** `src/commands/collections.rs`
**Cause:** `VectorParams` has a field `datatype` (likely for quantization/compression types) that is not being initialized.
**Fix:** Initialize `datatype: None` (or appropriate default). check definition.

## 4. Structural Mismatches (`ScrollRequestInternal`)
**Error:** `missing field 'order_by' in initializer of 'ScrollRequestInternal'`
**Location:** `src/commands/points.rs`
**Cause:** `ScrollRequestInternal` has a new field `order_by`.
**Fix:** Initialize `order_by: None`.

## 5. Wrapper Struct Mismatches (`PointRequest`)
**Error:** `struct 'PointRequest' has no field named 'ids'`, `'with_payload'`, `'with_vector'`
**Location:** `src/commands/points.rs`
**Cause:** `PointRequest` is a wrapper struct containing `point_request` and `shard_key`. The fields `ids`, `with_payload`, etc., belong to the inner type (likely `PointRequestInternal` or similar variant).
**Fix:** Construct the nested structure:
```rust
PointRequest {
    point_request: KeyType { // Need to identify key type (e.g. PointsSelector?)
       ids: ...,
       ...
    },
    shard_key: None
}
```

## 6. Type Mismatches (`delete_points`)
**Error:** `expected 'PointsSelector', found '&Vec<ExtendedPointId>'`
**Location:** `src/commands/points.rs`
**Cause:** `delete_points` takes a `PointsSelector` enum, but we are passing a vector of IDs directly.
**Fix:** Wrap IDs in `PointsSelector::PointIds(vec)`.

## 7. Type Inference (`map_err`)
**Error:** `type annotations needed`
**Location:** Various
**Cause:** Likely downstream of the missing `State` type making the method call on `state.qdrant` invalid, thus preventing return type inference.
**Fix:** Fixing item #1 (`State` import) should resolve most of these.

## Action Plan
1.  **Verify Definitions:** Inspect `VectorParams`, `PointRequest`, `ScrollRequestInternal`, and `PointsSelector` in `qdrant-lib` source to confirm field names and structure.
2.  **Refactor Code:** Apply fixes to `collections.rs` and `points.rs` based on verified definitions.
3.  **Validate:** Run `cargo check` again.
