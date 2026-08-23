//! KiCad .kicad_pcb S-expression parser.
//!
//! Parses KiCad 6/7/8 PCB files into [`BoardWorld`] with footprints, pads, nets,
//! board outline, and reference routing. Uses `symbolic_expressions` for S-expression
//! tokenization, then walks the tree to extract structured data.
//!
//! # Supported KiCad Versions
//!
//! - KiCad 6 (`version 20211014`): uses `module` keyword for footprints
//! - KiCad 7 (`version 20221018`): uses `footprint` keyword
//! - KiCad 8 (`version 20240108`): uses `footprint` keyword
//!
//! # Example
//!
//! ```rust,ignore
//! use cypcb_kicad::pcb_parser::parse_kicad_pcb;
//! use std::path::Path;
//!
//! let result = parse_kicad_pcb(Path::new("board.kicad_pcb"))?;
//! println!("Components: {}", result.metadata.component_count);
//! println!("Nets: {}", result.metadata.net_count);
//! ```

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use cypcb_core::{Nm, Point, Rect};
use cypcb_router::types::{RouteSegment, RoutingResult, RoutingStatus, ViaPlacement};
use cypcb_world::components::zone::Zone;
use cypcb_world::components::{BoardOutline, EdgeConnector, Layer, PadShape, Side};
use cypcb_world::footprint::{Footprint, FootprintLibrary, PadDef};
use cypcb_world::{
    BoardWorld, FootprintRef, NetConnections, NetId, PinConnection, Position, RefDes, Rotation,
    Stackup, StackupLayer, StackupLayerKind, Value,
};
use symbolic_expressions::Sexp;
use thiserror::Error;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors that can occur during KiCad PCB parsing.
#[derive(Error, Debug)]
pub enum KicadPcbError {
    /// File I/O error.
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    /// S-expression parse error.
    #[error("S-expression parse error: {0}")]
    SexprParseError(String),

    /// Required field missing from the PCB file.
    #[error("Missing field '{field}' in {context}")]
    MissingField {
        /// The missing field name.
        field: String,
        /// Where the field was expected.
        context: String,
    },

    /// Unsupported KiCad file version.
    #[error("Unsupported KiCad version {version} (this reader understands 20171130 and newer)")]
    UnsupportedVersion {
        /// The version number found.
        version: i64,
    },

    /// Invalid data in the PCB file.
    #[error("Invalid data: {0}")]
    InvalidData(String),
}

// ---------------------------------------------------------------------------
// Result types
// ---------------------------------------------------------------------------

/// Metadata extracted from a parsed KiCad PCB file.
#[derive(Debug, Clone, serde::Serialize)]
pub struct KicadPcbMetadata {
    /// KiCad file format version number.
    pub version: i64,
    /// Number of components (footprints) found.
    pub component_count: usize,
    /// Number of nets (excluding net 0 "").
    pub net_count: usize,
    /// Number of trace segments found.
    pub trace_segment_count: usize,
    /// Number of vias found.
    pub via_count: usize,
    /// Board size in mm as (width, height).
    pub board_size_mm: (f64, f64),
    /// Number of copper layers.
    pub layer_count: u8,
    /// Copper pours and rule areas carried into the board.
    pub zone_count: usize,
    /// Zones the file carried and this importer would not approximate, each
    /// with the reason. Reported rather than dropped in silence: a board that
    /// arrives without its ground plane and says nothing is a board whose
    /// Gerber ships without a ground plane.
    pub zone_refusals: Vec<String>,
    /// Stackup entries whose `(type ...)` this importer has no word for.
    ///
    /// Same rule as `zone_refusals`, and the same reason: a stackup short two
    /// entries is not a shorter description of the board, it is a different
    /// one. This is also the channel a KiCad release that adds a layer kind
    /// will arrive through.
    pub stackup_refusals: Vec<String>,
}

/// Complete result of parsing a KiCad PCB file.
pub struct KicadPcbParseResult {
    /// Populated board world with components, nets, and board outline.
    pub world: BoardWorld,
    /// Footprint library with pad geometry for all footprints found.
    pub library: FootprintLibrary,
    /// Reference routing extracted from existing traces and vias (if any).
    pub reference_routes: Option<RoutingResult>,
    /// Parse metadata for inspection.
    pub metadata: KicadPcbMetadata,
    /// This board's corner in the file's own coordinates.
    ///
    /// Every position in `world` is relative to it. Writing anything back into
    /// the file means adding it again.
    pub board_origin_mm: (f64, f64),
    /// Which KiCad net number each interned net came from.
    ///
    /// A `(segment ...)` written back has to name the file's own net number,
    /// not this crate's `NetId`. They agree by accident on a board whose nets
    /// were declared in order and disagree on every other.
    pub net_numbers: std::collections::HashMap<NetId, i64>,
}

/// Complexity tier for benchmark classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BenchmarkComplexity {
    /// Simple board: <10 components.
    Simple,
    /// Medium board: 10-50 components.
    Medium,
    /// Complex board: 50+ components.
    Complex,
}

/// Descriptor for a benchmark KiCad PCB file.
#[derive(Debug, Clone)]
pub struct KicadBenchmark {
    /// File path relative to the test fixtures directory.
    pub filename: &'static str,
    /// Human-readable description.
    pub description: &'static str,
    /// Expected complexity tier.
    pub complexity: BenchmarkComplexity,
    /// Expected component count (approximate).
    pub expected_component_count: usize,
    /// Expected net count (approximate).
    pub expected_net_count: usize,
}

/// Static benchmark fixture descriptors.
///
/// Each entry describes one `.kicad_pcb` file in `tests/fixtures/benchmark/`
/// with expected metadata for validation. Component and net counts should match
/// actual parse results within ±20% tolerance for medium/complex boards.
pub const BENCHMARKS: &[KicadBenchmark] = &[
    KicadBenchmark {
        filename: "led_blink.kicad_pcb",
        description: "Simple LED blink circuit — 7 components, 2-layer, 40×30mm",
        complexity: BenchmarkComplexity::Simple,
        expected_component_count: 7,
        expected_net_count: 7,
    },
    KicadBenchmark {
        filename: "stm32_breakout.kicad_pcb",
        description:
            "STM32F103C8T6 breakout with USB, SWD, GPIO headers — 29 components, 2-layer, 75×65mm",
        complexity: BenchmarkComplexity::Medium,
        expected_component_count: 29,
        expected_net_count: 40,
    },
    KicadBenchmark {
        filename: "multi_ic.kicad_pcb",
        description:
            "STM32F407 + Ethernet PHY + SPI Flash + CAN — 52 components, 4-layer, 100×80mm",
        complexity: BenchmarkComplexity::Complex,
        expected_component_count: 52,
        expected_net_count: 94,
    },
    // Added 2026-08-08, after every routing conclusion in `docs/routing.md`
    // had been drawn from the three above - two of them dense in the same way.
    // Generated by `tests/fixtures/benchmark/make_shift_driver.py`, which
    // knows nothing about the router, so it is a board no setting was fitted
    // on. Through-hole DIP beside 0805 chips is a mix the other three do not
    // have; they are SMD-dominant.
    KicadBenchmark {
        filename: "shift_driver.kicad_pcb",
        description: "Three 74HC595 in a chain driving 24 LEDs — 55 components, 2-layer, 58×42mm",
        complexity: BenchmarkComplexity::Medium,
        expected_component_count: 55,
        expected_net_count: 55,
    },
    // Added 2026-08-08. The four above leave one regime untouched: a
    // fine-pitch part on two layers. `stm32_breakout` has a TQFP at 0.8mm
    // pitch and `multi_ic` an LQFP-100 on four layers with room to escape
    // into; escaping 64 pins at 0.5mm with only a top and a bottom is the case
    // a two-layer router is hardest on. Generated by
    // `tests/fixtures/benchmark/make_qfp_fanout.py`.
    // Added 2026-08-08, and it is the first fixture with a ground plane.
    // Every routing number in `docs/routing.md` was measured on a board
    // without one, which is not a small gap: a plane changes what the router
    // is solving. Without it GND is a net like any other and every ground pin
    // needs a trace; with it those pins are connected the moment the pour is
    // filled, and the plane becomes copper the signal nets must respect.
    // Generated by `tests/fixtures/benchmark/make_plane_board.py`.
    KicadBenchmark {
        filename: "plane_board.kicad_pcb",
        description: "Sensor hub with a poured GND plane - 12 components, 2-layer, 50x38mm",
        complexity: BenchmarkComplexity::Medium,
        expected_component_count: 12,
        expected_net_count: 15,
    },
    KicadBenchmark {
        filename: "qfp_fanout.kicad_pcb",
        description: "LQFP-64 at 0.5mm pitch escaping on two layers — 19 components, 46×46mm",
        complexity: BenchmarkComplexity::Complex,
        expected_component_count: 19,
        expected_net_count: 50,
    },
];

