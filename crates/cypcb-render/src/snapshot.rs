//! Board snapshot types for JavaScript serialization.
//!
//! These types provide a flat, serializable view of the board state
//! suitable for transmission to JavaScript via serde-wasm-bindgen.
//!
//! All types use primitive types (i64, i32, u32, String) that serialize
//! cleanly to JavaScript numbers and strings.

use cypcb_drc::DrcViolation;
use cypcb_world::BoardWorld;
use serde::{Deserialize, Serialize};

/// Complete snapshot of the board state for rendering.
///
/// This is the main type returned by `PcbEngine::get_snapshot()`.
/// It contains all information needed to render the board in JavaScript.
#[derive(Debug, Serialize, Deserialize)]
pub struct BoardSnapshot {
    /// Board information (if a board has been defined).
    pub board: Option<BoardInfo>,
    /// All components on the board.
    pub components: Vec<ComponentInfo>,
    /// All nets and their connections.
    pub nets: Vec<NetInfo>,
    /// DRC violations found after loading.
    pub violations: Vec<ViolationInfo>,
    /// Copper traces (routed connections).
    pub traces: Vec<TraceInfo>,
    /// Vias (layer-to-layer connections).
    pub vias: Vec<ViaInfo>,
    /// Ratsnest lines (unrouted connections).
    pub ratsnest: Vec<RatsnestInfo>,
    /// Copper pours, as the copper they become.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pours: Vec<PourInfo>,
    /// Zones as the design states them: an outline, a layer and a net.
    ///
    /// Carried separately from `pours`, which is what a zone becomes. The host
    /// sends these in - it is the one holding the source text - and the engine
    /// sends back the copper it computed from them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub zones: Vec<ZoneInfo>,
    /// The stack the design says it wants pressed, when it says.
    ///
    /// Seven pieces of stackup vocabulary landed in this project between
    /// 2026-08-22 and 2026-08-23 - what the fabricator does to the board,
    /// colour, sheets, units, drill pairs, rigid-flex and the impedance
    /// solver - and the language was the only way to see any of it. A stack is
    /// the one part of a design that is a table rather than a list of
    /// statements, so it is the part a person most wants to look at rather
    /// than read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stackup: Option<StackupInfo>,
}

/// A stack as the design states it, flattened for JavaScript.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackupInfo {
    /// Every layer, top to bottom.
    pub layers: Vec<StackupLayerInfo>,
    /// The surface finish the design asks for, empty when it asks for none.
    pub finish: String,
    /// Copper on the routed outline.
    pub edges_plated: bool,
    /// Plated holes cut in half by the outline.
    pub castellated_pads: bool,
    /// `""`, `"plain"` or `"bevelled"`.
    pub edge_connector: String,
    /// The fabricator holds the dielectric to this stack.
    pub impedance_controlled: bool,
    /// The drill spans this build makes, as pairs of layer names.
    pub drill_pairs: Vec<[String; 2]>,
    /// The whole stack's thickness in nanometres, when every layer states one.
    ///
    /// `None` rather than a partial sum, for the reason
    /// `Stackup::total_thickness` answers `None`: a number built from half the
    /// layers reads like a measurement rather than like a gap in the design.
    pub total_thickness_nm: Option<i64>,
    /// The build over each area a layer stops at, in the order the stack names
    /// them.
    ///
    /// A rigid-flex board is not one stack, and a panel showing one column of
    /// layers says it is. Computed here rather than in the browser because the
    /// filter that decides which layers are over an area lives on the model -
    /// `Stackup::layers_in_area` - and the handoff document already asks it
    /// the same question. Two copies of that filter would drift one clause at
    /// a time and the screen and the fabricator's file would then disagree
    /// about one board.
    ///
    /// Empty when no layer states where it stops, which is every rigid build.
    pub areas: Vec<StackupAreaInfo>,
}

/// The stack over one named area of the board.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackupAreaInfo {
    /// The area's own name, as the design spelled it.
    pub name: String,
    /// Which entries of `StackupInfo::layers` are pressed over it, by index.
    pub layers: Vec<usize>,
    /// How thick the board is there, when every layer that is there states a
    /// thickness.
    pub thickness_nm: Option<i64>,
}

