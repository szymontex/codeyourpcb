# Phase 15: Documentation & Polish - Research

**Researched:** 2026-01-31
**Domain:** Technical documentation for a Rust+WASM PCB design tool
**Confidence:** HIGH

## Summary

This phase creates user-facing documentation for CodeYourPCB. The project already has significant internal documentation: `docs/SYNTAX.md` (419 lines, comprehensive DSL reference), `DESKTOP-SETUP.md` (setup scripts), 11 example `.cypcb` files, and extensive `.planning/` artifacts describing architecture decisions.

The work is primarily content authoring, not technical implementation. The main challenge is organizing existing knowledge into user-consumable guides and ensuring desktop vs web differences are clearly documented.

**Primary recommendation:** Build documentation as Markdown files in a `docs/` directory (already exists with SYNTAX.md). No static site generator needed for v1 -- plain Markdown is git-friendly and matches the project's code-first philosophy.

## Standard Stack

### Core
| Tool | Purpose | Why Standard |
|------|---------|--------------|
| Markdown files in `docs/` | All documentation | Git-friendly, already started, zero build step |
| Existing `docs/SYNTAX.md` | DSL reference (already 419 lines) | Expand, don't rewrite |
| `examples/*.cypcb` | Annotated example projects | 11 files already exist |

### Supporting
| Tool | Purpose | When to Use |
|------|---------|-------------|
| Mermaid diagrams | Architecture diagrams in Markdown | For CONTRIBUTING.md crate dependency graph |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Plain Markdown | mdBook / VitePress | Adds build complexity; overkill for initial docs |
| Inline examples | Separate tutorial repo | Fragmentation; keep examples co-located |

## Architecture Patterns

### Recommended Documentation Structure
```
docs/
├── SYNTAX.md              # Already exists - DSL reference
├── user-guide/
│   ├── getting-started.md # DOC-01: Creating .cypcb files
│   ├── library-management.md # DOC-02: Import, organize, search
│   ├── platform-differences.md # DOC-03: Desktop vs web
│   ├── project-structure.md # DOC-04: File organization
│   └── examples.md        # DOC-05: Walkthrough of example projects
├── api/
│   ├── lsp-server.md      # DOC-06: LSP usage
│   └── library-format.md  # DOC-07: Library file formats
├── CONTRIBUTING.md         # DOC-08 + DOC-09: Dev setup + architecture
examples/
├── blink.cypcb            # Already exists - add more comments
├── power-indicator.cypcb  # Already exists
├── simple-psu.cypcb       # Already exists
└── ...
```

### Pattern: Progressive Disclosure
**What:** Start each doc with the simplest case, then layer complexity
**When to use:** Every user-facing guide
**Example:** Getting started shows minimal board -> adds components -> adds nets -> adds constraints

### Anti-Patterns to Avoid
- **Documenting internals in user guides:** Users don't need to know about ECS or AST sync
- **Duplicating SYNTAX.md content:** Link to it, don't copy
- **Undocumented platform gaps:** Every desktop-only feature must be explicitly noted

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Documentation site | Custom SSG pipeline | Plain Markdown in `docs/` | Project is pre-1.0, keep simple |
| API docs from code | Manual API docs | `cargo doc` for Rust API | Already works, auto-generated |
| Diagram rendering | Custom SVG | Mermaid in Markdown | GitHub renders natively |

## Common Pitfalls

### Pitfall 1: Stale Examples
**What goes wrong:** Example files don't parse with current grammar
**How to avoid:** Every example file should be validated by `cypcb check` as part of docs verification

### Pitfall 2: Undocumented Platform Differences
**What goes wrong:** Users try desktop features on web, get confused
**How to avoid:** Create explicit feature matrix table (desktop vs web) and reference it from each guide

### Pitfall 3: Missing Workflow Documentation
**What goes wrong:** Syntax is documented but end-to-end workflows aren't
**How to avoid:** Each guide should cover a complete workflow (create -> edit -> check -> route -> export)

## Code Examples

### Existing Assets to Leverage

**docs/SYNTAX.md** - Already comprehensive (version, board, component, net, zone, trace, footprint, comments, units, common mistakes). Needs: library import syntax, module syntax if any.

**Example files available:**
- `blink.cypcb` - Simple LED circuit (good starter)
- `power-indicator.cypcb` - Net constraints demo
- `routing-test.cypcb` - Manual trace definitions
- `drc-test.cypcb` - DRC rule demonstrations
- `simple-psu.cypcb` - Power supply (more complex)

### Platform Differences to Document

Based on crate structure (`cypcb-platform/src/`):
| Feature | Desktop (Tauri) | Web |
|---------|----------------|-----|
| File access | Native filesystem (`fs_native.rs`) | File System Access API (`fs_web.rs`) |
| Storage | Native storage (`storage_native.rs`) | Browser storage (`storage_web.rs`) |
| Dialogs | Native OS dialogs (`dialog.rs`) | Browser dialogs |
| FreeRouting | Local JAR execution | Not available (or limited) |
| Library import | Direct KiCad path access | Upload-based |
| LSP | WASM bridge (no server) | WASM bridge (no server) |

### Crate Architecture for CONTRIBUTING.md

14 crates to document relationships:
```
cypcb-parser     → Tree-sitter grammar, AST
cypcb-core       → Core types, units
cypcb-world      → ECS board model
cypcb-drc        → Design rule checking
cypcb-export     → Gerber/drill export
cypcb-render     → WebGL rendering
cypcb-lsp        → Language server
cypcb-library    → Library management
cypcb-kicad      → KiCad import
cypcb-router     → FreeRouting integration
cypcb-platform   → Platform abstraction facade
cypcb-calc       → Calculations
cypcb-watcher    → File watching
cypcb-cli        → CLI interface
```

## State of the Art

Not applicable -- this is documentation authoring, not technology selection.

## Open Questions

1. **Should CONTRIBUTING.md live at repo root or in docs/?**
   - Convention: repo root for `CONTRIBUTING.md`
   - Recommendation: Root, with architecture details in `docs/` linked from it

2. **Are there undocumented DSL features?**
   - SYNTAX.md covers core syntax well
   - Need to verify: library import syntax, module/include syntax
   - Recommendation: Check grammar and parser for any syntax not in SYNTAX.md

3. **README.md doesn't exist at repo root**
   - This is a gap -- may want to create one as part of this phase
   - Recommendation: Add to scope if not already planned

## Sources

### Primary (HIGH confidence)
- `/workspace/codeyourpcb/docs/SYNTAX.md` - Existing DSL reference (419 lines)
- `/workspace/codeyourpcb/examples/*.cypcb` - 11 example files
- `/workspace/codeyourpcb/DESKTOP-SETUP.md` - Existing setup guide
- `/workspace/codeyourpcb/crates/` - 14 crate structure examined
- `/workspace/codeyourpcb/.planning/research/FEATURES.md` - Feature inventory

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - plain Markdown, no tooling decisions needed
- Architecture: HIGH - directory structure based on existing patterns
- Pitfalls: HIGH - common documentation issues, well understood

**Research date:** 2026-01-31
**Valid until:** No expiry (documentation patterns are stable)