/// Return all benchmark descriptors with file paths resolved relative to the
/// workspace root (`tests/fixtures/benchmark/`).
///
/// This is useful for test code that needs absolute or workspace-relative paths
/// to the benchmark `.kicad_pcb` files.
pub fn get_benchmarks() -> Vec<(KicadBenchmark, std::path::PathBuf)> {
    BENCHMARKS
        .iter()
        .map(|b| {
            let path = std::path::PathBuf::from("tests/fixtures/benchmark").join(b.filename);
            (b.clone(), path)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Parse a KiCad `.kicad_pcb` file from disk.
///
/// Reads the file and delegates to [`parse_kicad_pcb_str`].
///
/// # Errors
///
/// Returns [`KicadPcbError::IoError`] if the file cannot be read,
/// or any other variant if parsing fails.
pub fn parse_kicad_pcb(path: &Path) -> Result<KicadPcbParseResult, KicadPcbError> {
    let content = fs::read_to_string(path)?;
    parse_kicad_pcb_str(&content)
}

/// Parse a KiCad PCB from a string.
///
/// This is the core parser. It:
/// 1. Tokenizes the S-expression via `symbolic_expressions::parser::parse_str()`
/// 2. Validates the KiCad version (6/7/8 range)
/// 3. Extracts nets, board outline, footprints with pads, traces, and vias
/// 4. Populates a [`BoardWorld`] and [`FootprintLibrary`]
/// 5. Returns a [`KicadPcbParseResult`] with metadata
///
/// # Errors
///
/// Returns a [`KicadPcbError`] variant describing the failure.
pub fn parse_kicad_pcb_str(content: &str) -> Result<KicadPcbParseResult, KicadPcbError> {
    // Parse S-expression tree
    let sexp = symbolic_expressions::parser::parse_str(content)
        .map_err(|e| KicadPcbError::SexprParseError(format!("{}", e)))?;

    let root_list = sexp
        .list()
        .map_err(|e| KicadPcbError::SexprParseError(format!("Root is not a list: {}", e)))?;

    // Verify root element is "kicad_pcb"
    if root_list.is_empty() {
        return Err(KicadPcbError::SexprParseError(
            "Empty root element".to_string(),
        ));
    }
    let root_name = get_string(&root_list[0]).unwrap_or_default();
    if root_name != "kicad_pcb" {
        return Err(KicadPcbError::SexprParseError(format!(
            "Expected 'kicad_pcb' root, got '{}'",
            root_name
        )));
    }

    let elements = &root_list[1..];

    // 1. Extract version
    let version = extract_version(elements)?;

    // 2. Extract nets — build KiCad net number → name mapping
    let mut world = BoardWorld::new();
    let mut kicad_net_map = NetIndex::default();

    for elem in elements {
        if let Some(name) = list_name(elem) {
            if name == "net" {
                if let Ok(list) = elem.list() {
                    if list.len() >= 3 {
                        let net_num = get_i64(&list[1]).unwrap_or(0);
                        let net_name = get_string(&list[2]).unwrap_or_default();
                        if net_num != 0 && !net_name.is_empty() {
                            let net_id = world.intern_net(&net_name);
                            kicad_net_map.by_number.insert(net_num, net_id);
                            kicad_net_map.by_name.insert(net_name, net_id);
                        }
                    }
                }
            }
        }
    }
    // KiCad 10 has no table, so the names have to come off the things that
    // carry them. Only then: a board with a table has already named every net,
    // and walking it again reads the `(net 1)` on each segment as a name.
    if kicad_net_map.by_number.is_empty() {
        collect_named_nets(elements, &mut world, &mut kicad_net_map);
    }
    let net_count = kicad_net_map.len();

    // 3. Extract copper layer count
    let layer_count = extract_layer_count(elements);

    // The stackup, which used to be dropped whole: a board exported by this
    // project and read back lost the layer names and the laminate it had just
    // been given.
    let (stackup, stackup_refusals) = extract_stackup(elements);

    // 4. Extract board outline from Edge.Cuts
    let board_bounds = extract_board_outline(elements);
    // Board origin in KiCad coordinates — component positions are absolute,
    // so we translate them relative to the board's top-left corner.
    let board_origin = board_bounds
        .as_ref()
        .map(|b| {
            (
                b.min.x.0 as f64 / 1_000_000.0,
                b.min.y.0 as f64 / 1_000_000.0,
            )
        })
        .unwrap_or((0.0, 0.0));
    if let Some(ref bounds) = board_bounds {
        let width = Nm(bounds.max.x.0 - bounds.min.x.0);
        let height = Nm(bounds.max.y.0 - bounds.min.y.0);
        let board = world.set_board("KiCad PCB".to_string(), (width, height), layer_count);

        // The real edge, when Edge.Cuts describes one. The size above stays the
        // bounding box, which is what everything that only needs "how big" uses.
        if let Some(outline) = extract_board_ring(elements).and_then(BoardOutline::new) {
            world.ecs_mut().entity_mut(board).insert(outline);
        }

        // After `set_board`, which is what a stackup attaches to.
        if let Some(stackup) = stackup {
            world.set_stackup(stackup);
        }
    }

    // 5. Extract footprints (handles both `footprint` and `module` keywords)
    let mut library = FootprintLibrary::new();
    let mut component_count = 0usize;

    for elem in elements {
        if let Some(name) = list_name(elem) {
            if name == "footprint" || name == "module" {
                if let Ok(list) = elem.list() {
                    parse_footprint(
                        &list[1..],
                        &mut world,
                        &mut library,
                        &kicad_net_map,
                        board_origin,
                    )?;
                    component_count += 1;
                }
            }
        }
    }

    // 6. Extract trace segments
    let mut route_segments: Vec<RouteSegment> = Vec::new();
    for elem in elements {
        if let Some(name) = list_name(elem) {
            if name == "segment" {
                if let Some(seg) = parse_segment(elem, &kicad_net_map, board_origin)? {
                    route_segments.push(seg);
                }
            }
        }
    }

    // 7. Extract vias
    let mut via_placements: Vec<ViaPlacement> = Vec::new();
    for elem in elements {
        if let Some(name) = list_name(elem) {
            if name == "via" {
                if let Some(via) = parse_via(elem, &kicad_net_map, board_origin)? {
                    via_placements.push(via);
                }
            }
        }
    }

    // 8. Extract copper pours and rule areas
    let mut zone_refusals: Vec<String> = Vec::new();
    let mut zone_count = 0usize;
    for elem in elements {
        if list_name(elem).as_deref() != Some("zone") {
            continue;
        }
        match parse_zone(elem, &kicad_net_map, board_origin)? {
            ZoneImport::Carried(zone) => {
                world.spawn_entity((zone,));
                zone_count += 1;
            }
            ZoneImport::Refused(why) => zone_refusals.push(why),
            ZoneImport::Skipped => {}
        }
    }

    // Build reference routes
    let trace_segment_count = route_segments.len();
    let via_count_val = via_placements.len();
    let reference_routes = if route_segments.is_empty() && via_placements.is_empty() {
        None
    } else {
        Some(RoutingResult {
            status: RoutingStatus::Complete,
            routes: route_segments,
            vias: via_placements,
        })
    };

    // Build metadata
    let board_size_mm = if let Some(ref bounds) = board_bounds {
        let w = (bounds.max.x.0 - bounds.min.x.0) as f64 / 1_000_000.0;
        let h = (bounds.max.y.0 - bounds.min.y.0) as f64 / 1_000_000.0;
        (w, h)
    } else {
        (0.0, 0.0)
    };

    let net_numbers = kicad_net_map.numbers();

    let metadata = KicadPcbMetadata {
        zone_count,
        zone_refusals,
        stackup_refusals,
        version,
        component_count,
        net_count,
        trace_segment_count,
        via_count: via_count_val,
        board_size_mm,
        layer_count,
    };

    // The board carries its own footprints. Callers are handed the library
    // separately as well, but anything that only has the world - the DRC rules
    // that need pad copper or silkscreen artwork - has to be able to find them,
    // and an imported board that hides its footprints silently degrades every
    // one of those checks to a courtyard box.
    let mut world = world;
    world.set_footprints(library.clone());

    Ok(KicadPcbParseResult {
        world,
        library,
        reference_routes,
        metadata,
        board_origin_mm: board_origin,
        net_numbers,
    })
}

// ---------------------------------------------------------------------------
// Internal: version extraction
// ---------------------------------------------------------------------------

/// The nets a board names, in both spellings KiCad has used.
///
/// Through KiCad 9 a board carried a numbered table at the top and everything
/// on it referred to that table by number: `(net 1 "VCC")` in the table,
/// `(net 1 "VCC")` on the pad. KiCad 10 dropped the table and writes the name
/// alone - `(net "VCC")`.
///
/// A reader that only knows the numbered form reads a KiCad 10 board as having
/// no nets at all, and this one did. `parse-kicad` on a board KiCad had just
/// saved reported `net_count: 0` on every one of three test boards, so an
/// imported board arrived with its copper unconnected and nothing said why.
#[derive(Default)]
pub(crate) struct NetIndex {
    by_number: HashMap<i64, NetId>,
    by_name: HashMap<String, NetId>,
}

impl NetIndex {
    /// The net a `(net ...)` node refers to, whichever spelling it uses.
    ///
    /// The number is tried first and the name second, rather than deciding
    /// from the token's shape: a net may legitimately be *called* `5`, and on
    /// a board with a numbered table that name has to lose to the table.
    fn resolve(&self, node: &Sexp) -> Option<NetId> {
        let list = node.list().ok()?;
        let value = list.get(1)?;
        if let Some(number) = get_i64(value) {
            if let Some(id) = self.by_number.get(&number) {
                return Some(*id);
            }
        }
        self.by_name.get(&get_string(value)?).copied()
    }

    /// How many nets the board names, the unconnected net excluded.
    fn len(&self) -> usize {
        self.by_name.len()
    }

    /// The KiCad number each net had, for the callers that write one back out.
    fn numbers(&self) -> HashMap<NetId, i64> {
        self.by_number
            .iter()
            .map(|(number, net_id)| (*net_id, *number))
            .collect()
    }
}

/// Walk every `(net ...)` node in the tree and name the nets it finds.
///
/// Only for the tableless form, and the caller checks that before calling:
/// a two-element `(net ...)` node means two different things depending on
/// which it is. In KiCad 10 it is `(net "VBUS")`, the name. In every earlier
/// version - and in what this project's own writer produces - a segment and a
/// via carry `(net 1)`, the number, and reading that as a name interns a net
/// called "1" for every piece of copper on the board.
fn collect_named_nets(elements: &[Sexp], world: &mut BoardWorld, index: &mut NetIndex) {
    for elem in elements {
        if list_name(elem).as_deref() == Some("net") {
            if let Ok(list) = elem.list() {
                // Two elements and not a number: `(net "VBUS")`.
                if list.len() == 2 && get_i64(&list[1]).is_none() {
                    if let Some(name) = get_string(&list[1]) {
                        if !name.is_empty() && !index.by_name.contains_key(&name) {
                            let net_id = world.intern_net(&name);
                            index.by_name.insert(name, net_id);
                        }
                    }
                }
            }
        }
        if let Ok(children) = elem.list() {
            collect_named_nets(children, world, index);
        }
    }
}

/// The oldest board format this reader understands.
///
/// There used to be an upper bound of 20250101 beside this, and on 2026-08-10
/// it refused a board KiCad 10.0.5 had just written:
///
/// ```text
/// Unsupported KiCad version 20260206 (supported: 20171130-20250101)
/// ```
///
/// The file parsed perfectly once the number was changed by hand, so the gate
/// was not protecting the reader from anything - it was guessing which
/// versions would exist and refusing the ones it had not heard of. A newer
/// format either still carries the nodes this reader looks for, in which case
/// refusing it is pure loss, or it does not, in which case the shape checks
/// below are what report it. Guessing at a ceiling does neither.
const MIN_VERSION: i64 = 20171130; // KiCad 5

fn extract_version(elements: &[Sexp]) -> Result<i64, KicadPcbError> {
    for elem in elements {
        if let Some(name) = list_name(elem) {
            if name == "version" {
                if let Ok(list) = elem.list() {
                    if list.len() >= 2 {
                        let v = get_i64(&list[1]).ok_or_else(|| KicadPcbError::MissingField {
                            field: "version number".to_string(),
                            context: "kicad_pcb".to_string(),
                        })?;
                        if v < MIN_VERSION {
                            return Err(KicadPcbError::UnsupportedVersion { version: v });
                        }
                        return Ok(v);
                    }
                }
            }
        }
    }
    Err(KicadPcbError::MissingField {
        field: "version".to_string(),
        context: "kicad_pcb".to_string(),
    })
}

// ---------------------------------------------------------------------------
// Internal: layer count extraction
// ---------------------------------------------------------------------------

/// What a KiCad stackup entry's `(type ...)` means here.
///
/// Two spellings per surface finish, on purpose. `BuildDefaultStackupList`
/// sets the human labels that every board pcbnew writes carries, and
/// `BOARD_STACKUP_ITEM`'s constructor sets the bare keys underneath them - a
/// file written by something other than pcbnew may well carry either.
/// Accepting both on the way in cannot produce a wrong file; the writer beside
/// this one stays exact.
fn stackup_kind_of(type_name: &str) -> Option<StackupLayerKind> {
    Some(match type_name {
        "copper" => StackupLayerKind::Copper,
        "core" => StackupLayerKind::Core,
        "prepreg" => StackupLayerKind::Prepreg,
        "Top Solder Mask" | "Bottom Solder Mask" | "soldermask" => StackupLayerKind::Mask,
        "Top Silk Screen" | "Bottom Silk Screen" | "silkscreen" => StackupLayerKind::Silk,
        "Top Solder Paste" | "Bottom Solder Paste" | "solderpaste" => StackupLayerKind::Paste,
        _ => return None,
    })
}

/// The stackup the file describes, and the entries this importer could not read.
///
/// `(setup (stackup (layer "F.Cu" (type "copper") (thickness 0.035)) ...))`.
/// A dielectric may open either as a quoted `"dielectric 1"` or as the bare
/// pair `dielectric 1`, so both are read and both come out as the same name.
///
/// A `(thickness ...)` may carry `locked` after the number. What the stackup
/// states about itself that is not a layer - `copper_finish`, `edge_plating`,
/// `castellated_pads`, `edge_connector`, `dielectric_constraints` - is read
/// too: those five are what the board is quoted on, and dropping them meant a
/// board imported from KiCad and sent back out asked for a different build.
fn extract_stackup(elements: &[Sexp]) -> (Option<Stackup>, Vec<String>) {
    let mut refusals = Vec::new();
    let node = elements
        .iter()
        .filter(|elem| list_name(elem).as_deref() == Some("setup"))
        .filter_map(|setup| setup.list().ok())
        .flat_map(|list| list.iter())
        .find(|elem| list_name(elem).as_deref() == Some("stackup"));
    let Some(node) = node else {
        return (None, refusals);
    };
    let Ok(entries) = node.list() else {
        return (None, refusals);
    };

    let mut finish = None;
    let mut edges_plated = false;
    let mut castellated_pads = false;
    let mut edge_connector = None;
    let mut impedance_controlled = false;
    for entry in &entries[1..] {
        let Ok(fields) = entry.list() else {
            continue;
        };
        if fields.len() < 2 {
            continue;
        }
        // KiCad writes `yes` for a flag that is on and leaves the whole node
        // out when it is off, so the node being here is nearly the statement -
        // but `(edge_plating no)` does occur, and reading that as `yes` would
        // order plating nobody asked for.
        let on = |value: &Sexp| get_string(value).as_deref() != Some("no");
        match list_name(entry).as_deref() {
            Some("copper_finish") => finish = get_string(&fields[1]),
            Some("edge_plating") => edges_plated = on(&fields[1]),
            Some("castellated_pads") => castellated_pads = on(&fields[1]),
            Some("dielectric_constraints") => impedance_controlled = on(&fields[1]),
            Some("edge_connector") => {
                edge_connector = match get_string(&fields[1]).as_deref() {
                    Some("bevelled") => Some(EdgeConnector::Bevelled),
                    Some("no") => None,
                    Some(_) => Some(EdgeConnector::Plain),
                    None => None,
                }
            }
            _ => {}
        }
    }

    let mut layers = Vec::new();
    for entry in &entries[1..] {
        if list_name(entry).as_deref() != Some("layer") {
            continue;
        }
        let Ok(fields) = entry.list() else {
            continue;
        };
        if fields.len() < 2 {
            continue;
        }

        // `dielectric 1` is two atoms where every other name is one.
        let (name, first_child) = match (get_string(&fields[1]), fields.get(2).and_then(get_i64)) {
            (Some(word), Some(number)) if word == "dielectric" => {
                (format!("dielectric {number}"), 3)
            }
            (Some(name), _) => (name, 2),
            _ => continue,
        };

        let mut type_name = None;
        let mut thickness = None;
        let mut material = None;
        let mut color = None;
        let mut dk_x1000 = None;
        let mut df_x1000000 = None;
        for child in &fields[first_child..] {
            let Ok(pair) = child.list() else {
                continue;
            };
            if pair.len() < 2 {
                continue;
            }
            match get_string(&pair[0]).as_deref() {
                Some("type") => type_name = get_string(&pair[1]),
                Some("thickness") => thickness = get_f64(&pair[1]).map(Nm::from_mm),
                Some("material") => material = get_string(&pair[1]),
                // KiCad writes this on mask and silkscreen only, and it is
                // part of the order: a house charges for a mask that is not
                // green.
                Some("color") => color = get_string(&pair[1]),
                Some("epsilon_r") => {
                    dk_x1000 = get_f64(&pair[1])
                        .filter(|value| value.is_finite() && *value > 0.0)
                        .map(|value| (value * 1_000.0).round() as u32)
                }
                Some("loss_tangent") => {
                    df_x1000000 = get_f64(&pair[1])
                        .filter(|value| value.is_finite() && *value > 0.0)
                        .map(|value| (value * 1_000_000.0).round() as u32)
                }
                _ => {}
            }
        }

        let Some(type_name) = type_name else {
            refusals.push(format!("`{name}` states no type"));
            continue;
        };
        let Some(kind) = stackup_kind_of(&type_name) else {
            refusals.push(format!(
                "`{name}` is a `{type_name}`, which has no word here"
            ));
            continue;
        };
        layers.push(StackupLayer {
            kind,
            name: Some(name),
            thickness,
            // KiCad states a thickness in millimetres and nothing else, so
            // there is no other unit to remember here.
            written_as: None,
            material,
            color,
            dk_x1000,
            df_x1000000,
        });
    }

    if layers.is_empty() {
        return (None, refusals);
    }
    (
        Some(Stackup {
            layers,
            finish,
            edges_plated,
            castellated_pads,
            edge_connector,
            impedance_controlled,
        }),
        refusals,
    )
}

fn extract_layer_count(elements: &[Sexp]) -> u8 {
    let mut copper_count = 0u8;
    for elem in elements {
        if let Some(name) = list_name(elem) {
            if name == "layers" {
                if let Ok(list) = elem.list() {
                    for layer_def in &list[1..] {
                        if let Ok(sub) = layer_def.list() {
                            // (N "name" type)
                            if sub.len() >= 3 {
                                let layer_type = get_string(&sub[2]).unwrap_or_default();
                                if layer_type == "signal" || layer_type == "power" {
                                    copper_count += 1;
                                }
                            }
                        }
                    }
                }
                break;
            }
        }
    }
    if copper_count < 2 {
        2 // Default to 2-layer
    } else {
        copper_count
    }
}

// ---------------------------------------------------------------------------
// Internal: board outline extraction
// ---------------------------------------------------------------------------

fn extract_board_outline(elements: &[Sexp]) -> Option<Rect> {
    let mut min_x = f64::MAX;
    let mut min_y = f64::MAX;
    let mut max_x = f64::MIN;
    let mut max_y = f64::MIN;
    let mut found = false;

    for elem in elements {
        if let Some(name) = list_name(elem) {
            match name.as_str() {
                "gr_line" => {
                    if is_on_edge_cuts(elem) {
                        if let (Some(start), Some(end)) =
                            (find_xy_child(elem, "start"), find_xy_child(elem, "end"))
                        {
                            update_bounds(
                                start.0, start.1, &mut min_x, &mut min_y, &mut max_x, &mut max_y,
                            );
                            update_bounds(
                                end.0, end.1, &mut min_x, &mut min_y, &mut max_x, &mut max_y,
                            );
                            found = true;
                        }
                    }
                }
                "gr_rect" => {
                    if is_on_edge_cuts(elem) {
                        if let (Some(start), Some(end)) =
                            (find_xy_child(elem, "start"), find_xy_child(elem, "end"))
                        {
                            update_bounds(
                                start.0, start.1, &mut min_x, &mut min_y, &mut max_x, &mut max_y,
                            );
                            update_bounds(
                                end.0, end.1, &mut min_x, &mut min_y, &mut max_x, &mut max_y,
                            );
                            found = true;
                        }
                    }
                }
                "gr_poly" => {
                    if is_on_edge_cuts(elem) {
                        if let Ok(list) = elem.list() {
                            for child in list {
                                if list_name(child).as_deref() == Some("pts") {
                                    if let Ok(pts_list) = child.list() {
                                        for pt in &pts_list[1..] {
                                            if list_name(pt).as_deref() == Some("xy") {
                                                if let Ok(pt_list) = pt.list() {
                                                    if pt_list.len() >= 3 {
                                                        if let (Some(x), Some(y)) = (
                                                            get_f64(&pt_list[1]),
                                                            get_f64(&pt_list[2]),
                                                        ) {
                                                            update_bounds(
                                                                x, y, &mut min_x, &mut min_y,
                                                                &mut max_x, &mut max_y,
                                                            );
                                                            found = true;
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    if found {
        Some(Rect::from_points(
            Point::from_mm(min_x, min_y),
            Point::from_mm(max_x, max_y),
        ))
    } else {
        None
    }
}

fn is_on_edge_cuts(sexp: &Sexp) -> bool {
    if let Ok(list) = sexp.list() {
        for child in list {
            if list_name(child).as_deref() == Some("layer") {
                if let Ok(sub) = child.list() {
                    if sub.len() >= 2 {
                        let layer_name = get_string(&sub[1]).unwrap_or_default();
                        return layer_name == "Edge.Cuts";
                    }
                }
            }
        }
    }
    false
}

fn update_bounds(
    x: f64,
    y: f64,
    min_x: &mut f64,
    min_y: &mut f64,
    max_x: &mut f64,
    max_y: &mut f64,
) {
    if x < *min_x {
        *min_x = x;
    }
    if y < *min_y {
        *min_y = y;
    }
    if x > *max_x {
        *max_x = x;
    }
    if y > *max_y {
        *max_y = y;
    }
}

// ---------------------------------------------------------------------------
// Internal: footprint parsing
// ---------------------------------------------------------------------------

/// A number that has to be there, or an error naming what was expected.
///
/// The importer read every coordinate with `unwrap_or(0.0)`, so a malformed
/// one placed the part at the board origin in silence. That is the shape of
/// mistake a board file actually carries - a stray comma, a truncated write -
/// and putting a part 50mm from where the file says is worse than refusing to
/// read the file at all.
fn coordinate(value: &Sexp, what: &str) -> Result<f64, KicadPcbError> {
    get_f64(value).ok_or_else(|| {
        KicadPcbError::InvalidData(format!(
            "{what} is not a number: {value:?}. A coordinate must be a plain \
             decimal, so `(at 105, 80)` is one comma away from `(at 105 80)`."
        ))
    })
}

/// The key to store this geometry under, given what the library already holds.
///
/// The plain library name when it is free or already holds exactly these pads;
/// otherwise the name with a numbered suffix, so a second geometry sharing a
/// name gets a home of its own rather than silently inheriting the first.
///
/// Pads are compared as written - number, shape, position, size, drill and
/// layers - because those are what the router and the checker read. Two parts
/// that differ only in rotation are the same footprint placed differently and
/// share a key, which is what the `rotation` on the component is for.
fn resolve_library_key(library: &FootprintLibrary, name: &str, pads: &[PadDef]) -> String {
    let same = |existing: &Footprint| -> bool {
        existing.pads.len() == pads.len()
            && existing.pads.iter().zip(pads).all(|(a, b)| {
                a.number == b.number
                    && a.shape == b.shape
                    && a.position == b.position
                    && a.size == b.size
                    && a.drill == b.drill
                    && a.layers == b.layers
            })
    };

    match library.get(name) {
        None => name.to_string(),
        Some(existing) if same(existing) => name.to_string(),
        Some(_) => {
            // A name in use by a different geometry. Walk the suffixes until
            // one is free or one already holds this geometry.
            for n in 2.. {
                let candidate = format!("{name}#{n}");
                match library.get(&candidate) {
                    None => return candidate,
                    Some(existing) if same(existing) => return candidate,
                    Some(_) => continue,
                }
            }
            unreachable!("the suffix search is unbounded")
        }
    }
}

fn parse_footprint(
    elements: &[Sexp],
    world: &mut BoardWorld,
    library: &mut FootprintLibrary,
    kicad_net_map: &NetIndex,
    board_origin_mm: (f64, f64),
) -> Result<(), KicadPcbError> {
    // First element is the library link name (e.g., "Resistor_SMD:R_0402")
    let lib_link = if !elements.is_empty() {
        get_string(&elements[0]).unwrap_or_default()
    } else {
        String::new()
    };

    // Extract a short name from the lib_link for the footprint ref
    let fp_name = lib_link.rsplit(':').next().unwrap_or(&lib_link).to_string();

    // Parse position: (at X Y [angle])
    let mut pos_x = 0.0f64;
    let mut pos_y = 0.0f64;
    let mut angle = 0.0f64;
    let mut refdes_str = String::new();
    let mut value_str = String::new();
    let mut pads: Vec<ParsedPad> = Vec::new();
    // A footprint states its own face - `(layer "B.Cu")` means bottom. This is
    // the one place in the codebase where the side is data rather than a guess
    // from where the copper is.
    let mut side = Side::Top;

    let children = if elements.is_empty() {
        &[][..]
    } else {
        &elements[1..]
    };

    for child in children {
        if let Some(name) = list_name(child) {
            match name.as_str() {
                "at" => {
                    if let Ok(list) = child.list() {
                        if list.len() >= 3 {
                            // A coordinate that will not parse is an error, not
                            // a zero.
                            //
                            // `unwrap_or(0.0)` put a part at the board's origin
                            // and said nothing, and a board file is written by
                            // machines and edited by people: `multi_ic.kicad_pcb`
                            // carried `(at 105, 80)` - one comma - for as long as
                            // it has existed, and imported with its ferrite bead
                            // and its Ethernet transformer 50mm to the left of
                            // the board. Every routing number ever measured on
                            // that fixture was measured with them there.
                            pos_x = coordinate(&list[1], "footprint position x")?;
                            pos_y = coordinate(&list[2], "footprint position y")?;
                        }
                        if list.len() >= 4 {
                            angle = coordinate(&list[3], "footprint rotation")?;
                        }
                    }
                }
                "fp_text" | "property" => {
                    // KiCad 7/8 uses (property "Reference" "R1" ...)
                    // KiCad 5/6 uses (fp_text reference "R1" ...)
                    if let Ok(list) = child.list() {
                        if list.len() >= 3 {
                            let text_type = get_string(&list[1]).unwrap_or_default();
                            let text_value = get_string(&list[2]).unwrap_or_default();
                            match text_type.to_lowercase().as_str() {
                                "reference" => refdes_str = text_value,
                                "value" => value_str = text_value,
                                _ => {}
                            }
                        }
                    }
                }
                "layer" => {
                    if let Ok(sub) = child.list() {
                        if sub.len() >= 2 && get_string(&sub[1]).as_deref() == Some("B.Cu") {
                            side = Side::Bottom;
                        }
                    }
                }
                "pad" => {
                    if let Ok(list) = child.list() {
                        if let Some(pad) = parse_pad(&list[1..], kicad_net_map)? {
                            pads.push(pad);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    // Use a unique fp_name based on lib_link to avoid collisions
    let library_key = if lib_link.is_empty() {
        fp_name.clone()
    } else {
        lib_link.clone()
    };

    let pad_defs: Vec<PadDef> = pads
        .iter()
        .map(|p| PadDef {
            number: p.number.clone(),
            shape: p.shape,
            position: p.local_position,
            size: p.size,
            drill: p.drill,
            slot: p.slot,
            layers: p.layers.clone(),
        })
        .collect();

    // The key a footprint is stored under has to describe its geometry, not
    // just its name.
    //
    // This registered the first part it saw under a library name and reused
    // that geometry for every later part naming the same library:
    // `if !library.contains(&library_key)`. A board carrying two variants of
    // one library - a header laid along x beside the same header laid along y,
    // a footprint someone edited in place - imported as two copies of
    // whichever came first, and the model then disagreed with the file about
    // where the copper is. Found on `qfp_fanout`, where two of four headers
    // ran off the board in the model and nowhere near it in the file.
    let library_key = resolve_library_key(library, &library_key, &pad_defs);

    if !library.contains(&library_key) {
        // The courtyard has to be derived: KiCad keeps it as `F.CrtYd`
        // graphics inside the footprint and this reader carries pads only, so
        // there is nothing to read - `multi_ic` has no `F.CrtYd` line at all.
        //
        // Derived at the excess this project uses everywhere else. It was a
        // flat 0.5mm, twice `IPC_COURTYARD_EXCESS`, so an imported board lost
        // half a millimetre of apparent clearance between every neighbouring
        // pair and the checker invented overlaps the same board would not have
        // if it had been written in this project's own language.
        let bounds = calculate_pad_bounds(&pad_defs);
        let margin = cypcb_world::footprint::IPC_COURTYARD_EXCESS;
        let courtyard = Rect::from_points(
            Point::new(bounds.min.x - margin, bounds.min.y - margin),
            Point::new(bounds.max.x + margin, bounds.max.y + margin),
        );

        library.register(Footprint {
            name: library_key.clone(),
            description: String::new(),
            pads: pad_defs,
            bounds,
            courtyard,
            silk: Vec::new(),
        });
    }

    // Build net connections
    let mut net_connections = NetConnections::with_capacity(pads.len());
    for pad in &pads {
        if let Some(net_id) = pad.net_id {
            net_connections.add(PinConnection::new(pad.number.clone(), net_id));
        }
    }

    // Convert position mm → nm, translating from absolute KiCad coords
    // to board-relative coords (origin at board's top-left corner)
    let position = Position::from_mm(pos_x - board_origin_mm.0, pos_y - board_origin_mm.1);
    // Convert angle degrees → millidegrees
    let rotation = Rotation((angle * 1000.0).round() as i32);

    // Use refdes if available, otherwise generate one
    let refdes = if refdes_str.is_empty() {
        RefDes::new("??")
    } else {
        RefDes::new(refdes_str)
    };

    let entity = world.spawn_component(
        refdes,
        Value::new(value_str),
        position,
        rotation,
        FootprintRef::new(library_key),
        net_connections,
    );
    world.ecs_mut().entity_mut(entity).insert(side);

    Ok(())
}

// ---------------------------------------------------------------------------
// Internal: pad parsing
// ---------------------------------------------------------------------------

struct ParsedPad {
    number: String,
    shape: PadShape,
    local_position: Point,
    size: (Nm, Nm),
    drill: Option<Nm>,
    /// The hole's full size when KiCad wrote `(drill oval W H)`.
    slot: Option<(Nm, Nm)>,
    layers: Vec<Layer>,
    net_id: Option<NetId>,
}

fn parse_pad(
    elements: &[Sexp],
    kicad_net_map: &NetIndex,
) -> Result<Option<ParsedPad>, KicadPcbError> {
    // (pad "1" smd|thru_hole rect|circle|oval (at X Y) (size W H) (drill D) (layers ...) (net N "name"))
    // elements[0] = pad number
    // elements[1] = pad type (smd, thru_hole, np_thru_hole, connect)
    // elements[2] = pad shape (rect, circle, oval, roundrect, custom)
    // remaining = property lists

    if elements.len() < 3 {
        return Ok(None);
    }

    let number = get_string(&elements[0]).unwrap_or_default();
    let pad_type_str = get_string(&elements[1]).unwrap_or_default();
    let shape_str = get_string(&elements[2]).unwrap_or_default();

    let shape = match shape_str.as_str() {
        "rect" => PadShape::Rect,
        "circle" => PadShape::Circle,
        "oval" => PadShape::Oblong,
        "roundrect" => PadShape::round_rect(25), // Default corner ratio
        _ => PadShape::Rect,                     // Fallback
    };

    let is_through_hole = pad_type_str == "thru_hole" || pad_type_str == "np_thru_hole";
    // KiCad's word for a hole the fabricator must not plate: a mounting hole,
    // a tooling hole. It writes them with copper layers all the same -
    // `(layers "*.Cu" "*.Mask")` is what pcbnew emits for a stock
    // `MountingHole_3.2mm` - so the layer list in the file cannot be trusted
    // to say what the pad type already said.
    let is_non_plated = pad_type_str == "np_thru_hole";

    let mut local_pos = Point::ORIGIN;
    let mut size = (Nm::from_mm(1.0), Nm::from_mm(1.0));
    let mut drill: Option<Nm> = None;
    let mut slot: Option<(Nm, Nm)> = None;
    let mut layers: Vec<Layer> = Vec::new();
    let mut net_id: Option<NetId> = None;

    for prop in &elements[3..] {
        if let Some(name) = list_name(prop) {
            match name.as_str() {
                "at" => {
                    if let Ok(list) = prop.list() {
                        if list.len() >= 3 {
                            let x = coordinate(&list[1], "pad position x")?;
                            let y = coordinate(&list[2], "pad position y")?;
                            local_pos = Point::from_mm(x, y);
                        }
                    }
                }
                "size" => {
                    if let Ok(list) = prop.list() {
                        if list.len() >= 3 {
                            let w = coordinate(&list[1], "pad width")?;
                            let h = coordinate(&list[2], "pad height")?;
                            size = (Nm::from_mm(w), Nm::from_mm(h));
                        }
                    }
                }
                // `(drill 0.9)` is a round hole and `(drill oval 2.4 1.0)` a
                // slot, milled rather than drilled. Reading only the first
                // form did not lose the slot - it refused the whole board,
                // with a message about a stray comma. Every USB connector,
                // barrel jack and latching header has one.
                //
                // Either form may carry `(offset x y)`, which moves the hole
                // inside its pad. That is read where the pad is placed, not
                // here.
                "drill" => {
                    if let Ok(list) = prop.list() {
                        let is_oval = list.get(1).and_then(get_string).as_deref() == Some("oval");
                        if is_oval && list.len() >= 4 {
                            let w = coordinate(&list[2], "pad slot width")?;
                            let h = coordinate(&list[3], "pad slot height")?;
                            if w > 0.0 && h > 0.0 {
                                slot = Some((Nm::from_mm(w), Nm::from_mm(h)));
                                // Every rule about a drill means the narrow
                                // dimension: the bit the fab has to own, the
                                // width the plating reaches down.
                                drill = Some(Nm::from_mm(w.min(h)));
                            }
                        } else if !is_oval && list.len() >= 2 {
                            let d = coordinate(&list[1], "pad drill")?;
                            if d > 0.0 {
                                drill = Some(Nm::from_mm(d));
                            }
                        }
                    }
                }
                "layers" => {
                    if let Ok(list) = prop.list() {
                        for layer_sexp in &list[1..] {
                            let layer_name = get_string(layer_sexp).unwrap_or_default();
                            parse_layer_names(&layer_name, &mut layers);
                        }
                    }
                }
                "net" => {
                    net_id = kicad_net_map.resolve(prop);
                }
                _ => {}
            }
        }
    }

    // A hole with no copper is what non-plated means, so the copper the file
    // listed comes off. Everything else it asked for - mask, paste - stays.
    if is_non_plated {
        layers.retain(|layer| !layer.is_copper());
    }

    // If no layers parsed, use defaults based on pad type
    if layers.is_empty() && !is_non_plated {
        if is_through_hole {
            layers.push(Layer::TopCopper);
            layers.push(Layer::BottomCopper);
        } else {
            layers.push(Layer::TopCopper);
            layers.push(Layer::TopPaste);
            layers.push(Layer::TopMask);
        }
    }

    // For through-hole pads, ensure drill is set
    if is_through_hole && drill.is_none() {
        drill = Some(Nm::from_mm(0.8)); // Default drill size
    }

    Ok(Some(ParsedPad {
        number,
        shape,
        local_position: local_pos,
        size,
        drill,
        slot,
        layers,
        net_id,
    }))
}

// ---------------------------------------------------------------------------
// Internal: segment parsing
// ---------------------------------------------------------------------------

/// Read a `(zone ...)` from a KiCad board.
///
/// A copper pour is the largest thing on a real two-layer board and the
/// importer read none of them. A board with a ground plane arrived here with
/// no plane: the router then treated the whole area as free, the checker
/// measured clearances against copper that was not in the model, and the
/// exported Gerber shipped a board whose ground was missing.
///
/// What can be carried is carried exactly. This crate's `Zone` is a rectangle,
/// and KiCad's is a polygon, so a pour whose outline is an axis-aligned
/// rectangle comes across as itself and anything else is refused by name
/// rather than flattened to its bounding box. A bounding box is not a
/// conservative approximation of an L-shaped pour - it is copper where the
/// designer put none, and it would be wrong in exactly the places the shape
/// was drawn to avoid.
fn parse_zone(
    elem: &Sexp,
    net_map: &NetIndex,
    origin: (f64, f64),
) -> Result<ZoneImport, KicadPcbError> {
    let Ok(list) = elem.list() else {
        return Ok(ZoneImport::Skipped);
    };

    let mut net_id: Option<NetId> = None;
    let mut net_name = String::new();
    let mut layer_mask: u32 = 0;
    let mut points: Vec<(f64, f64)> = Vec::new();
    let mut is_keepout = false;

    for child in &list[1..] {
        match list_name(child).as_deref() {
            Some("net") => {
                net_id = net_map.resolve(child);
            }
            Some("net_name") => {
                if let Ok(name_list) = child.list() {
                    if name_list.len() >= 2 {
                        net_name = get_string(&name_list[1]).unwrap_or_default();
                    }
                }
            }
            // `layer` for one, `layers` for a pour repeated down the stack.
            Some("layer") | Some("layers") => {
                if let Ok(layer_list) = child.list() {
                    for layer_sexp in &layer_list[1..] {
                        let mut layers = Vec::new();
                        parse_layer_names(&get_string(layer_sexp).unwrap_or_default(), &mut layers);
                        for layer in layers {
                            layer_mask |= layer.to_copper_mask();
                        }
                    }
                }
            }
            // A KiCad "rule area": copper is kept out rather than poured.
            Some("keepout") => is_keepout = true,
            Some("polygon") => {
                if let Ok(poly) = child.list() {
                    for pts in &poly[1..] {
                        if list_name(pts).as_deref() != Some("pts") {
                            continue;
                        }
                        let Ok(pts_list) = pts.list() else { continue };
                        for pt in &pts_list[1..] {
                            if list_name(pt).as_deref() != Some("xy") {
                                continue;
                            }
                            let Ok(pt_list) = pt.list() else { continue };
                            if pt_list.len() >= 3 {
                                let x = coordinate(&pt_list[1], "zone outline x")?;
                                let y = coordinate(&pt_list[2], "zone outline y")?;
                                points.push((x - origin.0, y - origin.1));
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    let label = if net_name.is_empty() {
        "a zone".to_string()
    } else {
        format!("the zone on net {net_name}")
    };

    if layer_mask == 0 {
        return Ok(ZoneImport::Refused(format!(
            "{label} names no copper layer, so there is nothing to pour it on"
        )));
    }

    let Some(bounds) = axis_aligned_rectangle(&points) else {
        return Ok(ZoneImport::Refused(format!(
            "{label} is a {}-point outline; this importer carries rectangular \
             pours only, and a bounding box would put copper where the shape \
             was drawn to avoid it",
            points.len()
        )));
    };

    if is_keepout {
        return Ok(ZoneImport::Carried(Zone::keepout(bounds, layer_mask)));
    }

    match net_id {
        Some(net) => Ok(ZoneImport::Carried(
            Zone::copper_pour_for_net(bounds, layer_mask, net).with_name(net_name),
        )),
        // A pour with no net cannot be filled or checked, and KiCad does write
        // them - an unassigned pour is a common half-finished state.
        None => Ok(ZoneImport::Refused(format!(
            "{label} is poured to no net, so nothing connects to it"
        ))),
    }
}

/// What became of one `(zone ...)`.
pub enum ZoneImport {
    /// Carried into the board.
    Carried(Zone),
    /// Not carried, and why - reported rather than dropped in silence.
    Refused(String),
    /// Not a zone worth a word.
    Skipped,
}

/// The rectangle an outline describes, if it describes one.
///
/// KiCad closes a rectangle with four points; some writers repeat the first as
/// a fifth. Anything else - a rounded corner, an L, a pour cut around a
/// connector - is not a rectangle and is refused rather than approximated.
fn axis_aligned_rectangle(points: &[(f64, f64)]) -> Option<Rect> {
    let mut pts = points.to_vec();
    if pts.len() == 5 && pts[0] == pts[4] {
        pts.pop();
    }
    if pts.len() != 4 {
        return None;
    }

    let xs: Vec<f64> = pts.iter().map(|p| p.0).collect();
    let ys: Vec<f64> = pts.iter().map(|p| p.1).collect();
    let (min_x, max_x) = (
        xs.iter().cloned().fold(f64::MAX, f64::min),
        xs.iter().cloned().fold(f64::MIN, f64::max),
    );
    let (min_y, max_y) = (
        ys.iter().cloned().fold(f64::MAX, f64::min),
        ys.iter().cloned().fold(f64::MIN, f64::max),
    );

    // Every corner has to sit on the rectangle's own corners, or the outline
    // only happens to share its bounding box.
    let on_corner = |p: &(f64, f64)| (p.0 - min_x).abs() < 1e-6 || (p.0 - max_x).abs() < 1e-6;
    let on_edge = |p: &(f64, f64)| (p.1 - min_y).abs() < 1e-6 || (p.1 - max_y).abs() < 1e-6;
    if !pts.iter().all(|p| on_corner(p) && on_edge(p)) {
        return None;
    }
    if (max_x - min_x) < 1e-6 || (max_y - min_y) < 1e-6 {
        return None;
    }

    Some(Rect::new(
        Point::from_mm(min_x, min_y),
        Point::from_mm(max_x, max_y),
    ))
}

/// Read one `(segment ...)`.
///
/// `origin` is the board's own corner in file coordinates, and it is
/// subtracted here for the same reason it is subtracted from every pad: the
/// model puts the board's corner at zero. It was not, so on any board whose
/// outline does not start at the file's origin - which is every board KiCad
/// writes, since it lays them out on a sheet - the copper already in the file
/// arrived offset by the whole origin. `led_blink` has its outline at
/// (95, 55) and its traces at (110, 62); they imported onto a 40 x 30mm board
/// at 110mm, 62mm, which the checker reported as five pieces of copper
/// hanging off the edge.
fn parse_segment(
    sexp: &Sexp,
    kicad_net_map: &NetIndex,
    origin: (f64, f64),
) -> Result<Option<RouteSegment>, KicadPcbError> {
    let list = match sexp.list() {
        Ok(l) => l,
        Err(_) => return Ok(None),
    };

    let mut start: Option<Point> = None;
    let mut end: Option<Point> = None;
    let mut width = Nm::from_mm(0.25); // Default trace width
    let mut layer = Layer::TopCopper;
    let mut net_id = NetId::new(0);

    for child in &list[1..] {
        if let Some(name) = list_name(child) {
            match name.as_str() {
                "start" => {
                    if let Ok(sub) = child.list() {
                        if sub.len() >= 3 {
                            let x = coordinate(&sub[1], "segment start x")?;
                            let y = coordinate(&sub[2], "segment start y")?;
                            start = Some(Point::from_mm(x - origin.0, y - origin.1));
                        }
                    }
                }
                "end" => {
                    if let Ok(sub) = child.list() {
                        if sub.len() >= 3 {
                            let x = coordinate(&sub[1], "segment end x")?;
                            let y = coordinate(&sub[2], "segment end y")?;
                            end = Some(Point::from_mm(x - origin.0, y - origin.1));
                        }
                    }
                }
                "width" => {
                    if let Ok(sub) = child.list() {
                        if sub.len() >= 2 {
                            let w = get_f64(&sub[1]).unwrap_or(0.25);
                            width = Nm::from_mm(w);
                        }
                    }
                }
                "layer" => {
                    if let Ok(sub) = child.list() {
                        if sub.len() >= 2 {
                            let l = get_string(&sub[1]).unwrap_or_default();
                            layer = parse_layer_name(&l).unwrap_or(Layer::TopCopper);
                        }
                    }
                }
                "net" => {
                    if let Some(id) = kicad_net_map.resolve(child) {
                        net_id = id;
                    }
                }
                _ => {}
            }
        }
    }

    Ok(match (start, end) {
        (Some(s), Some(e)) => Some(RouteSegment::new(net_id, layer, width, s, e)),
        _ => None,
    })
}

// ---------------------------------------------------------------------------
// Internal: via parsing
// ---------------------------------------------------------------------------

/// Read one `(via ...)`. `origin` is subtracted for the same reason as in
/// [`parse_segment`].
fn parse_via(
    sexp: &Sexp,
    kicad_net_map: &NetIndex,
    origin: (f64, f64),
) -> Result<Option<ViaPlacement>, KicadPcbError> {
    let list = match sexp.list() {
        Ok(l) => l,
        Err(_) => return Ok(None),
    };

    let mut position: Option<Point> = None;
    let mut drill = Nm::from_mm(0.3); // Default drill
    let mut start_layer = Layer::TopCopper;
    let mut end_layer = Layer::BottomCopper;
    let mut net_id = NetId::new(0);

    for child in &list[1..] {
        if let Some(name) = list_name(child) {
            match name.as_str() {
                "at" => {
                    if let Ok(sub) = child.list() {
                        if sub.len() >= 3 {
                            let x = coordinate(&sub[1], "via position x")?;
                            let y = coordinate(&sub[2], "via position y")?;
                            position = Some(Point::from_mm(x - origin.0, y - origin.1));
                        }
                    }
                }
                "drill" => {
                    if let Ok(sub) = child.list() {
                        if sub.len() >= 2 {
                            let d = get_f64(&sub[1]).unwrap_or(0.3);
                            drill = Nm::from_mm(d);
                        }
                    }
                }
                "layers" => {
                    if let Ok(sub) = child.list() {
                        if sub.len() >= 3 {
                            let l1 = get_string(&sub[1]).unwrap_or_default();
                            let l2 = get_string(&sub[2]).unwrap_or_default();
                            start_layer = parse_layer_name(&l1).unwrap_or(Layer::TopCopper);
                            end_layer = parse_layer_name(&l2).unwrap_or(Layer::BottomCopper);
                        }
                    }
                }
                "net" => {
                    if let Some(id) = kicad_net_map.resolve(child) {
                        net_id = id;
                    }
                }
                _ => {}
            }
        }
    }

    Ok(position.map(|pos| ViaPlacement::new(net_id, pos, drill, start_layer, end_layer)))
}

// ---------------------------------------------------------------------------
// Internal: layer name parsing
// ---------------------------------------------------------------------------

/// Parse a KiCad layer name string to an internal [`Layer`].
///
/// KiCad uses string layer names like "F.Cu", "B.Cu", "In1.Cu", "Edge.Cuts".
/// This function maps them to the internal [`Layer`] enum.
pub fn parse_layer_name(name: &str) -> Option<Layer> {
    match name {
        "F.Cu" => Some(Layer::TopCopper),
        "B.Cu" => Some(Layer::BottomCopper),
        "F.SilkS" | "F.Silkscreen" => Some(Layer::TopSilk),
        "B.SilkS" | "B.Silkscreen" => Some(Layer::BottomSilk),
        "F.Mask" => Some(Layer::TopMask),
        "B.Mask" => Some(Layer::BottomMask),
        "F.Paste" => Some(Layer::TopPaste),
        "B.Paste" => Some(Layer::BottomPaste),
        "Edge.Cuts" => Some(Layer::Outline),
        _ => {
            // Handle inner copper layers: In1.Cu, In2.Cu, ...
            // `In1.Cu` is the first inner layer, and this project numbers
            // those from zero - `job.rs` writes `Layer::Inner(n)` as
            // `In{n + 1}` and the DSL reads `Inner1` as `Inner(0)`. Reading it
            // as `Inner(1)` put every imported inner trace one layer deeper
            // than the file said, and exported it under the next layer's name.
            if let Some(rest) = name.strip_prefix("In") {
                if let Some(num_str) = rest.strip_suffix(".Cu") {
                    if let Ok(n) = num_str.parse::<u8>() {
                        return n.checked_sub(1).map(Layer::Inner);
                    }
                }
            }
            None
        }
    }
}

/// Parse a KiCad layer name, handling wildcards like "*.Cu" and "*.Mask".
/// Appends matching layers to the `layers` vec.
fn parse_layer_names(name: &str, layers: &mut Vec<Layer>) {
    match name {
        "*.Cu" => {
            layers.push(Layer::TopCopper);
            layers.push(Layer::BottomCopper);
        }
        "*.Mask" => {
            layers.push(Layer::TopMask);
            layers.push(Layer::BottomMask);
        }
        "*.Paste" => {
            layers.push(Layer::TopPaste);
            layers.push(Layer::BottomPaste);
        }
        "*.SilkS" | "*.Silkscreen" => {
            layers.push(Layer::TopSilk);
            layers.push(Layer::BottomSilk);
        }
        _ => {
            if let Some(layer) = parse_layer_name(name) {
                layers.push(layer);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// S-expression tree helpers
// ---------------------------------------------------------------------------

/// Get the name of a list S-expression (first element as string).
fn list_name(sexp: &Sexp) -> Option<String> {
    if let Ok(list) = sexp.list() {
        if !list.is_empty() {
            return get_string(&list[0]);
        }
    }
    None
}

/// Extract a string from a Sexp.
fn get_string(sexp: &Sexp) -> Option<String> {
    sexp.string().ok().cloned()
}

/// Extract an f64 from a Sexp string.
fn get_f64(sexp: &Sexp) -> Option<f64> {
    sexp.string().ok().and_then(|s| s.parse::<f64>().ok())
}

/// Extract an i64 from a Sexp string.
fn get_i64(sexp: &Sexp) -> Option<i64> {
    sexp.string().ok().and_then(|s| s.parse::<i64>().ok())
}

/// Find a child list with name `child_name` that contains (name X Y).
fn find_xy_child(sexp: &Sexp, child_name: &str) -> Option<(f64, f64)> {
    if let Ok(list) = sexp.list() {
        for child in list {
            if list_name(child).as_deref() == Some(child_name) {
                if let Ok(sub) = child.list() {
                    if sub.len() >= 3 {
                        let x = get_f64(&sub[1])?;
                        let y = get_f64(&sub[2])?;
                        return Some((x, y));
                    }
                }
            }
        }
    }
    None
}

/// Calculate bounding box from pad definitions.
fn calculate_pad_bounds(pads: &[PadDef]) -> Rect {
    if pads.is_empty() {
        return Rect::default();
    }

    let mut min_x = i64::MAX;
    let mut min_y = i64::MAX;
    let mut max_x = i64::MIN;
    let mut max_y = i64::MIN;

    for pad in pads {
        let half_w = pad.size.0 .0 / 2;
        let half_h = pad.size.1 .0 / 2;

        min_x = min_x.min(pad.position.x.0 - half_w);
        min_y = min_y.min(pad.position.y.0 - half_h);
        max_x = max_x.max(pad.position.x.0 + half_w);
        max_y = max_y.max(pad.position.y.0 + half_h);
    }

    Rect::from_points(
        Point::new(Nm(min_x), Nm(min_y)),
        Point::new(Nm(max_x), Nm(max_y)),
    )
}

/// The board's outline as a ring of points, when Edge.Cuts describes one.
///
/// A bounding box is enough to say how big a board is and wrong for saying
/// where its edge runs. A cutout, a slot or a chamfer all live inside the same
/// box, and clearance measured against the box passes copper that sits outside
/// the actual edge.
///
/// `gr_poly` is already a ring and is taken as written. Loose `gr_line`
/// segments are walked end to end into one; anything that does not close, or
/// leaves segments over, yields nothing rather than a guess - a partial
/// outline would be worse than the bounding box it replaces.
fn extract_board_ring(elements: &[Sexp]) -> Option<Vec<Point>> {
    // A polygon states the ring directly.
    for elem in elements {
        if list_name(elem).as_deref() != Some("gr_poly") || !is_on_edge_cuts(elem) {
            continue;
        }
        let mut points = Vec::new();
        if let Ok(list) = elem.list() {
            for child in list {
                if list_name(child).as_deref() != Some("pts") {
                    continue;
                }
                let Ok(pts) = child.list() else { continue };
                for pt in &pts[1..] {
                    if list_name(pt).as_deref() != Some("xy") {
                        continue;
                    }
                    let Ok(pt_list) = pt.list() else { continue };
                    if pt_list.len() >= 3 {
                        if let (Some(x), Some(y)) = (get_f64(&pt_list[1]), get_f64(&pt_list[2])) {
                            points.push(Point::from_mm(x, y));
                        }
                    }
                }
            }
        }
        if points.len() >= 3 {
            return Some(points);
        }
    }

    // Otherwise, walk the loose segments into a ring.
    let mut segments: Vec<(Point, Point)> = Vec::new();
    for elem in elements {
        if list_name(elem).as_deref() != Some("gr_line") || !is_on_edge_cuts(elem) {
            continue;
        }
        if let (Some(start), Some(end)) = (find_xy_child(elem, "start"), find_xy_child(elem, "end"))
        {
            segments.push((
                Point::from_mm(start.0, start.1),
                Point::from_mm(end.0, end.1),
            ));
        }
    }
    if segments.len() < 3 {
        return None;
    }

    // A micrometre: KiCad writes millimetres with enough decimals that two
    // endpoints meant to touch land on the same nanometre, but not enough to
    // rely on it.
    const JOIN_TOLERANCE_NM: i64 = 1_000;
    let touches = |a: Point, b: Point| -> bool {
        (a.x.raw() - b.x.raw()).abs() <= JOIN_TOLERANCE_NM
            && (a.y.raw() - b.y.raw()).abs() <= JOIN_TOLERANCE_NM
    };

    let mut used = vec![false; segments.len()];
    let start = segments[0].0;
    let mut cursor = segments[0].1;
    let mut ring = vec![start];
    used[0] = true;

    for _ in 1..segments.len() {
        ring.push(cursor);
        #[allow(clippy::question_mark)] // `?` would return None from the walk, not from this arm
        let Some((index, next)) = segments.iter().enumerate().find_map(|(i, (a, b))| {
            if used[i] {
                return None;
            }
            if touches(*a, cursor) {
                Some((i, *b))
            } else if touches(*b, cursor) {
                Some((i, *a))
            } else {
                None
            }
        }) else {
            return None;
        };
        used[index] = true;
        cursor = next;
    }

    if !touches(cursor, start) {
        return None;
    }
    Some(ring)
}
#[cfg(test)]
mod tests {

    #[test]
    fn a_boards_edge_cuts_become_an_outline() {
        // Four loose segments, written in an order that does not follow the
        // ring, with one reversed - which is how a hand-edited board looks.
        let pcb = r#"(kicad_pcb (version 20240108) (generator pcbnew)
  (layers (0 "F.Cu" signal) (31 "B.Cu" signal))
  (gr_line (start 50 30) (end 0 30) (layer "Edge.Cuts") (width 0.05))
  (gr_line (start 0 0) (end 50 0) (layer "Edge.Cuts") (width 0.05))
  (gr_line (start 0 0) (end 0 30) (layer "Edge.Cuts") (width 0.05))
  (gr_line (start 50 0) (end 50 30) (layer "Edge.Cuts") (width 0.05))
)"#;

        let parsed = parse_kicad_pcb_str(pcb).expect("parse");
        let world = parsed.world;
        let board = world.board_entity().expect("board");

        let outline = world
            .ecs()
            .get::<BoardOutline>(board)
            .expect("edge cuts describe a ring, so the board has an outline");

        assert_eq!(outline.points.len(), 4);
        assert!(outline.contains(Point::from_mm(25.0, 15.0)));
        assert!(!outline.contains(Point::from_mm(60.0, 15.0)));
    }

    #[test]
    fn edge_cuts_that_do_not_close_yield_no_outline() {
        // Three sides of a rectangle. A partial ring is worse than none: it
        // would put an edge where the board has none.
        let pcb = r#"(kicad_pcb (version 20240108) (generator pcbnew)
  (layers (0 "F.Cu" signal) (31 "B.Cu" signal))
  (gr_line (start 0 0) (end 50 0) (layer "Edge.Cuts") (width 0.05))
  (gr_line (start 50 0) (end 50 30) (layer "Edge.Cuts") (width 0.05))
  (gr_line (start 50 30) (end 0 30) (layer "Edge.Cuts") (width 0.05))
)"#;

        let parsed = parse_kicad_pcb_str(pcb).expect("parse");
        let world = parsed.world;
        let board = world.board_entity().expect("board");
        assert!(world.ecs().get::<BoardOutline>(board).is_none());
    }

    #[test]
    fn a_footprint_states_which_face_it_is_on() {
        // `(layer "B.Cu")` on a footprint means the part is placed from the
        // bottom. Everything else in the codebase has to guess this from where
        // the copper is, which cannot tell a bottom-side through-hole part from
        // a top-side one; here it is data.
        let pcb = r#"(kicad_pcb (version 20240108) (generator pcbnew)
  (general (thickness 1.6))
  (layers (0 "F.Cu" signal) (31 "B.Cu" signal))
  (gr_line (start 0 0) (end 50 0) (layer "Edge.Cuts") (width 0.05))
  (gr_line (start 50 0) (end 50 30) (layer "Edge.Cuts") (width 0.05))
  (gr_line (start 50 30) (end 0 30) (layer "Edge.Cuts") (width 0.05))
  (gr_line (start 0 30) (end 0 0) (layer "Edge.Cuts") (width 0.05))
  (footprint "R_0402" (layer "F.Cu") (at 10 10)
    (property "Reference" "R1")
    (pad "1" smd rect (at -0.5 0) (size 0.6 0.5) (layers "F.Cu" "F.Paste" "F.Mask"))
  )
  (footprint "R_0402" (layer "B.Cu") (at 20 10)
    (property "Reference" "R2")
    (pad "1" smd rect (at -0.5 0) (size 0.6 0.5) (layers "B.Cu" "B.Paste" "B.Mask"))
  )
)"#;

        let parsed = parse_kicad_pcb_str(pcb).expect("parse");
        let mut world = parsed.world;

        let ecs = world.ecs_mut();
        let mut query = ecs.query::<(&RefDes, &Side)>();
        let mut sides: Vec<(String, Side)> = query
            .iter(ecs)
            .map(|(refdes, side)| (refdes.as_str().to_string(), *side))
            .collect();
        sides.sort();

        assert_eq!(
            sides,
            vec![
                ("R1".to_string(), Side::Top),
                ("R2".to_string(), Side::Bottom)
            ]
        );
    }
    use super::*;

    #[test]
    fn test_parse_layer_name() {
        assert_eq!(parse_layer_name("F.Cu"), Some(Layer::TopCopper));
        assert_eq!(parse_layer_name("B.Cu"), Some(Layer::BottomCopper));
        // Zero-based inside, one-based in the file. `cypcb-export`'s
        // `layer_tag` writes `Layer::Inner(n)` as `In{n + 1}`, and the DSL
        // reads `Inner1` as `Inner(0)`; this read `In1.Cu` as `Inner(1)`, so
        // an imported inner trace landed one layer deeper than the file said
        // and exported under the next layer's name.
        assert_eq!(parse_layer_name("In1.Cu"), Some(Layer::Inner(0)));
        assert_eq!(parse_layer_name("In2.Cu"), Some(Layer::Inner(1)));
        assert_eq!(parse_layer_name("In0.Cu"), None, "there is no In0 layer");
        assert_eq!(parse_layer_name("F.SilkS"), Some(Layer::TopSilk));
        assert_eq!(parse_layer_name("B.SilkS"), Some(Layer::BottomSilk));
        assert_eq!(parse_layer_name("F.Mask"), Some(Layer::TopMask));
        assert_eq!(parse_layer_name("Edge.Cuts"), Some(Layer::Outline));
        assert_eq!(parse_layer_name("Unknown"), None);
    }

    #[test]
    fn test_empty_input_returns_error() {
        let result = parse_kicad_pcb_str("");
        match result {
            Err(KicadPcbError::SexprParseError(_)) => {} // Expected
            Err(other) => panic!("Expected SexprParseError, got {:?}", other),
            Ok(_) => panic!("Expected error for empty input"),
        }
    }

    #[test]
    fn test_unsupported_version_returns_error() {
        let input = r#"(kicad_pcb (version 1))"#;
        let result = parse_kicad_pcb_str(input);
        match result {
            Err(KicadPcbError::UnsupportedVersion { version }) => {
                assert_eq!(version, 1);
            }
            Err(other) => panic!("Expected UnsupportedVersion, got {:?}", other),
            Ok(_) => panic!("Expected error for unsupported version"),
        }
    }
}
