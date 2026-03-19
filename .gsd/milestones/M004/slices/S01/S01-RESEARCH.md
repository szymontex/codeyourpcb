# S01: KiCad PCB Parser & Benchmark Fixtures — Research

**Date:** 2026-03-14

## Summary

S01 owns **R101** (KiCad .kicad_pcb Board Parser) and **R102** (Benchmark Suite from Real KiCad Projects). The goal is to parse real KiCad `.kicad_pcb` files into `BoardWorld`, produce 3+ benchmark fixtures with metadata, and create a `parse-kicad` CLI command.

**Key finding:** The existing `kicad_parse_gen` v0.7.3 crate already parses `.kicad_pcb` files via its `layout` module. It extracts modules (footprints with pads+nets), net definitions, net classes, graphical elements (gr_line for board outline on Edge.Cuts), and zones. However, it does **NOT** parse `segment` (trace) or `via` elements — those fall into `Element::Other(Sexp)`, preserving the raw S-expression but not structured data. This is actually acceptable for S01's primary use case (strip-and-reroute benchmarking) but segments/vias will need custom parsing from the `Other` variant for reference score comparison (R102/R114).

**Second finding:** KiCad 7/8 changed the element keyword from `module` to `footprint` for component definitions. The `kicad_parse_gen` crate (v0.7.3, 2016-era) only handles `module`. This means **KiCad 7/8 files will fail to parse** unless we either (a) pre-process the file to rename `footprint` → `module`, (b) fork/patch `kicad_parse_gen`, or (c) write our own S-expression parser. Since the roadmap targets KiCad 7+ files, this is a **blocking constraint** that determines architectural approach.

## Recommendation

**Write a custom S-expression parser scoped to our needs** rather than depending on `kicad_parse_gen` for `.kicad_pcb` parsing. Rationale:

1. `kicad_parse_gen` is KiCad 4/5-era and doesn't handle the `footprint` keyword (KiCad 7/8).
2. It silently drops `segment` and `via` elements into `Element::Other(Sexp)` — we need these for reference routing extraction.
3. The `.kicad_pcb` S-expression format is well-documented and we only need a subset: board outline, footprints, pads, nets, segments, vias.
4. Writing a focused parser gives us control over what we extract and clear error messages for unsupported features.

**Continue using `kicad_parse_gen` for `.kicad_mod`** footprint-only imports (existing code in `cypcb-kicad/src/footprint.rs` works fine).

**Parser implementation approach:**
- Use the `symbolic_expressions` crate (already a transitive dependency via `kicad_parse_gen`) for S-expression tokenization.
- Build a `pcb_parser` module in `cypcb-kicad` that walks the parsed S-expression tree and extracts what we need.
- Map to `BoardWorld` using existing spawn/intern APIs.

**Benchmark fixtures:**
- Download 3 open-source KiCad 7/8 projects from GitHub (LED blink, STM32 breakout, multi-IC).
- Store as `.kicad_pcb` files in `tests/fixtures/benchmark/`.
- Create `KicadBenchmark` metadata struct with component count, net count, trace count, reference complexity tier.
- Strip traces for re-routing tests; keep original traces for reference score comparison.

## Don't Hand-Roll

| Problem | Existing Solution | Why Use It |
|---------|------------------|------------|
| S-expression tokenization | `symbolic_expressions` crate (0.3.x, already in dep tree) | Handles tokenization, quoting, nesting — robust and tiny |
| Footprint file (.kicad_mod) import | `cypcb-kicad::footprint::import_footprint_from_str()` | Working, tested, handles pads/layers/courtyard |
| BoardWorld population | `BoardWorld::spawn_component()`, `intern_net()`, `set_board()` | Full API already exists for components, nets, board setup |
| Routing output types | `cypcb-router::types::{RouteSegment, ViaPlacement, RoutingResult}` | Standard output contract used by all routing code |
| Layer mapping | `cypcb-kicad::footprint::convert_single_layer()` | Maps KiCad layer names to internal `Layer` enum |
| Design rules extraction | `kicad_parse_gen::layout::NetClass` + `Setup` | Net class clearances/widths map to `DesignConstraints` |
| Pad rotation transforms | `cypcb-autoroute::orchestrator::rotate_point()` | Handles pad position rotation around component center |

