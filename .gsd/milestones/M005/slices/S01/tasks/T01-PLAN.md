---
estimated_steps: 5
estimated_files: 4
---

# T01: Create worker protocol types and extract parseSource to shared module

**Slice:** S01 — Web Worker WASM Routing
**Milestone:** M005

## Description

Create the shared infrastructure that both the Web Worker and main thread will depend on. Two new modules:

1. **`worker-protocol.ts`** — TypeScript discriminated union types for all messages between main thread and worker. Without shared types, message shape mismatches are a constant bug source.

2. **`parse-source.ts`** — Extract the `parseSource()` function from `wasm.ts` into its own module. The worker needs this function to convert board source text into a `BoardSnapshot` for `load_snapshot()`. Currently it's a private function inside `wasm.ts` (~300 lines of board DSL parsing). The extraction must be clean — `wasm.ts` should import from the new module, and all existing tests must continue to pass.

## Steps

1. **Create `viewer/src/worker-protocol.ts`** with these types:
   - `WorkerRequest` discriminated union (discriminant: `type` field):
     - `{ type: 'route', source: string }` — route a board
     - `{ type: 'route-with-params', source: string, params: string }` — route with JSON params
     - `{ type: 'route-variants', source: string }` — generate routing variants
   - `WorkerResponse` discriminated union (discriminant: `type` field):
     - `{ type: 'ready' }` — worker WASM initialized
     - `{ type: 'route-result', snapshot: BoardSnapshot, routeResult: string }` — routing complete
     - `{ type: 'variant-result', variants: string }` — variant generation complete
     - `{ type: 'error', message: string }` — worker error
   - Import `BoardSnapshot` from `./types`.

2. **Create `viewer/src/parse-source.ts`** by extracting `parseSource()` from `wasm.ts`:
   - Copy the `parseSource()` function (starts at line 224 of `wasm.ts`) and ALL helper functions/types it depends on that aren't already in `types.ts`.
   - Export `parseSource` as a named export.
   - The function signature is: `function parseSource(source: string): { snapshot: BoardSnapshot; errors: string[] }`.
   - Import types from `./types.ts` as needed.

3. **Update `viewer/src/wasm.ts`** to import `parseSource` from `./parse-source.ts`:
   - Remove the `parseSource()` function body and any helper functions that were moved to `parse-source.ts`.
   - Add `import { parseSource } from './parse-source';` at the top.
   - Keep all other code in `wasm.ts` unchanged — the `WasmPcbEngineAdapter` and `MockPcbEngine` classes still call `parseSource()` but now via the import.

4. **Verify types compile**: Run `cd viewer && npx tsc --noEmit` — must have no new errors.

5. **Verify existing tests pass**: Run `cd viewer && npx vitest run --reporter=verbose` — all tests must pass unchanged. The `parseSource` extraction should be invisible to consumers.

## Must-Haves

- [ ] `worker-protocol.ts` exports `WorkerRequest` and `WorkerResponse` discriminated union types
- [ ] `parse-source.ts` exports `parseSource()` with identical behavior to the original in `wasm.ts`
- [ ] `wasm.ts` imports `parseSource` from `parse-source.ts` — no duplicate definition
- [ ] All existing Vitest tests pass without modification
- [ ] TypeScript compiles cleanly (`npx tsc --noEmit`)

## Verification

- `cd viewer && npx tsc --noEmit` — zero errors
- `cd viewer && npx vitest run --reporter=verbose` — all existing tests pass
- `grep -c "parseSource" viewer/src/wasm.ts` shows only import usage, not function definition
- `grep "export.*WorkerRequest\|export.*WorkerResponse" viewer/src/worker-protocol.ts` shows both exports

## Inputs

- `viewer/src/wasm.ts` — contains `parseSource()` at line 224 (private function, ~300 lines of DSL parser)
- `viewer/src/types.ts` — contains `BoardSnapshot`, `ComponentInfo`, `PadInfo`, `NetInfo`, etc.
- Existing Vitest tests in `viewer/src/__tests__/` — must continue passing after extraction

## Expected Output

- `viewer/src/worker-protocol.ts` — new file with shared message types
- `viewer/src/parse-source.ts` — new file with extracted `parseSource()` function
- `viewer/src/wasm.ts` — modified to import `parseSource` from shared module (function body removed)

## Observability Impact

- **No new runtime signals**: This task is a pure refactor — no behavioral changes at runtime.
- **Future agent inspection**: `worker-protocol.ts` defines the contract for all worker messages. Any message shape mismatch between worker and main thread should be caught by TypeScript at compile time (discriminated unions enforce exhaustive handling).
- **Failure state visibility**: If `parse-source.ts` is imported incorrectly or has a different signature than expected, `npx tsc --noEmit` will fail with a type error at the import site. Tests exercising `loadWasm()` → `load_source()` will also fail.
