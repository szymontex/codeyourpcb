# S08: Platform Abstraction Layer

**Goal:** Create the cypcb-platform crate with build infrastructure, error types, cfg_aliases, and the FileSystem trait with native and web implementations.
**Demo:** Create the cypcb-platform crate with build infrastructure, error types, cfg_aliases, and the FileSystem trait with native and web implementations.

## Must-Haves


## Tasks

- [x] **T01: FileSystem Trait & Implementations**
  - Create the cypcb-platform crate with build infrastructure, error types, cfg_aliases, and the FileSystem trait with native and web implementations.

Purpose: Establishes the foundation crate that all platform abstractions live in, starting with the most complex abstraction (file system). This prevents platform-specific code from leaking into business logic crates.

Output: New cypcb-platform crate with FileSystem trait, two implementations, and build-time platform selection.
- [x] **T02: Dialog & Storage Traits**
  - Implement the Dialog wrapper and Storage trait with SQLite (native) and IndexedDB (web) backends.

Purpose: Completes the remaining platform abstractions needed for persistence and user interaction. Dialog wraps rfd (already cross-platform). Storage requires custom implementations since rusqlite doesn't support WASM.

Output: Dialog struct and Storage trait with two platform-specific implementations.
- [x] **T03: Menu Data Model & Platform Facade** `est:4min`
  - Add the Menu abstraction and Platform facade that provides a single entry point to all platform services.

Purpose: Menu completes the four required abstractions (PLAT-04). Platform facade gives application code one import to access all platform services, preventing business logic from importing platform-specific types. This is the integration layer.

Output: Menu types for declarative menu building, Platform struct aggregating all services.

## Files Likely Touched

- `crates/cypcb-platform/Cargo.toml`
- `crates/cypcb-platform/build.rs`
- `crates/cypcb-platform/src/lib.rs`
- `crates/cypcb-platform/src/error.rs`
- `crates/cypcb-platform/src/fs.rs`
- `crates/cypcb-platform/src/fs_native.rs`
- `crates/cypcb-platform/src/fs_web.rs`
- `Cargo.toml`
- `crates/cypcb-platform/src/dialog.rs`
- `crates/cypcb-platform/src/storage.rs`
- `crates/cypcb-platform/src/storage_native.rs`
- `crates/cypcb-platform/src/storage_web.rs`
- `crates/cypcb-platform/src/menu.rs`
- `crates/cypcb-platform/src/lib.rs`
- `crates/cypcb-platform/src/platform.rs`