/// One entry of a stack, with every sheet it is pressed from.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackupLayerInfo {
    /// `copper`, `prepreg`, `core`, `mask`, `silk`, `paste`, `coverlay` or
    /// `stiffener` - the word the language uses.
    pub kind: String,
    /// What the fabricator calls it, empty when the design did not say.
    pub name: String,
    /// Its own first sheet's thickness in nanometres, when stated.
    pub thickness_nm: Option<i64>,
    /// Every sheet including the first, so a reader sees what a slot is
    /// pressed from rather than only how thick its first sheet is.
    pub sheets_nm: Vec<i64>,
    /// The whole slot: its own sheet plus the rest.
    pub slot_thickness_nm: Option<i64>,
    /// The laminate or foil, empty when the design did not say.
    pub material: String,
    /// The colour asked for, empty when none. Mask and silkscreen only.
    pub color: String,
    /// Dielectric constant in thousandths, when stated.
    pub dk_x1000: Option<u32>,
    /// Loss tangent in millionths, when stated.
    pub df_x1000000: Option<u32>,
    /// The area this layer stops at, empty when it runs the whole panel.
    ///
    /// A rigid-flex build is not one stack: `stiffener 0.2mm outside bend` is
    /// a layer pressed over part of the panel. Sent so the 3D view can read
    /// the design's own sentence instead of the rule it used to apply, which
    /// was "a stiffener is not in the bend" - true of a stiffener and of
    /// nothing else.
    pub coverage_region: String,
    /// Whether the layer is over that area or everywhere but it.
    ///
    /// `true` is `covers bend`, `false` is `outside bend`. Meaningless when
    /// `coverage_region` is empty, and the reader is expected to ask that
    /// first.
    pub coverage_covers: bool,
}

/// A zone as written: a rectangle, a layer mask, and possibly a net.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoneInfo {
    /// Name the design gave it, empty when it gave none.
    pub name: String,
    /// `"pour"` for copper, `"keepout"` for an area nothing may enter.
    pub kind: String,
    /// Layers it covers, as a layer mask.
    pub layer_mask: u32,
    /// Net name it pours to, empty when it names none.
    pub net: String,
    /// Its outline: min x, min y, max x, max y, in nanometres.
    pub bounds: [i64; 4],
}

/// A copper pour, as the copper it actually becomes.
///
/// Sent as the rectangles a fabricator receives rather than as the zone the
/// designer drew: a plane is its outline minus every piece of foreign copper
/// and the clearance around it, so a viewer showing the outline shows a board
/// nobody will be sent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PourInfo {
    /// Net name this pour belongs to, empty when it names none.
    pub net: String,
    /// Copper layers it covers, as a layer mask.
    pub layer_mask: u32,
    /// The filled copper: min x, min y, max x, max y, in nanometres.
    pub rects: Vec<[i64; 4]>,
}

/// A DRC violation for display in the viewer.
///
/// This is a simplified representation of `cypcb_drc::DrcViolation`
/// suitable for JavaScript serialization and rendering.
///
/// A parse or sync error, with the line and column it points at.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceDiagnostic {
    /// What went wrong.
    pub message: String,
    /// 1-based line the span starts on.
    pub line: u32,
    /// 1-based column the span starts at.
    pub column: u32,
    /// 1-based line the span ends on.
    pub end_line: u32,
    /// 1-based column the span ends at.
    pub end_column: u32,
}

/// Where in the source a diagnostic points.
///
/// Byte offsets are what the parser records; an editor wants lines and
/// columns, and converting once here is cheaper and less error-prone than
/// asking every consumer to walk the source.
impl SourceDiagnostic {
    /// Build one from a byte range over `source`.
    pub fn from_span(message: String, source: &str, start: usize, end: usize) -> Self {
        let (line, column) = line_and_column(source, start);
        let (end_line, end_column) = line_and_column(source, end.max(start));
        SourceDiagnostic {
            message,
            line,
            column,
            end_line,
            end_column,
        }
    }
}

