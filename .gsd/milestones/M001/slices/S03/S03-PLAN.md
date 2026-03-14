# S03: Validation

**Goal:** Create the cypcb-drc crate with foundational types for Design Rule Checking.
**Demo:** Create the cypcb-drc crate with foundational types for Design Rule Checking.

## Must-Haves


## Tasks

- [x] **T01: DRC Crate Setup**
  - Create the cypcb-drc crate with foundational types for Design Rule Checking.

Purpose: Establish the DRC infrastructure that all rule implementations will use. This is the foundation for all validation work in Phase 3.

Output: A new cypcb-drc crate with DrcViolation type, ViolationKind enum, and DrcRule trait.
- [x] **T02: IC Footprints**
  - Add QFP/SOIC/SOT footprint families to the footprint library.

Purpose: Fulfill FTP-03 requirement for IC package support. These are essential for real-world PCB designs with microcontrollers and other ICs.

Output: Gull-wing footprint generators for SOIC, SOT, and QFP packages registered in FootprintLibrary.
- [x] **T03: Manufacturer Presets**
  - Implement manufacturer preset structs for JLCPCB and PCBWay design rules.

Purpose: Provide type-safe design rule configurations that DRC rules will check against. Users can select a manufacturer preset to validate their design against real fabrication constraints.

Output: DesignRules struct with constructors for JLCPCB 2-layer, JLCPCB 4-layer, PCBWay standard, and prototype presets.
- [x] **T04: Custom Footprint DSL**
  - Add DSL syntax for custom footprint definitions and wire them to the FootprintLibrary.

Purpose: Fulfill FTP-04 requirement allowing users to define custom footprints inline in their .cypcb files. This enables non-standard packages without external library files.

Output: Grammar rules, AST types, parser support, and sync logic for footprint definitions.
- [x] **T05: Clearance Checking**
  - Implement clearance checking rule using the spatial index.

Purpose: DRC-01 requirement - detect copper features that are too close together for manufacturing. This is the core DRC rule that catches most design errors.

Output: ClearanceRule struct implementing DrcRule trait with two-phase spatial checking.
- [x] **T06: Drill, Trace & Connectivity Rules**
  - Implement drill size, unconnected pin, and trace width DRC rules.

Purpose: DRC-02 (minimum trace width), DRC-03 (minimum drill size) and DRC-04 (unconnected pin detection) requirements. These catch manufacturability issues and incomplete designs.

Output: MinDrillSizeRule, UnconnectedPinRule, and MinTraceWidthRule (placeholder) structs implementing DrcRule trait.
- [x] **T07: DRC Rendering Integration**
  - Integrate DRC into the rendering pipeline so violations are computed on file load.

Purpose: Wire up the DRC engine to run automatically when a board is loaded, making violations available to the renderer. This enables DRC-05 real-time feedback.

Output: PcbEngine runs DRC after load_source(), violations included in BoardSnapshot.
- [x] **T08: Violation Display** `est:~15 minutes`
  - Implement violation display in the viewer with markers and status bar.

Purpose: Visual DRC feedback per user decisions - circle markers at violation locations, VS Code-style status bar with error count, click-to-zoom functionality.

Output: Viewer renders DRC violations as markers with non-invasive error panel.
- [x] **T09: Visual Verification**
  - Visual verification that DRC system works end-to-end.

Purpose: Human verification checkpoint to confirm the DRC system detects violations, displays markers, and provides usable feedback before completing the phase.

Output: Verified working DRC system ready for Phase 4.
- [x] **T10: Zones & Keepouts**
  - Implement zones and keepouts for board model and DRC.

Purpose: Fulfill BRD-05 requirement for zones and keepouts. Keepouts define regions where copper is prohibited, essential for antenna clearance, mechanical clearance, and manufacturing constraints.

Output: Zone AST, Zone ECS component, keepout sync, and KeepoutRule for DRC.

## Files Likely Touched

- `crates/cypcb-drc/Cargo.toml`
- `crates/cypcb-drc/src/lib.rs`
- `crates/cypcb-drc/src/violation.rs`
- `crates/cypcb-drc/src/rules/mod.rs`
- `Cargo.toml`
- `crates/cypcb-world/src/footprint/gullwing.rs`
- `crates/cypcb-world/src/footprint/mod.rs`
- `crates/cypcb-world/src/footprint/library.rs`
- `crates/cypcb-drc/src/presets/mod.rs`
- `crates/cypcb-drc/src/presets/jlcpcb.rs`
- `crates/cypcb-drc/src/presets/pcbway.rs`
- `crates/cypcb-drc/src/lib.rs`
- `crates/cypcb-drc/src/rules/mod.rs`
- `crates/cypcb-parser/grammar/grammar.js`
- `crates/cypcb-parser/src/ast.rs`
- `crates/cypcb-parser/src/parser.rs`
- `crates/cypcb-world/src/sync.rs`
- `crates/cypcb-world/src/footprint/library.rs`
- `crates/cypcb-drc/src/rules/clearance.rs`
- `crates/cypcb-drc/src/rules/mod.rs`
- `crates/cypcb-drc/src/rules/drill_size.rs`
- `crates/cypcb-drc/src/rules/connectivity.rs`
- `crates/cypcb-drc/src/rules/trace_width.rs`
- `crates/cypcb-drc/src/rules/mod.rs`
- `crates/cypcb-drc/src/lib.rs`
- `crates/cypcb-render/src/lib.rs`
- `crates/cypcb-render/src/snapshot.rs`
- `viewer/src/wasm.ts`
- `viewer/src/types.ts`
- `viewer/src/renderer.ts`
- `viewer/src/main.ts`
- `viewer/index.html`
- `viewer/src/layers.ts`
- `crates/cypcb-parser/grammar/grammar.js`
- `crates/cypcb-parser/src/ast.rs`
- `crates/cypcb-parser/src/parser.rs`
- `crates/cypcb-world/src/components/mod.rs`
- `crates/cypcb-world/src/components/zone.rs`
- `crates/cypcb-world/src/sync.rs`
- `crates/cypcb-drc/src/rules/keepout.rs`
- `crates/cypcb-drc/src/rules/mod.rs`
