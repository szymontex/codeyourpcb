# T09: Autorouter UI Integration

**Slice:** S05 — **Milestone:** M001

## Description

Integrate autorouting into CLI and viewer hot-reload workflow.

Purpose: Per CONTEXT.md: "Triggered on file save (same as DRC) - seamless hot-reload workflow" and "Progress indicator required". Users save file and see it routed automatically.
Output: CLI route command and automatic routing in viewer

## Must-Haves

- [ ] "CLI route command exports DSN, runs FreeRouting, imports SES"
- [ ] "Routing triggered on file save (hot reload workflow)"
- [ ] "Progress indicator shows routing is happening"
- [ ] "User can cancel routing"

## Files

- `crates/cypcb-cli/src/commands/route.rs`
- `crates/cypcb-cli/src/commands/mod.rs`
- `crates/cypcb-cli/src/main.rs`
- `viewer/src/main.ts`
- `viewer/src/wasm.ts`