/// 1-based line and column of a byte offset, counting characters rather than
/// bytes so a board with a non-ASCII comment still points at the right place.
fn line_and_column(source: &str, offset: usize) -> (u32, u32) {
    let offset = offset.min(source.len());
    let before = &source[..offset];
    let line = before.matches('\n').count() + 1;
    let column = before
        .rfind('\n')
        .map(|nl| before[nl + 1..].chars().count())
        .unwrap_or_else(|| before.chars().count())
        + 1;
    (line as u32, column as u32)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViolationInfo {
    /// Violation type (clearance, drill_size, unconnected_pin, etc.)
    pub kind: String,
    /// Board location X in nanometers.
    pub x_nm: i64,
    /// Board location Y in nanometers.
    pub y_nm: i64,
    /// Human-readable message.
    pub message: String,
    /// The copper this is about, where it is an area: min x, min y, max x,
    /// max y in nanometres. A point is enough for a clearance fault; an
    /// orphaned pour island is a sheet, and its centre looks like every other
    /// part of the plane.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub area: Option<[i64; 4]>,
    /// 1-based line of the definition this is about, when the model knows it.
    ///
    /// A violation is discovered in board coordinates, not source ones, so
    /// this comes from the offending entity's own span. Without it the editor
    /// pins every DRC marker to line 1 - which it did until 2026-08-08.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    /// 1-based column, alongside `line`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
}

impl ViolationInfo {
    /// Create a ViolationInfo from a DrcViolation.
    ///
    /// # Examples
    ///
    /// ```
    /// use cypcb_drc::{DrcViolation, ViolationKind};
    /// use cypcb_core::Point;
    /// use cypcb_world::Entity;
    /// use cypcb_render::ViolationInfo;
    ///
    /// let v = DrcViolation::unconnected_pin(
    ///     Entity::from_raw(1),
    ///     "1",
    ///     "R1",
    ///     Point::from_mm(10.0, 20.0),
    /// );
    /// let info = ViolationInfo::from_drc(&v);
    /// assert_eq!(info.kind, "unconnected-pin");
    /// assert!(info.message.contains("R1.1"));
    /// ```
    pub fn from_drc(v: &DrcViolation) -> Self {
        ViolationInfo {
            kind: format!("{}", v.kind),
            x_nm: v.location.x.0,
            y_nm: v.location.y.0,
            message: v.message.clone(),
            area: v
                .area
                .map(|rect| [rect.min.x.0, rect.min.y.0, rect.max.x.0, rect.max.y.0]),
            line: None,
            column: None,
        }
    }

    /// The same, with the line the offending definition sits on.
    ///
    /// The rules never fill `DrcViolation::source_span` - all seventeen
    /// construction sites pass `None` - so the route to a line is the entity
    /// the violation names and the span sync attached to it. Its stored line
    /// and column are placeholders (`1, 1` for components, `0, 0` for traces);
    /// the byte offset beside them is real, and that is what this converts.
    pub fn from_drc_located(v: &DrcViolation, world: &BoardWorld, source: &str) -> Self {
        let mut info = Self::from_drc(v);
        if let Some(span) = world.get::<cypcb_world::components::SourceSpan>(v.entity) {
            let (line, column) = line_and_column(source, span.start_byte);
            info.line = Some(line);
            info.column = Some(column);
        }
        info
    }
}

/// Board-level information.
#[derive(Debug, Serialize, Deserialize)]
pub struct BoardInfo {
    /// Board name/identifier.
    pub name: String,
    /// Board width in nanometers.
    pub width_nm: i64,
    /// Board height in nanometers.
    pub height_nm: i64,
    /// Number of copper layers.
    pub layer_count: u8,
    /// The board's real edge, when the design states one.
    ///
    /// `[x, y]` pairs in nanometres, closing back on the first. `None` means
    /// the board is the rectangle `width_nm` by `height_nm` describes - which
    /// is what the screen drew for every board, whatever shape it was, until
    /// this reached the snapshot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outline: Option<Vec<[i64; 2]>>,
}

/// Component information for rendering.
#[derive(Debug, Serialize, Deserialize)]
pub struct ComponentInfo {
    /// Reference designator (R1, C1, U1, etc.).
    pub refdes: String,
    /// Component value (10k, 100nF, etc.).
    pub value: String,
    /// X position in nanometers.
    pub x_nm: i64,
    /// Y position in nanometers.
    pub y_nm: i64,
    /// Rotation in millidegrees (0-359999).
    pub rotation_mdeg: i32,
    /// Footprint name/identifier.
    pub footprint: String,
    /// Pad definitions from the footprint.
    pub pads: Vec<PadInfo>,
    /// Component body width in nanometers (from footprint bounds).
    pub body_width_nm: i64,
    /// Component body height in nanometers (from footprint bounds).
    pub body_height_nm: i64,
    /// The catalogue part the design names, when it names one.
    ///
    /// The browser used to read this out of the raw source text with a regular
    /// expression, because the language did not have the property and the
    /// model never saw it. It has both now, and a second reader of the
    /// language is exactly what `docs/one-parser.md` exists to prevent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lcsc: Option<String>,
    /// Which face of the board the part is soldered to: `top` or `bottom`.
    ///
    /// The pads already say it - a bottom part's pads carry bottom-copper
    /// layer bits and mirrored coordinates, because the world holds the
    /// flipped footprint - but its ink does not. Silkscreen and the body
    /// outline are drawn in one colour from a footprint that has no layer, so
    /// without this the browser prints a bottom part's legend on the top of
    /// the board.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub side: Option<String>,
    /// Optional path/key to a GLB 3D model file (for future use).
    pub model_3d: Option<String>,
    /// The footprint's own silkscreen artwork, in footprint coordinates.
    ///
    /// Sent so the viewer draws the legend the engine holds rather than its
    /// own copy of the footprint. Empty when the part carries no artwork, and
    /// omitted from the JSON entirely in that case so a snapshot of a board
    /// without legends is no larger than it was.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub silk: Vec<SilkInfo>,
}

/// Pad information for rendering.
///
/// Positions are relative to the component origin.
#[derive(Debug, Serialize, Deserialize)]
pub struct PadInfo {
    /// Pad number/name (e.g., "1", "2", "A1", "VCC").
    pub number: String,
    /// X position relative to component origin, in nanometers.
    pub x_nm: i64,
    /// Y position relative to component origin, in nanometers.
    pub y_nm: i64,
    /// Pad width in nanometers.
    pub width_nm: i64,
    /// Pad height in nanometers.
    pub height_nm: i64,
    /// Shape as string: "circle", "rect", "roundrect", "oblong".
    pub shape: String,
    /// Copper layer bit mask (bit 0 = top, bit 1 = bottom, bits 2-31 = inner).
    pub layer_mask: u32,
    /// Drill diameter in nanometers (None for SMD pads).
    ///
    /// For a slot this is the narrow dimension, the same number the drill
    /// file's tool table carries.
    pub drill_nm: Option<i64>,
    /// The hole's full size when it is a slot, `[width, height]` in nanometers.
    ///
    /// Absent for a round hole, which is nearly every hole on nearly every
    /// board - and absent from the JSON entirely, so a snapshot written before
    /// slots existed still reads.
    ///
    /// The screen is the last place a slot could still look like something it
    /// is not: the files carry it, and a viewer drawing a 2.4x1.0mm slot as a
    /// 1mm circle shows a designer a hole their connector will not fit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot_nm: Option<(i64, i64)>,
}

/// Net information.
#[derive(Debug, Serialize, Deserialize)]
pub struct NetInfo {
    /// Net name.
    pub name: String,
    /// Net ID (internal identifier).
    pub id: u32,
    /// All pin connections to this net.
    pub connections: Vec<PinRef>,
    /// Trace width constraint in nanometers (from `[width 0.3mm]`). None = use default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width_nm: Option<i64>,
    /// Clearance constraint in nanometers (from `[clearance 0.2mm]`). None = use default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clearance_nm: Option<i64>,
    /// Current constraint in milliamps (from `[current 2A]`). None = no constraint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_ma: Option<f64>,
}

/// Reference to a component pin.
#[derive(Debug, Serialize, Deserialize)]
pub struct PinRef {
    /// Component reference designator.
    pub component: String,
    /// Pin number/name.
    pub pin: String,
}

/// A single segment of a trace (line from start to end).
///
/// TraceSegmentInfo is the JavaScript-serializable version of
/// `cypcb_world::components::trace::TraceSegment`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceSegmentInfo {
    /// Start X coordinate in nanometers.
    pub start_x: f64,
    /// Start Y coordinate in nanometers.
    pub start_y: f64,
    /// End X coordinate in nanometers.
    pub end_x: f64,
    /// End Y coordinate in nanometers.
    pub end_y: f64,
}

