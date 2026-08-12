//! How far each cell is from the nearest copper, in nanometres.
//!
//! Step 1 of `docs/router-plan.md`. Nothing reads this yet, and that is
//! deliberate: the plan's first step exists to build the machinery, measure
//! what it costs, and prove it perturbs nothing, before anything is allowed to
//! act on it.
//!
//! # Why a field at all
//!
//! [`RoutingGrid`] answers "is this cell free" with a bool, after bloating
//! every obstacle by `min_clearance + min_trace_width / 2` and rounding **up**
//! to whole cells. That rounding is the shape shared by seventeen instruments
//! this project built, measured and reverted. The arithmetic is written out in
//! `docs/routing.md` for the via case: a keepout of
//! `0.15 + 0.127 + 0.127 + 0.0635 = 0.4675mm` becomes a 2-cell disc at
//! 0.254mm per cell, which is 0.508mm against 0.277mm of real copper - an 83%
//! over-block the grid cannot avoid at any resolution it can afford.
//!
//! A field holds the quantity those instruments were reaching for. It is
//! seeded from copper **with no clearance bloat at all**, so a reader gets the
//! distance to the metal and can compare it against whatever that particular
//! pair actually requires, rather than against a number baked in when the grid
//! was built.
//!
//! # What it costs in accuracy
//!
//! The transform is the classic two-pass 3-4 chamfer. It is not Euclidean:
//! stepping orthogonally costs 3 and diagonally 4, so the unit distance is
//! `resolution / 3`. It is exact along an axis and **short** along a diagonal,
//! where it charges `4/3 = 1.3333` for a step whose true length is
//! `sqrt(2) = 1.4142` - 5.7% low. The bound is measured against a brute-force
//! Euclidean answer rather than quoted from the literature, in both
//! directions, by `the_error_is_within_the_stated_bound`.
//!
//! Reading low is the safe direction for a router: a cost term built on this
//! believes copper is nearer than it is, so it is conservative rather than
//! optimistic about a clearance. A reader that needs the error gone should
//! move to a 5-7-11 chamfer or an exact transform, and measure what that
//! costs - it is not free, and step 1 is the wrong place to spend it.
//!
//! Two passes over the cells is the whole cost. The grids this router builds
//! are small - stm32_breakout is 296 x 256 x 2, about 151 thousand cells -
//! against roughly 900 A* searches per iteration over the same grid.

use cypcb_world::components::rotate_about_origin;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::BoardWorld;

use crate::grid::{layer_to_index, RoutingGrid};

/// One orthogonal step, in chamfer units.
const STEP_ORTHOGONAL: i32 = 3;
/// One diagonal step, in chamfer units.
const STEP_DIAGONAL: i32 = 4;
/// How many chamfer units one cell of true distance is worth.
const UNITS_PER_CELL: i64 = STEP_ORTHOGONAL as i64;

/// A distance large enough to act as "no copper found" without overflowing
/// when a step is added to it.
const UNREACHED: i32 = i32::MAX / 4;

/// Distance from every cell to the nearest copper on its own layer.
///
/// Indexed the same way [`RoutingGrid`] indexes its planes, so a caller that
/// has a `(x, y, layer)` for one has it for the other.
pub struct ClearanceField {
    width: u32,
    height: u32,
    layer_count: u8,
    plane: usize,
    resolution: i64,
    /// Chamfer units to the nearest seed cell. Zero inside copper.
    dist: Vec<i32>,
}

impl ClearanceField {
    /// Build the field for the same board the grid was built for.
    ///
    /// Takes the grid rather than a width and a height so the two cannot
    /// drift apart: every dimension, the resolution and the origin come from
    /// the structure this field is meant to line up with.
    ///
    /// The seeding walks the same three sources the grid walks - pads, keepout
    /// zones and copper already on the board - with one difference that is the
    /// entire point: **no clearance bloat**. The grid marks
    /// `pad_radius + clearance_cells`; this marks `pad_radius`.
    pub fn from_board(
        world: &mut BoardWorld,
        library: &FootprintLibrary,
        grid: &RoutingGrid,
    ) -> Self {
        let width = grid.width();
        let height = grid.height();
        let layer_count = grid.layer_count();
        let plane = (width as usize) * (height as usize);

        let mut field = ClearanceField {
            width,
            height,
            layer_count,
            plane,
            resolution: grid.resolution(),
            dist: vec![UNREACHED; plane * layer_count as usize],
        };

        field.seed_pads(world, library, grid);
        field.seed_zones(world, grid);
        field.seed_copper(world, grid);
        field.transform();
        field
    }

