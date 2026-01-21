//! Board snapshot types for JavaScript serialization.
//!
//! These types provide a flat, serializable view of the board state
//! suitable for transmission to JavaScript via serde-wasm-bindgen.
//!
//! All types use primitive types (i64, i32, u32, String) that serialize
//! cleanly to JavaScript numbers and strings.

use serde::Serialize;

/// Complete snapshot of the board state for rendering.
///
/// This is the main type returned by `PcbEngine::get_snapshot()`.
/// It contains all information needed to render the board in JavaScript.
#[derive(Debug, Serialize)]
pub struct BoardSnapshot {
    /// Board information (if a board has been defined).
    pub board: Option<BoardInfo>,
    /// All components on the board.
    pub components: Vec<ComponentInfo>,
    /// All nets and their connections.
    pub nets: Vec<NetInfo>,
}

/// Board-level information.
#[derive(Debug, Serialize)]
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
#[derive(Debug, Serialize)]
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
#[derive(Debug, Serialize)]
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
#[derive(Debug, Serialize)]
pub struct NetInfo {
    /// Net name.
    pub name: String,
    /// Net ID (internal identifier).
    pub id: u32,
    /// All pin connections to this net.
    pub connections: Vec<PinRef>,
}

/// Reference to a component pin.
#[derive(Debug, Serialize)]
pub struct PinRef {
    /// Component reference designator.
    pub component: String,
    /// Pin number/name.
    pub pin: String,
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
        };

        // Verify it can serialize to JSON (serde-wasm-bindgen uses serde)
        let json = serde_json::to_string(&snapshot).unwrap();
        assert!(json.contains("\"name\":\"Test\""));
        assert!(json.contains("\"refdes\":\"R1\""));
        assert!(json.contains("\"name\":\"VCC\""));
    }
}