/// The curve a run of copper was drawn as, for a renderer that can draw one.
///
/// The segments are still there and are still what a hit test measures: this
/// is the sentence beside the copper, not instead of it. A canvas draws an arc
/// in one call, and a dozen chords at a high zoom look like what they are.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CurveInfo {
    /// Centre X in nanometers.
    pub centre_x: f64,
    /// Centre Y in nanometers.
    pub centre_y: f64,
    /// Radius in nanometers.
    pub radius: f64,
    /// Where the curve starts, in degrees counter-clockwise from `+X`.
    pub start_degrees: f64,
    /// How far it turns, in degrees. Negative turns clockwise.
    pub sweep_degrees: f64,
}

/// Trace information for rendering.
///
/// Represents a copper trace as a polyline with a given width.
/// Used by the JavaScript renderer to draw routed connections.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceInfo {
    /// Entity index for selection/hit-testing. Maps back to the ECS entity.
    pub id: u32,
    /// The polyline path as a vector of segments.
    pub segments: Vec<TraceSegmentInfo>,
    /// Trace width in nanometers.
    pub width: f64,
    /// Layer name ("Top" or "Bottom").
    pub layer: String,
    /// Net name this trace belongs to.
    pub net_name: String,
    /// Whether this trace is locked (manual, not to be modified).
    pub locked: bool,
    /// The curve this copper was drawn as, when it was drawn as one.
    #[serde(default)]
    pub curve: Option<CurveInfo>,
}