## Existing Code and Patterns

- **`crates/cypcb-kicad/src/footprint.rs`** — Complete `.kicad_mod` parser using `kicad_parse_gen`. Shows the pattern for converting KiCad structures to internal types (`convert_module`, `convert_pad`, `convert_layers`). The new `.kicad_pcb` parser should follow similar naming and structure.

- **`crates/cypcb-kicad/src/lib.rs`** — Module structure with re-exports. New `pcb_parser` module will be added here with `pub mod pcb_parser; pub use pcb_parser::{parse_kicad_pcb, KicadPcbError};`.

- **`crates/cypcb-world/src/world.rs`** — `BoardWorld` API is the target output. Key methods: `set_board(name, size, layers)`, `spawn_component(refdes, value, position, rotation, footprint, nets)`, `intern_net(name)`. The parser produces a `BoardWorld`.

- **`crates/cypcb-autoroute/src/orchestrator.rs`** — `extract_ratsnest()` shows how the autorouter queries `BoardWorld` for component positions, footprints, and net connections. The parsed board must produce identical structures for the router to consume.

- **`crates/cypcb-world/src/components/electrical.rs`** — `NetConnections`, `PinConnection`, `NetId` — these carry pad-to-net mapping. KiCad pads have `(net N "NetName")` which maps directly to this.

- **`crates/cypcb-world/src/footprint/library.rs`** — `FootprintLibrary` and `PadDef`. KiCad footprints embedded in `.kicad_pcb` files contain full pad geometry — we need to register these in the library for the autorouter to find them.

- **`crates/cypcb-world/src/components/trace.rs`** — `Trace`, `TraceSegment`, `Via` components. Reference routing from KiCad files can be stored as these for score comparison.

- **`crates/cypcb-kicad/Cargo.toml`** — Already depends on `cypcb-core`, `cypcb-world`, `kicad_parse_gen`, `thiserror`, `walkdir`.

## Constraints

- **KiCad 7/8 format requires custom parsing** — `kicad_parse_gen` only handles `module` keyword, not `footprint`. KiCad 7+ uses `footprint` exclusively. This is the primary architectural constraint.

- **All dimensions must convert mm → nm** — KiCad uses millimeters (f64), our model uses nanometers (i64 `Nm`). Use `Nm::from_mm()` consistently. Floating-point precision at nm conversion is acceptable (sub-nm error).

- **`BoardWorld` requires `&mut` for queries** — `bevy_ecs` query API needs mutable world reference. Parser should return an owned `BoardWorld`, not try to populate one passed by reference.

- **Footprints must register in `FootprintLibrary`** — The autorouter's `extract_ratsnest()` looks up pad geometry from the library by footprint name. KiCad `.kicad_pcb` files embed full footprint data in each `module`/`footprint` element — these must be extracted and registered.

