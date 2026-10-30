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

use crate::frame;
use cypcb_router::types::{RouteSegment, RoutingResult, RoutingStatus, ViaPlacement};
use cypcb_world::components::zone::{KeepoutKind, KeepoutKinds, Zone, ZoneOutline};
use cypcb_world::components::{
    ArcRun, BoardCutouts, BoardOutline, CurveSet, EdgeConnector, EdgeCurves, Layer, PadShape, Side,
    StackupSheet,
};
use cypcb_world::footprint::{Footprint, FootprintLibrary, PadDef, PlacementOrigin};
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

    /// Something follows the board's closing parenthesis.
    ///
    /// A `.kicad_pcb` is one `(kicad_pcb ...)` and nothing after it. The
    /// S-expression reader stops at the first closing parenthesis and ignores
    /// the rest, so a board with a design appended to it - what the viewer's
    /// save once wrote - read as the board alone, with its copper gone and no
    /// error.
    #[error(
        "text after the board's closing parenthesis at line {line}, column {column}: \
         a .kicad_pcb holds one (kicad_pcb ...) and nothing after it"
    )]
    TrailingContent {
        /// One-based line of the first character after the board.
        line: usize,
        /// One-based column of that character, in characters.
        column: usize,
    },

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

fn is_zero(n: &usize) -> bool {
    *n == 0
}

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
    /// Copper pours and rule areas carried into the board as rectangles.
    pub zone_count: usize,
    /// Rule areas and pours kept in the board as an outline, because they are
    /// not rectangles: the ring of corners the file states.
    ///
    /// A keepout is acted on whole, and a pour is filled from its ring.
    #[serde(skip_serializing_if = "is_zero")]
    pub outlined_zone_count: usize,
    /// Teardrop zones carried into the board as teardrop copper: a polygon on
    /// its layer and net, as KiCad keeps one.
    pub teardrop_count: usize,
    /// Zones the file carried and this importer would not approximate, each
    /// with the reason. Reported rather than dropped in silence: a board that
    /// arrives without its ground plane and says nothing is a board whose
    /// Gerber ships without a ground plane.
    pub zone_refusals: Vec<String>,
    /// Copper the file put on a layer this importer has no word for.
    ///
    /// Same rule as `zone_refusals`. The layer used to be read with
    /// `unwrap_or(Layer::TopCopper)`, so a name this parser did not know moved
    /// the copper to the top layer and said nothing - and this file already
    /// argues against exactly that, in the note on `coordinate`: putting a part
    /// 50mm from where the file says is worse than refusing to read the file at
    /// all. Copper on the wrong layer is the same mistake with a shorter
    /// distance. A newer KiCad that adds a layer name arrives through here.
    pub track_refusals: Vec<String>,
    /// Stackup entries whose `(type ...)` this importer has no word for.
    ///
    /// Same rule as `zone_refusals`, and the same reason: a stackup short two
    /// entries is not a shorter description of the board, it is a different
    /// one. This is also the channel a KiCad release that adds a layer kind
    /// will arrive through.
    pub stackup_refusals: Vec<String>,
    /// Pads whose shape this importer has no word for, read as rectangles.
    ///
    /// The board reader approximates rather than refuses, because one pad of
    /// an unknown shape should not cost a person the other nine hundred - but
    /// it approximates *copper*, and a `custom` pad is a polygon somebody drew
    /// for a reason. Silence here put a rectangle through the checker, the
    /// router and the Gerber writer as though it were the shape in the file.
    pub pad_approximations: Vec<String>,
    /// Footprints that state a point to be placed by and were not understood.
    ///
    /// A `FT Origin` or `Origin` field that asks for neither `Anchor` nor
    /// `Center` is ignored, as the plugin that reads it ignores it - and said,
    /// because the placement file is then not the one the person asked for.
    pub placement_notes: Vec<String>,
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