/// Via information for rendering.
///
/// Represents a plated through-hole connecting copper layers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViaInfo {
    /// Entity index for selection/hit-testing. Maps back to the ECS entity.
    pub id: u32,
    /// Center X coordinate in nanometers.
    pub x: f64,
    /// Center Y coordinate in nanometers.
    pub y: f64,
    /// Drill hole diameter in nanometers.
    pub drill: f64,
    /// Outer diameter (copper ring) in nanometers.
    pub outer_diameter: f64,
    /// Net name this via belongs to.
    pub net_name: String,
    /// Layer the via starts on, as the DSL names it: `Top`, `Bottom`, `Inner1`.
    ///
    /// A via that stops at an inner layer - blind or buried - is a different
    /// hole from one that goes through, and the viewer was drawing both the
    /// same because the span never left the board model.
    #[serde(default = "top_layer_name")]
    pub start_layer: String,
    /// Layer the via ends on.
    #[serde(default = "bottom_layer_name")]
    pub end_layer: String,
}

/// A via with no stated span goes through, which is what every via was before
/// the span was carried.
fn top_layer_name() -> String {
    "Top".to_string()
}

fn bottom_layer_name() -> String {
    "Bottom".to_string()
}

/// Ratsnest line information for rendering.
///
/// Represents an unrouted connection between two pins.
/// Shown as a thin line to indicate what still needs routing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RatsnestInfo {
    /// Start X coordinate in nanometers.
    pub start_x: f64,
    /// Start Y coordinate in nanometers.
    pub start_y: f64,
    /// End X coordinate in nanometers.
    pub end_x: f64,
    /// End Y coordinate in nanometers.
    pub end_y: f64,
    /// Net name this connection belongs to.
    pub net_name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_board_snapshot_serializes() {
        let snapshot = BoardSnapshot {
            board: Some(BoardInfo {
                name: "Test".to_string(),
                width_nm: 100_000_000,
                height_nm: 80_000_000,
                layer_count: 2,
                outline: None,
            }),
            components: vec![ComponentInfo {
                refdes: "R1".to_string(),
                value: "10k".to_string(),
                x_nm: 10_000_000,
                y_nm: 20_000_000,
                rotation_mdeg: 0,
                footprint: "0402".to_string(),
                pads: vec![
                    PadInfo {
                        number: "1".to_string(),
                        x_nm: -500_000,
                        y_nm: 0,
                        width_nm: 600_000,
                        height_nm: 500_000,
                        shape: "rect".to_string(),
                        layer_mask: 1,
                        drill_nm: None,
                        slot_nm: None,
                    },
                    PadInfo {
                        number: "2".to_string(),
                        x_nm: 500_000,
                        y_nm: 0,
                        width_nm: 600_000,
                        height_nm: 500_000,
                        shape: "rect".to_string(),
                        layer_mask: 1,
                        drill_nm: None,
                        slot_nm: None,
                    },
                ],
                body_width_nm: 1_000_000,
                body_height_nm: 500_000,
                lcsc: None,
                side: None,
                model_3d: None,
                silk: Vec::new(),
            }],
            nets: vec![NetInfo {
                name: "VCC".to_string(),
                id: 0,
                connections: vec![PinRef {
                    component: "R1".to_string(),
                    pin: "1".to_string(),
                }],
                width_nm: None,
                clearance_nm: None,
                current_ma: None,
            }],
            violations: vec![],
            traces: vec![],
            vias: vec![],
            ratsnest: vec![],
            pours: vec![],
            zones: vec![],
            stackup: None,
        };

        // Verify it can serialize to JSON (serde-wasm-bindgen uses serde)
        let json = serde_json::to_string(&snapshot).unwrap();
        assert!(json.contains("\"name\":\"Test\""));
        assert!(json.contains("\"refdes\":\"R1\""));
        assert!(json.contains("\"name\":\"VCC\""));
        assert!(json.contains("\"violations\":[]"));
        assert!(json.contains("\"traces\":[]"));
        assert!(json.contains("\"vias\":[]"));
        assert!(json.contains("\"ratsnest\":[]"));
        assert!(json.contains("\"body_width_nm\":1000000"));
        assert!(json.contains("\"body_height_nm\":500000"));
        assert!(json.contains("\"model_3d\":null"));
    }

    #[test]
    fn test_violation_info_from_drc() {
        use cypcb_core::Point;
        use cypcb_world::Entity;

        let violation = cypcb_drc::DrcViolation::unconnected_pin(
            Entity::from_raw(1),
            "1",
            "R1",
            Point::from_mm(10.0, 20.0),
        );
        let info = ViolationInfo::from_drc(&violation);

        assert_eq!(info.kind, "unconnected-pin");
        assert_eq!(info.x_nm, 10_000_000);
        assert_eq!(info.y_nm, 20_000_000);
        assert!(info.message.contains("R1.1"));
    }

    #[test]
    fn test_violation_info_serializes() {
        let violation = ViolationInfo {
            kind: "clearance".to_string(),
            x_nm: 5_000_000,
            y_nm: 10_000_000,
            message: "Clearance violation: 0.10mm actual, 0.15mm required".to_string(),
            area: None,
            line: None,
            column: None,
        };

        let json = serde_json::to_string(&violation).unwrap();
        assert!(json.contains("\"kind\":\"clearance\""));
        assert!(json.contains("\"x_nm\":5000000"));
        assert!(json.contains("\"message\""));
    }

    #[test]
    fn test_trace_info_serializes() {
        let trace = TraceInfo {
            id: 0,
            segments: vec![
                TraceSegmentInfo {
                    start_x: 0.0,
                    start_y: 0.0,
                    end_x: 10_000_000.0,
                    end_y: 0.0,
                },
                TraceSegmentInfo {
                    start_x: 10_000_000.0,
                    start_y: 0.0,
                    end_x: 10_000_000.0,
                    end_y: 5_000_000.0,
                },
            ],
            width: 200_000.0, // 0.2mm
            layer: "Top".to_string(),
            curve: None,
            net_name: "VCC".to_string(),
            locked: true,
        };

        let json = serde_json::to_string(&trace).unwrap();
        assert!(json.contains("\"layer\":\"Top\""));
        assert!(json.contains("\"net_name\":\"VCC\""));
        assert!(json.contains("\"locked\":true"));
        assert!(json.contains("\"width\":200000"));
        assert!(json.contains("\"segments\""));
    }

    #[test]
    fn test_via_info_serializes() {
        let via = ViaInfo {
            id: 0,
            x: 5_000_000.0,
            y: 10_000_000.0,
            drill: 300_000.0,          // 0.3mm
            outer_diameter: 600_000.0, // 0.6mm
            net_name: "GND".to_string(),
            start_layer: "Top".to_string(),
            end_layer: "Bottom".to_string(),
        };

        let json = serde_json::to_string(&via).unwrap();
        assert!(json.contains("\"x\":5000000"));
        assert!(json.contains("\"y\":10000000"));
        assert!(json.contains("\"drill\":300000"));
        assert!(json.contains("\"outer_diameter\":600000"));
        assert!(json.contains("\"net_name\":\"GND\""));
    }

    #[test]
    fn test_ratsnest_info_serializes() {
        let ratsnest = RatsnestInfo {
            start_x: 0.0,
            start_y: 0.0,
            end_x: 20_000_000.0,
            end_y: 15_000_000.0,
            net_name: "SIGNAL".to_string(),
        };

        let json = serde_json::to_string(&ratsnest).unwrap();
        assert!(json.contains("\"start_x\":0"));
        assert!(json.contains("\"start_y\":0"));
        assert!(json.contains("\"end_x\":20000000"));
        assert!(json.contains("\"end_y\":15000000"));
        assert!(json.contains("\"net_name\":\"SIGNAL\""));
    }

    #[test]
    fn test_snapshot_with_traces() {
        let snapshot = BoardSnapshot {
            board: Some(BoardInfo {
                name: "TraceTest".to_string(),
                width_nm: 50_000_000,
                height_nm: 30_000_000,
                layer_count: 2,
                outline: None,
            }),
            components: vec![],
            nets: vec![],
            violations: vec![],
            traces: vec![TraceInfo {
                id: 1,
                segments: vec![TraceSegmentInfo {
                    start_x: 5_000_000.0,
                    start_y: 5_000_000.0,
                    end_x: 25_000_000.0,
                    end_y: 5_000_000.0,
                }],
                width: 250_000.0,
                layer: "Top".to_string(),
                curve: None,
                net_name: "VCC".to_string(),
                locked: false,
            }],
            vias: vec![ViaInfo {
                id: 2,
                x: 25_000_000.0,
                y: 5_000_000.0,
                drill: 300_000.0,
                outer_diameter: 600_000.0,
                net_name: "VCC".to_string(),
                start_layer: "Top".to_string(),
                end_layer: "Bottom".to_string(),
            }],
            ratsnest: vec![],
            pours: vec![],
            zones: vec![],
            stackup: None,
        };

        let json = serde_json::to_string(&snapshot).unwrap();
        assert!(json.contains("\"traces\""));
        assert!(json.contains("\"vias\""));
        assert!(json.contains("\"VCC\""));
    }
}