- **Net ID 0 is the "no-net" net** — KiCad uses `(net 0 "")` for unconnected. Must handle this as a special case (don't intern as a real net).

- **Position Y-axis inversion** — KiCad uses top-left origin with Y increasing downward. Our system may use bottom-left origin. Need to verify coordinate convention and invert Y if necessary during import.

- **Existing `cypcb-kicad` crate is a leaf** — No circular dependencies allowed. The crate can depend on `cypcb-core` and `cypcb-world` but not on `cypcb-autoroute` or `cypcb-router`.

- **WASM compatibility** — `cypcb-kicad` is not a WASM dependency currently (KiCad import is CLI/desktop only). File I/O (`std::fs`) is fine; no need for WASM-safe abstractions.

- **Benchmark files must be deterministic** — Same `.kicad_pcb` input must produce same `BoardWorld` (same entity IDs, same net IDs, same positions). This is guaranteed by deterministic iteration order if we process elements sequentially.

## Common Pitfalls

- **`kicad_parse_gen` `module` vs `footprint` keyword** — The biggest trap. If we use `kicad_parse_gen::layout::parse()` on a KiCad 7+ file, all footprints will be dropped into `Element::Other(Sexp)` silently. Tests would pass with KiCad 5 fixture files but fail on real KiCad 7/8 projects. Avoid by writing our own parser or doing keyword normalization.

- **Pad rotation in absolute coordinates** — KiCad footprint pads have positions relative to the footprint origin. The footprint has an `(at X Y angle)`. Pad absolute position = footprint position + rotated(pad_offset, footprint_angle). The existing footprint parser returns relative positions; the PCB parser must apply the footprint transform.

- **Edge.Cuts parsing for board outline** — Board outline is defined by `gr_line` and `gr_arc` elements on the `Edge.Cuts` layer. These may not form a clean rectangle — they can be arbitrary polygons. For S01, extract the bounding box (axis-aligned rectangle) as the board size, matching existing `BoardSize` semantics.

- **KiCad `*.Cu` wildcard layer** — Through-hole pads use `(layers *.Cu *.Mask)`. The `*` means "all copper layers." The existing layer converter handles this for footprints but the PCB parser must do the same.

- **Net numbering mismatch** — KiCad assigns net numbers sequentially. Our `NetRegistry` interns by name and generates its own IDs. Don't try to preserve KiCad net numbers — intern by name and use our IDs. But keep a KiCad-number → our-NetId mapping for `segment` parsing.

- **Multiple footprints with same base name** — KiCad `.kicad_pcb` files can have multiple instances of the same footprint (e.g., three R_0402). Each instance appears as a separate `footprint` element. The footprint library should register the pad geometry once (by library link name), but each instance spawns a separate component entity with its own position and net connections.

- **KiCad version detection** — The `(version N)` field at the top of the file indicates the format version. KiCad 6 uses version 20211014, KiCad 7 uses 20221018, KiCad 8 uses 20240108. Parse this first to select the right keyword handling.

## Open Risks

- **KiCad 6/7/8 format divergence beyond module/footprint keyword** — There may be other structural differences (property syntax, layer naming, pad attributes) between KiCad versions that we haven't identified. Mitigate by testing against actual files from each version. Accept KiCad 7/8 as primary targets.

- **Benchmark board selection** — We need 3 open-source KiCad 7+ projects of varying complexity. Risk: good candidates may be hard to find with permissive licenses, 2-layer boards, and reasonable complexity. Mitigate by curating a list before implementation starts.

- **`symbolic_expressions` crate version compatibility** — `kicad_parse_gen` pins a specific version. If we use `symbolic_expressions` directly, version conflicts are possible. Mitigate by using the same version already in the dependency tree.

- **Board outline as polygon vs rectangle** — Real KiCad boards may have non-rectangular outlines (rounded corners, cutouts). `BoardSize` only supports rectangles. For S01, use bounding box; note this as a known limitation for complex boards.

- **Coordinate convention mismatch** — KiCad Y-axis is top-down, our renderer may expect bottom-up. If not handled correctly, boards will appear mirrored. Need to verify against existing import path and match convention.

- **Large file parsing performance** — Complex KiCad boards can be 1MB+ S-expression files. The `symbolic_expressions` parser loads everything into memory as a tree. Should be fine for benchmark boards but worth noting.

## Skills Discovered

| Technology | Skill | Status |
|------------|-------|--------|
| KiCad schematic | `kenchangh/kicad-schematic@kicad-schematic` (35 installs) | available — schematic-focused, not PCB parsing; low relevance |
| KiCad file format | `o2scale/electronics-agent-kit@kicad-file-format` (28 installs) | available — may contain .kicad_pcb format knowledge; moderate relevance |
| KiCad CLI | `o2scale/electronics-agent-kit@kicad-cli` (21 installs) | available — CLI usage, not parsing; low relevance |

None of these skills are directly relevant enough to install. The `.kicad_pcb` format is well-documented in KiCad's official docs, and our parsing scope is narrow enough that skill-level guidance isn't needed.

## Requirements Coverage

| Requirement | How S01 Delivers | Key Risk |
|-------------|-----------------|----------|
| **R101** — KiCad .kicad_pcb Board Parser | `parse_kicad_pcb(path) → Result<(BoardWorld, FootprintLibrary)>` extracting outline, footprints, pads, nets, traces, vias | KiCad 7/8 format change (`footprint` vs `module`) |
| **R102** — Benchmark Suite | 3+ `.kicad_pcb` files in `tests/fixtures/benchmark/` with `KicadBenchmark` metadata struct | Finding suitable open-source boards |

## Architecture Sketch

### Module Structure

```
crates/cypcb-kicad/
├── src/
│   ├── lib.rs              # Add: pub mod pcb_parser;
│   ├── footprint.rs         # Existing: .kicad_mod parser
│   ├── library.rs           # Existing: library scanning
│   └── pcb_parser.rs        # NEW: .kicad_pcb parser
│       ├── mod.rs            # Or single file:
│       │   ├── parse_kicad_pcb()
│       │   ├── KicadPcbError
│       │   ├── KicadBenchmark
│       │   └── internal conversion fns
```

### Data Flow

```
.kicad_pcb file (S-expression text)
  → symbolic_expressions::parser::parse_str()  [tokenize to Sexp tree]
  → walk Sexp tree extracting:
      (kicad_pcb
        (version N)              → version check
        (general ...)            → board metadata (net count, etc)
        (layers ...)             → layer stack count
        (net N "name")           → intern into NetRegistry  
        (net_class ...)          → design rules (optional for S01)
        (gr_line ... Edge.Cuts)  → board outline → BoardSize
        (footprint|module ...    → for each:
          (at X Y angle)           → Position, Rotation
          (fp_text reference ..)   → RefDes
          (fp_text value ..)       → Value
          (pad N type shape        → PadDef for FootprintLibrary
            (at x y)
            (size w h)
            (drill d)
            (layers ...)
            (net N "name"))        → NetConnections
        )
        (segment ...)            → reference RouteSegments (for scoring)
        (via ...)                → reference ViaPlacement (for scoring)
      )
  → BoardWorld + FootprintLibrary + Option<RoutingResult> (reference routes)
```

### Key Types

```rust
/// Result of parsing a .kicad_pcb file
pub struct KicadPcbParseResult {
    /// The populated board world
    pub world: BoardWorld,
    /// Footprint library with pad geometry for all footprints found
    pub library: FootprintLibrary,
    /// Reference routing (existing traces/vias from the original design)
    pub reference_routes: Option<RoutingResult>,
    /// Parse metadata
    pub metadata: KicadPcbMetadata,
}

/// Metadata about the parsed PCB
pub struct KicadPcbMetadata {
    pub version: i64,
    pub component_count: usize,
    pub net_count: usize,
    pub trace_segment_count: usize,
    pub via_count: usize,
    pub board_size_mm: (f64, f64),
    pub layer_count: u8,
}

/// Benchmark fixture descriptor
pub struct KicadBenchmark {
    pub name: String,
    pub pcb_file: PathBuf,
    pub complexity: BenchmarkComplexity,
    pub expected_component_count: usize,
    pub expected_net_count: usize,
    pub description: String,
}

pub enum BenchmarkComplexity {
    Simple,   // LED blink, <10 components, <10 nets
    Medium,   // STM32 breakout, 20-50 components, 20-80 nets
    Complex,  // Multi-IC, 50+ components, 80+ nets
}
```

## Sources

- KiCad .kicad_pcb S-expression format documentation (source: [KiCad Developer Docs](https://dev-docs.kicad.org/en/file-formats/sexpr-pcb/index))
- `kicad_parse_gen` v0.7.3 source code — layout module handles `module` but not `footprint` keyword; segments/vias are `Other(Sexp)` (source: cargo registry `/config/.cargo/registry/src/.../kicad_parse_gen-0.7.3/`)
- Existing `cypcb-kicad` crate — complete `.kicad_mod` footprint parser, layer mapping, courtyard extraction (source: `crates/cypcb-kicad/src/footprint.rs`)
- KiCad track segment format: `(segment (start X Y) (end X Y) (width W) (layer L) (net N))` (source: KiCad docs)
- KiCad via format: `(via (at X Y) (size D) (drill D) (layers L1 L2) (net N))` (source: KiCad docs)
- KiCad 7+ uses `footprint` keyword instead of `module` for board-level components (source: KiCad docs + `kicad_parse_gen` source analysis)