/// Refuse a file with anything but whitespace after its first expression.
///
/// Strings are skipped with their escapes, so a `)` inside a property value
/// does not end the board early. An expression that never closes is left for
/// the S-expression reader to report.
fn refuse_trailing_content(content: &str) -> Result<(), KicadPcbError> {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut end = None;
    for (at, c) in content.char_indices() {
        if in_string {
            match (escaped, c) {
                (true, _) => escaped = false,
                (false, '\\') => escaped = true,
                (false, '"') => in_string = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => in_string = true,
            '(' => depth += 1,
            ')' if depth > 0 => {
                depth -= 1;
                if depth == 0 {
                    end = Some(at + 1);
                    break;
                }
            }
            _ => {}
        }
    }
    let Some(end) = end else { return Ok(()) };
    let rest = &content[end..];
    let Some(offset) = rest.find(|c: char| !c.is_whitespace()) else {
        return Ok(());
    };
    let before = &content[..end + offset];
    let line = before.matches('\n').count() + 1;
    let column = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
    Err(KicadPcbError::TrailingContent { line, column })
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
    refuse_trailing_content(content)?;

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
    //
    // The board is the outer shape Edge.Cuts draws. Size and origin come from
    // that shape when this reader carries it as a ring; the shapes inside it
    // are holes. With no ring (a rectangle, or an edge with an arc in it) the
    // bounding box of the lines, rectangles and polygons stands, as it always
    // has.
    let edge = extract_edge_shapes(elements);
    let board_bounds = edge
        .ring
        .as_ref()
        .map(|ring| {
            Rect::from_points(
                Point::from_mm(ring.bounds.0, ring.bounds.1),
                Point::from_mm(ring.bounds.2, ring.bounds.3),
            )
        })
        .or_else(|| extract_board_outline(elements));
    // Board origin in KiCad coordinates - component positions are absolute,
    // so they are measured from the board's own corner, the bottom-left one,
    // with Y turned the board's way up. `frame` says why.
    let board_origin = board_bounds
        .as_ref()
        .map(|b| {
            frame::origin(
                b.min.x.0 as f64 / 1_000_000.0,
                b.max.y.0 as f64 / 1_000_000.0,
            )
        })
        .unwrap_or((0.0, 0.0));
    if let Some(ref bounds) = board_bounds {
        let width = Nm(bounds.max.x.0 - bounds.min.x.0);
        let height = Nm(bounds.max.y.0 - bounds.min.y.0);
        // The name the file's title block carries, when it has one. A board
        // written by this project states the design's own name there, so a
        // round trip keeps it; a board pcbnew wrote usually states nothing and
        // falls back to what this importer has always called them.
        let name = extract_title(elements).unwrap_or_else(|| "KiCad PCB".to_string());
        let board = world.set_board(name, (width, height), layer_count);

        // The real edge, when Edge.Cuts describes one. The size above stays the
        // bounding box, which is what everything that only needs "how big" uses.
        // The same origin every pad, trace and pour is measured from. The
        // ring was read in file coordinates while everything else was moved to
        // the board's own corner, so a design imported, written back out and
        // imported again walked away from its outline: measured on the USB
        // fixture, the parts moved 10mm left and 5mm up per trip while the
        // outline stayed on the sheet.
        let on_board = |shape: &EdgeRing| -> Vec<Point> {
            shape
                .points
                .iter()
                .map(|&(x, y)| frame::board_point(board_origin, x, y))
                .collect()
        };
        // What each shape was drawn as, for the board file to say again.
        let curves_of = |shape: &EdgeRing| -> CurveSet {
            CurveSet {
                arcs: shape
                    .arcs
                    .iter()
                    .map(|arc| ArcRun {
                        first: arc.first,
                        last: arc.last,
                        mid: frame::board_point(board_origin, arc.mid.0, arc.mid.1),
                    })
                    .collect(),
                circle: shape.circle.map(|((x, y), radius)| {
                    (frame::board_point(board_origin, x, y), Nm::from_mm(radius))
                }),
            }
        };
        if let Some(outline) = edge.ring.as_ref().map(on_board).and_then(BoardOutline::new) {
            world.ecs_mut().entity_mut(board).insert(outline);
        }
        if let Some(cutouts) = BoardCutouts::new(edge.cutouts.iter().map(on_board).collect()) {
            world.ecs_mut().entity_mut(board).insert(cutouts);
        }
        let curves = EdgeCurves {
            ring: edge.ring.as_ref().map(curves_of).unwrap_or_default(),
            cutouts: edge.cutouts.iter().map(curves_of).collect(),
        };
        if !curves.is_empty() {
            world.ecs_mut().entity_mut(board).insert(curves);
        }

        // After `set_board`, which is what a stackup attaches to.
        if let Some(stackup) = stackup {
            world.set_stackup(stackup);
        }
    }

    // 5. Extract footprints (handles both `footprint` and `module` keywords)
    let mut library = FootprintLibrary::new();
    let mut component_count = 0usize;
    let mut pad_approximations: Vec<String> = Vec::new();
    let mut placement_notes: Vec<String> = Vec::new();
    if edge.left_out > 0 {
        placement_notes.push(format!(
            "{} closed shape(s) on Edge.Cuts with a bezier curve in them are read neither as \
             the board edge nor as a cutout",
            edge.left_out
        ));
    }
    if edge.extra_outers > 0 {
        placement_notes.push(format!(
            "{} further outer shape(s) on Edge.Cuts: only the largest is the board edge",
            edge.extra_outers
        ));
    }

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
                        &mut pad_approximations,
                        &mut placement_notes,
                    )?;
                    component_count += 1;
                }
            }
        }
    }

    // 6. Extract trace segments
    // Copper on a layer this importer has no word for, named rather than moved.
    let mut track_refusals: Vec<String> = Vec::new();
    let mut route_segments: Vec<RouteSegment> = Vec::new();
    let mut no_net_segments: Vec<FileSegment> = Vec::new();
    for elem in elements {
        if let Some(name) = list_name(elem) {
            if name == "segment" {
                if let Some(seg) =
                    parse_segment(elem, &kicad_net_map, board_origin, &mut track_refusals)?
                {
                    match seg.net {
                        Some(net) => route_segments.push(RouteSegment::new(
                            net, seg.layer, seg.width, seg.start, seg.end,
                        )),
                        None => no_net_segments.push(seg),
                    }
                }
            }
        }
    }

    // 6b. Extract track arcs.
    //
    // KiCad holds a curve natively and this model holds straight segments, so
    // an arc arrives as the chords it stands for plus the curve they came
    // from. They are spawned here rather than handed over as reference routes
    // because a `RouteSegment` has nowhere to carry a curve, and a curve that
    // arrives as twelve anonymous segments is a curve the next save loses.
    let mut arc_count = 0usize;
    for elem in elements {
        if list_name(elem).as_deref() != Some("arc") {
            continue;
        }
        if let Some((trace, curve)) =
            parse_track_arc(elem, &kicad_net_map, board_origin, &mut track_refusals)?
        {
            // Copper on no net carries no `NetId`: the checker reads a missing
            // component as "belongs to no net", which is what it is.
            match trace.net_id {
                Some(net_id) => world.spawn_entity((trace, net_id, curve)),
                None => world.spawn_entity((trace, curve)),
            };
            arc_count += 1;
        }
    }
    let _ = arc_count;

    // 7. Extract vias
    let mut via_placements: Vec<ViaPlacement> = Vec::new();
    let mut no_net_vias: Vec<FileVia> = Vec::new();
    for elem in elements {
        if let Some(name) = list_name(elem) {
            if name == "via" {
                if let Some(via) =
                    parse_via(elem, &kicad_net_map, board_origin, &mut track_refusals)?
                {
                    match via.net {
                        Some(net) => via_placements.push(match via.outer_diameter {
                            Some(size) => ViaPlacement::from_file(
                                net,
                                via.position,
                                via.drill,
                                size,
                                via.start_layer,
                                via.end_layer,
                            ),
                            None => ViaPlacement::new(
                                net,
                                via.position,
                                via.drill,
                                via.start_layer,
                                via.end_layer,
                            ),
                        }),
                        None => no_net_vias.push(via),
                    }
                }
            }
        }
    }

    // 7b. Copper on no net (`(net 0)`, KiCad's unconnected net).
    //
    // It is not handed over as reference routes: a `RouteSegment` and a
    // `ViaPlacement` belong to a net, and giving these one is how they came
    // to join the first net on the board. Each is spawned alone, as a trace
    // or a via with no net and no `NetId` component. Copper on no net is
    // foreign to every net and to the other copper on no net, so two pieces
    // of it are never merged into one trace.
    for seg in &no_net_segments {
        world.spawn_entity((cypcb_world::components::trace::Trace {
            segments: vec![cypcb_world::components::trace::TraceSegment::new(
                seg.start, seg.end,
            )],
            width: seg.width,
            layer: seg.layer,
            net_id: None,
            locked: false,
            source: cypcb_world::components::trace::TraceSource::Manual,
        },));
    }
    for via in &no_net_vias {
        world.spawn_entity((cypcb_world::components::trace::Via {
            position: via.position,
            drill: via.drill,
            // The router's own ring for a via the file gives no size, as
            // `ViaPlacement::new` takes it.
            outer_diameter: via.outer_diameter.unwrap_or(Nm(via.drill.0 * 2)),
            start_layer: via.start_layer,
            end_layer: via.end_layer,
            net_id: None,
            locked: false,
        },));
    }

    // 8. Extract copper pours and rule areas
    let mut zone_refusals: Vec<String> = Vec::new();
    let mut zone_count = 0usize;
    let mut outlined_zone_count = 0usize;
    let mut teardrop_count = 0usize;
    let mut mask_teardrops = 0usize;
    for elem in elements {
        if list_name(elem).as_deref() != Some("zone") {
            continue;
        }
        match parse_zone(elem, &kicad_net_map, board_origin, layer_count)? {
            ZoneImport::Carried(zone, kinds) => {
                match kinds {
                    Some(kinds) => world.spawn_entity((zone, kinds)),
                    None => world.spawn_entity((zone,)),
                };
                zone_count += 1;
            }
            // Not a rectangle: kept as the ring of corners the file states.
            ZoneImport::Outlined {
                zone,
                outline,
                kinds,
            } => {
                let entity = world.spawn_outlined_zone(zone, outline);
                if let Some(kinds) = kinds {
                    world.ecs_mut().entity_mut(entity).insert(kinds);
                }
                outlined_zone_count += 1;
            }
            // A teardrop is copper of the net and layer it is on, whole: the
            // polygon in the file is the polygon, curved or not, and the
            // builder that makes this model's own teardrops leaves it alone
            // (no land).
            ZoneImport::Teardrop {
                outline,
                layer,
                net,
            } => {
                world.spawn_entity((
                    cypcb_world::teardrop::TeardropCopper {
                        outline,
                        layer,
                        order: FILE_TEARDROP_ORDER + teardrop_count as u32,
                        land: None,
                    },
                    net,
                ));
                teardrop_count += 1;
            }
            ZoneImport::MaskTeardrop => mask_teardrops += 1,
            ZoneImport::Refused(why) => zone_refusals.push(why),
            ZoneImport::Skipped => {}
        }
    }
    if mask_teardrops > 0 {
        zone_refusals.push(format!(
            "{mask_teardrops} teardrop zone(s) on solder mask layers are not carried: KiCad \
             makes one beside each copper teardrop to open the mask over it, and this model \
             derives the mask from the copper"
        ));
    }

    // Build reference routes
    let trace_segment_count = route_segments.len() + no_net_segments.len();
    let via_count_val = via_placements.len() + no_net_vias.len();
    let reference_routes = if route_segments.is_empty() && via_placements.is_empty() {
        None
    } else {
        Some(RoutingResult {
            status: RoutingStatus::Complete,
            routes: route_segments,
            vias: via_placements,
            smoothing: None,
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
        pad_approximations,
        placement_notes,
        zone_count,
        outlined_zone_count,
        teardrop_count,
        zone_refusals,
        track_refusals,
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

    // And the board goes into the spatial index before anybody is handed it.
    //
    // Every rule that pairs one thing with another - clearance first among
    // them - walks `world.spatial()`, and an index nobody filled is an index
    // with nothing in it: `ClearanceRule` on a board imported here compared
    // **no pairs at all** and reported a clean board every time. Measured
    // 2026-08-31 on the four KiCad boards in this repository: 0 entries before
    // this line, 4 to 14 after it, and the same 0 violations either way - the
    // boards really are clean, which is exactly why nothing noticed.
    //
    // The command-line path filled the index itself (`board_source.rs`), so
    // the shipped `cypcb check` was never blind; anything holding this crate
    // directly was. Filling it here means a caller cannot get it wrong.
    world.rebuild_spatial_index_from_library(&library);

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
/// What a layer is, read from its name first and its type second.
///
/// KiCad's stackup has no word for a coverlay or a stiffener, so the writer
/// beside this one puts them in the file as what they physically are - a
/// dielectric film and a dielectric sheet - under the names `F.Coverlay` and
/// `B.Stiffener`, which say which. Reading the type alone brought a
/// rigid-flex board back as prepreg and core: the information was in the file
/// and this reader walked past it, which made the trip lossy in one direction
/// only and therefore invisible to a writer test.
///
/// A name is only allowed to override the type towards these two. Nothing
/// else in a KiCad file is named this way, and a board that happens to carry a
/// layer called `F.Coverlay` means the same thing by it.
fn kind_of(name: &str, type_name: &str) -> Option<StackupLayerKind> {
    match name {
        "F.Coverlay" | "B.Coverlay" => return Some(StackupLayerKind::Coverlay),
        "F.Stiffener" | "B.Stiffener" => return Some(StackupLayerKind::Stiffener),
        _ => {}
    }
    stackup_kind_of(type_name)
}

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

        // Every extra sheet in this slot. KiCad writes the first sheet's
        // numbers on the layer itself and each one after it as `addsublayer`,
        // which is how a fabricator hits a target thickness with the prepreg
        // they stock. This project read the layer and dropped the rest, so a
        // six-layer board came back thinner than it went out.
        let mut sheets = Vec::new();
        for child in &fields[first_child..] {
            let Ok(sub) = child.list() else {
                continue;
            };
            if get_string(&sub[0]).as_deref() != Some("addsublayer") {
                continue;
            }
            let mut sheet = StackupSheet::default();
            for pair in &sub[1..] {
                let Ok(pair) = pair.list() else {
                    continue;
                };
                if pair.len() < 2 {
                    continue;
                }
                match get_string(&pair[0]).as_deref() {
                    Some("thickness") => sheet.thickness = get_f64(&pair[1]).map(Nm::from_mm),
                    Some("material") => sheet.material = get_string(&pair[1]),
                    Some("epsilon_r") => {
                        sheet.dk_x1000 = get_f64(&pair[1])
                            .filter(|value| value.is_finite() && *value > 0.0)
                            .map(|value| (value * 1_000.0).round() as u32)
                    }
                    Some("loss_tangent") => {
                        sheet.df_x1000000 = get_f64(&pair[1])
                            .filter(|value| value.is_finite() && *value > 0.0)
                            .map(|value| (value * 1_000_000.0).round() as u32)
                    }
                    _ => {}
                }
            }
            sheets.push(sheet);
        }

        let Some(type_name) = type_name else {
            refusals.push(format!("`{name}` states no type"));
            continue;
        };
        let Some(kind) = kind_of(&name, &type_name) else {
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
            sheets,
            dk_x1000,
            df_x1000000,
            // KiCad's stackup states a layer and never where it stops: a
            // rigid-flex build there is drawn with per-layer areas the board
            // file has no field for. Nothing to read, so nothing is claimed.
            coverage: None,
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
            // KiCad has no word for a drill pair, so a board imported from it
            // states none - which is not the same as stating that every span
            // is allowed, and the rule reads it that way.
            drill_pairs: Vec::new(),
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
/// Pads are compared as written - number, shape, position, size, drill, the
/// hole's offset and layers - because those are what the router and the checker read.
/// The point a part is placed by is compared too: the same pads asking for a
/// different point are placed at a different point in the pick-and-place file.
/// Two parts
/// that differ only in rotation are the same footprint placed differently and
/// share a key, which is what the `rotation` on the component is for.
fn resolve_library_key(
    library: &FootprintLibrary,
    name: &str,
    pads: &[PadDef],
    placement_origin: Option<PlacementOrigin>,
) -> String {
    let same = |existing: &Footprint| -> bool {
        existing.placement_origin == placement_origin
            && existing.pads.len() == pads.len()
            && existing.pads.iter().zip(pads).all(|(a, b)| {
                a.number == b.number
                    && a.shape == b.shape
                    && a.position == b.position
                    && a.size == b.size
                    && a.drill == b.drill
                    && a.layers == b.layers
                    && a.rotation == b.rotation
                    && a.hole_offset == b.hole_offset
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
    approximations: &mut Vec<String>,
    placement_notes: &mut Vec<String>,
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

    // The point the part asks a pick-and-place file to place it by, from the
    // custom fields of this one footprint.
    let (placement_origin, placement_note) =
        crate::footprint::placement_origin_of(children, &fp_name);
    // Said once per library entry: forty parts of one footprint share one note.
    if let Some(note) = placement_note {
        if !placement_notes.contains(&note) {
            placement_notes.push(note);
        }
    }

    // Use a unique fp_name based on lib_link to avoid collisions
    let library_key = if lib_link.is_empty() {
        fp_name.clone()
    } else {
        lib_link.clone()
    };

    for pad in &pads {
        if let Some(stated) = &pad.approximated_from {
            approximations.push(format!(
                "`{fp_name}` pad {} states shape `{stated}`, which this importer has no word \
                 for; it is read as the rectangle its `(size ...)` gives",
                pad.number
            ));
        }
    }

    // Degrees to millidegrees.
    let rotation = Rotation((angle * 1000.0).round() as i32);

    let pad_defs: Vec<PadDef> = pads
        .iter()
        .map(|p| {
            let turn = Rotation((p.angle.0 - rotation.0).rem_euclid(360_000));
            let (position, hole_offset) = p.copper_and_hole(turn);
            PadDef {
                hole_offset,
                number: p.number.clone(),
                shape: p.shape,
                position,
                size: p.size,
                drill: p.drill,
                slot: p.slot,
                layers: p.layers.clone(),
                mask_margin: None,
                // "The pad angle in the file is a board frame absolute value. If
                // it is missing, the pad is axis aligned regardless of the parent
                // footprint orientation" - KiCad's board reader,
                // `pcb_io_kicad_sexpr_parser.cpp` at a62d8cd4 (read 2026-09-26).
                // The footprint keeps a pad's turn inside itself, so the part's
                // turn comes off. A pad with no angle on a turned part is turned
                // back by the part's turn, which is what "axis aligned" means.
                rotation: turn,
            }
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
    let library_key = resolve_library_key(library, &library_key, &pad_defs, placement_origin);

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
            placement_origin,
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
    // to board-relative coords, Y up, from the board's bottom-left corner
    let position = Position(frame::board_point(board_origin_mm, pos_x, pos_y));
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

pub(crate) struct ParsedPad {
    pub(crate) number: String,
    pub(crate) shape: PadShape,
    pub(crate) local_position: Point,
    pub(crate) size: (Nm, Nm),
    pub(crate) drill: Option<Nm>,
    /// The hole's full size when KiCad wrote `(drill oval W H)`.
    pub(crate) slot: Option<(Nm, Nm)>,
    pub(crate) layers: Vec<Layer>,
    pub(crate) net_id: Option<NetId>,
    /// The mask opening this pad asks for, when it asks for its own.
    pub(crate) mask_margin: Option<Nm>,
    /// The shape the file stated, when this importer had no word for it.
    pub(crate) approximated_from: Option<String>,
    /// The third number of the pad's `(at x y angle)`, `ZERO` when the file
    /// writes none. In a footprint file it is the pad's turn inside its
    /// footprint; in a board file it is the pad's turn on the board, and the
    /// board reader takes the part's turn off it.
    pub(crate) angle: Rotation,
    /// The `(offset x y)` of the pad's `(drill ...)`, as a vector in this
    /// project's frame and the pad's own: how far KiCad puts the pad's COPPER
    /// from its hole. The pad's `(at)` is the hole - the anchor - and the
    /// copper is `at` plus this turned by the pad's angle (`PAD::ShapePos`,
    /// `pcbnew/pad.cpp`; KiCad master, read 2026-10-11).
    pub(crate) shape_offset: Option<Point>,
}

impl ParsedPad {
    /// Where the copper stands in the footprint, and where the hole is from
    /// the copper's middle, once the pad's turn inside the footprint is
    /// `rotation`.
    ///
    /// This project keeps a pad at its copper, and states the hole as an offset
    /// from it, so the two are made here from KiCad's one: the copper is the
    /// hole plus the file's offset turned, and the hole from the copper is the
    /// offset negated.
    pub(crate) fn copper_and_hole(&self, rotation: Rotation) -> (Point, Option<Point>) {
        let Some(offset) = self.shape_offset else {
            return (self.local_position, None);
        };
        let turned = cypcb_world::components::rotate_about_origin(offset, rotation.to_degrees());
        (
            Point::new(
                Nm(self.local_position.x.0 + turned.x.0),
                Nm(self.local_position.y.0 + turned.y.0),
            ),
            Some(Point::new(Nm(-offset.x.0), Nm(-offset.y.0))),
        )
    }
}

/// The corner a `roundrect` pad states, as a percentage of its short side.
///
/// KiCad writes `(roundrect_rratio 0.25)` inside the pad, and this read 25 for
/// every rounded pad whatever the file said. Measured on the KiCad files in
/// this repository: **427 pads state 0.25 and four state 0.2**, and those four
/// came back with corners a fifth larger than the ones their board was drawn
/// with. A pad's corner is the copper nearest its neighbour, so it is the
/// figure a clearance is measured from.
///
/// A pad that states no ratio keeps the 25% this reader has always used, which
/// is a fallback rather than a reading: nothing in the file says it.
///
/// Half the short side is a stadium and there is nothing past it to draw, so a
/// larger ratio is held to half - by `PadShape::round_rect`, which is why the
/// shape is built through it rather than by naming the variant.
fn corner_ratio(elements: &[Sexp]) -> u8 {
    for element in elements {
        if list_name(element).as_deref() == Some("roundrect_rratio") {
            if let Ok(list) = element.list() {
                if let Some(ratio) = list.get(1).and_then(get_f64) {
                    return (ratio * 100.0).round() as u8;
                }
            }
        }
    }
    25
}

pub(crate) fn parse_pad(
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

    // A shape this importer has no word for becomes a rectangle, and says so.
    // `trapezoid` and `custom` are the two KiCad writes that land here, and a
    // custom pad is a polygon somebody drew - reading it as its bounding size
    // is a guess about copper, not a smaller description of the same pad.
    let (shape, approximated_from) = match shape_str.as_str() {
        "rect" => (PadShape::Rect, None),
        "circle" => (PadShape::Circle, None),
        "oval" => (PadShape::Oblong, None),
        "roundrect" => (PadShape::round_rect(corner_ratio(elements)), None),
        other => (PadShape::Rect, Some(other.to_string())),
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
    let mut mask_margin: Option<Nm> = None;
    let mut angle = Rotation::ZERO;
    let mut shape_offset: Option<Point> = None;

    for prop in &elements[3..] {
        if let Some(name) = list_name(prop) {
            match name.as_str() {
                "at" => {
                    if let Ok(list) = prop.list() {
                        if list.len() >= 3 {
                            let x = coordinate(&list[1], "pad position x")?;
                            let y = coordinate(&list[2], "pad position y")?;
                            local_pos = frame::local_point(x, y);
                        }
                        if list.len() >= 4 {
                            let degrees = coordinate(&list[3], "pad angle")?;
                            angle = Rotation((degrees * 1000.0).round() as i32);
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
                // Either form may carry `(offset x y)`. It moves the pad's
                // COPPER off the hole, not the hole off the pad: `(at)` stays
                // the hole and the copper stands at `at` plus the offset turned
                // by the pad's angle (`PAD::ShapePos` in `pcbnew/pad.cpp`;
                // KiCad master, read 2026-10-11). Kept here as read and made
                // into a position and a hole offset by `copper_and_hole`. This
                // comment used to say it was "read where the pad is placed";
                // nothing read it anywhere.
                "drill" => {
                    if let Ok(list) = prop.list() {
                        for item in list.iter().skip(1) {
                            if list_name(item).as_deref() == Some("offset") {
                                if let Ok(pair) = item.list() {
                                    if pair.len() >= 3 {
                                        let x = coordinate(&pair[1], "pad drill offset x")?;
                                        let y = coordinate(&pair[2], "pad drill offset y")?;
                                        let moved = frame::local_point(x, y);
                                        if moved != Point::ORIGIN {
                                            shape_offset = Some(moved);
                                        }
                                    }
                                }
                            }
                        }
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
                // How far this pad's mask opening runs past its copper, when
                // the pad states its own instead of taking the board's. KiCad
                // writes `(solder_mask_margin 0.1016)` - 4 mil - and a
                // through-hole connector's pads carry it so the mask does not
                // creep onto copper a hand-soldered joint has to wet. Dropped
                // until now: 124 pads of the 2623 in this repository's KiCad
                // files state one, and every one of them was exported with the
                // board's figure instead.
                "solder_mask_margin" => {
                    if let Ok(list) = prop.list() {
                        if let Some(mm) = list.get(1).and_then(get_f64) {
                            mask_margin = Some(Nm::from_mm(mm));
                        }
                    }
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
        mask_margin,
        approximated_from,
        angle,
        shape_offset,
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
/// and KiCad's is a polygon, so a zone whose outline is an axis-aligned
/// rectangle comes across as itself and anything else is kept as the ring of
/// corners the file states - see `outlined` - and never flattened to its
/// bounding box. A bounding box is not a conservative approximation of an
/// L-shaped pour - it is copper where the designer put none, and it would be
/// wrong in exactly the places the shape was drawn to avoid.
fn parse_zone(
    elem: &Sexp,
    net_map: &NetIndex,
    origin: (f64, f64),
    copper_count: u8,
) -> Result<ZoneImport, KicadPcbError> {
    let Ok(list) = elem.list() else {
        return Ok(ZoneImport::Skipped);
    };

    let mut net_id: Option<NetId> = None;
    let mut net_name = String::new();
    let mut zone_name: Option<String> = None;
    let mut layer_mask: u32 = 0;
    let mut points: Vec<(f64, f64)> = Vec::new();
    let mut filled_points: Vec<(f64, f64)> = Vec::new();
    let mut layers_named: Vec<Layer> = Vec::new();
    let mut layer_strings: Vec<String> = Vec::new();
    let mut is_keepout = false;
    let mut keepout_kinds = KeepoutKinds::NONE;
    let mut is_teardrop = false;

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
                        let text = get_string(layer_sexp).unwrap_or_default();
                        let mut layers = Vec::new();
                        if !zone_layer_set(&text, copper_count, &mut layers) {
                            parse_layer_names(&text, &mut layers);
                        }
                        layer_strings.push(text);
                        for layer in layers {
                            layer_mask |= layer.to_copper_mask();
                            layers_named.push(layer);
                        }
                    }
                }
            }
            // The zone's own name, which KiCad does keep and this reader did
            // not read: a pour called `gnd_pour` came back called `GND`,
            // because the fallback is the net it fills. The writer has been
            // putting the name here all along, so a design's pour went out
            // named and came home renamed.
            Some("name") => {
                if let Ok(name_list) = child.list() {
                    if name_list.len() >= 2 {
                        let text = get_string(&name_list[1]).unwrap_or_default();
                        if !text.trim().is_empty() {
                            zone_name = Some(text);
                        }
                    }
                }
            }
            // A KiCad "rule area": copper is kept out rather than poured.
            Some("keepout") => {
                is_keepout = true;
                keepout_kinds = rule_area_kinds(child);
            }
            // `(attr (teardrop (type padvia)))`: the zone is a teardrop, which
            // KiCad stores as copper on its track's layer and net
            // (`pcb_io_kicad_sexpr_parser.cpp`, read 2026-10-11).
            Some("attr") => {
                if let Ok(attr) = child.list() {
                    if attr
                        .iter()
                        .skip(1)
                        .any(|item| list_name(item).as_deref() == Some("teardrop"))
                    {
                        is_teardrop = true;
                    }
                }
            }
            // The copper the zone is filled with, taken when the zone states no
            // outline of its own.
            Some("filled_polygon") if filled_points.is_empty() => {
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
                                let x = coordinate(&pt_list[1], "zone filled x")?;
                                let y = coordinate(&pt_list[2], "zone filled y")?;
                                filled_points.push(frame::board_mm(origin, x, y));
                            }
                        }
                    }
                }
            }
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
                                points.push(frame::board_mm(origin, x, y));
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

    if is_teardrop {
        // On a copper layer it is copper; on a mask layer it is the opening KiCad
        // cuts over that copper.
        if layer_mask == 0 {
            return Ok(ZoneImport::MaskTeardrop);
        }
        let outline_points = if points.is_empty() {
            &filled_points
        } else {
            &points
        };
        let (Some(layer), Some(net)) = (
            layers_named.iter().copied().find(|layer| layer.is_copper()),
            net_id,
        ) else {
            return Ok(ZoneImport::Refused(format!(
                "a teardrop zone with no net or no copper layer carries nothing ({label})"
            )));
        };
        if outline_points.len() < 3 {
            return Ok(ZoneImport::Refused(format!(
                "a teardrop zone with {} points is not a polygon ({label})",
                outline_points.len()
            )));
        }
        let outline = outline_points
            .iter()
            .map(|(x, y)| Point::from_mm(*x, *y))
            .collect();
        return Ok(ZoneImport::Teardrop {
            outline,
            layer,
            net,
        });
    }

    // A rule area is a keepout for what it does not allow, and for that alone.
    // One that allows tracks, vias, pads, copper pour and footprints alike
    // keeps nothing out: KiCad uses it for placement rules, and carrying it as
    // a keepout would flag every track that crosses it.
    if is_keepout && keepout_kinds.is_empty() {
        return Ok(ZoneImport::Refused(format!(
            "{label} is a rule area that allows tracks, vias, pads, copper pour and footprints, \
             so it keeps nothing out"
        )));
    }

    if layer_mask == 0 {
        let named = layer_strings
            .iter()
            .map(|text| format!("\"{text}\""))
            .collect::<Vec<_>>()
            .join(" ");
        return Ok(ZoneImport::Refused(format!(
            "{label} names no copper layer ({named}), so there is nothing to pour it on"
        )));
    }

    let Some(bounds) = axis_aligned_rectangle(&points) else {
        return Ok(outlined(
            &points,
            is_keepout.then_some(keepout_kinds),
            layer_mask,
            net_id,
            net_name,
            zone_name,
            &label,
        ));
    };

    if is_keepout {
        return Ok(ZoneImport::Carried(
            Zone::keepout(bounds, layer_mask),
            partial(keepout_kinds),
        ));
    }

    match net_id {
        // The name the file states, or the net it fills. KiCad writes no name
        // for a pour nobody has named, and those have always arrived under
        // their net's name.
        Some(net) => Ok(ZoneImport::Carried(
            Zone::copper_pour_for_net(bounds, layer_mask, net)
                .with_name(zone_name.unwrap_or(net_name)),
            None,
        )),
        // A pour with no net cannot be filled or checked, and KiCad does write
        // them - an unassigned pour is a common half-finished state.
        None => Ok(ZoneImport::Refused(format!(
            "{label} is poured to no net, so nothing connects to it"
        ))),
    }
}

/// A zone whose outline is not a rectangle.
///
/// Its corners go into the board as they are. A keepout is a promise about
/// where nothing goes, and a point in or out of the ring is all the router and
/// the checker ask of it. A pour is copper cut from the ring, minus the
/// clearance round foreign copper, the way a rectangle's is cut from its box;
/// it is never read as its bounding box, which would be copper where the
/// designer cut it away.
fn outlined(
    points: &[(f64, f64)],
    keepout: Option<KeepoutKinds>,
    layer_mask: u32,
    net_id: Option<NetId>,
    net_name: String,
    zone_name: Option<String>,
    label: &str,
) -> ZoneImport {
    let corners: Vec<Point> = points.iter().map(|(x, y)| Point::from_mm(*x, *y)).collect();
    let Some(outline) = ZoneOutline::new(corners) else {
        return ZoneImport::Refused(format!(
            "{label} is a {}-point outline that encloses no area",
            points.len()
        ));
    };

    if let Some(kinds) = keepout {
        return ZoneImport::Outlined {
            zone: Zone::keepout(outline.bounds(), layer_mask),
            outline,
            kinds: partial(kinds),
        };
    }

    let Some(net) = net_id else {
        return ZoneImport::Refused(format!(
            "{label} is poured to no net, so nothing connects to it"
        ));
    };
    // A ring that crosses itself has no one area: the even-odd reading and the
    // winding one are different shapes, and copper cut from it would be one of
    // them by accident. KiCad fills such a zone by its own repair of the ring;
    // this importer does not guess which.
    if !outline.is_simple() {
        return ZoneImport::Refused(format!(
            "{label} is a {}-point outline that crosses itself, so the copper it \
             pours is not one shape and none is cut from it",
            points.len()
        ));
    }
    ZoneImport::Outlined {
        zone: Zone::copper_pour_for_net(outline.bounds(), layer_mask, net)
            .with_name(zone_name.unwrap_or(net_name)),
        outline,
        kinds: None,
    }
}

/// Where teardrop copper read from a file starts in the order a board writes
/// its copper in: after every teardrop the builder makes.
const FILE_TEARDROP_ORDER: u32 = 1 << 30;

/// What became of one `(zone ...)`.
pub enum ZoneImport {
    /// Carried into the board. A keepout that forbids only some kinds comes
    /// with them; `None` is a pour, or a keepout that forbids all five.
    Carried(Zone, Option<KeepoutKinds>),
    /// Carried into the board as the ring of corners it is, not as a
    /// rectangle.
    Outlined {
        zone: Zone,
        outline: ZoneOutline,
        kinds: Option<KeepoutKinds>,
    },
    /// A teardrop zone on a copper layer: a polygon, the layer it is on and the
    /// net it carries.
    Teardrop {
        outline: Vec<Point>,
        layer: Layer,
        net: NetId,
    },
    /// A teardrop zone on a solder mask layer, counted and said once.
    MaskTeardrop,
    /// Not carried, and why - reported rather than dropped in silence.
    Refused(String),
    /// Not a zone worth a word.
    Skipped,
}

/// The board's name, out of the file's title block.
///
/// `(title_block (title "blink"))` is where pcbnew puts the name a person
/// reads, and this project writes the design's own name there. An empty title
/// is no title: KiCad writes the node with blank fields on a board nobody has
/// named.
fn extract_title(elements: &[Sexp]) -> Option<String> {
    for elem in elements {
        if list_name(elem).as_deref() != Some("title_block") {
            continue;
        }
        let Ok(list) = elem.list() else { continue };
        for child in &list[1..] {
            if list_name(child).as_deref() != Some("title") {
                continue;
            }
            let Ok(title) = child.list() else { continue };
            if title.len() < 2 {
                continue;
            }
            let text = get_string(&title[1]).unwrap_or_default();
            if !text.trim().is_empty() {
                return Some(text);
            }
        }
    }
    None
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

/// A track segment as the file states it. `net` is `None` for `(net 0)`, the
/// unconnected net, and for a net number the file never named.
struct FileSegment {
    net: Option<NetId>,
    layer: Layer,
    width: Nm,
    start: Point,
    end: Point,
}

/// A via as the file states it, with the same `net`.
struct FileVia {
    net: Option<NetId>,
    position: Point,
    drill: Nm,
    outer_diameter: Option<Nm>,
    start_layer: Layer,
    end_layer: Layer,
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
    refusals: &mut Vec<String>,
) -> Result<Option<FileSegment>, KicadPcbError> {
    let list = match sexp.list() {
        Ok(l) => l,
        Err(_) => return Ok(None),
    };

    let mut start: Option<Point> = None;
    let mut end: Option<Point> = None;
    let mut width = Nm::from_mm(0.25); // Default trace width
    let mut layer = Layer::TopCopper;
    // Set when the file names a layer this parser has no word for. The copper
    // is then refused rather than moved, because moving it is silent and a
    // refusal is not.
    let mut unknown_layer: Option<String> = None;
    let mut net: Option<NetId> = None;

    for child in &list[1..] {
        if let Some(name) = list_name(child) {
            match name.as_str() {
                "start" => {
                    if let Ok(sub) = child.list() {
                        if sub.len() >= 3 {
                            let x = coordinate(&sub[1], "segment start x")?;
                            let y = coordinate(&sub[2], "segment start y")?;
                            start = Some(frame::board_point(origin, x, y));
                        }
                    }
                }
                "end" => {
                    if let Ok(sub) = child.list() {
                        if sub.len() >= 3 {
                            let x = coordinate(&sub[1], "segment end x")?;
                            let y = coordinate(&sub[2], "segment end y")?;
                            end = Some(frame::board_point(origin, x, y));
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
                            match parse_layer_name(&l) {
                                Some(known) => layer = known,
                                None => unknown_layer = Some(l),
                            }
                        }
                    }
                }
                "net" => {
                    if let Some(id) = kicad_net_map.resolve(child) {
                        net = Some(id);
                    }
                }
                _ => {}
            }
        }
    }

    if let Some(named) = unknown_layer {
        refusals.push(format!(
            "a track segment on layer '{named}', which this importer has no word for - the copper is left out rather than moved to another layer"
        ));
        return Ok(None);
    }

    Ok(match (start, end) {
        (Some(start), Some(end)) => Some(FileSegment {
            net,
            layer,
            width,
            start,
            end,
        }),
        _ => None,
    })
}

/// Read one `(arc ...)` - a track arc, which KiCad states as three points on
/// the curve.
///
/// Comes back as the copper this model holds - the chords the curve flattens
/// into - and the curve itself, so a board that arrives with a curve leaves
/// with one.
#[allow(clippy::type_complexity)]
fn parse_track_arc(
    sexp: &Sexp,
    kicad_net_map: &NetIndex,
    origin: (f64, f64),
    refusals: &mut Vec<String>,
) -> Result<
    Option<(
        cypcb_world::components::trace::Trace,
        cypcb_world::components::trace::Curve,
    )>,
    KicadPcbError,
> {
    let list = match sexp.list() {
        Ok(l) => l,
        Err(_) => return Ok(None),
    };

    let mut start: Option<Point> = None;
    let mut mid: Option<Point> = None;
    let mut end: Option<Point> = None;
    let mut width = Nm::from_mm(0.25);
    let mut layer = Layer::TopCopper;
    // Set when the file names a layer this parser has no word for. The copper
    // is then refused rather than moved, because moving it is silent and a
    // refusal is not.
    let mut unknown_layer: Option<String> = None;
    let mut net_id: Option<NetId> = None;

    for child in &list[1..] {
        let Some(name) = list_name(child) else {
            continue;
        };
        match name.as_str() {
            "start" | "mid" | "end" => {
                if let Ok(sub) = child.list() {
                    if sub.len() >= 3 {
                        let x = coordinate(&sub[1], "arc x")?;
                        let y = coordinate(&sub[2], "arc y")?;
                        let point = frame::board_point(origin, x, y);
                        match name.as_str() {
                            "start" => start = Some(point),
                            "mid" => mid = Some(point),
                            _ => end = Some(point),
                        }
                    }
                }
            }
            "width" => {
                if let Ok(sub) = child.list() {
                    if sub.len() >= 2 {
                        width = Nm::from_mm(get_f64(&sub[1]).unwrap_or(0.25));
                    }
                }
            }
            "layer" => {
                if let Ok(sub) = child.list() {
                    if sub.len() >= 2 {
                        let named = get_string(&sub[1]).unwrap_or_default();
                        match parse_layer_name(&named) {
                            Some(known) => layer = known,
                            None => unknown_layer = Some(named),
                        }
                    }
                }
            }
            "net" => {
                if let Some(id) = kicad_net_map.resolve(child) {
                    net_id = Some(id);
                }
            }
            _ => {}
        }
    }

    if let Some(named) = unknown_layer {
        refusals.push(format!(
            "a track arc on layer '{named}', which this importer has no word for - the copper is left out rather than moved to another layer"
        ));
        return Ok(None);
    }

    let (Some(start), Some(mid), Some(end)) = (start, mid, end) else {
        return Ok(None);
    };
    // Three points that are not on one circle describe no arc. Refusing beats
    // spawning copper along a curve of radius near infinity.
    let Some(arc) = cypcb_world::arc::Arc::through(start, mid, end) else {
        return Ok(None);
    };

    let points = arc.flatten(cypcb_world::arc::Arc::DEFAULT_TOLERANCE);
    let segments: Vec<cypcb_world::components::trace::TraceSegment> = points
        .windows(2)
        .map(|pair| cypcb_world::components::trace::TraceSegment::new(pair[0], pair[1]))
        .collect();
    if segments.is_empty() {
        return Ok(None);
    }

    Ok(Some((
        cypcb_world::components::trace::Trace {
            segments,
            width,
            layer,
            net_id,
            locked: false,
            source: cypcb_world::components::trace::TraceSource::Manual,
        },
        cypcb_world::components::trace::Curve {
            centre: arc.centre,
            sweep_millideg: arc.sweep_millideg,
        },
    )))
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
    refusals: &mut Vec<String>,
) -> Result<Option<FileVia>, KicadPcbError> {
    let list = match sexp.list() {
        Ok(l) => l,
        Err(_) => return Ok(None),
    };

    let mut position: Option<Point> = None;
    let mut drill = Nm::from_mm(0.3); // Default drill
                                      // The ring the file states. KiCad writes `(size ...)` on every via and this
                                      // parser used to skip it, so a via came back with whatever `drill * 2`
                                      // happened to give - right for a 0.6/0.3 via by coincidence, wrong for
                                      // every other ring. `None` here means the file really said nothing, and the
                                      // router's own 2:1 is then the honest fallback rather than a guess about
                                      // somebody else's board.
    let mut outer_diameter: Option<Nm> = None;
    // As in the two track parsers: a layer name this importer has no word for
    // refuses the via rather than moving it.
    let mut unknown_layer: Option<String> = None;
    let mut start_layer = Layer::TopCopper;
    let mut end_layer = Layer::BottomCopper;
    let mut net: Option<NetId> = None;

    for child in &list[1..] {
        if let Some(name) = list_name(child) {
            match name.as_str() {
                "at" => {
                    if let Ok(sub) = child.list() {
                        if sub.len() >= 3 {
                            let x = coordinate(&sub[1], "via position x")?;
                            let y = coordinate(&sub[2], "via position y")?;
                            position = Some(frame::board_point(origin, x, y));
                        }
                    }
                }
                "size" => {
                    if let Ok(sub) = child.list() {
                        if sub.len() >= 2 {
                            if let Some(d) = get_f64(&sub[1]) {
                                outer_diameter = Some(Nm::from_mm(d));
                            }
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
                            // A via spans two named layers and both have to be
                            // known. Defaulting either one moved the hole to a
                            // layer pair the file never stated, which changes
                            // what the via connects - and `ViaSpanRule` grades
                            // a board on exactly that span.
                            match (parse_layer_name(&l1), parse_layer_name(&l2)) {
                                (Some(a), Some(b)) => {
                                    start_layer = a;
                                    end_layer = b;
                                }
                                (None, _) => unknown_layer = Some(l1),
                                (_, None) => unknown_layer = Some(l2),
                            }
                        }
                    }
                }
                "net" => {
                    if let Some(id) = kicad_net_map.resolve(child) {
                        net = Some(id);
                    }
                }
                _ => {}
            }
        }
    }

    if let Some(named) = unknown_layer {
        refusals.push(format!(
            "a via spanning layer '{named}', which this importer has no word for \
             - the hole is left out rather than moved to another layer pair"
        ));
        return Ok(None);
    }

    Ok(position.map(|position| FileVia {
        net,
        position,
        drill,
        outer_diameter,
        start_layer,
        end_layer,
    }))
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

/// What a `(keepout ...)` node forbids: each of tracks, vias, pads, copper
/// pour and footprints that is `not_allowed`, and no other.
///
/// A restriction the node does not state is allowed, as KiCad reads it. A node
/// that is not a list is taken to forbid everything, the safe reading of a
/// rule area this reader cannot make out.
fn rule_area_kinds(keepout: &Sexp) -> KeepoutKinds {
    let Ok(list) = keepout.list() else {
        return KeepoutKinds::ALL;
    };
    let mut kinds = Vec::new();
    for restriction in &list[1..] {
        let Ok(parts) = restriction.list() else {
            continue;
        };
        if parts.len() < 2 || get_string(&parts[1]).as_deref() != Some("not_allowed") {
            continue;
        }
        match get_string(&parts[0]).as_deref() {
            Some("tracks") => kinds.push(KeepoutKind::Tracks),
            Some("vias") => kinds.push(KeepoutKind::Vias),
            Some("pads") => kinds.push(KeepoutKind::Pads),
            Some("copperpour") => kinds.push(KeepoutKind::Pour),
            Some("footprints") => kinds.push(KeepoutKind::Footprints),
            _ => {}
        }
    }
    KeepoutKinds::of_kinds(&kinds)
}

/// The kinds a keepout carries as a component: `None` when it forbids all five,
/// which is what a keepout with no component means.
fn partial(kinds: KeepoutKinds) -> Option<KeepoutKinds> {
    (!kinds.is_all()).then_some(kinds)
}

/// The copper layers a zone's layer string stands for, when it is one of KiCad's
/// sets: `F&B.Cu` is the two faces, `*.Cu` every copper layer of the board and
/// `*In.Cu` the ones between the faces. Returns whether the string was one.
///
/// A pad has its own copper mask and this reader's wildcard list stops at the
/// faces for it, but a zone has no such mask: what it names is what it is on.
/// KiCad reads the same three strings the same way (`m_layerMasks` in its
/// s-expression parser), and `copper_count` is the board's, from the file's own
/// `(layers ...)` header, because `*.Cu` means the layers the board has.
fn zone_layer_set(name: &str, copper_count: u8, layers: &mut Vec<Layer>) -> bool {
    let inner = (0..copper_count.saturating_sub(2)).map(Layer::Inner);
    match name {
        "F&B.Cu" => layers.extend([Layer::TopCopper, Layer::BottomCopper]),
        "*.Cu" => {
            layers.extend([Layer::TopCopper, Layer::BottomCopper]);
            layers.extend(inner);
        }
        "*In.Cu" => layers.extend(inner),
        _ => return false,
    }
    true
}

/// Parse a KiCad layer name, handling wildcards like "*.Cu" and "*.Mask".
/// Appends matching layers to the `layers` vec.
fn parse_layer_names(name: &str, layers: &mut Vec<Layer>) {
    match name {
        // KiCad's "all of the copper layers". The list only has the two faces
        // to name, because this reader does not know the layer count; the
        // copper a plated pad has on the inner layers comes from
        // `PadDef::copper_mask`, not from this list, and every reader of pad
        // copper asks that.
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
pub(crate) fn list_name(sexp: &Sexp) -> Option<String> {
    if let Ok(list) = sexp.list() {
        if !list.is_empty() {
            return get_string(&list[0]);
        }
    }
    None
}

/// Extract a string from a Sexp.
pub(crate) fn get_string(sexp: &Sexp) -> Option<String> {
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
pub(crate) fn find_xy_child(sexp: &Sexp, child_name: &str) -> Option<(f64, f64)> {
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
        // Its sides along the footprint's axes, once its own turn is taken up.
        let (width, height) = pad.outline(Point::ORIGIN, Rotation::ZERO).size;
        let half_w = width.0 / 2;
        let half_h = height.0 / 2;

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

/// What Edge.Cuts holds, read the way KiCad reads it.
///
/// A bounding box is enough to say how big a board is and wrong for saying
/// where its edge runs. The edge is a set of closed shapes: loose `gr_line`,
/// `gr_arc` and `gr_curve` pieces walked end to end, and `gr_rect`, `gr_poly`
/// and `gr_circle`, closed on their own. KiCad (`doConvertOutlineToPolygon`
/// and `buildContourHierarchy` in `convert_shape_list_to_polygon.cpp`)
/// counts, for each shape, the shapes that contain its first point: an even
/// count is a board outline, an odd count is a hole in the shape around it.
///
/// The ring is the largest outer shape. A rectangle (`gr_rect`) is its own
/// bounding box and is no ring, as before. An arc or a circle is sampled to a
/// polygon, so every reader of the outline keeps reading a polygon, and
/// remembered as the arc or circle it is, so the board file can say so again.
/// A shape with a bezier curve in it is neither ring nor cutout: it takes part
/// in the counting, so a cutout inside it is still a cutout, and it is counted
/// in `left_out`. A chain that never closes is ignored.
struct EdgeShapes {
    /// The outer shape the board is cut along.
    ring: Option<EdgeRing>,
    /// The holes, in the order the file gave them.
    cutouts: Vec<EdgeRing>,
    /// Closed shapes with a bezier curve in them.
    left_out: usize,
    /// Outer shapes beyond the largest.
    extra_outers: usize,
}

/// A box in file millimetres: min x, min y, max x, max y.
type EdgeBox = (f64, f64, f64, f64);

/// A closed shape as a polygon in file millimetres, and what it was drawn as.
struct EdgeRing {
    points: Vec<(f64, f64)>,
    /// The runs of `points` that are one arc each.
    arcs: Vec<EdgeArc>,
    /// The centre and radius, when the whole shape is one circle.
    circle: Option<((f64, f64), f64)>,
    /// The box of the drawn shape: min x, min y, max x, max y. An arc reaches
    /// past its sample points, so this is not the box of `points`.
    bounds: EdgeBox,
}

/// `points[first..=last]` are one arc, from the point at `first` through `mid`
/// to the point at `last`. `last` equal to the number of points means the
/// first point again.
struct EdgeArc {
    first: usize,
    last: usize,
    mid: (f64, f64),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EdgeKind {
    /// Lines, arcs and circles walked into a loop, or a `gr_poly`: a ring this
    /// reader carries.
    Plain,
    /// A `gr_rect`.
    Rect,
    /// Anything with a bezier curve in it.
    Curved,
}

struct EdgeContour {
    kind: EdgeKind,
    ring: EdgeRing,
    /// The place in the file of the first piece, so the shapes keep the order
    /// the file draws them in whichever way they are drawn.
    at: usize,
}

enum PieceShape {
    Line,
    /// An arc as the file states it: the two ends and a point between.
    Arc {
        start: (f64, f64),
        mid: (f64, f64),
        end: (f64, f64),
    },
    Curve,
}

/// One open piece of an edge. An arc is sampled when the loop it is part of is
/// put together, because its ends are then the points of the loop.
struct EdgePiece {
    /// For a line or a curve the points along it; for an arc the two ends.
    points: Vec<(f64, f64)>,
    shape: PieceShape,
    at: usize,
}

/// How far from the drawn arc a sample may lie, in millimetres: KiCad's
/// `ARC_HIGH_DEF_MM` (`base_units.h`), which `BOARD_DESIGN_SETTINGS` takes as
/// the default `m_MaxError` the board outline is built with.
const EDGE_MAX_ERROR_MM: f64 = 0.005;

/// A micrometre: KiCad writes millimetres with six decimals, which is enough
/// that two endpoints meant to touch land within it, but not enough to rely on
/// them landing on the same nanometre.
const EDGE_JOIN_TOLERANCE_MM: f64 = 0.001;

fn union_bounds(a: EdgeBox, b: EdgeBox) -> EdgeBox {
    (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3))
}

fn bounds_of(points: &[(f64, f64)]) -> EdgeBox {
    points
        .iter()
        .fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |b, p| {
            (b.0.min(p.0), b.1.min(p.1), b.2.max(p.0), b.3.max(p.1))
        })
}

const NO_BOUNDS: EdgeBox = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);

fn extract_edge_shapes(elements: &[Sexp]) -> EdgeShapes {
    let touches = |a: (f64, f64), b: (f64, f64)| -> bool {
        (a.0 - b.0).abs() <= EDGE_JOIN_TOLERANCE_MM + 1e-9
            && (a.1 - b.1).abs() <= EDGE_JOIN_TOLERANCE_MM + 1e-9
    };

    let mut pieces: Vec<EdgePiece> = Vec::new();
    let mut contours: Vec<EdgeContour> = Vec::new();

    for (at, elem) in elements.iter().enumerate() {
        let Some(name) = list_name(elem) else {
            continue;
        };
        if !matches!(
            name.as_str(),
            "gr_line" | "gr_arc" | "gr_curve" | "gr_circle" | "gr_rect" | "gr_poly"
        ) || !is_on_edge_cuts(elem)
        {
            continue;
        }
        match name.as_str() {
            "gr_line" => {
                if let (Some(start), Some(end)) =
                    (find_xy_child(elem, "start"), find_xy_child(elem, "end"))
                {
                    pieces.push(EdgePiece {
                        points: vec![start, end],
                        shape: PieceShape::Line,
                        at,
                    });
                }
            }
            "gr_arc" => {
                if let Some((start, mid, end)) = arc_ends(elem) {
                    pieces.push(EdgePiece {
                        points: vec![start, end],
                        shape: PieceShape::Arc { start, mid, end },
                        at,
                    });
                }
            }
            "gr_curve" => {
                let points = points_of(elem);
                if points.len() >= 2 {
                    pieces.push(EdgePiece {
                        points,
                        shape: PieceShape::Curve,
                        at,
                    });
                }
            }
            "gr_circle" => {
                if let (Some(center), Some(end)) =
                    (find_xy_child(elem, "center"), find_xy_child(elem, "end"))
                {
                    // A nanometre, so a circle written back and read again is
                    // the same circle and samples to the same points.
                    let radius = ((end.0 - center.0).hypot(end.1 - center.1) * 1e6).round() / 1e6;
                    contours.push(EdgeContour {
                        kind: EdgeKind::Plain,
                        ring: EdgeRing {
                            points: circle_polyline(center, radius),
                            arcs: Vec::new(),
                            circle: Some((center, radius)),
                            bounds: (
                                center.0 - radius,
                                center.1 - radius,
                                center.0 + radius,
                                center.1 + radius,
                            ),
                        },
                        at,
                    });
                }
            }
            "gr_rect" => {
                if let (Some(a), Some(b)) =
                    (find_xy_child(elem, "start"), find_xy_child(elem, "end"))
                {
                    let points = vec![(a.0, a.1), (b.0, a.1), (b.0, b.1), (a.0, b.1)];
                    contours.push(EdgeContour {
                        kind: EdgeKind::Rect,
                        ring: EdgeRing {
                            bounds: bounds_of(&points),
                            points,
                            arcs: Vec::new(),
                            circle: None,
                        },
                        at,
                    });
                }
            }
            _ => {
                let points = points_of(elem);
                if points.len() >= 3 {
                    contours.push(EdgeContour {
                        kind: EdgeKind::Plain,
                        ring: EdgeRing {
                            bounds: bounds_of(&points),
                            points,
                            arcs: Vec::new(),
                            circle: None,
                        },
                        at,
                    });
                }
            }
        }
    }

    // Walk the open pieces into loops. From the first piece not yet used, on
    // from its end to whatever touches it, then back from its start; a piece
    // is taken whichever way round it lies.
    let ends = |piece: &EdgePiece, reversed: bool| -> ((f64, f64), (f64, f64)) {
        let (head, tail) = (piece.points[0], piece.points[piece.points.len() - 1]);
        if reversed {
            (tail, head)
        } else {
            (head, tail)
        }
    };
    let curved = |piece: &EdgePiece| matches!(piece.shape, PieceShape::Curve);
    let enough = |members: &std::collections::VecDeque<(usize, bool)>, pieces: &[EdgePiece]| {
        members.len() >= 3
            || (members.len() >= 2
                && members
                    .iter()
                    .any(|&(i, _)| !matches!(pieces[i].shape, PieceShape::Line)))
    };
    let mut used = vec![false; pieces.len()];
    for first in 0..pieces.len() {
        if used[first] {
            continue;
        }
        used[first] = true;
        let mut members: std::collections::VecDeque<(usize, bool)> =
            std::collections::VecDeque::from([(first, false)]);
        let start = ends(&pieces[first], false).0;
        let mut closed = false;
        loop {
            let &(last, reversed) = members.back().expect("a member");
            let cursor = ends(&pieces[last], reversed).1;
            if touches(cursor, start) && enough(&members, &pieces) {
                closed = true;
                break;
            }
            let next = (0..pieces.len()).find_map(|i| {
                if used[i] {
                    return None;
                }
                let (head, tail) = ends(&pieces[i], false);
                if touches(head, cursor) {
                    Some((i, false))
                } else if touches(tail, cursor) {
                    Some((i, true))
                } else {
                    None
                }
            });
            let Some((i, reversed)) = next else {
                break;
            };
            used[i] = true;
            members.push_back((i, reversed));
        }
        // Walked to a dead end: the rest may lie before the start.
        while !closed {
            let &(head_piece, head_reversed) = members.front().expect("a member");
            let head = ends(&pieces[head_piece], head_reversed).0;
            let next = (0..pieces.len()).find_map(|i| {
                if used[i] {
                    return None;
                }
                let (h, t) = ends(&pieces[i], false);
                if touches(t, head) {
                    Some((i, false))
                } else if touches(h, head) {
                    Some((i, true))
                } else {
                    None
                }
            });
            let Some((i, reversed)) = next else {
                break;
            };
            used[i] = true;
            members.push_front((i, reversed));
            let &(last, last_reversed) = members.back().expect("a member");
            let &(front, front_reversed) = members.front().expect("a member");
            if touches(
                ends(&pieces[last], last_reversed).1,
                ends(&pieces[front], front_reversed).0,
            ) && enough(&members, &pieces)
            {
                closed = true;
            }
        }
        if !closed {
            continue;
        }

        // The loop as one polygon, and where its arcs run along it. Each arc is
        // sampled from the point the loop is at to the point it goes on from,
        // which are the points of the loop and not quite the file's own when
        // the file leaves a gap of a few nanometres: the arc written back from
        // the polygon is then the arc that was sampled.
        let mut points: Vec<(f64, f64)> = Vec::new();
        let mut arcs = Vec::new();
        let mut lines_box = NO_BOUNDS;
        let mut arcs_box = NO_BOUNDS;
        let count = members.len();
        for (k, &(i, reversed)) in members.iter().enumerate() {
            let piece = &pieces[i];
            let from = points.len().saturating_sub(1);
            match piece.shape {
                PieceShape::Arc { start, mid, end } => {
                    let (mut s, mut e) = if reversed { (end, start) } else { (start, end) };
                    if k > 0 {
                        s = points[points.len() - 1];
                    } else {
                        // The loop closes on a straight piece: the arc starts
                        // where that piece ends.
                        let &(back, back_reversed) = &members[count - 1];
                        if !matches!(pieces[back].shape, PieceShape::Arc { .. }) {
                            s = ends(&pieces[back], back_reversed).1;
                        }
                    }
                    if k + 1 == count {
                        e = points[0];
                    } else {
                        // Where a straight piece takes up after the arc, the
                        // arc ends at the start of that piece, which is the
                        // point of the file the line is drawn from.
                        let &(next, next_reversed) = &members[k + 1];
                        if !matches!(pieces[next].shape, PieceShape::Arc { .. }) {
                            e = ends(&pieces[next], next_reversed).0;
                        }
                    }
                    let (along, abox) = arc_through(snap_to_nm(s), mid, snap_to_nm(e));
                    if k == 0 {
                        points.extend(along);
                    } else {
                        points.extend(along.into_iter().skip(1));
                    }
                    arcs.push(EdgeArc {
                        first: from,
                        last: points.len() - 1,
                        mid,
                    });
                    arcs_box = union_bounds(arcs_box, abox);
                }
                _ => {
                    let along: Vec<(f64, f64)> = if reversed {
                        piece.points.iter().rev().copied().collect()
                    } else {
                        piece.points.clone()
                    };
                    lines_box = union_bounds(lines_box, bounds_of(&along));
                    if k == 0 {
                        points.extend(along);
                    } else {
                        points.extend(along.into_iter().skip(1));
                    }
                }
            }
        }
        // The walk ends where it began.
        points.pop();
        let bounds = union_bounds(lines_box, arcs_box);
        let at = members
            .iter()
            .map(|&(i, _)| pieces[i].at)
            .min()
            .unwrap_or(0);
        let is_curved = members.iter().any(|&(i, _)| curved(&pieces[i]));
        contours.push(EdgeContour {
            kind: if is_curved {
                EdgeKind::Curved
            } else {
                EdgeKind::Plain
            },
            ring: EdgeRing {
                points,
                arcs,
                circle: None,
                bounds,
            },
            at,
        });
    }
    contours.sort_by_key(|c| c.at);

    // How many shapes contain each one's first point.
    let depth: Vec<usize> = contours
        .iter()
        .enumerate()
        .map(|(i, contour)| {
            let first = contour.ring.points[0];
            contours
                .iter()
                .enumerate()
                .filter(|(j, other)| *j != i && point_in_ring(first, &other.ring.points))
                .count()
        })
        .collect();

    let area = |c: &EdgeContour| ring_area(&c.ring.points);
    let mut outers: Vec<usize> = (0..contours.len())
        .filter(|&i| depth[i].is_multiple_of(2))
        .collect();
    // The largest first; the earlier one wins a tie.
    outers.sort_by(|&a, &b| {
        area(&contours[b])
            .partial_cmp(&area(&contours[a]))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.cmp(&b))
    });

    let mut take: Vec<Option<EdgeContour>> = contours.into_iter().map(Some).collect();
    let left_out = take
        .iter()
        .flatten()
        .filter(|c| c.kind == EdgeKind::Curved)
        .count();
    let ring = outers
        .first()
        .filter(|&&i| take[i].as_ref().is_some_and(|c| c.kind == EdgeKind::Plain))
        .and_then(|&i| take[i].take())
        .map(|c| c.ring);
    let cutouts = (0..take.len())
        .filter(|&i| depth[i] % 2 == 1)
        .filter_map(|i| take[i].take())
        .filter(|c| c.kind != EdgeKind::Curved)
        .map(|mut c| {
            // A ring is closed implicitly: a polygon that repeats its first
            // point loses the repeat.
            if c.ring.points.len() > 1 && c.ring.points.first() == c.ring.points.last() {
                c.ring.points.pop();
            }
            c.ring
        })
        .filter(|ring| ring.points.len() >= 3)
        .collect();

    EdgeShapes {
        ring,
        cutouts,
        left_out,
        extra_outers: outers.len().saturating_sub(1),
    }
}

/// The `(xy ...)` points of a shape's `(pts ...)`.
fn points_of(elem: &Sexp) -> Vec<(f64, f64)> {
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
                        points.push((x, y));
                    }
                }
            }
        }
    }
    points
}

/// The two ends of an arc and a point between them. Either the form KiCad
/// writes now, `(start) (mid) (end)`, or the older one, `(start centre) (end
/// first-point) (angle degrees)`.
#[allow(clippy::type_complexity)]
fn arc_ends(elem: &Sexp) -> Option<((f64, f64), (f64, f64), (f64, f64))> {
    let start = find_xy_child(elem, "start")?;
    let end = find_xy_child(elem, "end")?;
    if let Some(mid) = find_xy_child(elem, "mid") {
        return Some((snap_to_nm(start), snap_to_nm(mid), snap_to_nm(end)));
    }
    let angle = elem
        .list()
        .ok()?
        .iter()
        .find(|child| list_name(child).as_deref() == Some("angle"))
        .and_then(|child| {
            child
                .list()
                .ok()
                .and_then(|sub| sub.get(1).and_then(get_f64))
        })?;
    let (centre, first) = (start, end);
    let radius = (first.0 - centre.0).hypot(first.1 - centre.1);
    let t0 = (first.1 - centre.1).atan2(first.0 - centre.0);
    let sweep = angle.to_radians();
    Some((
        snap_to_nm(first),
        snap_to_nm(point_on_circle(centre, radius, t0 + sweep / 2.0)),
        snap_to_nm(point_on_circle(centre, radius, t0 + sweep)),
    ))
}

/// A point moved to the nearest nanometre. The board is kept in nanometres, so
/// an arc sampled from points that are already on that grid is the arc that
/// is sampled again from the points it is written back as.
fn snap_to_nm(point: (f64, f64)) -> (f64, f64) {
    ((point.0 * 1e6).round() / 1e6, (point.1 * 1e6).round() / 1e6)
}

/// The arc through three points, as the polygon KiCad makes of it and its
/// exact box.
fn arc_through(start: (f64, f64), mid: (f64, f64), end: (f64, f64)) -> (Vec<(f64, f64)>, EdgeBox) {
    let (ax, ay) = start;
    let (bx, by) = mid;
    let (cx, cy) = end;
    let d = 2.0 * (ax * (by - cy) + bx * (cy - ay) + cx * (ay - by));
    if d.abs() < 1e-12 {
        let points = vec![start, end];
        let bounds = bounds_of(&points);
        return (points, bounds);
    }
    let ux = ((ax * ax + ay * ay) * (by - cy)
        + (bx * bx + by * by) * (cy - ay)
        + (cx * cx + cy * cy) * (ay - by))
        / d;
    let uy = ((ax * ax + ay * ay) * (cx - bx)
        + (bx * bx + by * by) * (ax - cx)
        + (cx * cx + cy * cy) * (bx - ax))
        / d;
    let radius = (ax - ux).hypot(ay - uy);
    let angle_of = |p: (f64, f64)| (p.1 - uy).atan2(p.0 - ux);
    let (ta, tm, tb) = (angle_of(start), angle_of(mid), angle_of(end));
    let turn = |from: f64, to: f64| (to - from).rem_euclid(std::f64::consts::TAU);
    let sweep = if turn(ta, tm) < turn(ta, tb) {
        turn(ta, tb)
    } else {
        -turn(tb, ta)
    };
    // How far the middle point lies from the chord: an arc that bulges less
    // than half the error is a line.
    let chord = (end.0 - start.0).hypot(end.1 - start.1);
    let bulge = ((end.0 - start.0) * (start.1 - mid.1) - (start.0 - mid.0) * (end.1 - start.1))
        .abs()
        / chord.max(1e-12);
    (
        arc_polyline((ux, uy), radius, ta, sweep, start, end, bulge),
        arc_bounds((ux, uy), radius, ta, sweep, start, end),
    )
}

/// An arc as KiCad polygonises it (`SHAPE_ARC::ConvertToPolyline`,
/// `shape_arc.cpp`): the two ends exactly where the file puts them and, between,
/// one point at the middle of each of `n` equal steps, at the radius of the
/// circle through the ends of the steps, `r + e/2`, where `e` is the error that
/// many steps leave (`CircleToEndSegmentDeltaRadius`). We do the same, and not
/// the points on the circle itself, so that the edge a clearance is measured to
/// is the edge KiCad's own DRC measures to, within `e/2`, and the polygon errs
/// towards the copper.
fn arc_polyline(
    centre: (f64, f64),
    radius: f64,
    start_angle: f64,
    sweep: f64,
    start: (f64, f64),
    end: (f64, f64),
    bulge: f64,
) -> Vec<(f64, f64)> {
    let half_error = EDGE_MAX_ERROR_MM / 2.0;
    if radius < half_error || bulge < half_error {
        return vec![start, end];
    }
    let steps = arc_step_count(radius, sweep.to_degrees().abs());
    // Steps to the circle, whole: a sweep of exactly 180 degrees is 180.0000001
    // from one set of three points and 179.9999999 from the next, and the
    // difference must not be a different polygon.
    let per_circle = (steps as f64 * 360.0 / sweep.to_degrees().abs() + 1e-6)
        .floor()
        .max(3.0);
    let error = radius * (1.0 / (std::f64::consts::PI / per_circle).cos() - 1.0);
    let radius = radius + error / 2.0;
    let mut points = vec![start];
    for i in (1..2 * steps).step_by(2) {
        let angle = start_angle + sweep * (i as f64) / ((2 * steps) as f64);
        points.push(point_on_circle(centre, radius, angle));
    }
    points.push(end);
    points
}

/// How many steps an arc of this sweep takes (`GetArcToSegmentCount`,
/// `geometry_utils.cpp`): the largest angle a step may cover with its middle
/// no further than the error from the circle, at most 45 degrees, and at
/// least two steps.
fn arc_step_count(radius: f64, sweep_degrees: f64) -> usize {
    let increment = (2.0
        * (1.0 - EDGE_MAX_ERROR_MM / radius.max(1e-9))
            .acos()
            .to_degrees())
    .min(360.0 / 8.0);
    ((sweep_degrees / increment).round() as usize).max(2)
}

/// A whole circle as KiCad polygonises it: an arc of 360 degrees from the
/// point to the right of the centre. The closing point, the start again, is
/// left off.
fn circle_polyline(centre: (f64, f64), radius: f64) -> Vec<(f64, f64)> {
    let start = (centre.0 + radius, centre.1);
    let mut points = arc_polyline(
        centre,
        radius,
        0.0,
        std::f64::consts::TAU,
        start,
        start,
        radius,
    );
    points.pop();
    points
}

/// The exact box of an arc: its ends, and the extreme of the circle along an
/// axis where the arc passes it.
fn arc_bounds(
    centre: (f64, f64),
    radius: f64,
    start_angle: f64,
    sweep: f64,
    start: (f64, f64),
    end: (f64, f64),
) -> EdgeBox {
    let mut bounds = bounds_of(&[start, end]);
    for quarter in 0..4 {
        let axis = f64::from(quarter) * std::f64::consts::FRAC_PI_2;
        let along = if sweep >= 0.0 {
            (axis - start_angle).rem_euclid(std::f64::consts::TAU)
        } else {
            (start_angle - axis).rem_euclid(std::f64::consts::TAU)
        };
        if along <= sweep.abs() {
            let extreme = point_on_circle(centre, radius, axis);
            bounds = union_bounds(bounds, (extreme.0, extreme.1, extreme.0, extreme.1));
        }
    }
    bounds
}

/// The point at an angle round a circle. Only used to sample the edge shapes.
fn point_on_circle(centre: (f64, f64), radius: f64, angle: f64) -> (f64, f64) {
    (
        centre.0 + radius * angle.cos(),
        centre.1 + radius * angle.sin(),
    )
}

/// Whether a point is inside a ring: a ray along +x, counting crossings.
fn point_in_ring(point: (f64, f64), ring: &[(f64, f64)]) -> bool {
    let (x, y) = point;
    let mut inside = false;
    for i in 0..ring.len() {
        let (x1, y1) = ring[i];
        let (x2, y2) = ring[(i + 1) % ring.len()];
        if (y1 > y) != (y2 > y) && x < (x2 - x1) * (y - y1) / (y2 - y1) + x1 {
            inside = !inside;
        }
    }
    inside
}

/// The area a ring encloses, whichever way it is wound.
fn ring_area(ring: &[(f64, f64)]) -> f64 {
    let mut twice = 0.0;
    for i in 0..ring.len() {
        let (x1, y1) = ring[i];
        let (x2, y2) = ring[(i + 1) % ring.len()];
        twice += x1 * y2 - x2 * y1;
    }
    (twice / 2.0).abs()
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
