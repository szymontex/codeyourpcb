---
phase: 01-foundation
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - Cargo.toml
  - crates/cypcb-core/Cargo.toml
  - crates/cypcb-core/src/lib.rs
  - crates/cypcb-parser/Cargo.toml
  - crates/cypcb-parser/src/lib.rs
  - crates/cypcb-world/Cargo.toml
  - crates/cypcb-world/src/lib.rs
  - crates/cypcb-cli/Cargo.toml
  - crates/cypcb-cli/src/main.rs
  - .gitignore
  - rust-toolchain.toml
autonomous: true

must_haves:
  truths:
    - "cargo build succeeds for workspace"
    - "cargo test runs (even with no tests)"
    - "All four crates compile independently"
  artifacts:
    - path: "Cargo.toml"
      provides: "Workspace manifest with all dependencies"
      contains: "[workspace]"
    - path: "crates/cypcb-core/src/lib.rs"
      provides: "Core crate entry point"
    - path: "crates/cypcb-parser/src/lib.rs"
      provides: "Parser crate entry point"
    - path: "crates/cypcb-world/src/lib.rs"
      provides: "World crate entry point"
    - path: "crates/cypcb-cli/src/main.rs"
      provides: "CLI binary entry point"
  key_links:
    - from: "Cargo.toml"
      to: "crates/*/Cargo.toml"
      via: "workspace.members"
      pattern: 'members = \["crates/\*"\]'
---

<objective>
Initialize the Rust workspace with four crates for the CodeYourPCB foundation.

Purpose: Establish the modular architecture that enables parallel development and clean separation between parsing, board model, and CLI.

Output: Compilable Rust workspace with all dependencies declared and stub entry points.
</objective>

<execution_context>
@~/.claude/get-shit-done/workflows/execute-plan.md
@~/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/ROADMAP.md
@.planning/phases/01-foundation/01-RESEARCH.md
</context>

<tasks>

<task type="auto">
  <name>Task 1: Create workspace manifest and crate structure</name>
  <files>
    Cargo.toml
    crates/cypcb-core/Cargo.toml
    crates/cypcb-parser/Cargo.toml
    crates/cypcb-world/Cargo.toml
    crates/cypcb-cli/Cargo.toml
  </files>
  <action>
Create the workspace root Cargo.toml with:
- Virtual workspace (no root package)
- members = ["crates/*"]
- resolver = "2"
- workspace.dependencies for all shared deps:
  - tree-sitter = "0.25"
  - bevy_ecs = "0.15"
  - rstar = "0.12"
  - thiserror = "2.0"
  - miette = { version = "7.6", features = ["fancy"] }
  - serde = { version = "1.0", features = ["derive"] }
  - serde_json = "1.0"
  - clap = { version = "4.0", features = ["derive"] }
  - tracing = "0.1"

Create each crate's Cargo.toml:
- cypcb-core: minimal deps (serde, thiserror)
- cypcb-parser: tree-sitter, miette, thiserror, depends on cypcb-core
- cypcb-world: bevy_ecs, rstar, depends on cypcb-core
- cypcb-cli: clap, serde_json, miette, depends on parser + world
  </action>
  <verify>
`ls crates/*/Cargo.toml` shows 4 files
`grep "workspace" Cargo.toml` shows workspace config
  </verify>
  <done>All Cargo.toml files exist with correct dependencies</done>
</task>

<task type="auto">
  <name>Task 2: Create stub source files and config</name>
  <files>
    crates/cypcb-core/src/lib.rs
    crates/cypcb-parser/src/lib.rs
    crates/cypcb-world/src/lib.rs
    crates/cypcb-cli/src/main.rs
    .gitignore
    rust-toolchain.toml
  </files>
  <action>
Create minimal lib.rs for each library crate:
```rust
//! CodeYourPCB [crate name]
//!
//! [Brief description]
```

Create main.rs for CLI:
```rust
fn main() {
    println!("cypcb - CodeYourPCB CLI");
}
```

Create .gitignore:
- /target
- Cargo.lock (for libraries, but we keep it for workspace)
- *.swp, *.swo
- .DS_Store
- crates/cypcb-parser/grammar/src/ (generated Tree-sitter files)

Create rust-toolchain.toml:
- channel = "stable"
- This ensures consistent toolchain across environments
  </action>
  <verify>
`cargo build` succeeds
`cargo run -p cypcb-cli` prints "cypcb - CodeYourPCB CLI"
  </verify>
  <done>Workspace compiles and CLI runs</done>
</task>

</tasks>

<verification>
- `cargo build --workspace` completes without errors
- `cargo test --workspace` runs (no tests yet, but no failures)
- `cargo run -p cypcb-cli` executes successfully
- Directory structure matches research recommendations
</verification>

<success_criteria>
1. All four crates compile successfully
2. Dependencies resolve correctly (no version conflicts)
3. Workspace structure follows research recommendations
4. Git ignores appropriate files (target, generated parser)
</success_criteria>

<output>
After completion, create `.planning/phases/01-foundation/01-01-SUMMARY.md`
</output>
