---
status: resolved
trigger: "DRC in WASM returns violations: [] even though R1 and R2 are 0.5mm apart"
created: 2026-01-22T00:00:00Z
updated: 2026-01-22T00:20:00Z
---

## Current Focus

hypothesis: WasmPcbEngineAdapter returns JS-parsed snapshot instead of WASM engine snapshot. JS snapshot has empty violations array; WASM snapshot has real DRC results.
test: Examine get_snapshot() implementation in WasmPcbEngineAdapter
expecting: Adapter bypasses WASM engine snapshot and returns cached JS snapshot
next_action: Fix adapter to return WASM engine snapshot (which includes DRC violations)

## Symptoms

expected: DRC should detect clearance violations between R1 and R2 components that are 0.5mm apart
actual: DRC returns violations: [] (empty array)
errors: No explicit errors - just empty results
reproduction: Run DRC check in WASM mode with two 0402 resistors (R1, R2) placed 0.5mm apart
started: Unknown - investigate recent commits related to DRC WASM

## Eliminated

## Evidence

- timestamp: 2026-01-22T00:10:00Z
  checked: Native DRC tests with clearance violation
  found: Both test_drc_detects_clearance_violations and test_drc_from_snapshot_detects_violations PASS
  implication: Rust DRC code works correctly in native mode for both source parsing and snapshot loading

- timestamp: 2026-01-22T00:11:00Z
  checked: ClearanceRule implementation
  found: Rule iterates over world.spatial().iter(), expands bounds by min_clearance, queries for candidates
  implication: Depends on spatial index being correctly populated with entries

- timestamp: 2026-01-22T00:12:00Z
  checked: populate_from_snapshot implementation
  found: Calls world.rebuild_spatial_index() which queries FootprintRef components and uses lib.get(name) for bounds
  implication: Footprint lookup must succeed and return correct courtyard bounds

- timestamp: 2026-01-22T00:15:00Z
  checked: WasmPcbEngineAdapter in viewer/src/wasm.ts
  found: get_snapshot() returns this.currentSnapshot (JS-parsed) instead of this.wasmEngine.get_snapshot()
  implication: DRC violations computed by WASM are discarded; empty JS violations array is returned

- timestamp: 2026-01-22T00:16:00Z
  checked: Code comments in WasmPcbEngineAdapter
  found: Comment says "WASM spatial index isn't populated by load_snapshot" - this is WRONG, our tests prove it IS populated
  implication: Code was written with incorrect assumption; needs to be fixed to use WASM results

## Resolution

root_cause: WasmPcbEngineAdapter.get_snapshot() returns cached JS-parsed snapshot (which has empty violations[]) instead of calling this.wasmEngine.get_snapshot() (which has DRC results from Rust).

fix: Changed get_snapshot() to return this.wasmEngine.get_snapshot() and query_point() to use this.wasmEngine.query_point() instead of JS fallback implementations.

verification:
- All three WASM tests pass: test-wasm.mjs, test-wasm-integration.mjs, test-wasm-drc.mjs
- Clearance violations now detected: "Clearance violation: 0.00mm actual, 0.15mm required"
- Point queries now use WASM spatial index instead of naive JS loop
- Native Rust tests still pass (11 tests in cypcb-render)

files_changed:
- viewer/src/wasm.ts: Fixed get_snapshot() and query_point() to use WASM engine
- viewer/test-wasm.mjs: Added missing violations field to snapshot
- viewer/test-wasm-integration.mjs: Added missing violations field to snapshot
- viewer/test-wasm-drc.mjs: New test file for DRC clearance detection
- crates/cypcb-render/src/lib.rs: Added two new tests for DRC verification
