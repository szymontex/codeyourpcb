//! Routing grid for discretizing the PCB board into cells.
//!
//! The [`RoutingGrid`] converts the continuous board space into a uniform grid
//! where each cell tracks per-layer occupancy. This enables A* pathfinding
//! on a discrete graph structure.
//!
//! # Coordinate System
//!
//! The grid uses the same coordinate origin as the board (top-left corner).
//! Board positions in nanometers are converted to grid indices via integer
//! division by the grid resolution.

use cypcb_core::{Nm, Point};
use cypcb_rules::RoutingRuleSet;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{BoardWorld, Layer};

/// Cell occupancy flags (bitfield).
pub const CELL_FREE: u8 = 0;
pub const CELL_PAD: u8 = 1 << 0;
pub const CELL_TRACE: u8 = 1 << 1;
/// Copper a trace covers beside its centre line.
///
/// A minimum-width trace is 0.127mm on a 0.254mm cell, so the cell next to the
/// one the search walked holds copper the grid would otherwise call free. It
/// is marked separately from `CELL_TRACE` because it can be yielded: a net
/// with nowhere else to go may route through a halo cell, where routing
/// through the centre line of another net's trace is a short.
pub const CELL_HALO: u8 = 1 << 2;
pub const CELL_ZONE: u8 = 1 << 2;
pub const CELL_VIA: u8 = 1 << 3;
/// Generic obstacle (clearance bloat, board edge, etc.)
pub const CELL_OBSTACLE: u8 = 1 << 4;

/// Statistics about the routing grid.
#[derive(Debug, Clone, Default)]
pub struct GridStats {
    /// Grid width in cells.
    pub width: u32,
    /// Grid height in cells.
    pub height: u32,
    /// Number of routing layers.
    pub layers: u8,
    /// Total occupied cells across all layers.
    pub obstacle_cell_count: u64,
    /// Grid resolution in nanometers.
    pub resolution_nm: i64,
}

/// A discretized routing grid for the PCB board.
///
/// Each cell is a `u8` bitfield tracking what occupies that position
/// on each copper layer. The grid is indexed as `[layer][y * width + x]`.
///
/// # Grid Resolution
///
/// The default resolution is `min_clearance / 2` from the design rules,
/// typically ~63µm for JLCPCB. This ensures that clearance violations
/// are detectable at the grid level.
///
/// # Coordinate Conversion
///
/// Board coordinates (nanometers) are converted to grid indices:
/// - `grid_x = (nm_x - origin_x) / resolution`
/// - `grid_y = (nm_y - origin_y) / resolution`
///
/// The origin offset ensures the grid starts at (0,0) regardless of
/// the board's position in design space.
#[derive(Debug, Clone)]
pub struct RoutingGrid {
    /// Grid width in cells.
    width: u32,
    /// Grid height in cells.
    height: u32,
    /// Grid resolution (cell size) in nanometers.
    resolution: i64,
    /// Board origin offset in nanometers (top-left corner).
    origin_x: i64,
    /// Board origin offset in nanometers (top-left corner).
    origin_y: i64,
    /// Number of copper layers.
    layer_count: u8,
    /// Per-layer occupancy grids: `layers[layer_idx][y * width + x]`.
    layers: Vec<Vec<u8>>,
    /// Per-cell net ownership for dynamic route tracking.
    /// `net_map[layer_idx][y * width + x]` = net_id or u32::MAX if unowned.
    net_map: Vec<Vec<u32>>,

    /// Which net's pad copper a cell carries, or `u32::MAX` for none.
    ///
    /// Kept apart from `net_map` because a rip-up clears that one: a pad is
    /// not ripped up, and until this existed the grid knew a cell was
    /// somebody's pad without knowing whose. That is what forced a net's pad
    /// zone to open every cell near any of its pads, sibling pins included -
    /// 109 of stm32_breakout's 118 part-to-trace faults sit in exactly those
    /// cells.
    pad_net: Vec<Vec<u32>>,
}

