//! CodeYourPCB WASM Rendering Bridge
//!
//! This crate provides the interface for the CodeYourPCB web viewer.
//! It bridges Rust board data to JavaScript, enabling the web UI to:
//!
//! - Load and parse `.cypcb` source files
//! - Retrieve structured board snapshots for rendering
//! - Query components at specific coordinates
//!
//! # Architecture
//!
//! The rendering happens in JavaScript/Canvas - this crate only provides data.
//! The `PcbEngine` struct maintains the board state and provides query methods.

mod snapshot;

pub use snapshot::*;

use cypcb_core::{Nm, Point};
use cypcb_parser::parse;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{
    sync_ast_to_world, BoardWorld, Entity, FootprintRef, Layer, NetConnections, NetId, PadShape,
    PinConnection, Position, RefDes, Rotation, Value,
};

/// PCB Engine - main interface for JavaScript.
///
/// Maintains the board state and provides methods for loading source,
/// getting snapshots, and querying the board.
pub struct PcbEngine {
    world: BoardWorld,
    footprint_lib: FootprintLibrary,
    source: String,
}

impl PcbEngine {
    /// Create a new PcbEngine instance.
    pub fn new() -> PcbEngine {
        PcbEngine {
            world: BoardWorld::new(),
            footprint_lib: FootprintLibrary::new(),
            source: String::new(),
        }
    }

    /// Load and parse source code.
    ///
    /// Returns an empty string on success, or an error message on failure.
    /// The board state is updated even if there are errors (partial results).
    pub fn load_source(&mut self, source: &str) -> String {
        self.source = source.to_string();
        self.world.clear();

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

        if errors.is_empty() {
            String::new()
        } else {
            errors.join("\n")
        }
    }

    /// Get a snapshot of the current board state for rendering.
    pub fn get_snapshot(&mut self) -> BoardSnapshot {
        self.build_snapshot()
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

    /// Build a BoardSnapshot from the current world state.
    fn build_snapshot(&mut self) -> BoardSnapshot {
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

        BoardSnapshot {
            board,
            components,
            nets,
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

#[cfg(test)]
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
