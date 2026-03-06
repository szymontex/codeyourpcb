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
    components::trace::{Trace, Via},
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

    /// Load routes from a .routes file content string.
    ///
    /// This parses the routes file format and adds Trace/Via entities
    /// to the world. Existing autorouted traces are cleared first.
    ///
    /// Returns an empty string on success, or error message on failure.
    #[cfg(feature = "native")]
    pub fn load_routes(&mut self, routes_content: &str) -> String {
        let mut errors: Vec<String> = Vec::new();

        // Clear existing autorouted traces and vias
        self.clear_autorouted_traces();

        // Parse routes file
        for line in routes_content.lines() {
            let line = line.trim();

            // Skip comments and empty lines
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Parse segment: net_id layer width_nm x1 y1 x2 y2
            if line.starts_with("segment ") {
                if let Err(e) = self.parse_route_segment(line) {
                    errors.push(format!("Invalid segment: {} - {}", line, e));
                }
            }
            // Parse via: net_id x y drill_nm start_layer end_layer
            else if line.starts_with("via ") {
                if let Err(e) = self.parse_route_via(line) {
                    errors.push(format!("Invalid via: {} - {}", line, e));
                }
            }
            // Skip other lines (version, metrics, etc.)
        }

        if errors.is_empty() {
            String::new()
        } else {
            errors.join("\n")
        }
    }

    /// Clear autorouted traces and vias from the world.
    fn clear_autorouted_traces(&mut self) {
        use cypcb_world::components::trace::{Trace, TraceSource, Via};

        // Collect entities to remove
        let entities_to_remove: Vec<Entity> = {
            let ecs = self.world.ecs_mut();
            let mut trace_query = ecs.query::<(Entity, &Trace)>();
            let trace_entities: Vec<Entity> = trace_query
                .iter(ecs)
                .filter(|(_, trace)| trace.source == TraceSource::Autorouted && !trace.locked)
                .map(|(entity, _)| entity)
                .collect();
            trace_entities
        };

        let via_entities_to_remove: Vec<Entity> = {
            let ecs = self.world.ecs_mut();
            let mut via_query = ecs.query::<(Entity, &Via)>();
            let via_entities: Vec<Entity> = via_query
                .iter(ecs)
                .filter(|(_, via)| !via.locked)
                .map(|(entity, _)| entity)
                .collect();
            via_entities
        };

        // Remove entities
        let ecs = self.world.ecs_mut();
        for entity in entities_to_remove {
            ecs.despawn(entity);
        }
        for entity in via_entities_to_remove {
            ecs.despawn(entity);
        }
    }

    /// Parse a segment line from routes file.
    #[cfg(feature = "native")]
    fn parse_route_segment(&mut self, line: &str) -> Result<(), String> {
        use cypcb_world::components::trace::{Trace, TraceSegment, TraceSource};

        // segment net_id layer width_nm x1 y1 x2 y2
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() != 8 {
            return Err(format!("expected 8 parts, got {}", parts.len()));
        }

        let net_id_num: u32 = parts[1].parse().map_err(|e| format!("net_id: {}", e))?;
        let layer_str = parts[2];
        let width: i64 = parts[3].parse().map_err(|e| format!("width: {}", e))?;
        let x1: i64 = parts[4].parse().map_err(|e| format!("x1: {}", e))?;
        let y1: i64 = parts[5].parse().map_err(|e| format!("y1: {}", e))?;
        let x2: i64 = parts[6].parse().map_err(|e| format!("x2: {}", e))?;
        let y2: i64 = parts[7].parse().map_err(|e| format!("y2: {}", e))?;

        // Parse layer
        let layer = parse_layer(layer_str)?;

        // Create trace
        let trace = Trace {
            segments: vec![TraceSegment::new(
                Point::new(Nm(x1), Nm(y1)),
                Point::new(Nm(x2), Nm(y2)),
            )],
            width: Nm(width),
            layer,
            net_id: NetId::new(net_id_num),
            locked: false,
            source: TraceSource::Autorouted,
        };

        self.world.spawn_entity(trace);
        Ok(())
    }

    /// Parse a via line from routes file.
    #[cfg(feature = "native")]
    fn parse_route_via(&mut self, line: &str) -> Result<(), String> {
        use cypcb_world::components::trace::Via;

        // via net_id x y drill_nm start_layer end_layer
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() != 7 {
            return Err(format!("expected 7 parts, got {}", parts.len()));
        }

        let net_id_num: u32 = parts[1].parse().map_err(|e| format!("net_id: {}", e))?;
        let x: i64 = parts[2].parse().map_err(|e| format!("x: {}", e))?;
        let y: i64 = parts[3].parse().map_err(|e| format!("y: {}", e))?;
        let drill: i64 = parts[4].parse().map_err(|e| format!("drill: {}", e))?;
        let start_layer_str = parts[5];
        let end_layer_str = parts[6];

        let start_layer = parse_layer(start_layer_str)?;
        let end_layer = parse_layer(end_layer_str)?;

        let via = Via {
            position: Point::new(Nm(x), Nm(y)),
            drill: Nm(drill),
            outer_diameter: Nm(drill * 2), // Default annular ring
            start_layer,
            end_layer,
            net_id: NetId::new(net_id_num),
            locked: false,
        };

        self.world.spawn_entity(via);
        Ok(())
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

        // Build map of component.pin -> net_id from snapshot.nets
        // This is needed to populate NetConnections for each component (for DRC)
        let mut pin_to_net: std::collections::HashMap<String, NetId> =
            std::collections::HashMap::new();
        for net in &snapshot.nets {
            // Intern the net name to get a NetId
            let net_id = self.world.intern_net(&net.name);
            for conn in &net.connections {
                let key = format!("{}.{}", conn.component, conn.pin);
                pin_to_net.insert(key, net_id);
            }
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

        // Create component entities with proper NetConnections
        for comp in &snapshot.components {
            let refdes = RefDes::new(&comp.refdes);
            let value = Value::new(&comp.value);
            let position = Position(Point::new(Nm(comp.x_nm), Nm(comp.y_nm)));
            let rotation = Rotation(comp.rotation_mdeg);
            let footprint_ref = FootprintRef::new(&comp.footprint);

            // Build NetConnections from pin_to_net map
            let mut nets = NetConnections::new();
            // Get pad numbers from footprint library (since JS parser may not have pads)
            if let Some(fp) = self.footprint_lib.get(&comp.footprint) {
                for pad in &fp.pads {
                    let key = format!("{}.{}", comp.refdes, pad.number);
                    if let Some(&net_id) = pin_to_net.get(&key) {
                        nets.add(PinConnection::new(&pad.number, net_id));
                    }
                }
            }

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

        // Build trace info
        let traces = self.collect_traces();

        // Build via info
        let vias = self.collect_vias();

        // Build ratsnest info (unrouted connections)
        let ratsnest = self.collect_ratsnest(&nets);

        BoardSnapshot {
            board,
            components,
            nets,
            violations,
            traces,
            vias,
            ratsnest,
        }
    }

    /// Collect all traces from the world.
    fn collect_traces(&mut self) -> Vec<TraceInfo> {
        // First, collect trace data (cloning to avoid borrow issues)
        let trace_data: Vec<Trace> = {
            let world_ref = self.world.ecs_mut();
            let mut query = world_ref.query::<&Trace>();
            query.iter(world_ref).cloned().collect()
        };

        // Now process with net names
        let mut traces: Vec<TraceInfo> = Vec::new();
        for trace in trace_data {
            let layer_name = match trace.layer {
                Layer::TopCopper => "Top".to_string(),
                Layer::BottomCopper => "Bottom".to_string(),
                Layer::Inner(n) => format!("Inner{}", n),
                _ => "Top".to_string(),
            };

            let net_name = self.world.net_name(trace.net_id)
                .unwrap_or("(no net)")
                .to_string();

            let segments: Vec<TraceSegmentInfo> = trace.segments.iter().map(|seg| {
                TraceSegmentInfo {
                    start_x: seg.start.x.0 as f64,
                    start_y: seg.start.y.0 as f64,
                    end_x: seg.end.x.0 as f64,
                    end_y: seg.end.y.0 as f64,
                }
            }).collect();

            traces.push(TraceInfo {
                segments,
                width: trace.width.0 as f64,
                layer: layer_name,
                net_name,
                locked: trace.locked,
            });
        }

        traces
    }

    /// Collect all vias from the world.
    fn collect_vias(&mut self) -> Vec<ViaInfo> {
        // First, collect via data (copying to avoid borrow issues)
        let via_data: Vec<Via> = {
            let world_ref = self.world.ecs_mut();
            let mut query = world_ref.query::<&Via>();
            query.iter(world_ref).copied().collect()
        };

        // Now process with net names
        let mut vias: Vec<ViaInfo> = Vec::new();
        for via in via_data {
            let net_name = self.world.net_name(via.net_id)
                .unwrap_or("(no net)")
                .to_string();

            vias.push(ViaInfo {
                x: via.position.x.0 as f64,
                y: via.position.y.0 as f64,
                drill: via.drill.0 as f64,
                outer_diameter: via.outer_diameter.0 as f64,
                net_name,
            });
        }

        vias
    }

    /// Calculate ratsnest (unrouted connections).
    ///
    /// For each net with multiple pins, if there are no traces connecting
    /// all pins, we show ratsnest lines between unconnected pin pairs.
    ///
    /// Simple algorithm: For nets with pins but no traces, show lines
    /// from first pin to all other pins (star topology for visualization).
    fn collect_ratsnest(&mut self, nets: &[NetInfo]) -> Vec<RatsnestInfo> {
        use std::collections::HashMap;

        let mut ratsnest: Vec<RatsnestInfo> = Vec::new();

        // Get trace count per net to determine if net is routed
        let mut traces_per_net: HashMap<String, usize> = HashMap::new();
        for trace in self.collect_traces() {
            *traces_per_net.entry(trace.net_name.clone()).or_insert(0) += 1;
        }

        // For each net with connections
        for net in nets {
            if net.connections.len() < 2 {
                continue; // Need at least 2 pins to show ratsnest
            }

            // If net has traces, assume it's at least partially routed
            // (A full ratsnest would check actual connectivity, but this is MVP)
            if traces_per_net.contains_key(&net.name) {
                continue;
            }

            // Get pin positions
            let mut pin_positions: Vec<(f64, f64)> = Vec::new();

            for conn in &net.connections {
                // Find the component
                if let Some(entity) = self.world.find_by_refdes(&conn.component) {
                    if let Some(pos) = self.world.get::<Position>(entity) {
                        // Get the pad offset from footprint
                        let footprint_name = self.world.get::<FootprintRef>(entity)
                            .map(|f| f.as_str().to_string())
                            .unwrap_or_default();

                        let pad_offset = self.get_pad_offset(&footprint_name, &conn.pin);
                        let rotation = self.world.get::<Rotation>(entity)
                            .map(|r| r.0)
                            .unwrap_or(0);

                        // Apply rotation to pad offset
                        let radians = (rotation as f64 / 1000.0) * (std::f64::consts::PI / 180.0);
                        let cos = radians.cos();
                        let sin = radians.sin();

                        let rotated_x = pad_offset.0 * cos - pad_offset.1 * sin;
                        let rotated_y = pad_offset.0 * sin + pad_offset.1 * cos;

                        let pin_x = pos.0.x.0 as f64 + rotated_x;
                        let pin_y = pos.0.y.0 as f64 + rotated_y;

                        pin_positions.push((pin_x, pin_y));
                    }
                }
            }

            // Create star-topology ratsnest from first pin to all others
            if pin_positions.len() >= 2 {
                let (first_x, first_y) = pin_positions[0];
                for (x, y) in pin_positions.iter().skip(1) {
                    ratsnest.push(RatsnestInfo {
                        start_x: first_x,
                        start_y: first_y,
                        end_x: *x,
                        end_y: *y,
                        net_name: net.name.clone(),
                    });
                }
            }
        }

        ratsnest
    }

    /// Get pad offset from component origin for a given footprint and pin.
    fn get_pad_offset(&self, footprint_name: &str, pin: &str) -> (f64, f64) {
        if let Some(fp) = self.footprint_lib.get(footprint_name) {
            for pad in &fp.pads {
                if pad.number == pin {
                    return (pad.position.x.0 as f64, pad.position.y.0 as f64);
                }
            }
        }
        // Default to origin if pad not found
        (0.0, 0.0)
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

/// Parse layer string from routes file format.
fn parse_layer(layer_str: &str) -> Result<Layer, String> {
    match layer_str {
        "TopCopper" | "Top" => Ok(Layer::TopCopper),
        "BottomCopper" | "Bottom" => Ok(Layer::BottomCopper),
        _ if layer_str.starts_with("Inner(") && layer_str.ends_with(")") => {
            let inner = &layer_str[6..layer_str.len() - 1];
            let num: u8 = inner.parse().map_err(|e| format!("Invalid inner layer: {}", e))?;
            Ok(Layer::Inner(num))
        }
        _ if layer_str.starts_with("Inner") => {
            let num_str = &layer_str[5..];
            let num: u8 = num_str.parse().map_err(|e| format!("Invalid inner layer: {}", e))?;
            Ok(Layer::Inner(num))
        }
        _ => Err(format!("Unknown layer: {}", layer_str)),
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

    #[test]
    fn test_drc_detects_clearance_violations() {
        // Test clearance violation detection using native source parsing
        let mut engine = PcbEngine::new();
        let error = engine.load_source(
            r#"
            version 1
            board drc_test { size 30mm x 30mm layers 2 }
            component R1 resistor "0402" {
                value "10k"
                at 10mm, 15mm
            }
            component R2 resistor "0402" {
                value "10k"
                at 10.5mm, 15mm
            }
            "#,
        );
        assert!(error.is_empty(), "Unexpected error: {}", error);

        // With components 0.5mm apart and 0402 footprints (1.5mm courtyard),
        // the courtyards overlap significantly, so clearance should be violated
        let violations = engine.violation_count();
        assert!(violations > 0, "Expected clearance violations but found {}", violations);
    }

    #[test]
    fn test_drc_from_snapshot_detects_violations() {
        // Simulate WASM mode by creating a snapshot and loading it
        use crate::snapshot::*;

        let snapshot = BoardSnapshot {
            board: Some(BoardInfo {
                name: "drc_test".to_string(),
                width_nm: 30_000_000,  // 30mm
                height_nm: 30_000_000, // 30mm
                layer_count: 2,
            }),
            components: vec![
                ComponentInfo {
                    refdes: "R1".to_string(),
                    value: "10k".to_string(),
                    x_nm: 10_000_000,   // 10mm
                    y_nm: 15_000_000,   // 15mm
                    rotation_mdeg: 0,
                    footprint: "0402".to_string(),
                    pads: vec![], // Empty - should use builtin library
                },
                ComponentInfo {
                    refdes: "R2".to_string(),
                    value: "10k".to_string(),
                    x_nm: 10_500_000,   // 10.5mm (0.5mm from R1)
                    y_nm: 15_000_000,   // 15mm
                    rotation_mdeg: 0,
                    footprint: "0402".to_string(),
                    pads: vec![], // Empty - should use builtin library
                },
            ],
            nets: vec![],
            violations: vec![],
            traces: vec![],
            vias: vec![],
            ratsnest: vec![],
        };

        let mut engine = PcbEngine::new();
        engine.populate_from_snapshot(&snapshot);
        engine.run_drc_internal();

        // Check spatial index was built
        let spatial_count = engine.world.spatial().len();
        assert_eq!(spatial_count, 2, "Spatial index should have 2 entries, found {}", spatial_count);

        // Check for violations
        let violations = engine.violation_count();
        assert!(violations > 0, "Expected clearance violations but found {} - spatial entries: {}",
            violations, spatial_count);
    }
}