    /// Distance from a cell to the nearest copper on its layer, in nanometres.
    ///
    /// Returns `None` for a cell outside the grid, and `i64::MAX` through
    /// `Some` for a layer that carries no copper at all - which is a real
    /// answer rather than a missing one: nothing is near, at any distance.
    pub fn distance_nm(&self, x: u32, y: u32, layer: usize) -> Option<i64> {
        if x >= self.width || y >= self.height || layer >= self.layer_count as usize {
            return None;
        }
        let idx = layer * self.plane + (y as usize) * (self.width as usize) + (x as usize);
        let units = self.dist[idx];
        if units >= UNREACHED {
            return Some(i64::MAX);
        }
        Some(units as i64 * self.resolution / UNITS_PER_CELL)
    }

    /// How many cells this field covers, across every layer.
    pub fn cell_count(&self) -> usize {
        self.dist.len()
    }

    /// How many of them sit on copper.
    pub fn seed_count(&self) -> usize {
        self.dist.iter().filter(|&&d| d == 0).count()
    }

    /// Whether this cell holds copper - a seed of the transform rather than a
    /// result of it. Exposed so a test can measure the chamfer against the
    /// true Euclidean distance instead of quoting the error bound.
    pub fn is_copper(&self, x: u32, y: u32, layer: usize) -> bool {
        if x >= self.width || y >= self.height || layer >= self.layer_count as usize {
            return false;
        }
        let idx = layer * self.plane + (y as usize) * (self.width as usize) + (x as usize);
        self.dist[idx] == 0
    }

    // -----------------------------------------------------------------------
    // Seeding
    // -----------------------------------------------------------------------

    fn seed(&mut self, x: u32, y: u32, layer: usize) {
        if x >= self.width || y >= self.height || layer >= self.layer_count as usize {
            return;
        }
        let idx = layer * self.plane + (y as usize) * (self.width as usize) + (x as usize);
        self.dist[idx] = 0;
    }

    /// Seed a disc of cells around a point given in nanometres.
    fn seed_disc(
        &mut self,
        nm_x: i64,
        nm_y: i64,
        layer: usize,
        radius_cells: u32,
        grid: &RoutingGrid,
    ) {
        let cx = grid.nm_to_grid_x(nm_x) as i64;
        let cy = grid.nm_to_grid_y(nm_y) as i64;
        let r = radius_cells as i64;
        let r_sq = r * r;
        for y in (cy - r).max(0)..=(cy + r).min(self.height as i64 - 1) {
            for x in (cx - r).max(0)..=(cx + r).min(self.width as i64 - 1) {
                let dx = x - cx;
                let dy = y - cy;
                if dx * dx + dy * dy <= r_sq {
                    self.seed(x as u32, y as u32, layer);
                }
            }
        }
    }

    fn seed_pads(
        &mut self,
        world: &mut BoardWorld,
        library: &FootprintLibrary,
        grid: &RoutingGrid,
    ) {
        use cypcb_world::{FootprintRef, Position, Rotation};

        let components: Vec<(cypcb_core::Point, f64, String)> = {
            let ecs = world.ecs_mut();
            let mut query = ecs.query::<(&Position, &Rotation, &FootprintRef)>();
            query
                .iter(ecs)
                .map(|(pos, rot, fp)| (pos.0, rot.to_degrees(), fp.as_str().to_string()))
                .collect()
        };

        for (comp_pos, rotation_deg, fp_name) in &components {
            let Some(footprint) = library.get(fp_name) else {
                continue;
            };
            for pad in &footprint.pads {
                let pad_pos = rotate_about_origin(pad.position, *rotation_deg);
                let abs_x = comp_pos.x.raw() + pad_pos.x.raw();
                let abs_y = comp_pos.y.raw() + pad_pos.y.raw();

                // The copper's own radius, and nothing added to it.
                let pad_radius_nm = pad.size.0.raw().max(pad.size.1.raw()) / 2;
                let radius_cells = ((pad_radius_nm + self.resolution - 1) / self.resolution) as u32;

                if pad.is_non_plated() {
                    // A mounting hole has no copper, but it is still material
                    // nothing may cross, on every layer. It seeds the field for
                    // the same reason it blocks the grid.
                    for li in 0..self.layer_count {
                        self.seed_disc(abs_x, abs_y, li as usize, radius_cells, grid);
                    }
                    continue;
                }

                for layer in &pad.layers {
                    if let Some(li) = layer_to_index(*layer) {
                        if (li as u8) < self.layer_count {
                            self.seed_disc(abs_x, abs_y, li, radius_cells, grid);
                        }
                    }
                }
            }
        }
    }

