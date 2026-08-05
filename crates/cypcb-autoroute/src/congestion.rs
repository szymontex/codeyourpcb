//! Congestion tracking for PathFinder negotiated congestion routing.
//!
//! [`CongestionMap`] tracks per-cell occupancy and history costs across all
//! routing layers. The PathFinder algorithm uses this to penalize overused
//! cells and guide nets toward less congested paths.
//!
//! # Cost Model
//!
//! Each cell's congestion cost is:
//! ```text
//! (1.0 + history_cost) * (1.0 + present_factor * max(0, occupancy - capacity))
//! ```
//!
//! - **present_factor**: scales the impact of current overuse (default 1.0)
//! - **history_cost**: accumulates each iteration a cell is overused,
//!   preventing oscillation where two nets repeatedly swap the same cell.

/// Congestion map for VPR-style negotiated congestion routing.
///
/// Tracks per-cell occupancy (how many nets use each cell), present cost
/// (derived from current overuse), and history cost (accumulated over
/// iterations for persistently overused cells).
pub struct CongestionMap {
    /// Width of the grid in cells.
    width: u32,
    /// Height of the grid in cells.
    height: u32,
    /// Number of routing layers.
    layers: u8,

    /// Historical congestion cost per cell: `[layer][y * width + x]`.
    /// Accumulates over iterations for overused cells.
    history_cost: Vec<Vec<f64>>,

    /// Number of nets currently occupying each cell: `[layer][y * width + x]`.
    occupancy: Vec<Vec<u16>>,

    /// Cell capacity (typically 1 for PCB routing — one net per cell).
    capacity: u16,
}

impl CongestionMap {
    /// Create a new congestion map for the given grid dimensions.
    ///
    /// All cells start with zero occupancy and zero history cost.
    /// Capacity is set to 1 (standard for PCB routing where each cell
    /// can only carry one net).
    pub fn new(width: u32, height: u32, layers: u8) -> Self {
        let cell_count = (width as usize) * (height as usize);
        Self {
            width,
            height,
            layers,
            history_cost: (0..layers).map(|_| vec![0.0; cell_count]).collect(),
            occupancy: (0..layers).map(|_| vec![0u16; cell_count]).collect(),
            capacity: 1,
        }
    }

    /// Mark a set of cells as occupied by a net.
    ///
    /// Increments the occupancy counter for each cell in the list.
    /// Call this after successfully routing a net.
    pub fn mark_net(&mut self, cells: &[(u32, u32, u8)]) {
        for &(x, y, layer) in cells {
            if let Some(idx) = self.cell_index(x, y, layer) {
                self.occupancy[layer as usize][idx] =
                    self.occupancy[layer as usize][idx].saturating_add(1);
            }
        }
    }

    /// Unmark a set of cells previously occupied by a net.
    ///
    /// Decrements the occupancy counter for each cell in the list.
    /// Call this before ripping up a net's route.
    pub fn unmark_net(&mut self, cells: &[(u32, u32, u8)]) {
        for &(x, y, layer) in cells {
            if let Some(idx) = self.cell_index(x, y, layer) {
                self.occupancy[layer as usize][idx] =
                    self.occupancy[layer as usize][idx].saturating_sub(1);
            }
        }
    }

    /// Update history costs for overused cells.
    ///
    /// For each cell where `occupancy > capacity`, adds `alpha` to the
    /// history cost. This prevents oscillation — cells that are repeatedly
    /// overused accumulate cost, discouraging future nets from using them.
    pub fn update_history(&mut self, alpha: f64) {
        for layer in 0..self.layers as usize {
            let cap = self.capacity;
            for idx in 0..self.occupancy[layer].len() {
                if self.occupancy[layer][idx] > cap {
                    self.history_cost[layer][idx] += alpha;
                }
            }
        }
    }

    /// Compute the congestion cost for a cell.
    ///
    /// Returns `(1.0 + history) * (1.0 + max(0, occupancy - capacity))`.
    /// Cells at or under capacity return only the history component.
    /// Cells with zero history and no overuse return 0.0.
    #[inline]
    pub fn congestion_cost(&self, x: u32, y: u32, layer: u8) -> f64 {
        let idx = match self.cell_index(x, y, layer) {
            Some(i) => i,
            None => return 0.0,
        };
        let li = layer as usize;
        let history = self.history_cost[li][idx];
        let occ = self.occupancy[li][idx];
        let overuse = if occ > self.capacity {
            (occ - self.capacity) as f64
        } else {
            0.0
        };
        (1.0 + history) * (1.0 + overuse) - 1.0
    }

    /// Return a list of all overused cells (occupancy > capacity).
    pub fn overused_cells(&self) -> Vec<(u32, u32, u8)> {
        let mut result = Vec::new();
        for layer in 0..self.layers {
            let li = layer as usize;
            for y in 0..self.height {
                for x in 0..self.width {
                    let idx = (y as usize) * (self.width as usize) + (x as usize);
                    if self.occupancy[li][idx] > self.capacity {
                        result.push((x, y, layer));
                    }
                }
            }
        }
        result
    }

    /// Check if all cells are at or under capacity (no congestion).
    pub fn is_converged(&self) -> bool {
        self.overuse_count() == 0
    }

    /// Count the number of overused cells.
    pub fn overuse_count(&self) -> usize {
        let cap = self.capacity;
        self.occupancy
            .iter()
            .flat_map(|layer| layer.iter())
            .filter(|&&occ| occ > cap)
            .count()
    }

