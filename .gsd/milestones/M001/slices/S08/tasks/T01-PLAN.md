# T01: FileSystem Trait & Implementations

**Slice:** S08 — **Milestone:** M001

## Description

Create the cypcb-platform crate with build infrastructure, error types, cfg_aliases, and the FileSystem trait with native and web implementations.

Purpose: Establishes the foundation crate that all platform abstractions live in, starting with the most complex abstraction (file system). This prevents platform-specific code from leaking into business logic crates.

Output: New cypcb-platform crate with FileSystem trait, two implementations, and build-time platform selection.

## Must-Haves

- [ ] "FileSystem trait defines async read/write/pick operations without exposing platform details"
- [ ] "Native implementation uses std::fs with rfd for file picking"
- [ ] "Web implementation uses web-sys File System Access API with input fallback"
- [ ] "cfg_aliases in build.rs provide wasm/native shorthand for all modules"
- [ ] "Crate compiles for both native and wasm32-unknown-unknown targets"

## Files

- `crates/cypcb-platform/Cargo.toml`
- `crates/cypcb-platform/build.rs`
- `crates/cypcb-platform/src/lib.rs`
- `crates/cypcb-platform/src/error.rs`
- `crates/cypcb-platform/src/fs.rs`
- `crates/cypcb-platform/src/fs_native.rs`
- `crates/cypcb-platform/src/fs_web.rs`
- `Cargo.toml`
