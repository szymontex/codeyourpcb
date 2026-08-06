//! Board snapshot types for JavaScript serialization.
//!
//! These types provide a flat, serializable view of the board state
//! suitable for transmission to JavaScript via serde-wasm-bindgen.
//!
//! All types use primitive types (i64, i32, u32, String) that serialize
//! cleanly to JavaScript numbers and strings.

use cypcb_drc::DrcViolation;
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
}

/// A DRC violation for display in the viewer.
///
/// This is a simplified representation of `cypcb_drc::DrcViolation`
/// suitable for JavaScript serialization and rendering.
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
        }
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
    /// Optional path/key to a GLB 3D model file (for future use).
    pub model_3d: Option<String>,
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
    pub drill_nm: Option<i64>,
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
                    },
                ],
                body_width_nm: 1_000_000,
                body_height_nm: 500_000,
                model_3d: None,
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
            }],
            ratsnest: vec![],
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
#[derive(Debug, Clone, Deserialize)]
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
