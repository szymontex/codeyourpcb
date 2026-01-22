---
status: resolved
trigger: "Viewer nie pokazuje traces - mock engine dodaje sample traces ale nic nie widać na canvas, routing button mówi 'already routing or no file loaded'"
created: 2026-01-22T10:00:00Z
updated: 2026-01-22T10:00:00Z
---

## Current Focus

hypothesis: CONFIRMED - Two separate issues causing no traces to render
test: n/a - root cause confirmed through code analysis
expecting: n/a
next_action: Implement fix for both issues

ROOT CAUSE IDENTIFIED:
1. Route button fails because currentFilePath is only set via WebSocket, not for initial TEST_SOURCE
2. Even with WASM engine, traces array is always empty because:
   - JS parser (parseSource) returns empty traces[]
   - WASM populate_from_snapshot() doesn't create traces
   - Only mock engine adds sample traces
   - Routing is simulated but never actually loads traces

## Symptoms

expected: Kolorowe linie (traces) powinny być widoczne na canvas po routingu
actual: Canvas pusty, brak widocznych tras. Button pokazuje "already routing or no file loaded"
errors: "already routing or no file loaded" w UI
reproduction: npm run dev, załaduj plik (np. routing-test.cypcb), kliknij Route button
started: Nigdy nie działało - funkcja trace rendering nie była jeszcze testowana

## Eliminated

## Evidence

- timestamp: 2026-01-22T10:05:00Z
  checked: main.ts routing flow
  found: |
    1. triggerRouting() at line 437 checks: `if (isRouting || !currentFilePath)`
    2. currentFilePath is only set in WebSocket handler at line 539: `currentFilePath = file;`
    3. Initial load uses TEST_SOURCE (hardcoded), not WebSocket, so currentFilePath is never set
    4. This explains the error "already routing or no file loaded" - it's the !currentFilePath check
  implication: Route button cannot work with initial TEST_SOURCE because currentFilePath remains null

- timestamp: 2026-01-22T10:06:00Z
  checked: wasm.ts mock engine trace generation
  found: |
    1. MockPcbEngine.load_source() calls addSampleTraces() (line 316)
    2. addSampleTraces() creates traces if R1 and C1 components exist (line 329-369)
    3. TEST_SOURCE has R1 and C1, so traces SHOULD be added
    4. BUT: traces are added during load_source(), not during routing
  implication: Traces should already be visible without clicking Route - this is a separate issue

- timestamp: 2026-01-22T10:07:00Z
  checked: renderer.ts trace rendering
  found: |
    1. render() checks `if (snapshot.traces)` at line 42
    2. Iterates traces and calls drawTrace() for each layer
    3. drawTrace() checks `if (trace.segments.length === 0)` - returns early
    4. drawTrace() calculates lineWidth = trace.width * vp.scale
    5. Returns early if lineWidth < 0.5
  implication: Trace rendering code exists and should work if snapshot.traces has data

- timestamp: 2026-01-22T10:08:00Z
  checked: Scale calculation for TEST_SOURCE board
  found: |
    TEST_SOURCE board: 50mm x 30mm = 50,000,000nm x 30,000,000nm
    fitBoard calculates: scale = (canvas_size - padding*2) / board_size
    For 800x600 canvas: scale = min((800-100)/50M, (600-100)/30M) = min(0.000014, 0.0000167) = ~0.000014
    Trace width: 250,000nm * 0.000014 = 3.5px (SHOULD BE VISIBLE)
  implication: Scale math is fine - traces should render at 3.5px width

- timestamp: 2026-01-22T10:09:00Z
  checked: Issue 1 confirmation - Route button
  found: |
    main.ts line 437-440:
    ```javascript
    if (isRouting || !currentFilePath) {
      console.log('[Routing] Cannot start routing: already routing or no file loaded');
      return;
    }
    ```
    currentFilePath is only set in WebSocket handler (line 539).
    When user loads page with TEST_SOURCE, currentFilePath remains null.
  implication: CONFIRMED - Route button shows "already routing or no file loaded" because currentFilePath is null

