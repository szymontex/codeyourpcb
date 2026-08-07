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
    components::trace::{Trace, Via},
    BoardWorld, Entity, FootprintRef, Layer, NetConnections, NetId, PadInstance, PadShape,
    PinConnection, Position, RefDes, Rotation, Value,
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
    /// Cached net constraints from last parse (net_name → constraints).
    #[cfg(feature = "native")]
    net_constraints: std::collections::HashMap<String, NetConstraintCache>,
    /// Package name -> 3D model identifier, supplied by the host.
    ///
    /// The same story as a fetched footprint: nothing in a `.cypcb` file says
    /// which 3D model a package has, and the viewer learns it from a supplier
    /// at runtime.
    model_3d: std::collections::HashMap<String, String>,
}

/// Cached net constraint data extracted during parsing.
#[derive(Debug, Clone, Default)]
pub struct NetConstraintCache {
    /// Trace width in nm (from `[width ...]`).
    pub width_nm: Option<i64>,
    /// Clearance in nm (from `[clearance ...]`).
    pub clearance_nm: Option<i64>,
    /// Current in milliamps (from `[current ...]`).
    pub current_ma: Option<f64>,
}

// WASM-exposed methods
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl PcbEngine {
    /// Create a new PcbEngine instance.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(constructor))]
    pub fn new() -> PcbEngine {
        console_error_panic_hook::set_once();
        PcbEngine {
            world: BoardWorld::new(),
            footprint_lib: FootprintLibrary::new(),
            source: String::new(),
            violations: Vec::new(),
            drc_duration_ms: 0,
            #[cfg(feature = "native")]
            net_constraints: std::collections::HashMap::new(),
            model_3d: std::collections::HashMap::new(),
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

    /// Load a pre-parsed board snapshot from JSON (native mode).
    ///
    /// The browser hands the engine a snapshot as a `JsValue`; natively the
    /// same thing arrives as JSON, so the path a viewer takes can be tested
    /// without a browser. Without this the only proof that a zone sent in
    /// comes back as filled copper would be someone opening the page.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load_snapshot_json(&mut self, snapshot_json: &str) -> String {
        match serde_json::from_str::<BoardSnapshot>(snapshot_json) {
            Ok(snapshot) => {
                self.populate_from_snapshot(&snapshot);
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

    /// Register a footprint the host fetched at runtime (WASM version).
    ///
    /// A `.cypcb` file names a package - "LQFP-48" - without describing its
    /// pads, and for a part that is not in the built-in library the viewer
    /// fetches the geometry from a supplier after the fact. Registering it here
    /// puts it in the same library the parser resolves against, so the next
    /// `load_source` places real copper instead of an empty outline.
    ///
    /// This is what lets the engine own parsing. Without it the viewer has to
    /// keep its own parser purely so it can consult its own footprint registry.
    ///
    /// Registrations survive re-parsing: a footprint the design file defines
    /// itself still wins while it exists, and this one comes back when it goes.
    ///
    /// Returns an empty string on success, or the deserialisation error.
    #[cfg(target_arch = "wasm32")]
    pub fn register_footprint(
        &mut self,
        name: &str,
        pads_js: wasm_bindgen::JsValue,
        silk_js: wasm_bindgen::JsValue,
    ) -> String {
        let pads = match serde_wasm_bindgen::from_value::<Vec<PadInfo>>(pads_js) {
            Ok(pads) => pads,
            Err(e) => return format!("Failed to deserialize pads: {}", e),
        };
        // Silk is optional: a footprint may arrive with pads and no legend.
        let silk = if silk_js.is_undefined() || silk_js.is_null() {
            Vec::new()
        } else {
            match serde_wasm_bindgen::from_value::<Vec<SilkInfo>>(silk_js) {
                Ok(silk) => silk,
                Err(e) => return format!("Failed to deserialize silk: {}", e),
            }
        };
        self.register_footprint_pads(name, &pads, &silk);
        String::new()
    }

    /// Register a footprint the host fetched at runtime (native version).
    ///
    /// Takes the pads as a JSON array. See the WASM counterpart for why this
    /// exists.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn register_footprint(&mut self, name: &str, pads_json: &str, silk_json: &str) -> String {
        let pads = match serde_json::from_str::<Vec<PadInfo>>(pads_json) {
            Ok(pads) => pads,
            Err(e) => return format!("Failed to deserialize pads: {}", e),
        };
        let silk = if silk_json.trim().is_empty() {
            Vec::new()
        } else {
            match serde_json::from_str::<Vec<SilkInfo>>(silk_json) {
                Ok(silk) => silk,
                Err(e) => return format!("Failed to deserialize silk: {}", e),
            }
        };
        self.register_footprint_pads(name, &pads, &silk);
        String::new()
    }

    /// Minimum trace width for a current, in nanometers.
    ///
    /// IPC-2221 for an external layer, 10C rise, 1 oz copper - the same
    /// defaults the router and the language server use, because all three go
    /// through `cypcb-calc`. The viewer carried three copies of this
    /// arithmetic and the language server a fourth, and they had already
    /// drifted apart on how thick an ounce of copper is.
    ///
    /// Returns 0 for a current of zero or less: no constraint to check.
    pub fn min_trace_width_for_current_ma(&self, current_ma: f64) -> f64 {
        if current_ma <= 0.0 {
            return 0.0;
        }
        cypcb_calc::TraceWidthCalculator::min_width_for_current(current_ma / 1000.0, true).0 as f64
    }

    /// Record which 3D model a package uses.
    ///
    /// Takes plain strings, so the same method serves both targets. Like
    /// [`register_footprint`](Self::register_footprint), this exists because
    /// the fact arrives from a supplier after the source was written, and the
    /// engine has to own it for the viewer to stop keeping its own copy of the
    /// board model.
    pub fn register_3d_model(&mut self, package: &str, model: &str) {
        self.model_3d.insert(package.to_string(), model.to_string());
    }

    /// Parse `.cypcb` source and load it into the board model.
    ///
    /// Available wherever the tree-sitter parser is compiled in, which includes
    /// wasm32 - the parser builds for that target and the resulting module is
    /// no larger than one that leaves parsing to JavaScript.
    ///
    /// Returns an empty string on success, or the collected parse and semantic
    /// errors joined by newlines. The board state is updated even when there are
    /// errors, so partial results stay visible. DRC runs afterwards either way.
    #[cfg(feature = "native")]
    pub fn load_source(&mut self, source: &str) -> String {
        self.source = source.to_string();
        self.world.clear();
        self.violations.clear();
        self.drc_duration_ms = 0;
        self.net_constraints.clear();

        // Parse the source
        let parse_result = parse(source);

        // Collect parse errors
        let mut errors: Vec<String> = Vec::new();
        for e in &parse_result.errors {
            errors.push(format!("{}", e));
        }

        // Extract net constraints from AST before sync
        for def in &parse_result.value.definitions {
            if let cypcb_parser::ast::Definition::Net(net_def) = def {
                if let Some(ref constraints) = net_def.constraints {
                    let mut cache = NetConstraintCache::default();
                    if let Some(ref w) = constraints.width {
                        cache.width_nm = Some(w.to_nm().0);
                    }
                    if let Some(ref c) = constraints.clearance {
                        cache.clearance_nm = Some(c.to_nm().0);
                    }
                    if let Some(ref cur) = constraints.current {
                        cache.current_ma = Some(cur.to_milliamps());
                    }
                    self.net_constraints
                        .insert(net_def.name.value.clone(), cache);
                }
            }
        }

        // Sync AST to world
        let sync_result = sync_ast_to_world(
            &parse_result.value,
            source,
            &mut self.world,
            &mut self.footprint_lib,
        );

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

    // ========================================================================
    // Trace Mutation API
    // ========================================================================

    /// Add a trace to the board.
    ///
    /// Creates a Manual Trace entity with the given parameters,
    /// updates the spatial index, and returns the entity index.
    ///
    /// `segments_json` is a JSON array of `[x1, y1, x2, y2, ...]` coordinate
    /// pairs in nanometers (flat array, 4 values per segment).
    ///
    /// Returns the entity index (u32) on success, or u32::MAX on error.
    pub fn add_trace(
        &mut self,
        net_name: &str,
        layer_str: &str,
        width_nm: i64,
        segments_flat: &[i64],
    ) -> u32 {
        use cypcb_world::components::trace::{Trace as TraceComp, TraceSegment, TraceSource};

        // Parse layer
        let layer = match parse_layer(layer_str) {
            Ok(l) => l,
            Err(_) => return u32::MAX,
        };

        // Segments come as flat array: [x1, y1, x2, y2, x1, y1, x2, y2, ...]
        if segments_flat.len() < 4 || !segments_flat.len().is_multiple_of(4) {
            return u32::MAX;
        }

        let mut segments = Vec::with_capacity(segments_flat.len() / 4);
        for chunk in segments_flat.chunks_exact(4) {
            segments.push(TraceSegment::new(
                Point::new(Nm(chunk[0]), Nm(chunk[1])),
                Point::new(Nm(chunk[2]), Nm(chunk[3])),
            ));
        }

        // Intern the net name
        let net_id = self.world.intern_net(net_name);

        let trace = TraceComp {
            segments,
            width: Nm(width_nm),
            layer,
            net_id,
            locked: false,
            source: TraceSource::Manual,
        };

        let entity = self.world.spawn_entity((trace, net_id));

        // Rebuild spatial index to include the new trace
        self.rebuild_spatial_index_full();

        entity.index()
    }

    /// Add a trace from a JSON segments string (WASM-friendly).
    ///
    /// `segments_json` is a JSON array of flat coordinates:
    /// `[x1, y1, x2, y2, x3, y3, x4, y4, ...]` (4 values per segment).
    ///
    /// Returns the entity index (u32) on success, or u32::MAX on error.
    #[cfg(target_arch = "wasm32")]
    pub fn add_trace_json(
        &mut self,
        net_name: &str,
        layer_str: &str,
        width_nm: i64,
        segments_json: &str,
    ) -> u32 {
        let coords: Vec<i64> = match serde_json::from_str(segments_json) {
            Ok(v) => v,
            Err(_) => return u32::MAX,
        };
        self.add_trace(net_name, layer_str, width_nm, &coords)
    }

    /// Remove a trace by entity index.
    ///
    /// Returns `true` if the trace was found and removed, `false` otherwise.
    pub fn remove_trace(&mut self, trace_id: u32) -> bool {
        // Find the actual entity with this index among trace entities
        let entity_to_remove = self.find_trace_entity(trace_id);

        if let Some(entity) = entity_to_remove {
            self.world.ecs_mut().despawn(entity);
            self.rebuild_spatial_index_full();
            true
        } else {
            false
        }
    }

    /// Query for a trace entity at a given point with tolerance.
    ///
    /// Returns the trace entity index if found, or u32::MAX if not.
    /// Tolerance is in nanometers — the point is expanded into a
    /// query rectangle of `2*tolerance` side length.
    pub fn get_trace_at_point(&mut self, x_nm: i64, y_nm: i64, tolerance_nm: i64) -> u32 {
        use cypcb_core::Rect;
        use cypcb_world::components::trace::Trace as TraceComp;

        let query_rect = Rect::new(
            Point::new(Nm(x_nm - tolerance_nm), Nm(y_nm - tolerance_nm)),
            Point::new(Nm(x_nm + tolerance_nm), Nm(y_nm + tolerance_nm)),
        );

        let candidates = self.world.query_region(query_rect);
        let query_point = Point::new(Nm(x_nm), Nm(y_nm));

        // Find the closest trace among candidates
        let mut best_entity: Option<u32> = None;
        let mut best_dist: i64 = i64::MAX;

        for entity in candidates {
            let trace_opt: Option<&TraceComp> = self.world.get::<TraceComp>(entity);
            if let Some(trace) = trace_opt {
                // Check point-to-segment distance for each segment
                let half_width = trace.width.0 / 2;
                for seg in &trace.segments {
                    let dist = point_to_segment_distance(query_point, seg.start, seg.end);
                    // If within the copper width, it's a hit
                    if dist <= half_width + tolerance_nm && dist < best_dist {
                        best_dist = dist;
                        best_entity = Some(entity.index());
                    }
                }
            }
        }

        best_entity.unwrap_or(u32::MAX)
    }

    /// Run DRC and return violation count.
    ///
    /// This runs a full DRC check (incremental optimization deferred)
    /// and updates the internal violation list.
    pub fn run_drc_incremental(&mut self) -> usize {
        self.run_drc_internal();
        self.violations.len()
    }

    /// Rotate a component by delta millidegrees.
    ///
    /// Finds the component by reference designator and applies the rotation.
    /// Returns `true` on success, `false` if the component was not found.
    pub fn rotate_component(&mut self, refdes: &str, delta_mdeg: i32) -> bool {
        let ok = self.world.rotate_component(refdes, delta_mdeg);
        if ok {
            self.rebuild_spatial_index_full();
            self.run_drc_internal();
        }
        ok
    }

    /// Set the board outline size in nanometers.
    ///
    /// Returns `true` on success, `false` if no board entity exists.
    pub fn set_board_size(&mut self, width_nm: i64, height_nm: i64) -> bool {
        let ok = self.world.set_board_size(Nm(width_nm), Nm(height_nm));
        if ok {
            self.run_drc_internal();
        }
        ok
    }

    /// Get the number of trace entities in the world.
    pub fn trace_count(&mut self) -> usize {
        let ecs = self.world.ecs_mut();
        let mut query = ecs.query::<&cypcb_world::components::trace::Trace>();
        query.iter(ecs).count()
    }

    /// Export all traces and vias as DSL `trace` blocks.
    ///
    /// Iterates all Trace and Via entities in the ECS, groups them by net,
    /// and emits properly formatted DSL trace blocks with `path` coordinates.
    ///
    /// Coordinates use 6 decimal places in mm for deterministic round-trip:
    /// nm → mm string → parse → nm gives exactly the original value.
    ///
    /// Returns an empty string if there are no traces.
    pub fn export_traces_as_dsl(&mut self) -> String {
        cypcb_world::dsl::traces_as_dsl(&mut self.world)
    }

    /// Get the minimum copper clearance in nanometers.
    ///
    /// Returns the clearance value from the active design rules (default preset).
    /// Used by the JS routing engine to enforce clearance during interactive routing.
    pub fn get_min_clearance_nm(&self) -> i64 {
        DesignRules::default().min_clearance.0
    }

    /// Get DRC violations as JSON string (WASM-friendly).
    #[cfg(target_arch = "wasm32")]
    pub fn get_violations_json(&self) -> String {
        let violations: Vec<ViolationInfo> = self
            .violations
            .iter()
            .map(ViolationInfo::from_drc)
            .collect();
        serde_json::to_string(&violations).unwrap_or_else(|_| "[]".to_string())
    }

    /// Get DRC violations as JSON string (native).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn get_violations_json(&self) -> String {
        let violations: Vec<ViolationInfo> = self
            .violations
            .iter()
            .map(ViolationInfo::from_drc)
            .collect();
        serde_json::to_string(&violations).unwrap_or_else(|_| "[]".to_string())
    }

    /// Run the built-in A* autorouter on the current board.
    ///
    /// Clears existing autorouted traces, routes all unrouted nets,
    /// applies the results, and rebuilds the spatial index.
    ///
    /// Returns a JSON status string: `{"ok":true,"routed":N,"unrouted":N}` on success,
    /// or `{"ok":false,"error":"..."}` on failure.
    pub fn auto_route(&mut self) -> String {
        use cypcb_autoroute::{route_board, AutorouteConfig};
        use cypcb_router::apply_routes;
        use cypcb_rules::presets::{PresetRuleSet, RulesPreset};

        // Clear existing autorouted traces first
        self.clear_autorouted_traces();

        let preset = RulesPreset::from_name("jlcpcb").expect("jlcpcb preset must exist");
        let rules = PresetRuleSet::new(preset);
        let config = AutorouteConfig::default();

        let result = route_board(&mut self.world, &self.footprint_lib, &rules, &config);

        let routed = result.route_count();

        if result.status.is_failed() {
            let reason = match &result.status {
                cypcb_router::RoutingStatus::Failed { reason } => reason.clone(),
                _ => "Unknown routing error".into(),
            };
            format!(
                r#"{{"ok":false,"error":"{}"}}"#,
                reason.replace('"', r#"\""#)
            )
        } else {
            let unrouted = match &result.status {
                cypcb_router::RoutingStatus::Partial { unrouted_count } => *unrouted_count,
                _ => 0,
            };
            apply_routes(&mut self.world, &result);
            self.rebuild_spatial_index_full();
            self.run_drc_internal();
            format!(
                r#"{{"ok":true,"routed":{},"unrouted":{}}}"#,
                routed, unrouted
            )
        }
    }

    /// Run the autorouter with user-specified tuning parameters.
    ///
    /// `params_json` is a JSON string with fields:
    /// - `via_cost`: f64 (0.1–10.0, default 1.0) — higher = fewer vias
    /// - `layer_preference`: f64 (-1.0–1.0, default 0.0) — layer bias
    /// - `roundness`: f64 (0.0–1.0, default 0.5) — chamfer aggressiveness
    /// - `density`: f64 (0.5–2.0, default 1.0) — grid density multiplier
    ///
    /// Missing fields use defaults. Values are clamped to valid ranges.
    ///
    /// Returns a JSON status string: `{"ok":true,"routed":N,"unrouted":N}` on success,
    /// or `{"ok":false,"error":"..."}` on failure.
    pub fn auto_route_with_params(&mut self, params_json: String) -> String {
        use cypcb_autoroute::{route_board, AutorouteConfig, AutorouteParams};
        use cypcb_router::apply_routes;
        use cypcb_rules::presets::{PresetRuleSet, RulesPreset};

        // Deserialize params
        let params: AutorouteParams = match serde_json::from_str(&params_json) {
            Ok(p) => {
                let clamped = AutorouteParams::clamped(&p);
                tracing::info!(
                    via_cost = clamped.via_cost,
                    layer_preference = clamped.layer_preference,
                    roundness = clamped.roundness,
                    density = clamped.density,
                    "auto_route_with_params: received params"
                );
                clamped
            }
            Err(e) => {
                return format!(
                    r#"{{"ok":false,"error":"Invalid params JSON: {}"}}"#,
                    e.to_string().replace('"', r#"\""#)
                );
            }
        };

        // Clear existing autorouted traces first
        self.clear_autorouted_traces();

        let preset = RulesPreset::from_name("jlcpcb").expect("jlcpcb preset must exist");
        let rules = PresetRuleSet::new(preset);
        let config = AutorouteConfig {
            params,
            ..AutorouteConfig::default()
        };

        let result = route_board(&mut self.world, &self.footprint_lib, &rules, &config);

        let routed = result.route_count();

        if result.status.is_failed() {
            let reason = match &result.status {
                cypcb_router::RoutingStatus::Failed { reason } => reason.clone(),
                _ => "Unknown routing error".into(),
            };
            format!(
                r#"{{"ok":false,"error":"{}"}}"#,
                reason.replace('"', r#"\""#)
            )
        } else {
            let unrouted = match &result.status {
                cypcb_router::RoutingStatus::Partial { unrouted_count } => *unrouted_count,
                _ => 0,
            };
            apply_routes(&mut self.world, &result);
            self.rebuild_spatial_index_full();
            self.run_drc_internal();
            format!(
                r#"{{"ok":true,"routed":{},"unrouted":{}}}"#,
                routed, unrouted
            )
        }
    }

    /// Generate multiple routing variants with different strategies/configs,
    /// rank them by composite score, and auto-apply the best.
    ///
    /// Run routing with debug output — returns JSON with intermediate pipeline stages.
    pub fn auto_route_debug(&mut self, params_json: String) -> String {
        use cypcb_autoroute::debug_route::route_with_debug;
        use cypcb_autoroute::AutorouteConfig;
        use cypcb_rules::presets::{PresetRuleSet, RulesPreset};

        let params: cypcb_autoroute::AutorouteParams =
            serde_json::from_str(&params_json).unwrap_or_default();

        let preset = RulesPreset::from_name("jlcpcb").expect("jlcpcb preset");
        let rules = PresetRuleSet::new(preset);
        let mut config = AutorouteConfig::default();
        config.params = params.clamped();
        config.via_cost_multiplier = config.params.via_cost;

        let debug_output = route_with_debug(&mut self.world, &self.footprint_lib, &rules, &config);

        serde_json::to_string(&debug_output)
            .unwrap_or_else(|e| format!(r#"{{"ok":false,"error":"serialize: {}"}}"#, e))
    }

    /// Returns a JSON array of variant results:
    /// `[{ "name": "...", "score": { ... }, "routes": [...], "vias": [...] }]`
    ///
    /// The best variant (lowest composite score) is auto-applied to the world.
    /// On error, returns `{"ok":false,"error":"..."}`.
    pub fn auto_route_variants(&mut self) -> String {
        use cypcb_autoroute::variant::{default_variant_configs, generate_variants};
        use cypcb_drc::DesignRules;
        use cypcb_rules::presets::{PresetRuleSet, RulesPreset};

        // Clear existing autorouted traces first
        self.clear_autorouted_traces();

        let preset = match RulesPreset::from_name("jlcpcb") {
            Some(p) => p,
            None => {
                return r#"{"ok":false,"error":"jlcpcb preset not found"}"#.to_string();
            }
        };
        let rules = PresetRuleSet::new(preset);
        let design_rules = DesignRules::default();
        let configs = default_variant_configs();

        let results = generate_variants(
            &mut self.world,
            &self.footprint_lib,
            &rules,
            &design_rules,
            &configs,
        );

        if results.is_empty() {
            return r#"{"ok":false,"error":"All variants failed"}"#.to_string();
        }

        // Rebuild spatial index and run DRC after best variant is applied
        self.rebuild_spatial_index_full();
        self.run_drc_internal();

        serde_json::to_string(&results).unwrap_or_else(|e| {
            format!(
                r#"{{"ok":false,"error":"Serialization failed: {}"}}"#,
                e.to_string().replace('"', r#"\""#)
            )
        })
    }
}

// Internal methods (not exposed to WASM)
impl PcbEngine {
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

        // Rebuild spatial index to include newly loaded traces/vias
        self.rebuild_spatial_index_full();

        if errors.is_empty() {
            String::new()
        } else {
            errors.join("\n")
        }
    }

    /// Rebuild the spatial index including components, traces, and vias.
    fn rebuild_spatial_index_full(&mut self) {
        use cypcb_world::components::trace::{Trace as TraceComp, Via};
        use cypcb_world::components::{FootprintRef, Position, Rotation};
        use cypcb_world::SpatialEntry;
        use std::collections::HashMap;

        let mut entries = Vec::new();

        // ---- Index individual pad entities ----
        // Each pad was spawned as a separate entity with PadInstance + NetId + Position.
        // We look up the footprint to get pad size/layer for the AABB.
        {
            let ecs = self.world.ecs_mut();
            let mut query = ecs.query::<(Entity, &PadInstance, &Position)>();
            let pad_entities: Vec<_> = query
                .iter(ecs)
                .map(|(e, pi, pos)| (e, pi.parent, pos.0))
                .collect();

            // Also need parent component's footprint to get pad sizes
            let mut fp_query = ecs.query::<(Entity, &FootprintRef, &Position, &Rotation)>();
            let comps: Vec<_> = fp_query
                .iter(ecs)
                .map(|(e, f, p, r)| (e, f.as_str().to_string(), p.0, r.0))
                .collect();

            // Build parent entity -> (footprint_name, comp_pos, rotation) map
            let comp_map: HashMap<u32, (&str, Point, i32)> = comps
                .iter()
                .map(|(e, f, p, r)| (e.index(), (f.as_str(), *p, *r)))
                .collect();

            for (entity, parent, pad_pos) in &pad_entities {
                // Find pad definition by matching position
                if let Some(&(fp_name, _comp_pos, _rotation)) = comp_map.get(&parent.index()) {
                    if let Some(fp) = self.footprint_lib.get(fp_name) {
                        // Find the pad definition closest to this pad's position
                        // (since pad entities store world position)
                        let mut best_pad: Option<&cypcb_world::footprint::PadDef> = None;
                        let mut best_dist = i64::MAX;

                        let radians = (_rotation as f64 / 1000.0) * std::f64::consts::PI / 180.0;
                        let cos_r = radians.cos();
                        let sin_r = radians.sin();

                        for pd in &fp.pads {
                            let px = pd.position.x.0 as f64;
                            let py = pd.position.y.0 as f64;
                            let rx = (px * cos_r - py * sin_r) as i64;
                            let ry = (px * sin_r + py * cos_r) as i64;
                            let wx = _comp_pos.x.0 + rx;
                            let wy = _comp_pos.y.0 + ry;
                            let dist = (wx - pad_pos.x.0).abs() + (wy - pad_pos.y.0).abs();
                            if dist < best_dist {
                                best_dist = dist;
                                best_pad = Some(pd);
                            }
                        }

                        if let Some(pd) = best_pad {
                            let hw = pd.size.0 .0 / 2; // half width
                            let hh = pd.size.1 .0 / 2; // half height
                                                       // Compute tight AABB for rotated rectangle.
                                                       // |cos|*hw + |sin|*hh gives the axis-aligned half-extent.
                            let abs_cos = cos_r.abs();
                            let abs_sin = sin_r.abs();
                            let half_x = (abs_cos * hw as f64 + abs_sin * hh as f64) as i64;
                            let half_y = (abs_sin * hw as f64 + abs_cos * hh as f64) as i64;
                            let layer_mask = if pd.layers.is_empty() {
                                0xFFFFFFFF
                            } else {
                                pd.layers.iter().fold(0u32, |m, l| m | l.to_copper_mask())
                            };
                            entries.push(SpatialEntry::from_raw(
                                *entity,
                                pad_pos.x.0 - half_x,
                                pad_pos.y.0 - half_y,
                                pad_pos.x.0 + half_x,
                                pad_pos.y.0 + half_y,
                                layer_mask,
                            ));
                        }
                    }
                }
            }
        }

        // ---- Also index component courtyards (for non-copper DRC like courtyard overlap) ----
        {
            let ecs = self.world.ecs_mut();
            let mut query = ecs.query::<(Entity, &Position, &FootprintRef)>();
            let items: Vec<_> = query
                .iter(ecs)
                .map(|(e, p, f)| (e, p.0, f.as_str().to_string()))
                .collect();

            // Skip courtyard indexing for copper clearance — pads are indexed above.
            // We still keep courtyards for other DRC rules (courtyard clearance, etc.)
            // but mark them with layer_mask = 0 so copper clearance check skips them.
            for (entity, pos, footprint_name) in &items {
                if let Some(fp) = self.footprint_lib.get(footprint_name) {
                    let bounds = fp.courtyard;
                    let min =
                        Point::new(Nm(pos.x.0 + bounds.min.x.0), Nm(pos.y.0 + bounds.min.y.0));
                    let max =
                        Point::new(Nm(pos.x.0 + bounds.max.x.0), Nm(pos.y.0 + bounds.max.y.0));
                    // layer_mask = 0 means this entry won't match any copper layer check
                    entries.push(SpatialEntry::new(*entity, min, max, 0));
                }
            }
        }

        // ---- Index trace segments ----
        {
            let ecs = self.world.ecs_mut();
            let mut query = ecs.query::<(Entity, &TraceComp)>();
            let traces: Vec<_> = query
                .iter(ecs)
                .map(|(e, t)| {
                    let segs: Vec<_> = t.segments.iter().map(|s| (s.start, s.end)).collect();
                    (e, t.width.0, t.layer.to_copper_mask(), segs)
                })
                .collect();

            for (entity, width, layer_mask, segs) in &traces {
                let half_width = width / 2;
                for (start, end) in segs {
                    let min_x = start.x.0.min(end.x.0) - half_width;
                    let min_y = start.y.0.min(end.y.0) - half_width;
                    let max_x = start.x.0.max(end.x.0) + half_width;
                    let max_y = start.y.0.max(end.y.0) + half_width;
                    entries.push(SpatialEntry::from_raw(
                        *entity,
                        min_x,
                        min_y,
                        max_x,
                        max_y,
                        *layer_mask,
                    ));
                }
            }
        }

        // ---- Index vias ----
        {
            let ecs = self.world.ecs_mut();
            let mut query = ecs.query::<(Entity, &Via)>();
            let vias: Vec<_> = query
                .iter(ecs)
                .map(|(e, v)| {
                    (
                        e,
                        v.position,
                        v.outer_diameter.0 / 2,
                        v.start_layer.to_copper_mask() | v.end_layer.to_copper_mask(),
                    )
                })
                .collect();

            for (entity, position, radius, layer_mask) in &vias {
                entries.push(SpatialEntry::from_raw(
                    *entity,
                    position.x.0 - radius,
                    position.y.0 - radius,
                    position.x.0 + radius,
                    position.y.0 + radius,
                    *layer_mask,
                ));
            }
        }

        self.world
            .ecs_mut()
            .resource_mut::<cypcb_world::SpatialIndex>()
            .rebuild(entries);
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

        self.world.spawn_entity((trace, NetId::new(net_id_num)));
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

        self.world.spawn_entity((via, NetId::new(net_id_num)));
        Ok(())
    }

    /// Find a trace entity by its index.
    ///
    /// Searches all trace entities and returns the one whose entity index
    /// matches the given id. This is needed because `Entity::from_raw(id)`
    /// may not match the actual generation of the entity.
    fn find_trace_entity(&mut self, trace_id: u32) -> Option<Entity> {
        let ecs = self.world.ecs_mut();
        let mut query = ecs.query::<(Entity, &cypcb_world::components::trace::Trace)>();
        query
            .iter(ecs)
            .map(|(entity, _)| entity)
            .find(|&entity| entity.index() == trace_id)
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
    #[allow(dead_code)] // Reserved for snapshot-based rendering path
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

        // Zones the host parsed. Without these the engine has no idea a ground
        // plane was declared, so it computes no copper for one and the screen
        // shows a board nobody will be sent.
        for zone in &snapshot.zones {
            use cypcb_world::components::zone::{Zone, ZoneKind};
            let net = if zone.net.is_empty() {
                None
            } else {
                Some(self.world.intern_net(&zone.net))
            };
            self.world.spawn_entity(Zone {
                bounds: cypcb_core::Rect {
                    min: Point::new(Nm(zone.bounds[0]), Nm(zone.bounds[1])),
                    max: Point::new(Nm(zone.bounds[2]), Nm(zone.bounds[3])),
                },
                kind: if zone.kind == "pour" {
                    ZoneKind::CopperPour
                } else {
                    ZoneKind::Keepout
                },
                layer_mask: zone.layer_mask,
                name: if zone.name.is_empty() {
                    None
                } else {
                    Some(zone.name.clone())
                },
                net,
            });
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

            // Build NetConnections from pin_to_net map.
            // We iterate over the pin_to_net map (built from snapshot.nets)
            // and match entries belonging to this component's refdes.
            // This works regardless of whether the footprint library has pads.
            let mut nets = NetConnections::new();
            let prefix = format!("{}.", comp.refdes);
            for (key, &net_id) in &pin_to_net {
                if let Some(pin) = key.strip_prefix(&prefix) {
                    nets.add(PinConnection::new(pin, net_id));
                }
            }

            let comp_entity =
                self.world
                    .spawn_component(refdes, value, position, rotation, footprint_ref, nets);

            // Spawn individual pad entities for per-pad DRC clearance checking.
            // Each pad gets its own entity with NetId + PadInstance marker so the
            // clearance checker can do precise same-net exemption per pad, not per
            // component (which would incorrectly exempt all nets on the component).
            if let Some(fp) = self.footprint_lib.get(&comp.footprint) {
                let radians = (comp.rotation_mdeg as f64 / 1000.0) * std::f64::consts::PI / 180.0;
                let cos_r = radians.cos();
                let sin_r = radians.sin();

                for pad_def in &fp.pads {
                    // Look up which net this specific pad is on
                    let pad_key = format!("{}.{}", comp.refdes, pad_def.number);
                    if let Some(&net_id) = pin_to_net.get(&pad_key) {
                        // Compute world position (rotate pad around component origin)
                        let px = pad_def.position.x.0 as f64;
                        let py = pad_def.position.y.0 as f64;
                        let rx = (px * cos_r - py * sin_r) as i64;
                        let ry = (px * sin_r + py * cos_r) as i64;
                        let wx = comp.x_nm + rx;
                        let wy = comp.y_nm + ry;

                        // Spawn pad entity with NetId for per-pad DRC
                        let pad_marker = PadInstance::new(comp_entity);
                        let pad_pos = Position(Point::new(Nm(wx), Nm(wy)));
                        self.world.spawn_entity((pad_marker, net_id, pad_pos));
                    }
                }
            }
        }

        // Rebuild spatial index for DRC queries (includes traces and vias)
        self.rebuild_spatial_index_full();
    }

    /// Create a Footprint from PadInfo data.
    #[allow(dead_code)] // Reserved for snapshot-based rendering path
    /// Put a host-supplied footprint into the library the parser resolves
    /// against.
    fn register_footprint_pads(&mut self, name: &str, pads: &[PadInfo], silk: &[SilkInfo]) {
        let mut footprint = self.footprint_from_pads(name, pads);
        footprint.silk = silk.iter().flat_map(SilkInfo::to_shapes).collect();
        self.footprint_lib.register(footprint);
    }

    fn footprint_from_pads(
        &self,
        name: &str,
        pads: &[PadInfo],
    ) -> cypcb_world::footprint::Footprint {
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
            let half_w = pad.size.0 .0 / 2;
            let half_h = pad.size.1 .0 / 2;
            min_x = min_x.min(pad.position.x.0 - half_w);
            min_y = min_y.min(pad.position.y.0 - half_h);
            max_x = max_x.max(pad.position.x.0 + half_w);
            max_y = max_y.max(pad.position.y.0 + half_h);
        }

        use cypcb_core::Rect;
        let bounds = if min_x <= max_x && min_y <= max_y {
            Rect::new(
                Point::new(Nm(min_x), Nm(min_y)),
                Point::new(Nm(max_x), Nm(max_y)),
            )
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
            // A snapshot carries pads, not artwork; the host registers silk
            // separately when it has any.
            silk: Vec::new(),
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

            // Get pad info and body dimensions from footprint library
            let mut pads: Vec<PadInfo> = Vec::new();
            let mut body_width_nm: i64 = 0;
            let mut body_height_nm: i64 = 0;
            if let Some(fp) = self.footprint_lib.get(&footprint_name) {
                // Body dimensions from footprint bounds
                body_width_nm = fp.bounds.width().0;
                body_height_nm = fp.bounds.height().0;

                for pad in &fp.pads {
                    let mut layer_mask: u32 = 0;
                    for layer in &pad.layers {
                        let layer: &Layer = layer;
                        layer_mask |= layer.to_copper_mask();
                    }
                    let drill_nm: Option<i64> = pad.drill.map(|d| d.0);
                    pads.push(PadInfo {
                        number: pad.number.clone(),
                        x_nm: pad.position.x.0,
                        y_nm: pad.position.y.0,
                        width_nm: pad.size.0 .0,
                        height_nm: pad.size.1 .0,
                        shape: pad_shape_to_string(&pad.shape),
                        layer_mask,
                        drill_nm,
                    });
                }
            }

            // Fallback: compute body dimensions from pad bounding box if bounds are zero
            if body_width_nm == 0 && body_height_nm == 0 && !pads.is_empty() {
                let mut min_x = i64::MAX;
                let mut min_y = i64::MAX;
                let mut max_x = i64::MIN;
                let mut max_y = i64::MIN;
                for pad in &pads {
                    let hw = pad.width_nm / 2;
                    let hh = pad.height_nm / 2;
                    min_x = min_x.min(pad.x_nm - hw);
                    min_y = min_y.min(pad.y_nm - hh);
                    max_x = max_x.max(pad.x_nm + hw);
                    max_y = max_y.max(pad.y_nm + hh);
                }
                body_width_nm = max_x - min_x;
                body_height_nm = max_y - min_y;
            }

            // The legend the engine holds, so the host does not need its own
            // copy of the footprint to draw one.
            let silk: Vec<SilkInfo> = self
                .footprint_lib
                .get(&footprint_name)
                .map(|fp| fp.silk.iter().map(SilkInfo::from_shape).collect())
                .unwrap_or_default();

            let refdes_str: String = refdes.as_str().to_string();
            components.push(ComponentInfo {
                refdes: refdes_str,
                silk,
                value,
                x_nm: position.0.x.0,
                y_nm: position.0.y.0,
                rotation_mdeg: rotation,
                footprint: footprint_name,
                pads,
                body_width_nm,
                body_height_nm,
                model_3d: None,
            });
        }

        // Attach 3D models. Done after the loop above, whose ECS query holds a
        // borrow of the world for its whole body.
        for component in &mut components {
            component.model_3d = self.model_3d.get(&component.footprint).cloned();
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

            // Look up cached constraints for this net
            #[cfg(feature = "native")]
            let (width_nm, clearance_nm, current_ma) = {
                let c = self.net_constraints.get(&net_name);
                (
                    c.and_then(|c| c.width_nm),
                    c.and_then(|c| c.clearance_nm),
                    c.and_then(|c| c.current_ma),
                )
            };
            #[cfg(not(feature = "native"))]
            let (width_nm, clearance_nm, current_ma): (
                Option<i64>,
                Option<i64>,
                Option<f64>,
            ) = (None, None, None);

            nets.push(NetInfo {
                name: net_name,
                id: net_id.0,
                connections,
                width_nm,
                clearance_nm,
                current_ma,
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

        // Build the copper the pours actually become
        let pours = self.collect_pours();

        // And the zones as written, so a host that sent them gets them back
        // rather than losing them on the round trip.
        let zones = self.collect_zones();

        BoardSnapshot {
            board,
            components,
            nets,
            violations,
            traces,
            vias,
            ratsnest,
            pours,
            zones,
        }
    }

    /// Collect the zones as the design states them.
    fn collect_zones(&mut self) -> Vec<ZoneInfo> {
        use cypcb_world::components::zone::ZoneKind;

        self.world
            .zones()
            .into_iter()
            .map(|(_, zone)| ZoneInfo {
                name: zone.name.clone().unwrap_or_default(),
                kind: match zone.kind {
                    ZoneKind::CopperPour => "pour".to_string(),
                    _ => "keepout".to_string(),
                },
                layer_mask: zone.layer_mask,
                net: zone
                    .net
                    .and_then(|id| self.world.net_name(id).map(|n| n.to_string()))
                    .unwrap_or_default(),
                bounds: [
                    zone.bounds.min.x.0,
                    zone.bounds.min.y.0,
                    zone.bounds.max.x.0,
                    zone.bounds.max.y.0,
                ],
            })
            .collect()
    }

    /// Collect every copper pour, as the copper it becomes.
    ///
    /// A zone as written is a rectangle; the copper made from it is that
    /// rectangle minus the clearance around everything else on the layer. The
    /// exporter has always sent the fabricator the second one, so a viewer
    /// drawing the first shows a board nobody will be sent - it hides exactly
    /// the mistakes a pour causes: a plane swallowing a pad, an island cut off
    /// from the net it is supposed to be.
    fn collect_pours(&mut self) -> Vec<PourInfo> {
        use cypcb_world::components::zone::ZoneKind;

        let zones: Vec<_> = self
            .world
            .zones()
            .into_iter()
            .map(|(_, zone)| zone)
            .filter(|zone| zone.kind == ZoneKind::CopperPour)
            .collect();

        let options = cypcb_core::pour::PourOptions::default();
        let mut pours = Vec::new();

        for zone in zones {
            let net = zone
                .net
                .and_then(|id| self.world.net_name(id).map(|name| name.to_string()))
                .unwrap_or_default();

            // A zone can span several layers, and what obstructs it differs on
            // each, so each layer is filled on its own.
            for layer in [Layer::TopCopper, Layer::BottomCopper] {
                if zone.layer_mask & layer.to_copper_mask() == 0 {
                    continue;
                }
                let library = self.footprint_lib.clone();
                let filled =
                    cypcb_world::copper::fill_zone(&mut self.world, &library, layer, &zone, &options);
                let rects: Vec<[i64; 4]> = filled
                    .all()
                    .map(|r| [r.min.x.0, r.min.y.0, r.max.x.0, r.max.y.0])
                    .collect();
                if rects.is_empty() {
                    continue;
                }
                pours.push(PourInfo {
                    net: net.clone(),
                    layer_mask: layer.to_copper_mask(),
                    rects,
                });
            }
        }

        pours
    }

    /// Collect all traces from the world.
    fn collect_traces(&mut self) -> Vec<TraceInfo> {
        // Collect trace data with entity IDs (cloning to avoid borrow issues)
        let trace_data: Vec<(u32, Trace)> = {
            let world_ref = self.world.ecs_mut();
            let mut query = world_ref.query::<(Entity, &Trace)>();
            query
                .iter(world_ref)
                .map(|(e, t)| (e.index(), t.clone()))
                .collect()
        };

        // Now process with net names
        let mut traces: Vec<TraceInfo> = Vec::new();
        for (entity_id, trace) in trace_data {
            let layer_name = match trace.layer {
                Layer::TopCopper => "Top".to_string(),
                Layer::BottomCopper => "Bottom".to_string(),
                // The DSL calls the first inner layer `Inner1`, and this
                // said `Inner0` for the same copper - the same layer with two
                // names depending on which way it travelled.
                Layer::Inner(n) => format!("Inner{}", n + 1),
                _ => "Top".to_string(),
            };

            let net_name = self
                .world
                .net_name(trace.net_id)
                .unwrap_or("(no net)")
                .to_string();

            let segments: Vec<TraceSegmentInfo> = trace
                .segments
                .iter()
                .map(|seg| TraceSegmentInfo {
                    start_x: seg.start.x.0 as f64,
                    start_y: seg.start.y.0 as f64,
                    end_x: seg.end.x.0 as f64,
                    end_y: seg.end.y.0 as f64,
                })
                .collect();

            traces.push(TraceInfo {
                id: entity_id,
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
        // Collect via data with entity IDs (copying to avoid borrow issues)
        let via_data: Vec<(u32, Via)> = {
            let world_ref = self.world.ecs_mut();
            let mut query = world_ref.query::<(Entity, &Via)>();
            query
                .iter(world_ref)
                .map(|(e, v)| (e.index(), *v))
                .collect()
        };

        // Now process with net names
        let mut vias: Vec<ViaInfo> = Vec::new();
        for (entity_id, via) in via_data {
            let net_name = self
                .world
                .net_name(via.net_id)
                .unwrap_or("(no net)")
                .to_string();

            vias.push(ViaInfo {
                id: entity_id,
                x: via.position.x.0 as f64,
                y: via.position.y.0 as f64,
                drill: via.drill.0 as f64,
                outer_diameter: via.outer_diameter.0 as f64,
                net_name,
                start_layer: layer_name(via.start_layer),
                end_layer: layer_name(via.end_layer),
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
                        let footprint_name = self
                            .world
                            .get::<FootprintRef>(entity)
                            .map(|f| f.as_str().to_string())
                            .unwrap_or_default();

                        let pad_offset = self.get_pad_offset(&footprint_name, &conn.pin);
                        let rotation = self.world.get::<Rotation>(entity).map(|r| r.0).unwrap_or(0);

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

/// Compute the shortest distance from a point to a line segment.
///
/// Returns the perpendicular distance if the projection falls on the segment,
/// otherwise the distance to the nearest endpoint.
fn point_to_segment_distance(p: Point, a: Point, b: Point) -> i64 {
    let dx = b.x.0 - a.x.0;
    let dy = b.y.0 - a.y.0;
    let len_sq = (dx as i128) * (dx as i128) + (dy as i128) * (dy as i128);

    if len_sq == 0 {
        // Degenerate segment (a == b), distance to point
        let ex = (p.x.0 - a.x.0) as i128;
        let ey = (p.y.0 - a.y.0) as i128;
        return ((ex * ex + ey * ey) as f64).sqrt() as i64;
    }

    // Project p onto line ab, compute parameter t
    let apx = (p.x.0 - a.x.0) as i128;
    let apy = (p.y.0 - a.y.0) as i128;
    let dot = apx * (dx as i128) + apy * (dy as i128);

    // Clamp t to [0, 1]
    let t = (dot as f64) / (len_sq as f64);
    let t = t.clamp(0.0, 1.0);

    // Closest point on segment
    let cx = a.x.0 as f64 + t * dx as f64;
    let cy = a.y.0 as f64 + t * dy as f64;

    let ex = p.x.0 as f64 - cx;
    let ey = p.y.0 as f64 - cy;

    (ex * ex + ey * ey).sqrt() as i64
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
/// A layer as the DSL writes it, which is what the viewer reads.
fn layer_name(layer: Layer) -> String {
    match layer {
        Layer::TopCopper => "Top".to_string(),
        Layer::BottomCopper => "Bottom".to_string(),
        Layer::Inner(n) => format!("Inner{}", n + 1),
        other => format!("{:?}", other),
    }
}

fn parse_layer(layer_str: &str) -> Result<Layer, String> {
    match layer_str {
        "TopCopper" | "Top" => Ok(Layer::TopCopper),
        "BottomCopper" | "Bottom" => Ok(Layer::BottomCopper),
        _ if layer_str.starts_with("Inner(") && layer_str.ends_with(")") => {
            let inner = &layer_str[6..layer_str.len() - 1];
            let num: u8 = inner
                .parse()
                .map_err(|e| format!("Invalid inner layer: {}", e))?;
            Ok(Layer::Inner(num))
        }
        // `Inner1` is the first inner layer, as the DSL writes it, and it is
        // `Layer::Inner(0)` in the model. `Inner(0)` above is the debug form,
        // which is already zero-based - both spellings arrive here from
        // different callers.
        _ if layer_str.starts_with("Inner") => {
            let num_str = &layer_str[5..];
            let num: u8 = num_str
                .parse()
                .map_err(|e| format!("Invalid inner layer: {}", e))?;
            Ok(Layer::Inner(num.saturating_sub(1)))
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
    fn a_fetched_footprint_reaches_the_parser() {
        // A package the built-in library has never heard of, with geometry that
        // only exists because the host went and got it.
        let pads = r#"[
            {"number":"1","x_nm":-500000,"y_nm":0,"width_nm":300000,
             "height_nm":400000,"shape":"rect","layer_mask":1,"drill_nm":null},
            {"number":"2","x_nm":500000,"y_nm":0,"width_nm":300000,
             "height_nm":400000,"shape":"rect","layer_mask":1,"drill_nm":null}
        ]"#;

        let source = "version 1\n\nboard b {\n    size 20mm x 20mm\n    layers 2\n}\n\n\
                      component U1 ic \"XKCD-2\" {\n    value \"part\"\n    at 10mm, 10mm\n}\n";

        let mut without = PcbEngine::new();
        assert!(
            without.load_source(source).contains("unknown footprint"),
            "the engine cannot place a package it has never been given"
        );

        let mut with = PcbEngine::new();
        assert_eq!(with.register_footprint("XKCD-2", pads, ""), "");
        assert_eq!(
            with.load_source(source),
            "",
            "the same source parses cleanly once the footprint has been handed over"
        );

        let after = with.get_snapshot();
        assert!(
            after.contains("\"number\":\"1\"") && after.contains("\"number\":\"2\""),
            "the fetched pads must reach the snapshot: {after}"
        );
    }

    #[test]
    fn a_fetched_footprint_survives_reparsing() {
        let pads = r#"[{"number":"1","x_nm":0,"y_nm":0,"width_nm":300000,
                        "height_nm":300000,"shape":"rect","layer_mask":1,"drill_nm":null}]"#;
        let source = "version 1\n\nboard b {\n    size 20mm x 20mm\n    layers 2\n}\n\n\
                      component U1 ic \"XKCD-1\" {\n    value \"part\"\n    at 10mm, 10mm\n}\n";

        let mut engine = PcbEngine::new();
        assert_eq!(engine.register_footprint("XKCD-1", pads, ""), "");

        // Every keystroke in the editor re-parses. The fetch happened once.
        for _ in 0..3 {
            assert_eq!(engine.load_source(source), "");
            assert!(
                engine.get_snapshot().contains("\"number\":\"1\""),
                "re-parsing must not drop a footprint the host registered"
            );
        }
    }

    #[test]
    fn trace_width_for_current_comes_from_the_calculator() {
        let engine = PcbEngine::new();

        assert_eq!(engine.min_trace_width_for_current_ma(0.0), 0.0);
        assert_eq!(engine.min_trace_width_for_current_ma(-5.0), 0.0);

        let one_amp = engine.min_trace_width_for_current_ma(1000.0);
        assert_eq!(
            one_amp,
            cypcb_calc::TraceWidthCalculator::min_width_for_current(1.0, true).0 as f64,
            "the engine must not do its own arithmetic"
        );

        // The number the copies disagreed about: 1 oz copper as 1.378 mils
        // against the language server's 1.37.
        println!("1A external, 10C rise: {:.4}mm", one_amp / 1_000_000.0);
        let with_old_constant = one_amp * 1.378 / 1.37;
        println!(
            "same current at 1.37 mils/oz: {:.4}mm, a {:.2}% difference",
            with_old_constant / 1_000_000.0,
            (with_old_constant - one_amp) / one_amp * 100.0
        );

        // More current needs more copper, monotonically.
        assert!(engine.min_trace_width_for_current_ma(2000.0) > one_amp);
    }

    #[test]
    fn a_registered_3d_model_reaches_the_snapshot() {
        let source = "version 1\n\nboard b {\n    size 20mm x 20mm\n    layers 2\n}\n\n\
                      component U1 ic \"SOIC-8\" {\n    value \"part\"\n    at 10mm, 10mm\n}\n";

        let mut engine = PcbEngine::new();
        assert_eq!(engine.load_source(source), "");
        assert!(
            engine.get_snapshot().contains("\"model_3d\":null"),
            "nothing in the source names a 3D model"
        );

        engine.register_3d_model("SOIC-8", "abc123");
        assert_eq!(engine.load_source(source), "");
        assert!(
            engine.get_snapshot().contains("\"model_3d\":\"abc123\""),
            "the registered model must reach the component it belongs to"
        );

        // A package nobody registered stays empty rather than borrowing one.
        engine.register_3d_model("QFN-24", "def456");
        assert_eq!(engine.load_source(source), "");
        let snapshot = engine.get_snapshot();
        assert!(snapshot.contains("\"model_3d\":\"abc123\""));
        assert!(!snapshot.contains("def456"));
    }

    #[test]
    fn load_source_produces_a_snapshot_with_traces() {
        // The viewer keeps its own board model because the engine's snapshot
        // "would have empty traces". That is true of the load_snapshot path,
        // where JavaScript only ever hands over components and the board. It
        // is not true of load_source, and the difference is what decides
        // whether the duplicate model in the viewer can go.
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../examples/uat-routing-locked.cypcb"),
        )
        .expect("fixture");

        let mut engine = PcbEngine::new();
        assert_eq!(engine.load_source(&source), "");

        let snapshot = engine.get_snapshot();
        assert!(
            snapshot.contains("\"net_name\":\"VCC\""),
            "the trace block in the source must reach the snapshot: {snapshot}"
        );
        assert!(
            !snapshot.contains("\"traces\":[]"),
            "traces must not be empty"
        );
    }

    #[test]
    fn a_fetched_footprint_can_bring_its_legend() {
        // A supplier's footprint arrives with artwork as well as copper. Until
        // the engine could take it, that artwork had nowhere to go.
        let pads = r#"[{"number":"1","x_nm":0,"y_nm":0,"width_nm":300000,
                        "height_nm":300000,"shape":"rect","layer_mask":1,"drill_nm":null}]"#;
        let silk = r#"[
            {"type":"segment","x1":-500000,"y1":0,"x2":500000,"y2":0,"width":150000},
            {"type":"circle","cx":0,"cy":600000,"radius":100000,"width":150000},
            {"type":"arc","cx":0,"cy":0,"radius":100000,"width":150000}
        ]"#;

        let mut engine = PcbEngine::new();
        assert_eq!(engine.register_footprint("XKCD-3", pads, silk), "");

        let stored = engine
            .footprint_lib
            .get("XKCD-3")
            .expect("registered")
            .silk
            .clone();

        // The segment and the circle survive as themselves. The arc has no
        // shape in the model, so it arrives as ink: 32 segments to the turn,
        // and this one states no angles, which means all the way round.
        assert_eq!(
            stored.len(),
            2 + 32,
            "the arc has to become segments rather than disappear"
        );
        assert_eq!(
            stored
                .iter()
                .filter(|shape| matches!(shape, cypcb_world::footprint::SilkShape::Circle { .. }))
                .count(),
            1,
            "the circle stays a circle - only the arc is approximated"
        );
    }

    #[test]
    fn malformed_silk_is_reported_not_swallowed() {
        let pads = r#"[{"number":"1","x_nm":0,"y_nm":0,"width_nm":300000,
                        "height_nm":300000,"shape":"rect","layer_mask":1,"drill_nm":null}]"#;
        let mut engine = PcbEngine::new();
        let error = engine.register_footprint("XKCD-4", pads, "not json");
        assert!(
            error.starts_with("Failed to deserialize silk"),
            "got {error}"
        );
    }

    #[test]
    fn malformed_pads_are_reported_not_swallowed() {
        let mut engine = PcbEngine::new();
        let error = engine.register_footprint("BROKEN", "not json", "");
        assert!(
            error.starts_with("Failed to deserialize pads"),
            "got {error}"
        );
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
        assert!(
            violations > 0,
            "Expected clearance violations but found {}",
            violations
        );
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
                    x_nm: 10_000_000, // 10mm
                    y_nm: 15_000_000, // 15mm
                    rotation_mdeg: 0,
                    footprint: "0402".to_string(),
                    pads: vec![], // Empty - should use builtin library
                    body_width_nm: 0,
                    body_height_nm: 0,
                    model_3d: None,
                    silk: Vec::new(),
                },
                ComponentInfo {
                    refdes: "R2".to_string(),
                    value: "10k".to_string(),
                    x_nm: 10_500_000, // 10.5mm (0.5mm from R1)
                    y_nm: 15_000_000, // 15mm
                    rotation_mdeg: 0,
                    footprint: "0402".to_string(),
                    pads: vec![], // Empty - should use builtin library
                    body_width_nm: 0,
                    body_height_nm: 0,
                    model_3d: None,
                    silk: Vec::new(),
                },
            ],
            nets: vec![],
            violations: vec![],
            traces: vec![],
            vias: vec![],
            ratsnest: vec![],
            pours: vec![],
            zones: vec![],
        };

        let mut engine = PcbEngine::new();
        engine.populate_from_snapshot(&snapshot);
        engine.run_drc_internal();

        // Check spatial index was built
        let spatial_count = engine.world.spatial().len();
        assert_eq!(
            spatial_count, 2,
            "Spatial index should have 2 entries, found {}",
            spatial_count
        );

        // Check for violations
        let violations = engine.violation_count();
        assert!(
            violations > 0,
            "Expected clearance violations but found {} - spatial entries: {}",
            violations,
            spatial_count
        );
    }

    // ====================================================================
    // Trace Mutation API tests
    // ====================================================================

    #[test]
    fn test_trace_add_returns_valid_id() {
        let mut engine = PcbEngine::new();
        engine.load_source(
            r#"
            version 1
            board test { size 50mm x 30mm layers 2 }
            "#,
        );

        // Add a horizontal trace: (5mm,5mm) → (20mm,5mm)
        let segments = [5_000_000i64, 5_000_000, 20_000_000, 5_000_000];
        let id = engine.add_trace("VCC", "Top", 200_000, &segments);
        assert_ne!(id, u32::MAX, "add_trace should return a valid entity id");
        assert_eq!(engine.trace_count(), 1);
    }

    #[test]
    fn test_trace_add_multiple() {
        let mut engine = PcbEngine::new();
        engine.load_source(
            r#"
            version 1
            board test { size 50mm x 30mm layers 2 }
            "#,
        );

        let seg1 = [5_000_000i64, 5_000_000, 20_000_000, 5_000_000];
        let seg2 = [5_000_000i64, 10_000_000, 20_000_000, 10_000_000];
        let id1 = engine.add_trace("VCC", "Top", 200_000, &seg1);
        let id2 = engine.add_trace("GND", "Bottom", 250_000, &seg2);

        assert_ne!(id1, id2);
        assert_eq!(engine.trace_count(), 2);
    }

    #[test]
    fn test_trace_add_appears_in_snapshot() {
        let mut engine = PcbEngine::new();
        engine.load_source(
            r#"
            version 1
            board test { size 50mm x 30mm layers 2 }
            "#,
        );

        let segments = [5_000_000i64, 5_000_000, 20_000_000, 5_000_000];
        let id = engine.add_trace("VCC", "Top", 200_000, &segments);

        let snapshot = engine.build_snapshot();
        assert_eq!(snapshot.traces.len(), 1);
        assert_eq!(snapshot.traces[0].id, id);
        assert_eq!(snapshot.traces[0].net_name, "VCC");
        assert_eq!(snapshot.traces[0].layer, "Top");
        assert_eq!(snapshot.traces[0].width, 200_000.0);
        assert!(!snapshot.traces[0].locked);
    }

    #[test]
    fn test_trace_remove() {
        let mut engine = PcbEngine::new();
        engine.load_source(
            r#"
            version 1
            board test { size 50mm x 30mm layers 2 }
            "#,
        );

        let segments = [5_000_000i64, 5_000_000, 20_000_000, 5_000_000];
        let id = engine.add_trace("VCC", "Top", 200_000, &segments);
        assert_eq!(engine.trace_count(), 1);

        let removed = engine.remove_trace(id);
        assert!(
            removed,
            "remove_trace should return true for existing trace"
        );
        assert_eq!(engine.trace_count(), 0);

        // Snapshot should be empty
        let snapshot = engine.build_snapshot();
        assert!(snapshot.traces.is_empty());
    }

    #[test]
    fn test_trace_remove_nonexistent() {
        let mut engine = PcbEngine::new();
        let removed = engine.remove_trace(9999);
        assert!(
            !removed,
            "remove_trace should return false for nonexistent trace"
        );
    }

    #[test]
    fn test_trace_get_at_point_hit() {
        let mut engine = PcbEngine::new();
        engine.load_source(
            r#"
            version 1
            board test { size 50mm x 30mm layers 2 }
            "#,
        );

        // Horizontal trace from (5mm,10mm) to (25mm,10mm), 0.2mm wide
        let segments = [5_000_000i64, 10_000_000, 25_000_000, 10_000_000];
        let id = engine.add_trace("SIG", "Top", 200_000, &segments);

        // Query right on the trace centerline
        let found = engine.get_trace_at_point(15_000_000, 10_000_000, 100_000);
        assert_eq!(found, id, "Should find the trace at its centerline");
    }

    #[test]
    fn test_trace_get_at_point_near() {
        let mut engine = PcbEngine::new();
        engine.load_source(
            r#"
            version 1
            board test { size 50mm x 30mm layers 2 }
            "#,
        );

        // Horizontal trace at y=10mm, 0.2mm wide (so copper extends 0.1mm above/below)
        let segments = [5_000_000i64, 10_000_000, 25_000_000, 10_000_000];
        let id = engine.add_trace("SIG", "Top", 200_000, &segments);

        // Query 0.05mm above centerline — within copper width
        let found = engine.get_trace_at_point(15_000_000, 10_050_000, 10_000);
        assert_eq!(found, id, "Should find trace slightly off-center");
    }

    #[test]
    fn test_trace_get_at_point_miss() {
        let mut engine = PcbEngine::new();
        engine.load_source(
            r#"
            version 1
            board test { size 50mm x 30mm layers 2 }
            "#,
        );

        let segments = [5_000_000i64, 10_000_000, 25_000_000, 10_000_000];
        engine.add_trace("SIG", "Top", 200_000, &segments);

        // Query 2mm above trace — outside copper + tolerance
        let found = engine.get_trace_at_point(15_000_000, 12_000_000, 100_000);
        assert_eq!(found, u32::MAX, "Should not find trace 2mm away");
    }

    #[test]
    fn test_trace_add_remove_add_cycle() {
        let mut engine = PcbEngine::new();
        engine.load_source(
            r#"
            version 1
            board test { size 50mm x 30mm layers 2 }
            "#,
        );

        // Add → remove → add again
        let seg = [5_000_000i64, 5_000_000, 20_000_000, 5_000_000];
        let id1 = engine.add_trace("VCC", "Top", 200_000, &seg);
        assert_eq!(engine.trace_count(), 1);

        engine.remove_trace(id1);
        assert_eq!(engine.trace_count(), 0);

        let seg2 = [10_000_000i64, 10_000_000, 30_000_000, 10_000_000];
        let id2 = engine.add_trace("GND", "Bottom", 250_000, &seg2);
        assert_eq!(engine.trace_count(), 1);
        assert_ne!(id2, u32::MAX);

        // The new trace should be queryable
        let found = engine.get_trace_at_point(20_000_000, 10_000_000, 100_000);
        assert_eq!(found, id2);
    }

    #[test]
    fn test_trace_add_bad_segments() {
        let mut engine = PcbEngine::new();

        // Too few coordinates
        let id = engine.add_trace("VCC", "Top", 200_000, &[1, 2, 3]);
        assert_eq!(id, u32::MAX, "Should reject segments with < 4 coords");

        // Empty
        let id = engine.add_trace("VCC", "Top", 200_000, &[]);
        assert_eq!(id, u32::MAX, "Should reject empty segments");
    }

    #[test]
    fn test_trace_add_bad_layer() {
        let mut engine = PcbEngine::new();
        let seg = [0i64, 0, 10_000_000, 0];
        let id = engine.add_trace("VCC", "InvalidLayer", 200_000, &seg);
        assert_eq!(id, u32::MAX, "Should reject invalid layer name");
    }

    #[test]
    fn test_trace_multi_segment() {
        let mut engine = PcbEngine::new();
        engine.load_source(
            r#"
            version 1
            board test { size 50mm x 30mm layers 2 }
            "#,
        );

        // L-shaped trace: (5mm,5mm)→(15mm,5mm)→(15mm,15mm)
        let segments = [
            5_000_000i64,
            5_000_000,
            15_000_000,
            5_000_000, // horizontal
            15_000_000,
            5_000_000,
            15_000_000,
            15_000_000, // vertical
        ];
        let id = engine.add_trace("SIG", "Top", 200_000, &segments);
        assert_ne!(id, u32::MAX);

        let snapshot = engine.build_snapshot();
        assert_eq!(snapshot.traces.len(), 1);
        assert_eq!(snapshot.traces[0].segments.len(), 2);

        // Hit-test on horizontal segment
        let h = engine.get_trace_at_point(10_000_000, 5_000_000, 100_000);
        assert_eq!(h, id);

        // Hit-test on vertical segment
        let v = engine.get_trace_at_point(15_000_000, 10_000_000, 100_000);
        assert_eq!(v, id);
    }

    #[test]
    fn test_run_drc_incremental() {
        let mut engine = PcbEngine::new();
        engine.load_source(
            r#"
            version 1
            board test { size 50mm x 30mm layers 2 }
            "#,
        );

        let count = engine.run_drc_incremental();
        // Empty board should have no violations
        assert_eq!(count, 0);
    }

    #[test]
    fn test_component_body_dimensions_from_footprint() {
        let mut engine = PcbEngine::new();
        engine.load_source(
            r#"
            version 1
            board test { size 50mm x 30mm layers 2 }
            component R1 resistor "0402" {
                value "10k"
                at 10mm, 10mm
            }
            "#,
        );

        let snapshot = engine.build_snapshot();
        assert_eq!(snapshot.components.len(), 1);
        let comp = &snapshot.components[0];
        assert_eq!(comp.refdes, "R1");
        // 0402 footprint should have non-zero body dimensions from bounds
        assert!(
            comp.body_width_nm > 0,
            "body_width_nm should be > 0, got {}",
            comp.body_width_nm
        );
        assert!(
            comp.body_height_nm > 0,
            "body_height_nm should be > 0, got {}",
            comp.body_height_nm
        );
        assert!(comp.model_3d.is_none());
    }

    #[test]
    fn test_point_to_segment_distance_on_segment() {
        // Point directly on segment midpoint
        let p = Point::from_mm(5.0, 0.0);
        let a = Point::from_mm(0.0, 0.0);
        let b = Point::from_mm(10.0, 0.0);
        let dist = point_to_segment_distance(p, a, b);
        assert_eq!(dist, 0, "Point on segment should have distance 0");
    }

    #[test]
    fn test_point_to_segment_distance_perpendicular() {
        // Point 3mm above segment midpoint
        let p = Point::from_mm(5.0, 3.0);
        let a = Point::from_mm(0.0, 0.0);
        let b = Point::from_mm(10.0, 0.0);
        let dist = point_to_segment_distance(p, a, b);
        // Should be ~3mm = 3_000_000 nm
        assert!(
            (dist - 3_000_000).abs() < 100,
            "Expected ~3mm, got {} nm",
            dist
        );
    }

    #[test]
    fn test_point_to_segment_distance_endpoint() {
        // Point beyond segment end
        let p = Point::from_mm(15.0, 0.0);
        let a = Point::from_mm(0.0, 0.0);
        let b = Point::from_mm(10.0, 0.0);
        let dist = point_to_segment_distance(p, a, b);
        // Should be 5mm = 5_000_000 nm
        assert!(
            (dist - 5_000_000).abs() < 100,
            "Expected ~5mm, got {} nm",
            dist
        );
    }

    // ====================================================================
    // Trace Persistence Tests
    // ====================================================================

    #[test]
    fn test_export_traces_empty() {
        let mut engine = PcbEngine::new();
        engine.load_source("version 1\nboard t { size 50mm x 30mm; layers 2 }");
        let dsl = engine.export_traces_as_dsl();
        assert!(dsl.is_empty(), "Expected empty export, got: {}", dsl);
    }

    #[test]
    fn test_export_traces_basic() {
        let mut engine = PcbEngine::new();
        engine.load_source("version 1\nboard t { size 50mm x 30mm; layers 2 }\nnet VCC { }");

        // Add a trace manually via the API
        let segments = [
            5_000_000i64,
            10_000_000,
            15_000_000,
            10_000_000, // seg1: (5,10) -> (15,10)
            15_000_000,
            10_000_000,
            15_000_000,
            20_000_000, // seg2: (15,10) -> (15,20)
        ];
        let id = engine.add_trace("VCC", "Top", 250_000, &segments);
        assert_ne!(id, u32::MAX, "add_trace failed");

        let dsl = engine.export_traces_as_dsl();
        assert!(dsl.contains("trace VCC"), "Missing net name: {}", dsl);
        assert!(dsl.contains("layer Top"), "Missing layer: {}", dsl);
        assert!(dsl.contains("width 0.250000mm"), "Missing width: {}", dsl);
        assert!(dsl.contains("path "), "Missing path: {}", dsl);
        assert!(
            dsl.contains("5.000000mm,10.000000mm"),
            "Missing start coord: {}",
            dsl
        );
        assert!(
            dsl.contains("15.000000mm,20.000000mm"),
            "Missing end coord: {}",
            dsl
        );
    }

    #[test]
    fn test_trace_round_trip_determinism() {
        // Phase 1: create engine, add traces, export to DSL
        let mut engine1 = PcbEngine::new();
        engine1.load_source(
            "version 1\nboard t { size 60mm x 40mm\nlayers 2 }\nnet VCC { }\nnet GND { }",
        );

        // Add traces with various coordinate values (including tricky float cases)
        let segs_vcc = [
            3_731_260i64,
            19_999_960,
            4_879_340,
            19_999_960,
            4_879_340,
            19_999_960,
            10_000_001,
            15_555_555,
        ];
        let segs_gnd = [1_000_000i64, 2_000_000, 30_000_000, 2_000_000];
        engine1.add_trace("VCC", "Top", 203_200, &segs_vcc);
        engine1.add_trace("GND", "Bottom", 150_000, &segs_gnd);

        let dsl1 = engine1.export_traces_as_dsl();

        // Phase 2: load the exported DSL into a fresh engine, export again
        let source2 = format!(
            "version 1\nboard t {{ size 60mm x 40mm\nlayers 2 }}\nnet VCC {{ }}\nnet GND {{ }}\n{}",
            dsl1
        );
        let mut engine2 = PcbEngine::new();
        let err = engine2.load_source(&source2);
        assert!(err.is_empty(), "Load error on round-trip: {}", err);

        let dsl2 = engine2.export_traces_as_dsl();

        // Phase 3: the two exports must be IDENTICAL (determinism)
        assert_eq!(
            dsl1, dsl2,
            "Round-trip NOT deterministic!\n--- First export ---\n{}\n--- Second export ---\n{}",
            dsl1, dsl2
        );

        // Phase 4: verify coordinates survived exactly
        let snapshot = engine2.build_snapshot();
        let vcc_traces: Vec<_> = snapshot
            .traces
            .iter()
            .filter(|t| t.net_name == "VCC")
            .collect();
        assert_eq!(vcc_traces.len(), 1, "Expected 1 VCC trace");
        let vcc = &vcc_traces[0];
        assert_eq!(vcc.segments.len(), 2, "Expected 2 segments");

        // Check exact nm values survived
        assert_eq!(vcc.segments[0].start_x as i64, 3_731_260);
        assert_eq!(vcc.segments[0].start_y as i64, 19_999_960);
        assert_eq!(vcc.segments[1].end_x as i64, 10_000_001);
        assert_eq!(vcc.segments[1].end_y as i64, 15_555_555);
    }

    #[test]
    fn test_export_traces_locked() {
        let mut engine = PcbEngine::new();
        engine.load_source(
            "version 1\nboard t { size 50mm x 30mm; layers 2 }\nnet VCC { }\ntrace VCC {\n    layer Top\n    width 0.25mm\n    path 5mm,10mm -> 15mm,10mm\n    locked\n}",
        );

        let dsl = engine.export_traces_as_dsl();
        assert!(dsl.contains("locked"), "Missing locked flag: {}", dsl);
    }

    #[test]
    fn test_export_traces_multi_layer() {
        let mut engine = PcbEngine::new();
        engine.load_source("version 1\nboard t { size 50mm x 30mm; layers 2 }\nnet SIG { }");

        // Add traces on both layers
        let seg_top = [5_000_000i64, 10_000_000, 15_000_000, 10_000_000];
        let seg_bot = [15_000_000i64, 10_000_000, 25_000_000, 10_000_000];
        engine.add_trace("SIG", "Top", 200_000, &seg_top);
        engine.add_trace("SIG", "Bottom", 200_000, &seg_bot);

        let dsl = engine.export_traces_as_dsl();
        assert!(dsl.contains("layer Top"), "Missing Top layer: {}", dsl);
        assert!(
            dsl.contains("layer Bottom"),
            "Missing Bottom layer: {}",
            dsl
        );
    }
}