    fn seed_zones(&mut self, world: &mut BoardWorld, grid: &RoutingGrid) {
        let zones = world.zones();
        for (_entity, zone) in &zones {
            if !zone.is_keepout() {
                continue;
            }
            let min_gx = grid.nm_to_grid_x(zone.bounds.min.x.raw());
            let min_gy = grid.nm_to_grid_y(zone.bounds.min.y.raw());
            let max_gx = grid
                .nm_to_grid_x(zone.bounds.max.x.raw())
                .min(self.width.saturating_sub(1));
            let max_gy = grid
                .nm_to_grid_y(zone.bounds.max.y.raw())
                .min(self.height.saturating_sub(1));

            for layer_idx in 0..self.layer_count {
                if zone.on_layer(layer_idx) {
                    for gy in min_gy..=max_gy {
                        for gx in min_gx..=max_gx {
                            self.seed(gx, gy, layer_idx as usize);
                        }
                    }
                }
            }
        }
    }

    fn seed_copper(&mut self, world: &mut BoardWorld, grid: &RoutingGrid) {
        use cypcb_world::components::trace::Trace;

        let traces: Vec<Trace> = {
            let ecs = world.ecs_mut();
            let mut query = ecs.query::<&Trace>();
            query.iter(ecs).cloned().collect()
        };

        for trace in &traces {
            let Some(layer_idx) = layer_to_index(trace.layer) else {
                continue;
            };
            if (layer_idx as u8) >= self.layer_count {
                continue;
            }
            let half_width_nm = trace.width.raw() / 2;
            let radius_cells = ((half_width_nm + self.resolution - 1) / self.resolution) as u32;

            for seg in &trace.segments {
                let x0 = seg.start.x.raw();
                let y0 = seg.start.y.raw();
                let x1 = seg.end.x.raw();
                let y1 = seg.end.y.raw();
                let dx = (x1 - x0) as f64;
                let dy = (y1 - y0) as f64;
                let length_nm = (dx * dx + dy * dy).sqrt() as i64;
                let steps = (length_nm / self.resolution).max(1);
                for i in 0..=steps {
                    let t = i as f64 / steps as f64;
                    let x = x0 + (dx * t) as i64;
                    let y = y0 + (dy * t) as i64;
                    self.seed_disc(x, y, layer_idx, radius_cells, grid);
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // The transform
    // -----------------------------------------------------------------------

    /// Two passes per layer: forward over the already-visited neighbours,
    /// backward over the rest. This is the whole algorithm - every cell's
    /// answer is settled by the time the second pass leaves it.
    fn transform(&mut self) {
        let w = self.width as usize;
        let h = self.height as usize;
        if w == 0 || h == 0 {
            return;
        }

        for layer in 0..self.layer_count as usize {
            let base = layer * self.plane;

            for y in 0..h {
                for x in 0..w {
                    let here = base + y * w + x;
                    let mut best = self.dist[here];
                    if best == 0 {
                        continue;
                    }
                    if y > 0 {
                        let up = here - w;
                        best = best.min(self.dist[up] + STEP_ORTHOGONAL);
                        if x > 0 {
                            best = best.min(self.dist[up - 1] + STEP_DIAGONAL);
                        }
                        if x + 1 < w {
                            best = best.min(self.dist[up + 1] + STEP_DIAGONAL);
                        }
                    }
                    if x > 0 {
                        best = best.min(self.dist[here - 1] + STEP_ORTHOGONAL);
                    }
                    self.dist[here] = best;
                }
            }

            for y in (0..h).rev() {
                for x in (0..w).rev() {
                    let here = base + y * w + x;
                    let mut best = self.dist[here];
                    if best == 0 {
                        continue;
                    }
                    if y + 1 < h {
                        let down = here + w;
                        best = best.min(self.dist[down] + STEP_ORTHOGONAL);
                        if x > 0 {
                            best = best.min(self.dist[down - 1] + STEP_DIAGONAL);
                        }
                        if x + 1 < w {
                            best = best.min(self.dist[down + 1] + STEP_DIAGONAL);
                        }
                    }
                    if x + 1 < w {
                        best = best.min(self.dist[here + 1] + STEP_ORTHOGONAL);
                    }
                    self.dist[here] = best;
                }
            }
        }
    }
}
