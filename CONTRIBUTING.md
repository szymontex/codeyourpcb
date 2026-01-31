# Contributing to CodeYourPCB

Thank you for your interest in contributing to CodeYourPCB! This guide will help you set up your development environment and understand the codebase structure.

## Prerequisites

Before you begin, ensure you have the following installed:

### Required

- **Rust** (stable channel, 1.75+)
  - Install from [rustup.rs](https://rustup.rs/)
  - Add WASM target: `rustup target add wasm32-unknown-unknown`

- **Node.js** 18+ and npm
  - Download from [nodejs.org](https://nodejs.org/)

- **wasm-pack** for building WASM modules
  - Install: `cargo install wasm-pack`

### Optional (for Desktop Development)

- **Tauri Prerequisites** (platform-specific system libraries)
  - **Linux**: `sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev pkg-config`
  - **macOS**: Xcode Command Line Tools
  - **Windows**: WebView2 Runtime (usually pre-installed)
  - See [Tauri Prerequisites Guide](https://tauri.app/v2/guides/prerequisites/) for detailed instructions

## Quick Start

### 1. Clone the Repository

```bash
git clone https://github.com/codeyourpcb/codeyourpcb.git
cd codeyourpcb
```

### 2. Verify Rust Compilation

Build all Rust crates to verify your toolchain is working:

```bash
cargo build
```

This compiles the entire workspace (14 crates). The first build will take several minutes as dependencies are compiled.

### 3. Set Up Frontend

Install Node.js dependencies for the web viewer:

```bash
cd viewer
npm install
```

### 4. Build WASM Module

Build the Rust render engine as a WASM module:

```bash
cd viewer
./build-wasm.sh
```

This runs `wasm-pack build` and outputs to `viewer/pkg/`.

Alternatively, run manually:
```bash
wasm-pack build crates/cypcb-render --target web --out-dir ../../viewer/pkg
```

### 5. Start Development Server

Launch the Vite development server:

```bash
cd viewer
npm run dev
```

Visit http://localhost:5173 to see the PCB viewer.

The dev server supports hot reload:
- TypeScript changes rebuild automatically
- Rust changes require re-running `./build-wasm.sh` and refreshing the browser

## Building for Production

### Web Build

Create an optimized production build for static hosting:

```bash
cd viewer
npm run build:web
```

Output will be in `viewer/dist/`.

### Desktop Build

Build the native desktop application (requires Tauri prerequisites):

```bash
cd viewer
npm run build:desktop
```

This creates platform-specific installers in `src-tauri/target/release/bundle/`.

## Running Tests

### Rust Tests

Run all Rust unit and integration tests:

```bash
cargo test
```

Run tests for a specific crate:

```bash
cargo test -p cypcb-parser
```

Run tests with output visible:

```bash
cargo test -- --nocapture
```

### Test Organization

- **Unit tests**: Located in `#[cfg(test)] mod tests` blocks within source files
- **Integration tests**: Located in `crates/*/tests/` directories
- **Example files**: Located in `examples/*.cypcb` (used for manual testing)

## Desktop Development

To work on the Tauri desktop application:

### 1. Install System Dependencies

See "Optional" section in Prerequisites above for platform-specific dependencies.

### 2. Start Desktop Dev Mode

```bash
cd viewer
npm run dev:desktop
```

This launches the Tauri development window with hot reload enabled.

Changes to `src-tauri/src/` (Rust) rebuild automatically. Changes to `viewer/src/` (TypeScript) rebuild automatically.

### 3. Desktop-Specific Features

The desktop build includes:
- Native file dialogs
- Native menu bar
- File associations (.cypcb files)
- SQLite-based library storage
- Keyboard shortcuts (Ctrl+O, Ctrl+S, etc.)

## Code Style

### Rust

- **Formatting**: Run `cargo fmt` before committing
- **Linting**: Run `cargo clippy` and fix warnings
- **Naming**: Follow Rust conventions (snake_case for functions/variables, PascalCase for types)
- **Error handling**: Use `Result<T, E>` with `thiserror` for custom error types
- **Documentation**: Add doc comments (`///`) for public APIs

### TypeScript

- **Formatting**: Follow project conventions (2-space indentation)
- **Types**: Prefer explicit types over `any`
- **Naming**: camelCase for functions/variables, PascalCase for classes/types
- **Modules**: Use ES6 modules (`import`/`export`)

### Commit Messages

Use conventional commit format:

```
type(scope): description

- Detailed change 1
- Detailed change 2
```

**Types:** `feat`, `fix`, `docs`, `refactor`, `test`, `chore`, `perf`

**Scopes:** Use phase-plan format for v1.1+ work (e.g., `15-03`) or crate name (e.g., `parser`)

**Example:**
```
feat(library): add FTS5 full-text search for components

- Implement BM25 ranking for relevance scoring
- Add search filters for manufacturer, package type
- Create component_search_fts5 table with automatic sync
```

## Project Structure

```
codeyourpcb/
├── crates/              # Rust workspace crates
│   ├── cypcb-calc/      # Electrical calculations (IPC-2221, impedance)
│   ├── cypcb-cli/       # Command-line interface (cypcb binary)
│   ├── cypcb-core/      # Core types (units, geometry, coordinates)
│   ├── cypcb-drc/       # Design rule checking engine
│   ├── cypcb-export/    # Manufacturing export (Gerber, drill files)
│   ├── cypcb-kicad/     # KiCad library import (.kicad_mod parser)
│   ├── cypcb-library/   # Component library management (SQLite, FTS5)
│   ├── cypcb-lsp/       # Language Server Protocol (diagnostics, hover)
│   ├── cypcb-parser/    # Tree-sitter grammar and AST
│   ├── cypcb-platform/  # Platform abstraction (FileSystem, Dialog, Storage)
│   ├── cypcb-render/    # WebGL rendering and WASM entry point
│   ├── cypcb-router/    # FreeRouting integration (DSN/SES formats)
│   ├── cypcb-watcher/   # File watching for hot reload
│   └── cypcb-world/     # ECS board model (components, nets, zones)
├── viewer/              # TypeScript/Vite web frontend
│   ├── src/             # TypeScript source (main.ts, theme, editor)
│   ├── pkg/             # WASM build output (generated)
│   └── package.json     # Node.js dependencies
├── src-tauri/           # Tauri desktop application
│   ├── src/             # Rust desktop integration code
│   ├── icons/           # Application icons
│   └── tauri.conf.json  # Tauri configuration
├── examples/            # Example .cypcb files for testing
├── docs/                # Documentation
└── Cargo.toml           # Rust workspace configuration
```

For detailed architecture and crate relationships, see [docs/architecture.md](docs/architecture.md).

## Development Tips

### Incremental Builds

For faster iteration on WASM changes:

```bash
# In one terminal: watch for Rust changes and rebuild WASM
cd viewer
./build-wasm.sh && npm run dev
```

### Debugging

- **Rust**: Use `dbg!()` macro or `tracing` logs
- **WASM**: Use `console.log()` via `web_sys::console::log_1()`
- **Browser DevTools**: Check console for WASM panics and JS errors
- **LSP**: Use VS Code with rust-analyzer extension

### Common Issues

**Error: "wasm32-unknown-unknown not installed"**
- Solution: `rustup target add wasm32-unknown-unknown`

**Error: "pkg-config not found" (Linux)**
- Solution: `sudo apt install pkg-config libgtk-3-dev` for desktop builds
- Or: Build without desktop features using `cargo build --no-default-features`

**Error: "Cannot find module './pkg'" (TypeScript)**
- Solution: Run `./build-wasm.sh` to generate WASM bindings

**WASM module crashes with "unreachable executed"**
- Solution: Check browser console for panic message, often indicates missing error handling

## Feature Flags

Some crates have optional features:

- **cypcb-library**: `jlcpcb` - Enable JLCPCB API integration (requires API key)
- **cypcb-platform**: `native-dialogs` - Enable native file dialogs (requires system libraries)
- **cypcb-lsp**: `server` - Build LSP server binary (disabled by default in dev due to proc-macro issues)
- **cypcb-render**: `native` vs `wasm` - Build target (native includes tree-sitter parser)

Build with specific features:

```bash
cargo build --features "cypcb-library/jlcpcb,cypcb-platform/native-dialogs"
```

## Getting Help

- **Documentation**: See [docs/](docs/) directory
- **Issues**: Check existing [GitHub issues](https://github.com/codeyourpcb/codeyourpcb/issues)
- **Discussions**: Ask questions in [GitHub Discussions](https://github.com/codeyourpcb/codeyourpcb/discussions)

## License

By contributing, you agree that your contributions will be licensed under the same MIT OR Apache-2.0 dual license as the project.
