//! CodeYourPCB WASM Rendering Bridge
//!
//! This crate provides the interface for the CodeYourPCB web viewer.
//! It bridges Rust board data to JavaScript, enabling the web UI to:
//!
//! - Load and parse `.cypcb` source files (native mode)
//! - Load pre-parsed JSON snapshots (WASM mode)
//! - Retrieve structured board snapshots for rendering
//! - Query components at specific coordinates
//!
//! # Feature Flags
//!
//! - `native` (default): Full parsing support with tree-sitter
//! - `wasm`: WASM-compatible build without tree-sitter (parsing done in JavaScript)
//!
//! # Architecture
//!
//! The rendering happens in JavaScript/Canvas - this crate only provides data.
//! The `PcbEngine` struct maintains the board state and provides query methods.
//!
//! In native mode, `load_source()` parses the .cypcb source directly.
//! In WASM mode, `load_snapshot()` receives pre-parsed JSON from JavaScript.

mod snapshot;

pub use snapshot::*;

use cypcb_core::{Nm, Point};
use cypcb_drc::{run_drc, DesignRules, DrcViolation};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{
    BoardWorld, Entity, FootprintRef, Layer, NetConnections, NetId, PadShape, PinConnection,
    Position, RefDes, Rotation, Value,
};

// Import sync and parse only in native mode
#[cfg(feature = "native")]
use cypcb_parser::parse;
#[cfg(feature = "native")]
use cypcb_world::sync_ast_to_world;

// WASM-specific imports (only when targeting wasm32)
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

/// PCB Engine - main interface for JavaScript.
///
/// Maintains the board state and provides methods for loading source,
/// getting snapshots, and querying the board.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub struct PcbEngine {
    world: BoardWorld,
    footprint_lib: FootprintLibrary,
    source: String,
    /// DRC violations from the last load.
    violations: Vec<DrcViolation>,
    /// Time taken for last DRC run in milliseconds.
    drc_duration_ms: u64,
}

// WASM-exposed methods
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl PcbEngine {
    /// Create a new PcbEngine instance.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(constructor))]
    pub fn new() -> PcbEngine {
        PcbEngine {
            world: BoardWorld::new(),
            footprint_lib: FootprintLibrary::new(),
            source: String::new(),
            violations: Vec::new(),
            drc_duration_ms: 0,
        }
    }

    /// Load a pre-parsed board snapshot (WASM mode).
    ///
    /// This method receives a BoardSnapshot that was parsed in JavaScript
    /// and populates the internal world state for queries.
    ///
    /// Returns an empty string on success, or an error message on failure.
    #[cfg(target_arch = "wasm32")]
    pub fn load_snapshot(&mut self, snapshot_js: wasm_bindgen::JsValue) -> String {
        // Deserialize the snapshot from JavaScript
        let snapshot: Result<BoardSnapshot, _> = serde_wasm_bindgen::from_value(snapshot_js);
        match snapshot {
            Ok(snap) => {
                self.populate_from_snapshot(&snap);
                // Run DRC after populating world
                self.run_drc_internal();
                String::new()
            }
            Err(e) => format!("Failed to deserialize snapshot: {}", e),
        }
    }

    /// Get a snapshot of the current board state for rendering (WASM version).
    ///
    /// Returns a JsValue that can be used directly in JavaScript.
    #[cfg(target_arch = "wasm32")]
    pub fn get_snapshot(&mut self) -> wasm_bindgen::JsValue {
        let snapshot = self.build_snapshot();
        serde_wasm_bindgen::to_value(&snapshot).unwrap_or(wasm_bindgen::JsValue::NULL)
    }

    /// Get a snapshot of the current board state for rendering (native version).
    ///
    /// Returns a JSON string for non-WASM targets.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn get_snapshot(&mut self) -> String {
        let snapshot = self.build_snapshot();
        serde_json::to_string(&snapshot).unwrap_or_else(|_| "{}".to_string())
    }

    /// Query components at a specific point.
    ///
    /// Returns reference designator strings.
    pub fn query_point(&mut self, x_nm: i64, y_nm: i64) -> Vec<String> {
        let point: Point = Point::new(Nm(x_nm), Nm(y_nm));
        let entities: Vec<Entity> = self.world.query_point(point);

        let mut refdes_list: Vec<String> = Vec::new();
        for entity in entities {
            let maybe_refdes: Option<&RefDes> = self.world.get::<RefDes>(entity);
            if let Some(refdes) = maybe_refdes {
                let s: String = refdes.as_str().to_string();
                refdes_list.push(s);
            }
        }

        refdes_list
    }
}