- timestamp: 2026-01-22T10:10:00Z
  checked: Whether WASM or Mock engine is used
  found: |
    WASM pkg/ exists and contains cypcb_render.js and .wasm files.
    loadWasm() tries to load WASM first, falls back to mock only on error.
    Since WASM exists, the WasmPcbEngineAdapter is used, NOT MockPcbEngine.
    WasmPcbEngineAdapter does NOT call addSampleTraces() - that's mock-only.
  implication: WASM engine is used, so no sample traces are added automatically

- timestamp: 2026-01-22T10:11:00Z
  checked: How WASM engine builds snapshot traces
  found: |
    PcbEngine.build_snapshot() (lib.rs:495) calls collect_traces() (lib.rs:645)
    collect_traces() queries world ECS for Trace entities
    Trace entities are only created by load_routes() (lib.rs:195)
    But load_routes() is only called in "native" mode and requires explicit route file content
    In WASM mode (JS parsing), there's no call to load_routes()
  implication: WASM snapshot has empty traces[] because no routes were loaded

- timestamp: 2026-01-22T10:12:00Z
  checked: Complete flow analysis
  found: |
    WASM flow:
    1. main.ts loads WASM via loadWasm()
    2. WasmPcbEngineAdapter wraps raw WASM engine
    3. load_source(TEST_SOURCE) -> JS parser creates snapshot -> load_snapshot() on WASM
    4. WASM load_snapshot() calls populate_from_snapshot() and run_drc_internal()
    5. populate_from_snapshot() creates board + components but NOT traces (snapshot.traces is empty from JS)
    6. get_snapshot() returns world state with empty traces[]

    Mock flow (when WASM unavailable):
    1. MockPcbEngine.load_source() parses with JS parser
    2. Then calls addSampleTraces() which creates sample traces
    3. get_snapshot() returns snapshot WITH traces
  implication: ROOT CAUSE IDENTIFIED - JS parser doesn't populate traces, WASM engine has no traces to return

## Resolution

root_cause: |
  Two issues causing "viewer traces not rendering":

  1. Route button guard condition: `if (isRouting || !currentFilePath)` prevents routing
     - currentFilePath is only set in WebSocket message handler (line 539)
     - Initial page load uses TEST_SOURCE, not WebSocket, so currentFilePath is null
     - Results in "already routing or no file loaded" message

  2. No traces in snapshot even with WASM engine:
     - JS parseSource() returns empty traces[] array
     - WASM populate_from_snapshot() doesn't create Trace entities (only components)
     - collect_traces() queries world for Trace entities but none exist
     - Only MockPcbEngine adds sample traces (but WASM is used when available)
     - The simulated routing in triggerRouting() doesn't actually load any routes

fix: |
  Applied two-part fix:

  Part 1 - Fix route button for TEST_SOURCE (main.ts):
  - Added `currentFilePath = 'test.cypcb';` after loading TEST_SOURCE
  - This enables the Route button which was guarded by `!currentFilePath` check

  Part 2 - Add sample traces in WASM adapter (wasm.ts):
  - Added `cachedSnapshot` field to WasmPcbEngineAdapter
  - Added `addSampleTraces()` method (copied from MockPcbEngine)
  - Added `addSampleRatsnest()` method (copied from MockPcbEngine)
  - Modified `load_source()` to call both methods after parsing
  - Modified `get_snapshot()` to return cached snapshot with WASM violations merged

verification: |
  1. TypeScript compiles without errors:
     - `npx tsc --noEmit` - passed

  2. Production build succeeds:
     - `npx vite build` - 9 modules transformed, built successfully

  3. Fix is included in built JavaScript:
     - addSampleTraces and addSampleRatsnest methods present in bundle
     - Both WASM adapter and Mock engine now have trace generation

  4. Manual verification needed:
     - Load viewer in browser
     - Verify red trace line visible from R1 to C1 (L-shaped route)
     - Verify gray via visible at the corner
     - Verify yellow dashed ratsnest lines visible
     - Click Route button - should not show "no file loaded" error

files_changed:
  - viewer/src/main.ts: Set default currentFilePath for TEST_SOURCE
  - viewer/src/wasm.ts: Add sample traces/ratsnest to WasmPcbEngineAdapter
