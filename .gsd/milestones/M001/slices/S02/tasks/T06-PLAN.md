# T06: Hot Reload

**Slice:** S02 — **Milestone:** M001

## Description

Implement hot reload for instant feedback when .cypcb files change.

Purpose: Enable the core development workflow - edit code, see changes immediately. This is critical for verifying the concept works.

Output: File watcher that triggers browser re-render on save, preserving viewport and selection.

## Must-Haves

- [ ] "Saving .cypcb file triggers re-render within 500ms"
- [ ] "Viewport preserved after reload (same zoom/pan)"
- [ ] "Status shows 'Reloaded' notification"
- [ ] "Selection preserved across reloads"

## Files

- `crates/cypcb-watcher/Cargo.toml`
- `crates/cypcb-watcher/src/lib.rs`
- `viewer/server.ts`
- `viewer/src/main.ts`
- `viewer/package.json`
- `Cargo.toml`