// Internal methods (not exposed to WASM)
impl PcbEngine {
    /// Load and parse source code (native mode only).
    ///
    /// Returns an empty string on success, or an error message on failure.
    /// The board state is updated even if there are errors (partial results).
    /// DRC is run automatically after successful sync.
    ///
    /// In WASM mode, use `load_snapshot()` instead.
    #[cfg(feature = "native")]
    pub fn load_source(&mut self, source: &str) -> String {
        self.source = source.to_string();
        self.world.clear();
        self.violations.clear();
        self.drc_duration_ms = 0;

        // Parse the source
        let parse_result = parse(source);

        // Collect parse errors
        let mut errors: Vec<String> = Vec::new();
        for e in &parse_result.errors {
            errors.push(format!("{}", e));
        }

        // Sync AST to world
        let sync_result =
            sync_ast_to_world(&parse_result.value, source, &mut self.world, &self.footprint_lib);

        // Collect sync errors
        for err in &sync_result.errors {
            errors.push(format!("{}", err));
        }

        // Run DRC after sync (even if there were parse/sync errors, check what we have)
        self.run_drc_internal();

        if errors.is_empty() {
            String::new()
        } else {
            errors.join("\n")
        }
    }

    /// Run DRC using default rules (JLCPCB 2-layer).
    fn run_drc_internal(&mut self) {
        let rules = DesignRules::default();
        let result = run_drc(&mut self.world, &rules);
        self.violations = result.violations;
        self.drc_duration_ms = result.duration_ms;
    }

    /// Get the number of DRC violations from the last load.
    pub fn violation_count(&self) -> usize {
        self.violations.len()
    }

    /// Get the time taken for the last DRC run in milliseconds.
    pub fn drc_duration_ms(&self) -> u64 {
        self.drc_duration_ms
    }

    /// Populate the world from a BoardSnapshot.
    fn populate_from_snapshot(&mut self, snapshot: &BoardSnapshot) {
        self.world.clear();
        self.violations.clear();

        // Create board entity if present
        if let Some(board) = &snapshot.board {
            self.world.set_board(
                board.name.clone(),
                (Nm(board.width_nm), Nm(board.height_nm)),
                board.layer_count,
            );
        }

        // Register footprints from snapshot data (needed for DRC)
        // If snapshot has pads, use those. Otherwise use builtin library.
        // Note: JS parser doesn't populate pads, so we fall back to builtin library.
        let mut registered: std::collections::HashSet<String> = std::collections::HashSet::new();
        for comp in &snapshot.components {
            if !comp.footprint.is_empty() && !registered.contains(&comp.footprint) {
                if !comp.pads.is_empty() {
                    // Use pads from snapshot (custom footprint)
                    let footprint = self.footprint_from_pads(&comp.footprint, &comp.pads);
                    self.footprint_lib.register(footprint);
                }
                // If pads are empty, the builtin library (loaded in new()) should have it
                registered.insert(comp.footprint.clone());
            }
        }

        // Create component entities
        for comp in &snapshot.components {
            let refdes = RefDes::new(&comp.refdes);
            let value = Value::new(&comp.value);
            let position = Position(Point::new(Nm(comp.x_nm), Nm(comp.y_nm)));
            let rotation = Rotation(comp.rotation_mdeg);
            let footprint_ref = FootprintRef::new(&comp.footprint);
            let nets = NetConnections::new();

            self.world.spawn_component(refdes, value, position, rotation, footprint_ref, nets);
        }

        // Rebuild spatial index for DRC queries
        let lib = &self.footprint_lib;
        self.world.rebuild_spatial_index(|name| {
            lib.get(name)
                .map(|fp| fp.courtyard)
                .unwrap_or_else(|| {
                    // Default 1mm x 1mm bounds for unknown footprints
                    cypcb_core::Rect::from_center_size(
                        Point::ORIGIN,
                        (Nm::from_mm(1.0), Nm::from_mm(1.0)),
                    )
                })
        });

        // Note: Nets are stored in the snapshot for rendering but not
        // re-created in the world as ECS entities (they're derived data).
    }