/// A piece of silkscreen artwork as the host describes it.
///
/// Mirrors the viewer's `SilkShape`: tagged by `type`, coordinates in
/// nanometres, relative to the footprint's origin. Arcs are accepted and
/// dropped rather than refused - the board model has no arc, and rejecting a
/// whole footprint over a rounded corner would be worse than printing it
/// without one. That trade is recorded in the tracker rather than hidden here.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum SilkInfo {
    /// A straight line.
    Segment {
        /// Start X, relative to the footprint origin.
        x1: i64,
        /// Start Y.
        y1: i64,
        /// End X.
        x2: i64,
        /// End Y.
        y2: i64,
        /// Stroke width.
        width: i64,
    },
    /// A circle outline.
    Circle {
        /// Centre X.
        cx: i64,
        /// Centre Y.
        cy: i64,
        /// Radius.
        radius: i64,
        /// Stroke width.
        width: i64,
    },
    /// An arc, approximated as segments on the way into the board model.
    ///
    /// The model has no arc, and a legend is ink rather than geometry anything
    /// reasons about - the exporter emits a circle as a 32-sided polygon for
    /// the same reason. Dropping arcs instead meant a part fetched from a
    /// supplier arrived with a rounded outline and lost it silently.
    Arc {
        /// Centre X.
        cx: i64,
        /// Centre Y.
        cy: i64,
        /// Radius.
        radius: i64,
        /// Stroke width.
        width: i64,
        /// Where the arc starts, in degrees, counter-clockwise from +X.
        ///
        /// Defaulted so a payload written before arcs carried angles still
        /// deserialises - as a full circle, which is what it meant.
        #[serde(default)]
        start_angle: f64,
        /// Where it ends. Equal to the start means a full turn.
        #[serde(default = "full_turn")]
        end_angle: f64,
    },
}

