# Phase 9: Platform Abstraction Layer - Research

**Researched:** 2026-01-29
**Domain:** Rust cross-platform development (desktop native + WASM)
**Confidence:** HIGH

## Summary

Platform abstraction for Rust applications targeting both desktop (native) and web (WASM) requires careful architectural decisions around conditional compilation, trait design, and dependency management. The standard approach uses Cargo features with `target_arch = "wasm32"` conditional compilation to select platform-specific implementations at build time.

Research reveals that the Rust ecosystem has mature solutions for cross-platform file dialogs (rfd crate), but storage abstraction (SQLite vs IndexedDB) requires custom implementation since rusqlite doesn't support WASM targets. The File System Access API provides browser file operations but requires web-sys bindings and has limited browser support (Chromium-based browsers only).

The key architectural insight is that **abstraction boundaries must be defined at the trait level**, with platform-specific implementations in separate modules selected via `#[cfg_attr]`. This prevents the "800% code duplication risk" mentioned in project requirements by ensuring business logic remains platform-agnostic.

**Primary recommendation:** Use Cargo feature flags (`desktop` and `web`) combined with trait-based abstractions and the rfd crate for file dialogs. Implement custom storage traits for SQLite/IndexedDB bridging, avoiding premature abstraction of menu systems until requirements are clearer.

## Standard Stack