    /// Create a Footprint from PadInfo data.
    fn footprint_from_pads(&self, name: &str, pads: &[PadInfo]) -> cypcb_world::footprint::Footprint {
        use cypcb_world::footprint::{Footprint, PadDef};

        let mut pad_defs: Vec<PadDef> = Vec::with_capacity(pads.len());

        for pad in pads {
            // Convert shape string to PadShape
            let shape = match pad.shape.as_str() {
                "circle" => PadShape::Circle,
                "roundrect" => PadShape::RoundRect { corner_ratio: 25 },
                "oblong" => PadShape::Oblong,
                _ => PadShape::Rect, // default to rect
            };

            // Convert layer_mask to Vec<Layer>
            let mut layers: Vec<Layer> = Vec::new();
            if pad.layer_mask & 1 != 0 {
                layers.push(Layer::TopCopper);
            }
            if pad.layer_mask & 2 != 0 {
                layers.push(Layer::BottomCopper);
            }
            for i in 0..30 {
                if pad.layer_mask & (1 << (2 + i)) != 0 {
                    layers.push(Layer::Inner(i));
                }
            }
            // If no layers specified, default to top copper
            if layers.is_empty() {
                layers.push(Layer::TopCopper);
            }

            pad_defs.push(PadDef {
                number: pad.number.clone(),
                shape,
                position: Point::new(Nm(pad.x_nm), Nm(pad.y_nm)),
                size: (Nm(pad.width_nm), Nm(pad.height_nm)),
                drill: pad.drill_nm.map(Nm),
                layers,
            });
        }

        // Calculate bounds from pads
        let mut min_x = i64::MAX;
        let mut min_y = i64::MAX;
        let mut max_x = i64::MIN;
        let mut max_y = i64::MIN;

        for pad in &pad_defs {
            let half_w = pad.size.0.0 / 2;
            let half_h = pad.size.1.0 / 2;
            min_x = min_x.min(pad.position.x.0 - half_w);
            min_y = min_y.min(pad.position.y.0 - half_h);
            max_x = max_x.max(pad.position.x.0 + half_w);
            max_y = max_y.max(pad.position.y.0 + half_h);
        }

        use cypcb_core::Rect;
        let bounds = if min_x <= max_x && min_y <= max_y {
            Rect::new(Point::new(Nm(min_x), Nm(min_y)), Point::new(Nm(max_x), Nm(max_y)))
        } else {
            Rect::new(Point::new(Nm(0), Nm(0)), Point::new(Nm(0), Nm(0)))
        };

        // Courtyard is bounds with margin
        let margin = Nm(250_000); // 0.25mm margin
        let courtyard = Rect::new(
            Point::new(Nm(min_x - margin.0), Nm(min_y - margin.0)),
            Point::new(Nm(max_x + margin.0), Nm(max_y + margin.0)),
        );

        Footprint {
            name: name.to_string(),
            description: format!("Reconstructed from snapshot: {}", name),
            pads: pad_defs,
            bounds,
            courtyard,
        }
    }

    /// Build a BoardSnapshot from the current world state.
    pub fn build_snapshot(&mut self) -> BoardSnapshot {
        // Build board info
        let board: Option<BoardInfo> = match self.world.board_info() {
            Some((size, layers)) => {
                let name: String = self.world.board_name().unwrap_or("").to_string();
                Some(BoardInfo {
                    name,
                    width_nm: size.width.0,
                    height_nm: size.height.0,
                    layer_count: layers.count,
                })
            }
            None => None,
        };

        // Build component info
        let component_data: Vec<(Entity, RefDes, Position)> = self.world.components();
        let mut components: Vec<ComponentInfo> = Vec::with_capacity(component_data.len());

        for tuple in component_data {
            let entity: Entity = tuple.0;
            let refdes: RefDes = tuple.1;
            let position: Position = tuple.2;

            // Get value
            let value: String = match self.world.get::<Value>(entity) {
                Some(v) => v.as_str().to_string(),
                None => String::new(),
            };

            // Get rotation
            let rotation: i32 = match self.world.get::<Rotation>(entity) {
                Some(r) => r.0,
                None => 0,
            };

            // Get footprint name
            let footprint_name: String = match self.world.get::<FootprintRef>(entity) {
                Some(f) => f.as_str().to_string(),
                None => String::new(),
            };

            // Get pad info from footprint library
            let mut pads: Vec<PadInfo> = Vec::new();
            if let Some(fp) = self.footprint_lib.get(&footprint_name) {
                for pad in &fp.pads {
                    let mut layer_mask: u32 = 0;
                    for layer in &pad.layers {
                        let layer: &Layer = layer;
                        layer_mask |= layer.to_copper_mask();
                    }
                    let drill_nm: Option<i64> = match pad.drill {
                        Some(d) => Some(d.0),
                        None => None,
                    };
                    pads.push(PadInfo {
                        number: pad.number.clone(),
                        x_nm: pad.position.x.0,
                        y_nm: pad.position.y.0,
                        width_nm: pad.size.0.0,
                        height_nm: pad.size.1.0,
                        shape: pad_shape_to_string(&pad.shape),
                        layer_mask,
                        drill_nm,
                    });
                }
            }

            let refdes_str: String = refdes.as_str().to_string();
            components.push(ComponentInfo {
                refdes: refdes_str,
                value,
                x_nm: position.0.x.0,
                y_nm: position.0.y.0,
                rotation_mdeg: rotation,
                footprint: footprint_name,
                pads,
            });
        }

        // Build net info - collect nets first to avoid borrow issues
        let mut net_list: Vec<(NetId, String)> = Vec::new();
        for pair in self.world.nets() {
            let id: NetId = pair.0;
            let name: &str = pair.1;
            net_list.push((id, name.to_string()));
        }

        let mut nets: Vec<NetInfo> = Vec::with_capacity(net_list.len());

        for (net_id, net_name) in net_list {
            // Find all connections to this net
            let mut connections: Vec<PinRef> = Vec::new();

            let components_for_net: Vec<(Entity, RefDes, Position)> = self.world.components();
            for tuple in components_for_net {
                let entity: Entity = tuple.0;
                let refdes: RefDes = tuple.1;

                let net_conns_opt: Option<&NetConnections> =
                    self.world.get::<NetConnections>(entity);
                if let Some(net_conns) = net_conns_opt {
                    for conn in net_conns.iter() {
                        let conn: &PinConnection = conn;
                        if conn.net == net_id {
                            let comp_str: String = refdes.as_str().to_string();
                            connections.push(PinRef {
                                component: comp_str,
                                pin: conn.pin.clone(),
                            });
                        }
                    }
                }
            }

            nets.push(NetInfo {
                name: net_name,
                id: net_id.0,
                connections,
            });
        }

        // Build violations info
        let violations: Vec<ViolationInfo> = self
            .violations
            .iter()
            .map(ViolationInfo::from_drc)
            .collect();

        BoardSnapshot {
            board,
            components,
            nets,
            violations,
        }
    }
}

