# Stack Research

**Domain:** Code-first PCB Design Tool (EDA)
**Researched:** 2026-01-21
**Confidence:** HIGH

## Recommended Stack

### Core Technologies

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| Rust | 1.84+ | Core language | Memory safe, compiles to WASM, 30+ year longevity, used by Mozilla/Google/Microsoft |
| WebAssembly | 2.0 | Browser/portable runtime | W3C standard, near-native performance (8-10x faster than JS for compute), universal browser support |
| Tauri | 2.0 | Desktop shell | 50% less RAM than Electron (~30MB vs ~200MB), <10MB bundle, Rust backend integration |
| Tree-sitter | 0.25 | DSL parser | Incremental parsing, error-tolerant, used by GitHub/Neovim/Zed, Rust native |
| wgpu | 24.0 | 2D/GPU rendering | WebGPU standard, cross-platform (Vulkan/Metal/DX12/WebGL), compute shaders for routing |
| Three.js | r170+ | 3D preview | Lightweight (168kB), massive ecosystem, WebGPU support coming |

### Supporting Libraries

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| nalgebra | 0.33 | Linear algebra | Geometry calculations, transforms, matrix operations |
| glam | 0.29 | Fast 3D math | Hot rendering paths, SIMD-optimized |
| rstar | 0.12 | R*-tree spatial index | DRC collision detection, selection/picking |
| bevy_ecs | 0.15 | Entity Component System | Board data model, parallel queries |
| serde | 1.0 | Serialization | JSON, MessagePack, bincode support |
| tower-lsp | 0.20 | LSP framework | IDE integration (hover, completion, diagnostics) |
| thiserror | 2.0 | Library errors | Structured error types for parser/DRC |
| anyhow | 1.0 | Application errors | Error context and propagation |
| proptest | 1.5 | Property testing | Fuzzing parser, testing geometry algorithms |
| notify | 7.0 | File watching | Hot reload on .pcb file changes |
| gerber-types | latest | Gerber format | Export to manufacturing format |

### Development Tools

| Tool | Purpose | Notes |
|------|---------|-------|
| trunk | WASM bundler | `trunk serve` for dev, `trunk build --release` for production |
| wasm-pack | WASM packaging | Alternative to trunk, produces npm packages |
| cargo-watch | Auto-rebuild | `cargo watch -x check -x test` during development |
| criterion | Benchmarking | Performance regression testing for parser/DRC |
| insta | Snapshot testing | Gerber output stability, AST snapshots |

## Installation

```toml
# Cargo.toml

[package]
name = "cypcb"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
# Core
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Math & Geometry
nalgebra = "0.33"
glam = "0.29"
rstar = "0.12"

# ECS
bevy_ecs = "0.15"

# Parsing
tree-sitter = "0.25"

# Error handling
thiserror = "2.0"
anyhow = "1.0"

# Serialization
bincode = "1.3"
rmp-serde = "1.3"

# File watching
notify = "7.0"
notify-debouncer-full = "0.4"

# LSP
tower-lsp = "0.20"
lsp-types = "0.97"

# Async
tokio = { version = "1", features = ["full"] }

# Logging
tracing = "0.1"
tracing-subscriber = "0.3"

# PCB Export
gerber-types = "0.1"

[target.'cfg(target_arch = "wasm32")'.dependencies]
wasm-bindgen = "0.2"
web-sys = { version = "0.3", features = [
    "CanvasRenderingContext2d",
    "HtmlCanvasElement",
    "Window",
    "Document",
] }
serde-wasm-bindgen = "0.6"
console_error_panic_hook = "0.1"

[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
tauri = "2.0"

[dev-dependencies]
proptest = "1.5"
criterion = "0.5"
insta = { version = "1.40", features = ["json"] }

[profile.release]
lto = true
opt-level = 3

[profile.release-wasm]
inherits = "release"
opt-level = "s"  # Optimize for size
```

## Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| Rust | C++ | Existing C++ codebase, need Altium/KiCad plugin integration |
| Tauri | Electron | Need Chrome DevTools debugging, complex native node modules |
| Tree-sitter | LALRPOP | Simpler grammar, don't need incremental parsing |
| wgpu | Canvas 2D | Very simple renders, don't need compute shaders |
| bevy_ecs | specs/hecs | Lighter weight, don't need Bevy's full ecosystem |
| nalgebra | cgmath | Legacy code compatibility (cgmath unmaintained) |
| bincode | MessagePack | Need human-readable cache files, cross-language interop |

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| Electron | 200MB+ RAM, 80MB+ bundle, slow startup | Tauri 2.0 |
| cgmath | Unmaintained since 2021 | nalgebra or glam |
| nom (for DSL) | Not incremental, poor error recovery | Tree-sitter |
| OpenGL | Legacy, no compute shaders, poor WASM support | wgpu (WebGPU) |
| Custom autorouter (MVP) | Months of work for inferior results | FreeRouting (proven) |
| SQLite for board storage | Overkill, poor diff/merge | Custom binary + JSON |
| XML for file format | Verbose, poor human readability | Custom DSL |
| Floating-point coordinates | Precision issues, non-determinism | Integer nanometers (like KiCad) |

## Stack Patterns by Variant

**If targeting web-only:**
- Skip Tauri entirely
- Use trunk for WASM bundling
- Consider Yew or Leptos for UI framework

**If targeting desktop-only:**
- Can use native file dialogs via Tauri
- Consider egui for immediate-mode UI
- Can use native threads instead of web workers

**If needing 3D CAD integration:**
- Consider three-d crate for native 3D
- Or OpenCASCADE bindings via opencascade-sys
- STEP file export becomes important

## Version Compatibility

| Package A | Compatible With | Notes |
|-----------|-----------------|-------|
| bevy_ecs 0.15 | Rust 1.84+ | MSRV increased in 0.15 |
| wgpu 24.0 | naga 24.0 | Must match versions |
| tree-sitter 0.25 | tree-sitter-cli 0.25 | Grammar and runtime must match |
| Tauri 2.0 | @tauri-apps/api 2.0 | Frontend/backend versions must match |
| tower-lsp 0.20 | lsp-types 0.97 | Check compatibility on upgrade |

## Sources

- Brainstorm research session (extensive benchmarks gathered)
- [WebAssembly 3.0 Rust vs C++ Benchmarks](https://markaicode.com/webassembly-3-performance-rust-cpp-benchmarks-2025/)
- [Tauri vs Electron 2025](https://codeology.co.nz/articles/tauri-vs-electron-2025-desktop-development.html)
- [wgpu Documentation](https://wgpu.rs/)
- [Tree-sitter GitHub](https://github.com/tree-sitter/tree-sitter)
- [KiCad Developer Docs](https://dev-docs.kicad.org/)

---
*Stack research for: Code-first PCB Design Tool*
*Researched: 2026-01-21*
