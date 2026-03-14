---
estimated_steps: 7
estimated_files: 5
---

# T01: Build core .kicad_pcb S-expression parser

**Slice:** S01 — KiCad PCB Parser & Benchmark Fixtures
**Milestone:** M004

## Description

Create a custom `.kicad_pcb` parser in `cypcb-kicad` that handles KiCad 7/8 format (which uses `footprint` keyword instead of `module`). The parser uses `symbolic_expressions` crate directly for S-expression tokenization, then walks the tree to extract board outline, footprints with pads, nets, traces, and vias. Returns a `KicadPcbParseResult` containing a populated `BoardWorld`, `FootprintLibrary`, optional reference `RoutingResult`, and parse metadata.

## Steps

1. **Add dependencies** to `crates/cypcb-kicad/Cargo.toml`: `symbolic_expressions = "0.4"` and `cypcb-router = { path = "../cypcb-router" }` (for `RoutingResult`, `RouteSegment`, `ViaPlacement` types).

2. **Create `crates/cypcb-kicad/src/pcb_parser.rs`** with:
   - `KicadPcbError` enum (thiserror) — `IoError`, `SexprParseError`, `MissingField`, `UnsupportedVersion`, `InvalidData`
   - `KicadPcbMetadata` struct — version, component_count, net_count, trace_segment_count, via_count, board_size_mm, layer_count
   - `KicadPcbParseResult` struct — world, library, reference_routes, metadata
   - `KicadBenchmark` struct and `BenchmarkComplexity` enum (used by T02 but defined here)
   - `parse_kicad_pcb(path: &Path) -> Result<KicadPcbParseResult, KicadPcbError>` — reads file, calls `parse_kicad_pcb_str()`
   - `parse_kicad_pcb_str(content: &str) -> Result<KicadPcbParseResult, KicadPcbError>` — the core parser:
     - Call `symbolic_expressions::parser::parse_str()` to get `Sexp` tree
     - Extract `(version N)` — validate KiCad 6/7/8 range
     - Extract `(net N "name")` entries → `BoardWorld::intern_net()`, skip net 0 ("")
     - Extract `(layers ...)` → count copper layers
     - Extract board outline: `(gr_line ... (layer "Edge.Cuts"))` or `(gr_rect ... (layer "Edge.Cuts"))` → bounding box → `BoardWorld::set_board()`
     - Extract footprints: `(footprint "lib:name" ... (at X Y angle) (fp_text reference "R1") (fp_text value "10k") (pad ...))` — for each:
       - Parse position/rotation, convert mm→nm
       - Parse each `(pad N type shape (at x y) (size w h) (drill d) (layers ...) (net N "name"))` → `PadDef` for library + `PinConnection` for net mapping
       - Register footprint geometry in `FootprintLibrary` (once per unique library link)
       - Call `BoardWorld::spawn_component()` with refdes, value, position, rotation, footprint ref, net connections
     - Also handle `(module ...)` keyword for KiCad 5/6 backward compat (same parsing logic)
     - Extract segments: `(segment (start X Y) (end X Y) (width W) (layer L) (net N))` → `RouteSegment`
     - Extract vias: `(via (at X Y) (size S) (drill D) (layers L1 L2) (net N))` → `ViaPlacement`
     - Build metadata from counts
   - Helper functions: `find_child()`, `get_float()`, `get_string()`, `parse_layer_name()` for walking S-expr tree

3. **Implement layer name parsing** — KiCad uses string layer names ("F.Cu", "B.Cu", "Edge.Cuts", "F.SilkS", etc.). Write `parse_layer_name(name: &str) -> Option<Layer>` mapping these to internal `Layer` enum. Handle `*.Cu` wildcard for through-hole pads (map to both TopCopper + BottomCopper).

4. **Update `crates/cypcb-kicad/src/lib.rs`** — add `pub mod pcb_parser;` and re-export key types: `parse_kicad_pcb`, `parse_kicad_pcb_str`, `KicadPcbError`, `KicadPcbParseResult`, `KicadPcbMetadata`, `KicadBenchmark`, `BenchmarkComplexity`.