    /// Convert grid coordinates to a flat array index, with bounds checking.
    #[inline]
    fn cell_index(&self, x: u32, y: u32, layer: u8) -> Option<usize> {
        if x >= self.width || y >= self.height || layer >= self.layers {
            return None;
        }
        Some((y as usize) * (self.width as usize) + (x as usize))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_map_is_empty() {
        let map = CongestionMap::new(10, 10, 2);
        assert_eq!(map.overuse_count(), 0);
        assert!(map.is_converged());
        assert!(map.overused_cells().is_empty());
    }

    #[test]
    fn mark_unmark_symmetry() {
        let mut map = CongestionMap::new(10, 10, 1);
        let cells = vec![(1, 1, 0), (2, 2, 0), (3, 3, 0)];

        map.mark_net(&cells);
        // One net per cell — at capacity, not overused
        assert_eq!(map.overuse_count(), 0);
        assert!(map.is_converged());

        // Mark a second net on the same cells — now overused
        map.mark_net(&cells);
        assert_eq!(map.overuse_count(), 3);
        assert!(!map.is_converged());

        // Unmark one net — back to capacity
        map.unmark_net(&cells);
        assert_eq!(map.overuse_count(), 0);
        assert!(map.is_converged());
    }

    #[test]
    fn congestion_cost_zero_when_no_overuse() {
        let map = CongestionMap::new(10, 10, 1);
        assert_eq!(map.congestion_cost(5, 5, 0), 0.0);
    }

    #[test]
    fn congestion_cost_increases_with_overuse() {
        let mut map = CongestionMap::new(10, 10, 1);
        let cells = vec![(5, 5, 0)];

        map.mark_net(&cells);
        // At capacity — cost should be 0 (no overuse, no history)
        let cost_at_cap = map.congestion_cost(5, 5, 0);
        assert_eq!(cost_at_cap, 0.0);

        map.mark_net(&cells);
        // Overuse by 1 — cost should be (1+0)*(1+1) - 1 = 1.0
        let cost_overused = map.congestion_cost(5, 5, 0);
        assert!(
            cost_overused > cost_at_cap,
            "Overused cost {cost_overused} should exceed at-capacity cost {cost_at_cap}"
        );
        assert!((cost_overused - 1.0).abs() < 1e-9);
    }

    #[test]
    fn history_cost_accumulates() {
        let mut map = CongestionMap::new(10, 10, 1);
        let cells = vec![(3, 3, 0)];

        // Create overuse
        map.mark_net(&cells);
        map.mark_net(&cells);

        // Update history twice
        map.update_history(0.5);
        map.update_history(0.5);

        // History should be 1.0 (0.5 * 2)
        // Cost: (1 + 1.0) * (1 + 1) - 1 = 3.0
        let cost = map.congestion_cost(3, 3, 0);
        assert!((cost - 3.0).abs() < 1e-9, "Expected 3.0, got {cost}");

        // Now unmark both nets — overuse gone but history remains
        map.unmark_net(&cells);
        map.unmark_net(&cells);
        assert!(map.is_converged());

        // Cost with history but no overuse: (1 + 1.0) * (1 + 0) - 1 = 1.0
        let cost_after = map.congestion_cost(3, 3, 0);
        assert!(
            (cost_after - 1.0).abs() < 1e-9,
            "Expected 1.0, got {cost_after}"
        );
    }

    #[test]
    fn overused_cells_returns_correct_positions() {
        let mut map = CongestionMap::new(5, 5, 2);
        let cells_a = vec![(1, 1, 0), (2, 2, 1)];
        let cells_b = vec![(1, 1, 0)]; // overlaps on (1,1,0)

        map.mark_net(&cells_a);
        map.mark_net(&cells_b);

        let overused = map.overused_cells();
        assert_eq!(overused.len(), 1);
        assert_eq!(overused[0], (1, 1, 0));
    }

    #[test]
    fn out_of_bounds_is_safe() {
        let mut map = CongestionMap::new(5, 5, 1);
        // Should not panic
        map.mark_net(&[(10, 10, 0)]);
        map.unmark_net(&[(10, 10, 0)]);
        assert_eq!(map.congestion_cost(10, 10, 0), 0.0);
        assert_eq!(map.congestion_cost(0, 0, 5), 0.0);
    }

    #[test]
    fn multi_layer_tracking() {
        let mut map = CongestionMap::new(5, 5, 2);

        // Net on layer 0 only
        map.mark_net(&[(2, 2, 0)]);
        // Net on layer 1 only
        map.mark_net(&[(2, 2, 1)]);

        // No overuse — different layers
        assert!(map.is_converged());

        // Now add a second net on layer 0
        map.mark_net(&[(2, 2, 0)]);
        assert_eq!(map.overuse_count(), 1);

        let overused = map.overused_cells();
        assert_eq!(overused[0], (2, 2, 0));
    }

    #[test]
    fn update_history_only_affects_overused() {
        let mut map = CongestionMap::new(5, 5, 1);

        // Cell (1,1) at capacity, cell (2,2) overused
        map.mark_net(&[(1, 1, 0), (2, 2, 0)]);
        map.mark_net(&[(2, 2, 0)]);

        map.update_history(1.0);

        // (1,1) should have no history cost
        assert_eq!(map.congestion_cost(1, 1, 0), 0.0);

        // Unmark (2,2) to isolate history effect
        map.unmark_net(&[(2, 2, 0)]);
        map.unmark_net(&[(2, 2, 0)]);

        // (2,2) has history 1.0, no overuse: cost = (1+1)*(1+0) - 1 = 1.0
        let cost = map.congestion_cost(2, 2, 0);
        assert!((cost - 1.0).abs() < 1e-9, "Expected 1.0, got {cost}");
    }
}