/// The end angle of an arc that says nothing: all the way round.
fn full_turn() -> f64 {
    360.0
}

impl SilkInfo {
    /// How many segments approximate a full turn.
    ///
    /// Thirty-two is what the Gerber exporter already uses for a circle: at
    /// the scale a legend is printed the polygon differs from the curve by
    /// less than the width of the line drawing it.
    const SEGMENTS_PER_TURN: usize = 32;

    /// Convert to the board model's shapes.
    ///
    /// One shape for a segment or a circle, a chain of segments for an arc.
    /// Empty only for an arc with no length.
    pub fn to_shapes(&self) -> Vec<cypcb_world::footprint::SilkShape> {
        use cypcb_core::{Nm, Point};
        use cypcb_world::footprint::SilkShape;

        if let SilkInfo::Arc {
            cx,
            cy,
            radius,
            width,
            start_angle,
            end_angle,
        } = self
        {
            let sweep = {
                let raw = end_angle - start_angle;
                // A full turn is what an arc means when both angles agree.
                if raw.abs() < f64::EPSILON {
                    360.0
                } else {
                    raw
                }
            };
            let steps = ((sweep.abs() / 360.0) * Self::SEGMENTS_PER_TURN as f64).ceil() as usize;
            let steps = steps.max(1);

            let point_at = |degrees: f64| {
                let radians = degrees.to_radians();
                Point::new(
                    Nm(cx + (*radius as f64 * radians.cos()).round() as i64),
                    Nm(cy + (*radius as f64 * radians.sin()).round() as i64),
                )
            };

            return (0..steps)
                .map(|i| {
                    let a = start_angle + sweep * (i as f64 / steps as f64);
                    let b = start_angle + sweep * ((i + 1) as f64 / steps as f64);
                    SilkShape::Segment {
                        start: point_at(a),
                        end: point_at(b),
                        width: Nm(*width),
                    }
                })
                .collect();
        }

        self.to_shape().into_iter().collect()
    }

