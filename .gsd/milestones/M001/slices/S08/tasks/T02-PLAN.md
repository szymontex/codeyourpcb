# T02: Dialog & Storage Traits

**Slice:** S08 — **Milestone:** M001

## Description

Implement the Dialog wrapper and Storage trait with SQLite (native) and IndexedDB (web) backends.

Purpose: Completes the remaining platform abstractions needed for persistence and user interaction. Dialog wraps rfd (already cross-platform). Storage requires custom implementations since rusqlite doesn't support WASM.

Output: Dialog struct and Storage trait with two platform-specific implementations.

## Must-Haves

- [ ] "Dialog trait wraps rfd for message/confirm dialogs with identical API on both platforms"
- [ ] "Storage trait provides key-value persistence with table namespacing"
- [ ] "Native storage uses SQLite via rusqlite"
- [ ] "Web storage uses IndexedDB via web-sys bindings"
- [ ] "Both storage implementations compile for their respective targets"

## Files

- `crates/cypcb-platform/src/dialog.rs`
- `crates/cypcb-platform/src/storage.rs`
- `crates/cypcb-platform/src/storage_native.rs`
- `crates/cypcb-platform/src/storage_web.rs`