impl RoutingGrid {
    /// Build a routing grid from a board world and design rules.
    ///
    /// Iterates all pads, zones, and locked traces in the board, marking
    /// grid cells as occupied with appropriate clearance bloat.
    ///
    /// # Arguments
    ///
    /// * `world` - Board world with components, zones, and traces
    /// * `library` - Footprint library for pad geometry
    /// * `rules` - Design rules for clearance values
    /// * `resolution_nm` - Grid cell size in nanometers
    ///
    /// # Panics
    ///
    /// Returns `None` if the board has no board entity set.
    pub fn from_board(
        world: &mut BoardWorld,
        library: &FootprintLibrary,
        rules: &dyn RoutingRuleSet,
        resolution_nm: i64,
    ) -> Option<Self> {
        let _span = tracing::info_span!("grid_construction").entered();

        let (board_size, layer_stack) = world.board_info()?;

        let board_width_nm = board_size.width.raw();
        let board_height_nm = board_size.height.raw();
        let layer_count = layer_stack.count.min(32);

        // Only track copper layers for routing
        // For a 2-layer board: layers 0 (top) and 1 (bottom)
        let routing_layers = layer_count.min(32);

        let grid_w = ((board_width_nm + resolution_nm - 1) / resolution_nm) as u32;
        let grid_h = ((board_height_nm + resolution_nm - 1) / resolution_nm) as u32;

        let total_cells = grid_w as u64 * grid_h as u64;
        if total_cells > 10_000_000 {
            tracing::warn!(
                width = grid_w,
                height = grid_h,
                total_cells,
                resolution_nm,
                "Grid exceeds 10M cells — consider increasing resolution"
            );
        }

        tracing::info!(
            board_width_mm = board_width_nm as f64 / 1_000_000.0,
            board_height_mm = board_height_nm as f64 / 1_000_000.0,
            grid_width = grid_w,
            grid_height = grid_h,
            resolution_um = resolution_nm as f64 / 1_000.0,
            layers = routing_layers,
            "Grid constructed"
        );

        let cell_count = (grid_w as usize) * (grid_h as usize);
        let layers_vec: Vec<Vec<u8>> = (0..routing_layers)
            .map(|_| vec![CELL_FREE; cell_count])
            .collect();
        let pad_net: Vec<Vec<u32>> = (0..routing_layers)
            .map(|_| vec![u32::MAX; cell_count])
            .collect();
        let net_map: Vec<Vec<u32>> = (0..routing_layers)
            .map(|_| vec![u32::MAX; cell_count])
            .collect();

        let mut grid = RoutingGrid {
            width: grid_w,
            height: grid_h,
            resolution: resolution_nm,
            origin_x: 0,
            origin_y: 0,
            layer_count: routing_layers,
            layers: layers_vec,
            net_map,
            pad_net,
        };

        // Bloat obstacles by the clearance *plus half a trace*, because the
        // grid tracks a route's centre line: a path node sitting exactly
        // min_clearance from a pad still has half its copper inside that gap.
        // Without the trace term, a legal-looking route lands 0.0635mm from a
        // pad that requires 0.127mm.
        let constraints = rules.constraints_for_net(0);
        let keepout_nm = constraints.min_clearance.raw() + constraints.min_trace_width.raw() / 2;
        let clearance_cells = ((keepout_nm + resolution_nm - 1) / resolution_nm) as u32;

        // Mark pads as obstacles
        grid.populate_pads(world, library, clearance_cells);

        // Mark zones as obstacles
        grid.populate_zones(world, clearance_cells);

        // Mark locked traces as obstacles
        grid.populate_locked_traces(world, clearance_cells);

        Some(grid)
    }

