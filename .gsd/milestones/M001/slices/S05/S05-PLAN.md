# S05: Intelligence

**Goal:** Add Trace and Via ECS components to the board model, plus DSL extensions for net electrical constraints, manual traces, and trace locking.
**Demo:** Add Trace and Via ECS components to the board model, plus DSL extensions for net electrical constraints, manual traces, and trace locking.

## Must-Haves


## Tasks

- [x] **T01: Trace/Via ECS & Net Constraints**
  - Add Trace and Via ECS components to the board model, plus DSL extensions for net electrical constraints, manual traces, and trace locking.

Purpose: The autorouter (Plan 05-06) produces traces that must be stored in the ECS. Additionally, users need to define electrical constraints (current, width, clearance) on nets for the router to respect.
Output: Trace/Via components, extended grammar with net constraints and manual trace syntax
- [x] **T02: IPC-2221 Trace Width Calculator**
  - Create the cypcb-calc crate implementing IPC-2221 trace width calculation from current requirements.

Purpose: Users need trace width suggestions based on current carrying capacity. This calculator is used by both the LSP (for hover hints) and the router (for automatic width selection). INT-02 requirement.
Output: Working IPC-2221 calculator with proper limits and warnings
- [x] **T03: KiCad Footprint Import**
  - Create the cypcb-kicad crate to import KiCad .kicad_mod footprint files using the kicad_parse_gen library.

Purpose: Enable users to use KiCad's extensive footprint libraries directly in their designs. FTP-05 requirement. This allows full KiCad compatibility - if it exports to Gerber from KiCad, it should work here.
Output: Working KiCad footprint import with library scanning
- [x] **T04: FreeRouting DSN Export** `est:6 minutes`
  - Create the cypcb-router crate and implement Specctra DSN export for FreeRouting autorouter integration.

Purpose: FreeRouting requires DSN format input. This plan creates the export path; Plan 05-06 handles the import and CLI integration. INT-01 requirement (first half).
Output: Working DSN export that FreeRouting can read
- [x] **T05: LSP Server Setup** `est:45 minutes`
  - Create the cypcb-lsp crate with basic LSP functionality: hover, diagnostics from DRC, and document synchronization.

Purpose: IDE integration provides autocomplete, hover info, and real-time error feedback. This plan implements the core LSP infrastructure and two key features (hover, diagnostics). DEV-02 requirement (first half).
Output: Working LSP server with hover and DRC diagnostics
- [x] **T06: FreeRouting SES Import & CLI**
  - Implement SES import and FreeRouting CLI integration to complete the autorouting pipeline.

Purpose: After DSN export (05-04), we need to import FreeRouting's output and manage the external process. INT-01 requirement (second half).
Output: Complete FreeRouting integration with import and CLI wrapper
- [x] **T07: LSP Completions & Go-to-Definition**
  - Add autocomplete and go-to-definition to the LSP server for a complete IDE experience.

Purpose: Autocomplete reduces typing and prevents errors. Go-to-definition enables quick navigation. DEV-02 requirement (second half).
Output: Working completions and navigation in IDE
- [x] **T08: Trace & Ratsnest Rendering**
  - Add trace and ratsnest rendering to the viewer so routing results are visible.

Purpose: Users need to see traces after autorouting. Ratsnest shows what still needs routing. Per CONTEXT.md: "Full trace rendering: actual width, copper layer colors, vias visible" and "Ratsnest: toggle option in layer controls".
Output: Traces and vias visible in viewer, ratsnest toggleable
- [x] **T09: Autorouter UI Integration**
  - Integrate autorouting into CLI and viewer hot-reload workflow.

Purpose: Per CONTEXT.md: "Triggered on file save (same as DRC) - seamless hot-reload workflow" and "Progress indicator required". Users save file and see it routed automatically.
Output: CLI route command and automatic routing in viewer
- [x] **T10: Visual Verification**
  - Verify Phase 5 Intelligence features work end-to-end through human testing.

Purpose: Confirm autorouting, LSP integration, trace rendering, and all Phase 5 requirements are functional before marking complete.
Output: Verified working Phase 5 or identified issues for gap closure
- [x] **T11: DSL Syntax Documentation** `est:100s`
  - Document the correct net constraint syntax to close UAT gap

Purpose: Users reported "Syntax error: unexpected token: 'current 500mA'" because they placed constraints inside net braces instead of in square brackets before braces. The grammar is working as designed - this is a documentation gap, not a code bug.

Output: DSL syntax reference doc + updated example files demonstrating correct usage

## Files Likely Touched

- `crates/cypcb-world/src/components/trace.rs`
- `crates/cypcb-world/src/components/mod.rs`
- `crates/cypcb-world/src/lib.rs`
- `crates/cypcb-parser/grammar/grammar.js`
- `crates/cypcb-parser/src/ast.rs`
- `crates/cypcb-parser/src/parser.rs`
- `crates/cypcb-calc/Cargo.toml`
- `crates/cypcb-calc/src/lib.rs`
- `crates/cypcb-calc/src/trace_width.rs`
- `Cargo.toml`
- `crates/cypcb-kicad/Cargo.toml`
- `crates/cypcb-kicad/src/lib.rs`
- `crates/cypcb-kicad/src/footprint.rs`
- `crates/cypcb-kicad/src/library.rs`
- `Cargo.toml`
- `crates/cypcb-router/Cargo.toml`
- `crates/cypcb-router/src/lib.rs`
- `crates/cypcb-router/src/dsn.rs`
- `crates/cypcb-router/src/types.rs`
- `Cargo.toml`
- `crates/cypcb-lsp/Cargo.toml`
- `crates/cypcb-lsp/src/lib.rs`
- `crates/cypcb-lsp/src/main.rs`
- `crates/cypcb-lsp/src/backend.rs`
- `crates/cypcb-lsp/src/document.rs`
- `crates/cypcb-lsp/src/hover.rs`
- `crates/cypcb-lsp/src/diagnostics.rs`
- `Cargo.toml`
- `crates/cypcb-router/src/ses.rs`
- `crates/cypcb-router/src/freerouting.rs`
- `crates/cypcb-router/src/lib.rs`
- `crates/cypcb-lsp/src/completion.rs`
- `crates/cypcb-lsp/src/goto.rs`
- `crates/cypcb-lsp/src/backend.rs`
- `crates/cypcb-render/src/snapshot.rs`
- `crates/cypcb-render/src/lib.rs`
- `viewer/src/types.ts`
- `viewer/src/renderer.ts`
- `viewer/src/layers.ts`
- `crates/cypcb-cli/src/commands/route.rs`
- `crates/cypcb-cli/src/commands/mod.rs`
- `crates/cypcb-cli/src/main.rs`
- `viewer/src/main.ts`
- `viewer/src/wasm.ts`
- `docs/SYNTAX.md`
- `examples/power-indicator.cypcb`
- `examples/blink.cypcb`