impl Default for PcbEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert PadShape enum to string for JS serialization.
fn pad_shape_to_string(shape: &PadShape) -> String {
    match shape {
        PadShape::Circle => "circle".to_string(),
        PadShape::Rect => "rect".to_string(),
        PadShape::RoundRect { .. } => "roundrect".to_string(),
        PadShape::Oblong => "oblong".to_string(),
    }
}

#[cfg(all(test, feature = "native"))]
mod tests {
    use super::*;

    #[test]
    fn test_engine_new() {
        let engine = PcbEngine::new();
        assert!(engine.source.is_empty());
    }

    #[test]
    fn test_load_source_success() {
        let mut engine = PcbEngine::new();
        let error = engine.load_source(
            r#"
            version 1
            board test {
                size 100mm x 80mm
                layers 2
            }
            "#,
        );
        assert!(error.is_empty(), "Unexpected error: {}", error);
    }

    #[test]
    fn test_load_source_with_component() {
        let mut engine = PcbEngine::new();
        let error = engine.load_source(
            r#"
            version 1
            board test {
                size 100mm x 80mm
                layers 2
            }
            component R1 resistor "0402" {
                value "10k"
                at 10mm, 10mm
            }
            "#,
        );
        assert!(error.is_empty(), "Unexpected error: {}", error);

        let snapshot = engine.build_snapshot();
        assert!(snapshot.board.is_some());
        assert_eq!(snapshot.components.len(), 1);
        assert_eq!(snapshot.components[0].refdes, "R1");
        assert_eq!(snapshot.components[0].value, "10k");
    }

    #[test]
    fn test_load_source_parse_error() {
        let mut engine = PcbEngine::new();
        let error = engine.load_source("invalid { syntax");
        assert!(!error.is_empty());
    }

    #[test]
    fn test_build_snapshot_empty() {
        let mut engine = PcbEngine::new();
        let snapshot = engine.build_snapshot();
        assert!(snapshot.board.is_none());
        assert!(snapshot.components.is_empty());
        assert!(snapshot.nets.is_empty());
    }

    #[test]
    fn test_snapshot_with_nets() {
        let mut engine = PcbEngine::new();
        let error = engine.load_source(
            r#"
            version 1
            board test { size 50mm x 30mm layers 2 }
            component R1 resistor "0402" { at 10mm, 10mm }
            component R2 resistor "0402" { at 20mm, 10mm }
            net VCC {
                R1.1
                R2.1
            }
            "#,
        );
        assert!(error.is_empty(), "Unexpected error: {}", error);

        let snapshot = engine.build_snapshot();
        assert_eq!(snapshot.nets.len(), 1);
        assert_eq!(snapshot.nets[0].name, "VCC");
        assert_eq!(snapshot.nets[0].connections.len(), 2);
    }
}