    /// Populate pad obstacles from all components in the world.
    fn populate_pads(
        &mut self,
        world: &mut BoardWorld,
        library: &FootprintLibrary,
        clearance_cells: u32,
    ) {
        use cypcb_world::{FootprintRef, Position, Rotation};

        // Collect component data to avoid borrow conflict
        let components: Vec<(Point, f64, String, Vec<(String, u32)>)> = {
            let ecs = world.ecs_mut();
            let mut query = ecs.query::<(
                &Position,
                &Rotation,
                &FootprintRef,
                Option<&cypcb_world::components::NetConnections>,
            )>();
            query
                .iter(ecs)
                .map(|(pos, rot, fp, nets)| {
                    let pins: Vec<(String, u32)> = nets
                        .map(|n| n.iter().map(|c| (c.pin.clone(), c.net.id())).collect())
                        .unwrap_or_default();
                    (pos.0, rot.to_degrees(), fp.as_str().to_string(), pins)
                })
                .collect()
        };

        for (comp_pos, rotation_deg, fp_name, pins) in &components {
            if let Some(footprint) = library.get(fp_name) {
                for pad in &footprint.pads {
                    // Rotate pad position around component origin
                    let pad_pos = rotate_point(pad.position, *rotation_deg);
                    let abs_x = comp_pos.x.raw() + pad_pos.x.raw();
                    let abs_y = comp_pos.y.raw() + pad_pos.y.raw();

                    // Determine pad radius for obstacle marking
                    let pad_radius_nm = pad.size.0.raw().max(pad.size.1.raw()) / 2;
                    let pad_radius_cells =
                        ((pad_radius_nm + self.resolution - 1) / self.resolution) as u32;

                    // Mark on each layer the pad exists on
                    for layer in &pad.layers {
                        if let Some(li) = layer_to_index(*layer) {
                            if (li as u8) < self.layer_count {
                                self.mark_obstacle_at_nm(
                                    abs_x,
                                    abs_y,
                                    li,
                                    pad_radius_cells + clearance_cells,
                                    CELL_PAD,
                                );

                                // Whose pad it is, so a net's own zone can be
                                // opened for its own pin and not for the pin
                                // beside it.
                                if let Some((_, net)) =
                                    pins.iter().find(|(pin, _)| *pin == pad.number)
                                {
                                    self.mark_pad_owner_at_nm(
                                        abs_x,
                                        abs_y,
                                        li,
                                        pad_radius_cells + clearance_cells,
                                        *net,
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Populate zone obstacles from all zones in the world.
    fn populate_zones(&mut self, world: &mut BoardWorld, clearance_cells: u32) {
        let zones = world.zones();
        for (_entity, zone) in &zones {
            if !zone.is_keepout() {
                // Copper pour zones are not obstacles for routing
                // (they fill around traces, not block them)
                continue;
            }

            // Mark all cells within the zone bounds + clearance
            let min_gx = self.nm_to_grid_x(zone.bounds.min.x.raw());
            let min_gy = self.nm_to_grid_y(zone.bounds.min.y.raw());
            let max_gx = self.nm_to_grid_x(zone.bounds.max.x.raw());
            let max_gy = self.nm_to_grid_y(zone.bounds.max.y.raw());

            // Expand by clearance
            let min_gx = min_gx.saturating_sub(clearance_cells);
            let min_gy = min_gy.saturating_sub(clearance_cells);
            let max_gx = (max_gx + clearance_cells).min(self.width.saturating_sub(1));
            let max_gy = (max_gy + clearance_cells).min(self.height.saturating_sub(1));

            for layer_idx in 0..self.layer_count {
                if zone.on_layer(layer_idx) {
                    for gy in min_gy..=max_gy {
                        for gx in min_gx..=max_gx {
                            self.set_cell(gx, gy, layer_idx as usize, CELL_ZONE);
                        }
                    }
                }
            }
        }
    }

    /// Populate locked trace obstacles.
    /// Copper already on the board, whether or not it is locked.
    ///
    /// Only locked traces used to be marked, so a trace the designer drew and
    /// did not lock was invisible to the router - measured on a four-layer
    /// board where a hand trace crossed a routed one at 0.00mm. `locked` means
    /// "do not rip this up"; unlocked copper is still copper.
    fn populate_locked_traces(&mut self, world: &mut BoardWorld, clearance_cells: u32) {
        use cypcb_world::components::trace::Trace;

        // Collect trace data to avoid borrow conflict
        let traces: Vec<Trace> = {
            let ecs = world.ecs_mut();
            let mut query = ecs.query::<&Trace>();
            query.iter(ecs).cloned().collect()
        };

        for trace in &traces {
            if let Some(layer_idx) = layer_to_index(trace.layer) {
                if (layer_idx as u8) >= self.layer_count {
                    continue;
                }
                let half_width_nm = trace.width.raw() / 2;
                let half_width_cells =
                    ((half_width_nm + self.resolution - 1) / self.resolution) as u32;
                let radius_cells = half_width_cells + clearance_cells;

                for seg in &trace.segments {
                    // Rasterize the segment by stepping along it
                    self.rasterize_segment(
                        seg.start.x.raw(),
                        seg.start.y.raw(),
                        seg.end.x.raw(),
                        seg.end.y.raw(),
                        layer_idx,
                        radius_cells,
                        CELL_TRACE,
                    );
                }
            }
        }
    }

    /// Rasterize a line segment onto the grid, marking cells within `radius_cells`.
    #[allow(clippy::too_many_arguments)]
    fn rasterize_segment(
        &mut self,
        x0_nm: i64,
        y0_nm: i64,
        x1_nm: i64,
        y1_nm: i64,
        layer: usize,
        radius_cells: u32,
        flag: u8,
    ) {
        // Step along the segment at grid resolution
        let dx = x1_nm - x0_nm;
        let dy = y1_nm - y0_nm;
        let length_nm = ((dx as f64).powi(2) + (dy as f64).powi(2)).sqrt() as i64;
        let steps = (length_nm / self.resolution).max(1);

        for i in 0..=steps {
            let t = i as f64 / steps as f64;
            let px = x0_nm + (dx as f64 * t) as i64;
            let py = y0_nm + (dy as f64 * t) as i64;

            let gx = self.nm_to_grid_x(px);
            let gy = self.nm_to_grid_y(py);

            self.mark_obstacle(gx, gy, layer, radius_cells, flag);
        }
    }

    // ========================================================================
    // Coordinate conversion
    // ========================================================================

    /// Convert a nanometer X coordinate to a grid X index.
    #[inline]
    pub fn nm_to_grid_x(&self, nm: i64) -> u32 {
        let adjusted = nm - self.origin_x;
        if adjusted < 0 {
            0
        } else {
            ((adjusted / self.resolution) as u32).min(self.width.saturating_sub(1))
        }
    }

    /// Convert a nanometer Y coordinate to a grid Y index.
    #[inline]
    pub fn nm_to_grid_y(&self, nm: i64) -> u32 {
        let adjusted = nm - self.origin_y;
        if adjusted < 0 {
            0
        } else {
            ((adjusted / self.resolution) as u32).min(self.height.saturating_sub(1))
        }
    }

    /// Convert a grid X index back to nanometers (center of cell).
    #[inline]
    pub fn grid_to_nm_x(&self, gx: u32) -> i64 {
        self.origin_x + (gx as i64) * self.resolution + self.resolution / 2
    }

    /// Convert a grid Y index back to nanometers (center of cell).
    #[inline]
    pub fn grid_to_nm_y(&self, gy: u32) -> i64 {
        self.origin_y + (gy as i64) * self.resolution + self.resolution / 2
    }

    /// Convert a nanometer position to grid coordinates.
    #[inline]
    pub fn nm_to_grid(&self, pos: Point) -> (u32, u32) {
        (
            self.nm_to_grid_x(pos.x.raw()),
            self.nm_to_grid_y(pos.y.raw()),
        )
    }

    /// Convert grid coordinates to a nanometer position (center of cell).
    #[inline]
    pub fn grid_to_nm(&self, gx: u32, gy: u32) -> Point {
        Point::new(
            Nm::new(self.grid_to_nm_x(gx)),
            Nm::new(self.grid_to_nm_y(gy)),
        )
    }

    // ========================================================================
    // Cell access
    // ========================================================================

    /// Check if a cell is free (no obstacles) on the given layer.
    #[inline]
    pub fn is_free(&self, x: u32, y: u32, layer: usize) -> bool {
        if x >= self.width || y >= self.height || layer >= self.layers.len() {
            return false;
        }
        let idx = (y as usize) * (self.width as usize) + (x as usize);
        self.layers[layer][idx] == CELL_FREE
    }

    /// Whether a cell is free once the copper a trace merely brushes is
    /// ignored.
    ///
    /// A net that finds no path at all is worse than a net that runs close to
    /// another: an abandoned connection is a board that does not work, and a
    /// tight gap is a violation the checker will name. This is the second
    /// attempt's test, never the first.
    #[inline]
    pub fn is_free_ignoring_halo(&self, x: u32, y: u32, layer: usize) -> bool {
        if x >= self.width || y >= self.height || layer >= self.layers.len() {
            return false;
        }
        let idx = (y as usize) * (self.width as usize) + (x as usize);
        self.layers[layer][idx] & !CELL_HALO == CELL_FREE
    }

    /// Whether this cell holds only the copper a neighbouring trace brushes.
    #[inline]
    pub fn is_halo_only(&self, x: u32, y: u32, layer: usize) -> bool {
        if x >= self.width || y >= self.height || layer >= self.layers.len() {
            return false;
        }
        let idx = (y as usize) * (self.width as usize) + (x as usize);
        self.layers[layer][idx] == CELL_HALO
    }

    /// Get the occupancy flags for a cell.
    #[inline]
    pub fn cell(&self, x: u32, y: u32, layer: usize) -> u8 {
        if x >= self.width || y >= self.height || layer >= self.layers.len() {
            return u8::MAX; // Out of bounds = fully blocked
        }
        let idx = (y as usize) * (self.width as usize) + (x as usize);
        self.layers[layer][idx]
    }

    /// Set an occupancy flag on a cell.
    #[inline]
    fn set_cell(&mut self, x: u32, y: u32, layer: usize, flag: u8) {
        if x >= self.width || y >= self.height || layer >= self.layers.len() {
            return;
        }
        let idx = (y as usize) * (self.width as usize) + (x as usize);
        self.layers[layer][idx] |= flag;
    }

    /// Mark a circular area around (cx, cy) as occupied on the given layer.
    pub fn mark_obstacle(&mut self, cx: u32, cy: u32, layer: usize, radius_cells: u32, flag: u8) {
        if layer >= self.layers.len() {
            return;
        }

        let r = radius_cells as i64;
        let r_sq = r * r;

        let min_x = (cx as i64 - r).max(0) as u32;
        let max_x = (cx as i64 + r).min(self.width as i64 - 1) as u32;
        let min_y = (cy as i64 - r).max(0) as u32;
        let max_y = (cy as i64 + r).min(self.height as i64 - 1) as u32;

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let dx = x as i64 - cx as i64;
                let dy = y as i64 - cy as i64;
                if dx * dx + dy * dy <= r_sq {
                    self.set_cell(x, y, layer, flag);
                }
            }
        }
    }

    /// Mark obstacle given absolute nanometer coordinates.
    fn mark_obstacle_at_nm(
        &mut self,
        nm_x: i64,
        nm_y: i64,
        layer: usize,
        radius_cells: u32,
        flag: u8,
    ) {
        let gx = self.nm_to_grid_x(nm_x);
        let gy = self.nm_to_grid_y(nm_y);
        self.mark_obstacle(gx, gy, layer, radius_cells, flag);
    }

    /// Record which net a pad's copper belongs to, over the same disc the
    /// obstacle marking covers.
    ///
    /// A cell already claimed by another net's pad keeps its first owner: two
    /// pads of different nets whose clearances overlap is a placement fault,
    /// and the router should treat that copper as foreign either way.
    fn mark_pad_owner_at_nm(&mut self, nm_x: i64, nm_y: i64, layer: usize, radius: u32, net: u32) {
        let cx = self.nm_to_grid_x(nm_x) as i64;
        let cy = self.nm_to_grid_y(nm_y) as i64;
        let r = radius as i64;

        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy > r * r {
                    continue;
                }
                let (x, y) = (cx + dx, cy + dy);
                if x < 0 || y < 0 || x >= self.width as i64 || y >= self.height as i64 {
                    continue;
                }
                if layer >= self.pad_net.len() {
                    continue;
                }
                let idx = y as usize * self.width as usize + x as usize;
                if self.pad_net[layer][idx] == u32::MAX {
                    self.pad_net[layer][idx] = net;
                }
            }
        }
    }

    /// Which net's pad copper this cell carries, if any.
    pub fn pad_owner(&self, x: u32, y: u32, layer: usize) -> Option<u32> {
        if x >= self.width || y >= self.height || layer >= self.pad_net.len() {
            return None;
        }
        let idx = (y as usize) * (self.width as usize) + (x as usize);
        match self.pad_net[layer][idx] {
            u32::MAX => None,
            net => Some(net),
        }
    }

    /// Mark cells along a route for a specific net.
    pub fn mark_route(&mut self, x: u32, y: u32, layer: usize, net_id: u32) {
        if x >= self.width || y >= self.height || layer >= self.layers.len() {
            return;
        }
        let idx = (y as usize) * (self.width as usize) + (x as usize);
        self.layers[layer][idx] |= CELL_TRACE;
        self.net_map[layer][idx] = net_id;
    }

    /// Reserve the copper a routed path actually occupies.
    ///
    /// `mark_route` marks a single cell per path node, but a trace is wider
    /// than one cell: at the default resolution of `min_clearance / 2` a
    /// minimum-width trace covers two, and the neighbouring net takes the
    /// other one legitimately. That is where the 0.00mm clearance violations
    /// come from - the grid never knew the copper was there.
    ///
    /// Marks every cell within `radius` of each node, skipping cells another
    /// net already owns (rip-up decides who yields, not this). Returns the
    /// cells it marked so the caller can record them for congestion accounting
    /// and clear them on rip-up.
    pub fn mark_route_footprint(
        &mut self,
        path: &[(u16, u16, u8)],
        net_id: u32,
        radius: u32,
    ) -> Vec<(u32, u32, u8)> {
        let mut marked = Vec::with_capacity(path.len() * ((2 * radius as usize + 1).pow(2)));

        for &(nx, ny, nl) in path {
            let layer = nl as usize;
            if layer >= self.layers.len() {
                continue;
            }
            let min_x = (nx as u32).saturating_sub(radius);
            let max_x = (nx as u32 + radius).min(self.width.saturating_sub(1));
            let min_y = (ny as u32).saturating_sub(radius);
            let max_y = (ny as u32 + radius).min(self.height.saturating_sub(1));

            for cy in min_y..=max_y {
                for cx in min_x..=max_x {
                    let idx = (cy as usize) * (self.width as usize) + (cx as usize);
                    let owner = self.net_map[layer][idx];
                    if owner != u32::MAX && owner != net_id {
                        continue; // Another net's copper - leave it alone
                    }
                    let flag = if cx == nx as u32 && cy == ny as u32 {
                        CELL_TRACE
                    } else {
                        CELL_HALO
                    };
                    self.layers[layer][idx] |= flag;
                    self.net_map[layer][idx] = net_id;
                    marked.push((cx, cy, nl));
                }
            }
        }

        marked.sort_unstable();
        marked.dedup();
        marked
    }

    /// The cells a via's copper covers on both of the layers it joins.
    ///
    /// A path marks one cell where it changes layer, but a via's ring is far
    /// bigger than a cell - 0.554mm against 0.0635mm for JLCPCB, about nine
    /// cells across. Handing these cells to the congestion map lets other nets
    /// see the ring and price it, without blocking the board outright.
    pub fn via_footprint_cells(
        &self,
        x: u32,
        y: u32,
        layers: (u8, u8),
        radius: u32,
    ) -> Vec<(u32, u32, u8)> {
        let mut marked = Vec::new();
        let r = radius as i64;
        let r_sq = r * r;

        for layer in [layers.0, layers.1] {
            let li = layer as usize;
            if li >= self.layers.len() {
                continue;
            }
            let min_x = (x as i64 - r).max(0) as u32;
            let max_x = ((x as i64 + r) as u32).min(self.width.saturating_sub(1));
            let min_y = (y as i64 - r).max(0) as u32;
            let max_y = ((y as i64 + r) as u32).min(self.height.saturating_sub(1));

            for cy in min_y..=max_y {
                let dy = cy as i64 - y as i64;
                for cx in min_x..=max_x {
                    let dx = cx as i64 - x as i64;
                    if dx * dx + dy * dy > r_sq {
                        continue; // Outside the ring
                    }
                    marked.push((cx, cy, layer));
                }
            }
        }

        marked.sort_unstable();
        marked.dedup();
        marked
    }

    /// Clear a known set of cells belonging to a net (for rip-up).
    ///
    /// Equivalent to [`clear_route`](RoutingGrid::clear_route) when the caller
    /// already knows which cells the net occupies, but costs `cells.len()` steps
    /// instead of a full `width * height * layers` scan.
    ///
    /// Cells the net no longer owns are left alone - during negotiated congestion
    /// another net may have taken them over, and its trace must survive.
    pub fn clear_cells(&mut self, cells: &[(u32, u32, u8)], net_id: u32) {
        for &(x, y, layer) in cells {
            let layer = layer as usize;
            if x >= self.width || y >= self.height || layer >= self.layers.len() {
                continue;
            }
            let idx = (y as usize) * (self.width as usize) + (x as usize);
            if self.net_map[layer][idx] == net_id {
                self.net_map[layer][idx] = u32::MAX;
                self.layers[layer][idx] &= !(CELL_TRACE | CELL_HALO);
            }
        }
    }

    /// Clear all cells belonging to a specific net (for rip-up).
    pub fn clear_route(&mut self, net_id: u32) {
        for layer in 0..self.layer_count as usize {
            let w = self.width as usize;
            let h = self.height as usize;
            for idx in 0..(w * h) {
                if self.net_map[layer][idx] == net_id {
                    self.net_map[layer][idx] = u32::MAX;
                    // Clear the trace bits but keep other obstacle flags
                    self.layers[layer][idx] &= !(CELL_TRACE | CELL_HALO);
                }
            }
        }
    }

    // ========================================================================
    // Statistics
    // ========================================================================

    /// Get grid dimensions and obstacle statistics.
    pub fn stats(&self) -> GridStats {
        let mut obstacle_count: u64 = 0;
        for layer_data in &self.layers {
            obstacle_count += layer_data.iter().filter(|&&c| c != CELL_FREE).count() as u64;
        }

        GridStats {
            width: self.width,
            height: self.height,
            layers: self.layer_count,
            obstacle_cell_count: obstacle_count,
            resolution_nm: self.resolution,
        }
    }

    /// Grid width in cells.
    #[inline]
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Grid height in cells.
    #[inline]
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Number of routing layers.
    #[inline]
    pub fn layer_count(&self) -> u8 {
        self.layer_count
    }

    /// Grid resolution in nanometers.
    #[inline]
    pub fn resolution(&self) -> i64 {
        self.resolution
    }

    /// Get the net ID that owns a cell, or `None` if unowned.
    #[inline]
    pub fn net_at(&self, x: u32, y: u32, layer: usize) -> Option<u32> {
        if x >= self.width || y >= self.height || layer >= self.net_map.len() {
            return None;
        }
        let idx = (y as usize) * (self.width as usize) + (x as usize);
        let net = self.net_map[layer][idx];
        if net == u32::MAX {
            None
        } else {
            Some(net)
        }
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Convert a `Layer` enum to a routing layer index (0-based).
///
/// Returns `None` for non-copper layers.
pub fn layer_to_index(layer: Layer) -> Option<usize> {
    match layer {
        Layer::TopCopper => Some(0),
        Layer::BottomCopper => Some(1),
        Layer::Inner(n) => Some(2 + n as usize),
        _ => None,
    }
}

/// Convert a routing layer index back to a `Layer` enum.
pub fn index_to_layer(index: usize) -> Layer {
    match index {
        0 => Layer::TopCopper,
        1 => Layer::BottomCopper,
        n => Layer::Inner((n - 2) as u8),
    }
}

/// Rotate a point around the origin by the given angle in degrees.
fn rotate_point(p: Point, degrees: f64) -> Point {
    if degrees.abs() < 0.001 {
        return p;
    }
    let rad = degrees.to_radians();
    let cos = rad.cos();
    let sin = rad.sin();
    let x = p.x.raw() as f64;
    let y = p.y.raw() as f64;
    Point::new(
        Nm::new((x * cos - y * sin).round() as i64),
        Nm::new((x * sin + y * cos).round() as i64),
    )
}

/// Create a minimal grid for unit testing (no board needed).
pub fn make_test_grid(width: u32, height: u32, resolution_nm: i64, layers: u8) -> RoutingGrid {
    let cell_count = (width as usize) * (height as usize);
    RoutingGrid {
        width,
        height,
        resolution: resolution_nm,
        origin_x: 0,
        origin_y: 0,
        layer_count: layers,
        layers: (0..layers).map(|_| vec![CELL_FREE; cell_count]).collect(),
        net_map: (0..layers).map(|_| vec![u32::MAX; cell_count]).collect(),
        pad_net: (0..layers).map(|_| vec![u32::MAX; cell_count]).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cypcb_core::Point;

    fn make_test_grid(width: u32, height: u32, resolution_nm: i64, layers: u8) -> RoutingGrid {
        let cell_count = (width as usize) * (height as usize);
        RoutingGrid {
            width,
            height,
            resolution: resolution_nm,
            origin_x: 0,
            origin_y: 0,
            layer_count: layers,
            layers: (0..layers).map(|_| vec![CELL_FREE; cell_count]).collect(),
            net_map: (0..layers).map(|_| vec![u32::MAX; cell_count]).collect(),
            pad_net: (0..layers).map(|_| vec![u32::MAX; cell_count]).collect(),
        }
    }

    #[test]
    fn clear_cells_frees_every_cell_of_the_net() {
        let mut grid = make_test_grid(10, 10, 63_500, 2);
        let cells: Vec<(u32, u32, u8)> = (0..5).map(|x| (x, 3, 0)).collect();
        for &(x, y, layer) in &cells {
            grid.mark_route(x, y, layer as usize, 7);
        }
        assert!(!grid.is_free(2, 3, 0));

        grid.clear_cells(&cells, 7);

        for &(x, y, layer) in &cells {
            assert!(
                grid.is_free(x, y, layer as usize),
                "cell ({x},{y}) must not stay blocked after rip-up"
            );
            assert_eq!(grid.net_at(x, y, layer as usize), None);
        }
    }

    #[test]
    fn clear_cells_leaves_cells_taken_over_by_another_net() {
        let mut grid = make_test_grid(10, 10, 63_500, 2);
        grid.mark_route(1, 1, 0, 7);
        grid.mark_route(2, 1, 0, 7);
        // Net 9 negotiated cell (2,1) away from net 7.
        grid.mark_route(2, 1, 0, 9);

        grid.clear_cells(&[(1, 1, 0), (2, 1, 0)], 7);

        assert!(grid.is_free(1, 1, 0), "net 7's own cell is released");
        assert_eq!(grid.net_at(2, 1, 0), Some(9), "net 9 keeps its cell");
        assert!(!grid.is_free(2, 1, 0));
    }

    #[test]
    fn coordinate_round_trip_accuracy() {
        // Grid: 100 cells wide, resolution = 63500 nm (63.5µm ≈ min_clearance/2)
        let grid = make_test_grid(100, 100, 63_500, 2);

        // Test several positions
        for &nm_x in &[0i64, 63_500, 500_000, 3_175_000, 6_300_000] {
            let gx = grid.nm_to_grid_x(nm_x);
            let back = grid.grid_to_nm_x(gx);
            let error = (back - nm_x).abs();
            // Round-trip should be within 1 grid cell
            assert!(
                error <= grid.resolution,
                "Round-trip error {error} exceeds resolution {} for nm_x={nm_x}",
                grid.resolution
            );
        }
    }

    #[test]
    fn coordinate_round_trip_point() {
        let grid = make_test_grid(200, 200, 50_000, 2);

        let original = Point::from_mm(5.0, 3.0);
        let (gx, gy) = grid.nm_to_grid(original);
        let back = grid.grid_to_nm(gx, gy);

        let err_x = (back.x.raw() - original.x.raw()).abs();
        let err_y = (back.y.raw() - original.y.raw()).abs();

        assert!(
            err_x <= grid.resolution,
            "X round-trip error {err_x} > {}",
            grid.resolution
        );
        assert!(
            err_y <= grid.resolution,
            "Y round-trip error {err_y} > {}",
            grid.resolution
        );
    }

    #[test]
    fn obstacle_marking_circular() {
        let mut grid = make_test_grid(20, 20, 100_000, 1);

        // Mark obstacle at center (10, 10) with radius 3
        grid.mark_obstacle(10, 10, 0, 3, CELL_PAD);

        // Center should be marked
        assert!(!grid.is_free(10, 10, 0));
        assert_eq!(grid.cell(10, 10, 0) & CELL_PAD, CELL_PAD);

        // Cell at radius 3 (along axis) should be marked
        assert!(!grid.is_free(13, 10, 0));
        assert!(!grid.is_free(7, 10, 0));

        // Cell at radius 4 along axis should be free (outside radius 3)
        assert!(grid.is_free(14, 10, 0));

        // Corner cell at (13, 13): distance = sqrt(9+9) ≈ 4.24 > 3, should be free
        assert!(grid.is_free(13, 13, 0));
    }

    #[test]
    fn clearance_bloat_extends_obstacle() {
        let mut grid = make_test_grid(30, 30, 100_000, 1);

        // Mark pad with radius 2, plus clearance 3 = total radius 5
        let pad_radius = 2u32;
        let clearance = 3u32;
        grid.mark_obstacle(15, 15, 0, pad_radius + clearance, CELL_PAD);

        // Cell at distance 4 from center should be blocked (within 5)
        assert!(!grid.is_free(19, 15, 0)); // distance 4

        // Cell at distance 6 along axis should be free (outside 5)
        assert!(grid.is_free(21, 15, 0));
    }

    #[test]
    fn layer_isolation() {
        let mut grid = make_test_grid(20, 20, 100_000, 2);

        // Mark obstacle only on layer 0
        grid.mark_obstacle(10, 10, 0, 2, CELL_PAD);

        // Layer 0 should be blocked
        assert!(!grid.is_free(10, 10, 0));

        // Layer 1 should still be free
        assert!(grid.is_free(10, 10, 1));
    }

    #[test]
    fn is_free_out_of_bounds() {
        let grid = make_test_grid(10, 10, 100_000, 2);

        // Out of bounds should return false (not free)
        assert!(!grid.is_free(10, 0, 0)); // x == width
        assert!(!grid.is_free(0, 10, 0)); // y == height
        assert!(!grid.is_free(0, 0, 2)); // layer == layer_count
        assert!(!grid.is_free(100, 100, 0));
    }

    #[test]
    fn mark_and_clear_route() {
        let mut grid = make_test_grid(10, 10, 100_000, 1);

        // Mark a route for net 42
        grid.mark_route(3, 3, 0, 42);
        grid.mark_route(4, 3, 0, 42);
        grid.mark_route(5, 3, 0, 42);

        assert!(!grid.is_free(3, 3, 0));
        assert!(!grid.is_free(4, 3, 0));
        assert!(!grid.is_free(5, 3, 0));

        // Clear net 42
        grid.clear_route(42);

        assert!(grid.is_free(3, 3, 0));
        assert!(grid.is_free(4, 3, 0));
        assert!(grid.is_free(5, 3, 0));
    }

    #[test]
    fn clear_route_preserves_other_obstacles() {
        let mut grid = make_test_grid(10, 10, 100_000, 1);

        // Mark a pad at (5, 5)
        grid.set_cell(5, 5, 0, CELL_PAD);

        // Mark a route over it for net 7
        grid.mark_route(5, 5, 0, 7);

        // Clear net 7's route
        grid.clear_route(7);

        // The pad should still be there
        assert!(!grid.is_free(5, 5, 0));
        assert_eq!(grid.cell(5, 5, 0) & CELL_PAD, CELL_PAD);
    }

    #[test]
    fn stats_counts_obstacles() {
        let mut grid = make_test_grid(10, 10, 100_000, 2);

        // Initially all free
        let stats = grid.stats();
        assert_eq!(stats.obstacle_cell_count, 0);
        assert_eq!(stats.width, 10);
        assert_eq!(stats.height, 10);
        assert_eq!(stats.layers, 2);

        // Mark some cells
        grid.set_cell(0, 0, 0, CELL_PAD);
        grid.set_cell(1, 1, 1, CELL_ZONE);

        let stats = grid.stats();
        assert_eq!(stats.obstacle_cell_count, 2);
    }

    #[test]
    fn layer_to_index_mapping() {
        assert_eq!(layer_to_index(Layer::TopCopper), Some(0));
        assert_eq!(layer_to_index(Layer::BottomCopper), Some(1));
        assert_eq!(layer_to_index(Layer::Inner(0)), Some(2));
        assert_eq!(layer_to_index(Layer::Inner(1)), Some(3));
        assert_eq!(layer_to_index(Layer::TopSilk), None);
        assert_eq!(layer_to_index(Layer::Outline), None);
    }

    #[test]
    fn rotate_point_90_degrees() {
        let p = Point::from_mm(1.0, 0.0);
        let rotated = rotate_point(p, 90.0);
        // After 90° rotation: (1, 0) -> (0, 1)
        assert!(
            (rotated.x.raw()).abs() < 1000, // within 1µm
            "x should be ~0, got {}",
            rotated.x.raw()
        );
        assert!(
            (rotated.y.raw() - 1_000_000).abs() < 1000,
            "y should be ~1mm, got {}",
            rotated.y.raw()
        );
    }

    #[test]
    fn rotate_point_zero_is_identity() {
        let p = Point::from_mm(3.5, 7.2);
        let rotated = rotate_point(p, 0.0);
        assert_eq!(rotated.x.raw(), p.x.raw());
        assert_eq!(rotated.y.raw(), p.y.raw());
    }
}