5. **Create synthetic test fixture** `crates/cypcb-kicad/tests/fixtures/minimal.kicad_pcb` — a hand-written KiCad 8 format file containing:
   - `(version 20240108)` (KiCad 8)
   - 2 nets: `(net 0 "")`, `(net 1 "VCC")`, `(net 2 "GND")`
   - Board outline via `(gr_rect (start 0 0) (end 30 20) (layer "Edge.Cuts"))`
   - 2 footprints: one 2-pad resistor (SMD), one 2-pad LED (through-hole)
   - Pads with net assignments
   - 1 segment and 1 via for reference route testing

6. **Write unit tests** in `crates/cypcb-kicad/tests/pcb_parser_tests.rs`:
   - `test_parse_minimal_fixture` — parse the synthetic file, assert:
     - metadata.component_count == 2
     - metadata.net_count == 2 (excluding net 0)
     - metadata.version == 20240108
     - board size is 30mm × 20mm (in nm)
   - `test_component_positions` — verify footprint positions match expected mm→nm conversion
   - `test_pad_net_assignments` — verify pads map to correct nets
   - `test_reference_routes_extracted` — verify 1 segment + 1 via in reference_routes
   - `test_footprint_library_registered` — verify both footprints registered in library with correct pad count
   - `test_net_zero_skipped` — verify net 0 ("") is not interned as a real net

7. **Verify compilation and tests**: `cargo test -p cypcb-kicad pcb_parser` passes, `cargo check -p cypcb-kicad` clean.

## Must-Haves

- [ ] `parse_kicad_pcb_str()` handles both `footprint` (KiCad 7/8) and `module` (KiCad 5/6) keywords
- [ ] Board outline extracted from Edge.Cuts layer elements → BoardSize in nm
- [ ] All footprint pads extracted with correct shape, size, position, layers, net
- [ ] Footprint pad geometry registered in FootprintLibrary by library link name
- [ ] Net 0 ("") not interned; all other nets interned via `BoardWorld::intern_net()`
- [ ] Dimensions convert mm → nm via `Nm::from_mm()`
- [ ] Trace segments and vias extracted into `RoutingResult`
- [ ] KiCad net numbers mapped to internal NetId for segment/via net association

## Verification

- `cargo test -p cypcb-kicad pcb_parser` — all 6 unit tests pass
- `cargo check -p cypcb-kicad` — no compile errors or warnings
- `cargo clippy -p cypcb-kicad -- -D warnings` — clean (excluding known allows)

## Inputs

- `crates/cypcb-kicad/src/footprint.rs` — existing `.kicad_mod` parser pattern (convert_module, convert_pad, convert_layers). Follow similar naming and error conventions.
- `crates/cypcb-world/src/world.rs` — `BoardWorld` API: `set_board()`, `spawn_component()`, `intern_net()`
- `crates/cypcb-world/src/footprint/library.rs` — `FootprintLibrary::register()` for pad geometry
- `crates/cypcb-router/src/types.rs` — `RouteSegment`, `ViaPlacement`, `RoutingResult` types
- S01-RESEARCH.md — architecture sketch, data flow, pitfalls, constraints

## Expected Output

- `crates/cypcb-kicad/src/pcb_parser.rs` — complete parser module (~300-450 lines)
- `crates/cypcb-kicad/src/lib.rs` — updated with module declaration and re-exports
- `crates/cypcb-kicad/Cargo.toml` — updated with new dependencies
- `crates/cypcb-kicad/tests/fixtures/minimal.kicad_pcb` — synthetic test fixture
- `crates/cypcb-kicad/tests/pcb_parser_tests.rs` — 6 unit tests all passing

## Observability Impact

- **New structured error type:** `KicadPcbError` with variants `IoError`, `SexprParseError`, `MissingField`, `UnsupportedVersion`, `InvalidData` — each carries context (field name, version, details) for diagnosable failures without debugger.
- **Parse metadata as inspection surface:** `KicadPcbMetadata` returned from every parse contains version, component_count, net_count, trace_segment_count, via_count, board_size_mm, layer_count. Future agents inspect by calling `parse_kicad_pcb()` and reading metadata fields.
- **Failure visibility:** Unsupported KiCad versions return `UnsupportedVersion` with the actual version number. Missing fields return `MissingField` naming the parent element. Invalid data returns `InvalidData` with description. All are `Display`-formatted for logs.
- **Test-visible error paths:** Unit tests assert `SexprParseError` for empty input and `UnsupportedVersion` for out-of-range versions.