The established libraries/tools for this domain:

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Cargo features | Built-in | Build-time conditional compilation | Official Rust mechanism for platform selection |
| cfg_if | 1.x | Ergonomic conditional blocks | Industry standard for complex platform logic (though rustfmt doesn't format contents) |
| cfg_aliases | Latest | Simplify complex cfg expressions | Reduces `#[cfg(all(unix, not(target_arch = "wasm32")))]` to readable aliases |
| rfd | 0.15+ | Cross-platform file dialogs | Only mature library with native + WASM support (32.3k dependents) |
| wasm-bindgen | 0.2 | WASM bindings to JavaScript | Required for all browser API access from WASM |
| web-sys | 0.3 | Browser API bindings | Official way to access File System Access API and other web APIs |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| rusqlite | 0.32+ | SQLite bindings for native | Desktop persistence (does NOT support WASM) |
| indxdb | Latest | IndexedDB abstraction for WASM | WASM-only persistence via SurrealDB project |
| async-trait | 0.1 | Async trait definitions | When abstracting async operations across platforms |
| js-sys | 0.3 | JavaScript global bindings | Accessing JavaScript built-ins from WASM |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| rfd | tauri-plugin-dialog | Only works in Tauri apps, not standalone WASM |
| Custom storage traits | sqlx | SQLx doesn't compile for wasm32-unknown-unknown |
| cfg_if | Manual #[cfg] everywhere | More verbose, harder to maintain |
| Feature flags | Runtime checks with cfg!() | Performance cost, larger binary size |

**Installation:**
```toml
[dependencies]
cfg-if = "1"
rfd = { version = "0.15", features = ["async"] }

[target.'cfg(target_arch = "wasm32")'.dependencies]
wasm-bindgen = "0.2"
web-sys = { version = "0.3", features = ["Window", "FileSystemFileHandle"] }
js-sys = "0.3"
indxdb = "0.1"  # For IndexedDB abstraction

[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
rusqlite = { version = "0.32", features = ["bundled"] }

[features]
default = []
desktop = []
web = []
```

## Architecture Patterns

### Recommended Project Structure
```
crates/cypcb-platform/
├── src/
│   ├── lib.rs              # Public trait definitions
│   ├── fs.rs               # FileSystem trait
│   ├── fs_native.rs        # Native implementation
│   ├── fs_web.rs           # WASM implementation
│   ├── dialog.rs           # Dialog trait (thin wrapper over rfd)
│   ├── storage.rs          # Storage trait
│   ├── storage_native.rs   # SQLite implementation
│   ├── storage_web.rs      # IndexedDB implementation
│   └── menu.rs             # Menu trait (desktop-only initially)
├── Cargo.toml
└── build.rs                # cfg_aliases setup
```

### Pattern 1: Module Selection with cfg_attr
**What:** Use `#[cfg_attr]` to select entire module files based on target platform
**When to use:** When implementations diverge significantly (file system, storage)
**Example:**
```rust
// src/lib.rs
// Source: https://doc.rust-lang.org/reference/conditional-compilation.html

#[cfg_attr(target_arch = "wasm32", path = "fs_web.rs")]
#[cfg_attr(not(target_arch = "wasm32"), path = "fs_native.rs")]
mod fs_impl;

pub use fs_impl::FileSystemImpl;

// Public trait visible to all consumers
pub trait FileSystem {
    async fn read_file(&self, path: &str) -> Result<Vec<u8>, Error>;
    async fn write_file(&self, path: &str, data: &[u8]) -> Result<(), Error>;
}
```

### Pattern 2: Trait-First Abstraction
**What:** Define traits in platform-agnostic modules, implement in platform-specific modules
**When to use:** For all public APIs that business logic depends on
**Example:**
```rust
// src/storage.rs
#[async_trait::async_trait]
pub trait Storage {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, Error>;
    async fn set(&self, key: &str, value: &[u8]) -> Result<(), Error>;
    async fn delete(&self, key: &str) -> Result<(), Error>;
}

// src/storage_native.rs
// Source: Official Cargo features documentation
#[cfg(not(target_arch = "wasm32"))]
pub struct SqliteStorage {
    conn: rusqlite::Connection,
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait::async_trait]
impl Storage for SqliteStorage {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, Error> {
        // SQLite implementation
    }
    // ...
}
```

### Pattern 3: Using cfg_aliases for Readability
**What:** Define platform aliases in build.rs to avoid complex cfg expressions
**When to use:** When the same platform checks appear multiple times
**Example:**
```rust
// build.rs
// Source: https://docs.rs/cfg_aliases
fn main() {
    cfg_aliases::cfg_aliases! {
        wasm: { target_arch = "wasm32" },
        native: { not(target_arch = "wasm32") },
        native_unix: { all(unix, not(target_arch = "wasm32")) },
    }
}

// Then in code:
#[cfg(wasm)]
use web_sys::Window;

#[cfg(native)]
use std::fs;
```

### Pattern 4: Dependency Wrapper Pattern (rfd)
**What:** Wrap third-party abstractions to control API surface and reduce coupling
**When to use:** When a crate like rfd already handles platform differences
**Example:**
```rust
// src/dialog.rs
// Source: https://docs.rs/rfd
pub struct Dialog;

impl Dialog {
    pub async fn pick_file(&self, filters: &[(&str, &[&str])]) -> Result<Option<PathBuf>, Error> {
        use rfd::AsyncFileDialog;

        let mut dialog = AsyncFileDialog::new();
        for (name, extensions) in filters {
            dialog = dialog.add_filter(name, extensions);
        }

        let handle = dialog.pick_file().await;
        Ok(handle.map(|h| h.path().to_path_buf()))
    }
}
```

### Pattern 5: Conditional Dependencies in Cargo.toml
**What:** Use `[target.'cfg(...)'.dependencies]` to include platform-specific crates
**When to use:** Always for WASM-specific (wasm-bindgen) or native-specific (rusqlite) deps
**Example:**
```toml
# Source: https://doc.rust-lang.org/cargo/reference/features.html
[dependencies]
# Shared dependencies
async-trait = "0.1"
thiserror = "2.0"

[target.'cfg(target_arch = "wasm32")'.dependencies]
wasm-bindgen = "0.2"
web-sys = { version = "0.3", features = ["Window"] }

[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
rusqlite = { version = "0.32", features = ["bundled"] }
```

### Anti-Patterns to Avoid
- **Scattered #[cfg] throughout business logic:** Creates the "800% duplication" problem. Keep platform checks in abstraction layer only.
- **Runtime platform detection with cfg!() macro:** Increases binary size (includes both implementations), adds runtime overhead. Use compile-time #[cfg] instead.
- **Over-abstraction before requirements are clear:** Don't create Menu trait until you know both desktop and web menu needs. Start with desktop-only.
- **Mutually exclusive features without compile_error!:** If `desktop` and `web` features shouldn't both be enabled, add explicit check in lib.rs.
- **Enabling target-specific dependencies in base [dependencies]:** Always use `[target.'cfg(...)'.dependencies]` to avoid build errors on other platforms.

## Don't Hand-Roll

Problems that look simple but have existing solutions:

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Cross-platform file dialogs | Custom FFI to native dialogs + HTML input elements | rfd crate | 32.3k projects use it; handles GTK3/XDG Portal/Windows/macOS/WASM with single API |
| WASM JavaScript bindings | Manual extern "C" declarations | wasm-bindgen + web-sys | Official tooling, auto-generated from WebIDL specs, type-safe |
| Async traits | Manual Future implementations | async-trait crate | Handles Box<dyn Future> complexity, Send bounds for non-WASM |
| Complex cfg expressions | Nested #[cfg(all(not(...)))] | cfg_aliases in build.rs | Centralizes platform logic, improves readability dramatically |
| SQLite WASM compatibility | Custom VFS layer | Separate native/web storage traits | rusqlite fundamentally incompatible with WASM; trait abstraction is cleaner than VFS hacks |

**Key insight:** The Rust WASM ecosystem has matured significantly (2025-2026). Most "bridge the gap" problems now have crate solutions. Custom solutions are only needed for domain-specific abstractions (your storage trait), not for platform bridging itself.

## Common Pitfalls

### Pitfall 1: Feature Unification Across Build Targets
**What goes wrong:** Cargo resolver v1 unifies features across all dependency edges, so platform-specific dependencies can leak features to wrong target
**Why it happens:** Default resolver doesn't separate build-dependencies from runtime dependencies
**How to avoid:** Use `resolver = "2"` in workspace Cargo.toml (already set in your project)
**Warning signs:** Build errors like "wasm-bindgen not found" when building for native, or rusqlite linker errors on WASM

**Source:** https://doc.rust-lang.org/cargo/reference/features.html

### Pitfall 2: Forgetting async/await in WASM File Operations
**What goes wrong:** Synchronous file APIs (std::fs) work on desktop but File System Access API is async-only
**Why it happens:** Browser security model requires user gesture + async I/O
**How to avoid:** Design all FileSystem trait methods as `async fn` from the start, even if native impl uses blocking I/O internally
**Warning signs:** "User gesture required" errors in browser, can't use std::fs in WASM

**Source:** https://rustwasm.github.io/book/reference/add-wasm-support-to-crate.html

### Pitfall 3: Path Handling Differences
**What goes wrong:** Desktop uses actual paths (PathBuf), WASM uses opaque FileHandle references or content:// URIs on Android
**Why it happens:** Browser sandbox doesn't expose real file paths for security
**How to avoid:** Abstract over PathBuf vs FileHandle in trait design; use content-addressable storage or opaque handles
**Warning signs:** Path operations work in native builds but fail in WASM with security errors

**Source:** https://v2.tauri.app/plugin/dialog/ and https://developer.chrome.com/docs/capabilities/web-apis/file-system-access

### Pitfall 4: Assuming rusqlite Works in WASM
**What goes wrong:** Adding rusqlite as optional dependency and expecting conditional compilation to exclude it on WASM
**Why it happens:** Rusqlite requires native SQLite C library, fundamentally incompatible with WASM
**How to avoid:** Use separate storage crates for native (rusqlite) and WASM (indxdb), selected via `[target.'cfg(...)'.dependencies]`
**Warning signs:** Link errors or "SQLite not found" when building for wasm32-unknown-unknown

**Source:** https://github.com/rusqlite/rusqlite

### Pitfall 5: Premature Menu Abstraction
**What goes wrong:** Creating Menu trait before understanding web requirements leads to leaky abstraction
**Why it happens:** Tauri menus (native OS menus) and HTML menus are fundamentally different UI paradigms
**How to avoid:** Start desktop-only with tauri::menu directly, defer web menu until requirements solidify
**Warning signs:** Menu trait with lots of #[cfg(not(target_arch = "wasm32"))] or Option<> returns indicating incomplete abstraction

**Source:** Community best practices from https://medium.com/@wedevare/traits-generics-where-abstraction-mistakes-in-rust-you-must-avoid-cf972d369797

### Pitfall 6: File System Access API Browser Compatibility
**What goes wrong:** Code works in Chrome but fails silently in Firefox
**Why it happens:** Firefox marks user filesystem access as harmful, only supports Origin Private File System (OPFS)
**How to avoid:** Feature-detect with `'showOpenFilePicker' in window`, fall back to `<input type="file">` wrapper
**Warning signs:** Different behavior across browsers, missing API errors in Firefox/Safari

**Source:** https://github.com/rustwasm/wasm-bindgen/issues/2868

## Code Examples

Verified patterns from official sources:

### FileSystem Trait Definition
```rust
// Source: Synthesized from Rust Reference + rustwasm book
use async_trait::async_trait;

#[async_trait]
pub trait FileSystem {
    type Handle: FileHandle;

    async fn pick_file(&self) -> Result<Option<Self::Handle>, Error>;
    async fn read(&self, handle: &Self::Handle) -> Result<Vec<u8>, Error>;
    async fn write(&self, handle: &Self::Handle, data: &[u8]) -> Result<(), Error>;
}

pub trait FileHandle: Send + Sync {
    fn name(&self) -> &str;
    // Note: Don't expose path() - not available in WASM
}
```

### Native FileSystem Implementation
```rust
// Source: std::fs documentation + async patterns
#[cfg(not(target_arch = "wasm32"))]
mod native {
    use super::*;
    use std::path::PathBuf;

    pub struct NativeFileSystem;
    pub struct NativeHandle {
        path: PathBuf,
        name: String,
    }

    impl FileHandle for NativeHandle {
        fn name(&self) -> &str {
            &self.name
        }
    }

    #[async_trait]
    impl FileSystem for NativeFileSystem {
        type Handle = NativeHandle;

        async fn pick_file(&self) -> Result<Option<Self::Handle>, Error> {
            // Uses rfd internally
            let file = rfd::AsyncFileDialog::new()
                .pick_file()
                .await;

            Ok(file.map(|f| NativeHandle {
                name: f.file_name(),
                path: f.path().to_path_buf(),
            }))
        }

        async fn read(&self, handle: &Self::Handle) -> Result<Vec<u8>, Error> {
            // Spawn blocking to avoid blocking async runtime
            tokio::task::spawn_blocking({
                let path = handle.path.clone();
                move || std::fs::read(&path)
            }).await?
        }
    }
}
```

### WASM FileSystem Implementation
```rust
// Source: https://rustwasm.github.io/wasm-bindgen/api/web_sys/
#[cfg(target_arch = "wasm32")]
mod web {
    use super::*;
    use wasm_bindgen::prelude::*;
    use web_sys::{Window, FileSystemFileHandle};

    pub struct WebFileSystem {
        window: Window,
    }

    pub struct WebHandle {
        handle: FileSystemFileHandle,
        name: String,
    }

    impl FileHandle for WebHandle {
        fn name(&self) -> &str {
            &self.name
        }
    }

    #[async_trait(?Send)]  // WASM is single-threaded
    impl FileSystem for WebFileSystem {
        type Handle = WebHandle;

        async fn pick_file(&self) -> Result<Option<Self::Handle>, Error> {
            // Note: Requires --cfg=web_sys_unstable_apis
            let promise = self.window.show_open_file_picker()?;
            let handle_array = wasm_bindgen_futures::JsFuture::from(promise).await?;

            if handle_array.length() > 0 {
                let handle: FileSystemFileHandle = handle_array.get(0).into();
                let name = handle.name();
                Ok(Some(WebHandle { handle, name }))
            } else {
                Ok(None)
            }
        }

        async fn read(&self, handle: &Self::Handle) -> Result<Vec<u8>, Error> {
            let file_promise = handle.handle.get_file();
            let file = wasm_bindgen_futures::JsFuture::from(file_promise).await?;

            // Read file contents via ArrayBuffer
            let buffer_promise = file.array_buffer();
            let buffer = wasm_bindgen_futures::JsFuture::from(buffer_promise).await?;

            let array = js_sys::Uint8Array::new(&buffer);
            Ok(array.to_vec())
        }
    }
}
```

### Storage Trait with Platform Implementations
```rust
// Source: Design pattern synthesized from indxdb + rusqlite docs
#[async_trait::async_trait]
pub trait Storage: Send + Sync {
    async fn init(&mut self) -> Result<(), Error>;
    async fn get(&self, table: &str, key: &str) -> Result<Option<Vec<u8>>, Error>;
    async fn set(&self, table: &str, key: &str, value: &[u8]) -> Result<(), Error>;
    async fn delete(&self, table: &str, key: &str) -> Result<(), Error>;
    async fn list_keys(&self, table: &str) -> Result<Vec<String>, Error>;
}

#[cfg(not(target_arch = "wasm32"))]
pub struct SqliteStorage {
    // Implementation uses rusqlite
}

#[cfg(target_arch = "wasm32")]
pub struct IndexedDbStorage {
    // Implementation uses indxdb or web_sys::IdbDatabase
}
```

### Conditional Compilation with cfg_aliases
```rust
// build.rs
// Source: https://docs.rs/cfg_aliases
use cfg_aliases::cfg_aliases;

fn main() {
    // Setup cfg aliases
    cfg_aliases! {
        // Platforms
        wasm: { target_arch = "wasm32" },
        native: { not(target_arch = "wasm32") },

        // Specific combinations
        native_unix: { all(unix, not(target_arch = "wasm32")) },
        native_windows: { all(windows, not(target_arch = "wasm32")) },
    }
}

// lib.rs
#[cfg(wasm)]
compile_error!("This module is not supported on WASM");

#[cfg(native)]
pub mod native_only_feature;
```

### Feature Detection in WASM
```rust
// Source: https://developer.chrome.com/docs/capabilities/web-apis/file-system-access
#[cfg(target_arch = "wasm32")]
pub fn supports_file_system_access() -> bool {
    use wasm_bindgen::JsCast;

    let window = web_sys::window().expect("no global window");

    // Check for API support
    js_sys::Reflect::has(&window, &"showOpenFilePicker".into())
        .unwrap_or(false)
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Runtime platform detection with cfg!() | Compile-time #[cfg] attributes | Always available, emphasized in 2024+ docs | Smaller binaries, zero runtime cost |
| Manual extern declarations for JS | wasm-bindgen + web-sys | wasm-bindgen stable 2019, best practice 2024+ | Type safety, automatic binding generation |
| Custom file dialog FFI per platform | rfd crate | rfd 0.1 in 2020, WASM support added ~2021 | Single API, 97% less code |
| absurd-sql for SQLite in WASM | Separate storage abstractions | Community shift 2024-2025 | Cleaner architecture, better performance |
| Cargo resolver v1 | Cargo resolver v2 | Stabilized 2021, default in 2024 edition | Fixes feature unification issues |
| web_sys unstable APIs require manual cfg | web_sys stable File System Access | Still unstable as of 2026 | Requires --cfg=web_sys_unstable_apis flag |

**Deprecated/outdated:**
- **stdweb:** Replaced by wasm-bindgen ecosystem (deprecated 2020)
- **absurd-sql for production use:** wa-sqlite or separate trait abstraction preferred for IndexedDB persistence
- **cargo-web:** Replaced by wasm-pack and trunk (cargo-web unmaintained since 2021)
- **Resolver v1 in new projects:** Use resolver = "2" in Cargo.toml workspace

## Open Questions

Things that couldn't be fully resolved:

1. **File System Access API stability in web-sys**
   - What we know: Requires `--cfg=web_sys_unstable_apis` flag as of early 2026
   - What's unclear: Timeline for stabilization; whether to use web-sys directly or wrap in compatibility layer
   - Recommendation: Use rfd for file picker (hides this complexity), only use web-sys FileSystemFileHandle directly if you need write-back to same file

2. **Menu abstraction viability**
   - What we know: Tauri uses native OS menus, web would need HTML/CSS menus (completely different UX paradigms)
   - What's unclear: Whether project actually needs web menus, or if web version uses different UI pattern (toolbar, context menus)
   - Recommendation: Defer Menu trait until Phase 11 (Desktop Integration) clarifies desktop-only vs cross-platform menu needs

3. **IndexedDB performance for large library data**
   - What we know: indxdb provides trait abstraction, IndexedDB has async API
   - What's unclear: Performance characteristics for 10k+ component library; whether to cache in memory
   - Recommendation: Prototype with indxdb in Phase 10 (Library Management), add caching layer if benchmarks show need

4. **WASM threading support**
   - What we know: WASM threads experimental in 2026, not widely supported
   - What's unclear: Whether to design for future threading or assume single-threaded WASM
   - Recommendation: Assume single-threaded for now (#[async_trait(?Send)] on WASM), can add Send bound later if threads stabilize

## Sources

### Primary (HIGH confidence)
- [Conditional Compilation - Rust Reference](https://doc.rust-lang.org/reference/conditional-compilation.html) - Official cfg attributes
- [Features - The Cargo Book](https://doc.rust-lang.org/cargo/reference/features.html) - Official Cargo features documentation
- [Adding WASM Support - Rust and WebAssembly Book](https://rustwasm.github.io/book/reference/add-wasm-support-to-crate.html) - Official WASM guidance
- [rfd crate documentation](https://docs.rs/rfd) - File dialog API reference
- [cfg_aliases documentation](https://docs.rs/cfg_aliases) - Build script alias setup
- [File System Access API - Chrome Developers](https://developer.chrome.com/docs/capabilities/web-apis/file-system-access) - Browser API official docs

### Secondary (MEDIUM confidence)
- [WebAssembly as a Platform for Abstraction](https://adventures.michaelfbryan.com/posts/wasm-as-a-platform-for-abstraction/) - Architecture patterns (2019 but still relevant)
- [Tauri v2 Plugin Dialog](https://v2.tauri.app/plugin/dialog/) - Tauri dialog patterns
- [Conditional Compilation in Rust - Wasmtime Docs](https://docs.wasmtime.dev/contributing-conditional-compilation.html) - Best practices from major WASM project
- [rfd GitHub repository](https://github.com/PolyMeilex/rfd) - Implementation examples
- [indxdb GitHub repository](https://github.com/surrealdb/indxdb) - IndexedDB abstraction by SurrealDB

### Tertiary (LOW confidence)
- Web search results on SQLite WASM compatibility - Community discussions, no single authoritative source
- rusqlite WASM support GitHub issues - Clarifies what's NOT possible, but implementation details speculative
- File System Access API browser support discussions - Marked for validation; Firefox/Safari support uncertain

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - rfd, wasm-bindgen, cfg patterns well-documented in official sources
- Architecture: HIGH - Rust Reference and Cargo Book provide authoritative guidance on conditional compilation
- Pitfalls: MEDIUM - Mix of official docs (feature unification) and community wisdom (menu abstraction timing)
- Storage abstraction: MEDIUM - Need to verify indxdb performance, but pattern is sound
- File System Access API: MEDIUM - API itself documented, but browser support and web-sys stability need validation

**Research date:** 2026-01-29
**Valid until:** 2026-02-28 (30 days) - WASM ecosystem evolving but core patterns stable