    /// Describe a shape the board model holds, for sending to a host.
    ///
    /// The model has no arc, so nothing here produces one: an arc that came in
    /// was turned into segments on the way and goes back out as segments.
    pub fn from_shape(shape: &cypcb_world::footprint::SilkShape) -> Self {
        use cypcb_world::footprint::SilkShape;
        match shape {
            SilkShape::Segment { start, end, width } => SilkInfo::Segment {
                x1: start.x.0,
                y1: start.y.0,
                x2: end.x.0,
                y2: end.y.0,
                width: width.0,
            },
            SilkShape::Circle {
                centre,
                radius,
                width,
            } => SilkInfo::Circle {
                cx: centre.x.0,
                cy: centre.y.0,
                radius: radius.0,
                width: width.0,
            },
        }
    }

    /// Convert to the board model's shape, if it is a single one.
    pub fn to_shape(&self) -> Option<cypcb_world::footprint::SilkShape> {
        use cypcb_core::{Nm, Point};
        use cypcb_world::footprint::SilkShape;
        match self {
            SilkInfo::Segment {
                x1,
                y1,
                x2,
                y2,
                width,
            } => Some(SilkShape::Segment {
                start: Point::new(Nm(*x1), Nm(*y1)),
                end: Point::new(Nm(*x2), Nm(*y2)),
                width: Nm(*width),
            }),
            SilkInfo::Circle {
                cx,
                cy,
                radius,
                width,
            } => Some(SilkShape::Circle {
                centre: Point::new(Nm(*cx), Nm(*cy)),
                radius: Nm(*radius),
                width: Nm(*width),
            }),
            SilkInfo::Arc { .. } => None,
        }
    }
}
