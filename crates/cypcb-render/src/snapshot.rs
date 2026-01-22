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
            }],
            nets: vec![NetInfo {
                name: "VCC".to_string(),
                id: 0,
                connections: vec![PinRef {
                    component: "R1".to_string(),
                    pin: "1".to_string(),
                }],
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
            x: 5_000_000.0,
            y: 10_000_000.0,
            drill: 300_000.0,       // 0.3mm
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
