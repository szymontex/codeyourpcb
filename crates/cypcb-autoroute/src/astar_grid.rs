//! A* over the routing grid, with its working memory owned by the caller.
//!
//! The router used `pathfinding::directed::astar`, which is a fine general
//! A* and allocates like one: a `HashMap` for costs, a `HashMap` for parents
//! and a `BinaryHeap`, all built and dropped **per connection**. A board
//! routes hundreds of connections per iteration and the loop runs up to fifty
//! iterations, so those three allocations happen tens of thousands of times,
//! and every node lookup pays a hash.
//!
//! A grid has something a general graph does not: every node has a number.
//! `(x, y, layer)` is an index into a flat array, so the costs and the parents
//! are arrays, and a lookup is an index rather than a hash.
//!
//! The arrays are **never cleared between searches**. Each search bumps an
//! epoch counter and stamps the cells it touches; a cell whose stamp is not
//! the current epoch has no cost and no parent, whatever bytes it holds. That
//! turns the setup cost of a search from "zero three arrays the size of the
//! board" into "add one to a number" - which matters, because most searches
//! touch a small corner of a grid that is 296 by 256 by 4.

use std::collections::BinaryHeap;

use crate::pathfinder::GridNode;

/// One entry in the frontier.
///
/// Ordered by estimated total cost, smallest first, and among equal estimates
/// by cost so far, **largest** first.
///
/// The second half is the standard A* tie-break: two cells with the same
/// estimated total are equally promising, and the one that has already spent
/// more is nearer the goal, so following it gets there with fewer expansions.
///
/// It is also what makes this search interchangeable with the one it replaced.
/// Taking the **smaller** cost first is a defensible reading of the same rule
/// and it routes different boards: measured on the six fixtures, `led_blink`
/// came out 24 segments and 3 violations against 21 and 2, `plane_board` 209
/// and 30 against 181 and 28. With the larger cost first every fixture comes
/// out byte for byte as `pathfinding::astar` left it - 899/99, 945/119,
/// 1478/186, 671/60, 181/20 segments and vias, and 180, 291, 309, 65, 28
/// violations.
///
/// Both numbers live in one `u64` key, and the ordering is the derived one on
/// that key. `estimated` is stored complemented in the high half, so the
/// largest key is the smallest estimate; `cost` sits in the low half, so among
/// equal estimates the largest key is the largest cost. **The comparison
/// outcome is the same on every input as the two-field version it replaces**,
/// including which pairs compare equal - so `BinaryHeap` sifts identically and
/// the boards do not move. What changes is 24 bytes to 16 and two integer
/// comparisons to one, in the function that is 9.45% of the profile.
///
/// Both halves are clamped to `u32::MAX`. Measured on the six benchmarks the
/// largest key either number reaches is **526,738**, on `qfp_fanout` - a
/// headroom of about 8,000x - so the clamp is unreachable rather than
/// merely unlikely. It is there because a step is `(f * 1000.0).round()` and
/// `f` carries penalties a sweep can set high, and a wrap would reorder the
/// frontier silently.
#[derive(Copy, Clone, Eq, PartialEq)]
struct Frontier {
    key: u64,
    cell: u32,
}

impl Ord for Frontier {
    /// The key and nothing else.
    ///
    /// Deriving this would compare `cell` after `key` and break ties by cell
    /// index, which is a different search: measured, `led_blink` came out 20
    /// segments against 21, `qfp_fanout` 1489 against 1478 and 359 violations
    /// against 318. The old two-field `Ord` ignored `cell` and so does this.
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.key.cmp(&other.key)
    }
}

impl PartialOrd for Frontier {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Frontier {
    /// Pack an estimate and a cost into one comparable key.
    #[inline]
    fn key(estimated: u64, cost: u64) -> u64 {
        let ceiling = u32::MAX as u64;
        let estimated = estimated.min(ceiling);
        let cost = cost.min(ceiling);
        ((ceiling - estimated) << 32) | cost
    }
}

/// The working memory of a grid search, kept between searches.
///
/// Sized once for a grid and reused for every connection routed on it. Rebuilt
/// only when the grid's shape changes, which it does not during a routing run.
pub struct GridSearchScratch {
    width: u32,
    height: u32,
    layers: usize,
    /// Cost from the start to this cell, meaningful only when stamped.
    cost: Vec<u64>,
    /// The cell this one was reached from, meaningful only when stamped.
    parent: Vec<u32>,
    /// Which search last wrote this cell.
    stamp: Vec<u32>,
    /// The current search's number. Never zero, so a zeroed `stamp` array
    /// reads as "no search has touched this".
    epoch: u32,
    frontier: BinaryHeap<Frontier>,
    /// Reused by `reconstruct`, so returning a path allocates nothing.
    path: Vec<GridNode>,
}

impl GridSearchScratch {
    /// Working memory for a grid of this shape.
    pub fn for_grid(width: u32, height: u32, layers: usize) -> Self {
        let cells = width as usize * height as usize * layers;
        GridSearchScratch {
            width,
            height,
            layers,
            cost: vec![0; cells],
            parent: vec![0; cells],
            stamp: vec![0; cells],
            epoch: 0,
            frontier: BinaryHeap::with_capacity(1024),
            path: Vec::with_capacity(256),
        }
    }

    /// Whether this scratch space is the right shape for a grid.
    pub fn fits(&self, width: u32, height: u32, layers: usize) -> bool {
        self.width == width && self.height == height && self.layers == layers
    }

    #[inline]
    fn index(&self, node: GridNode) -> u32 {
        let (x, y, layer) = node;
        (layer as u32 * self.height + y as u32) * self.width + x as u32
    }

    #[inline]
    fn node(&self, cell: u32) -> GridNode {
        let layer = cell / (self.width * self.height);
        let rest = cell % (self.width * self.height);
        let y = rest / self.width;
        let x = rest % self.width;
        (x as u16, y as u16, layer as u8)
    }

    /// Start a new search. No array is cleared; the epoch invalidates them.
    fn begin(&mut self) {
        self.frontier.clear();
        self.path.clear();
        // On the four-billionth search in one process the epoch wraps, and a
        // stale stamp could be mistaken for a live one. Zeroing then costs one
        // pass over the arrays, once, rather than a branch per lookup.
        if self.epoch == u32::MAX {
            self.stamp.fill(0);
            self.epoch = 0;
        }
        self.epoch += 1;
    }

    #[inline]
    fn visited(&self, cell: u32) -> bool {
        self.stamp[cell as usize] == self.epoch
    }
}

/// Find the cheapest path from `start` to a cell `success` accepts.
///
/// `successors` is called once per expanded cell and writes its neighbours
/// into the buffer it is given, which the caller owns - so expanding a node
/// allocates nothing at all.
///
/// Returns the path from `start` to the goal inclusive, or `None`.
pub fn astar_grid<S, H, G>(
    scratch: &mut GridSearchScratch,
    start: GridNode,
    mut successors: S,
    mut heuristic: H,
    mut success: G,
) -> Option<&[GridNode]>
where
    S: FnMut(GridNode, &mut Vec<(GridNode, u64)>),
    H: FnMut(GridNode) -> u64,
    G: FnMut(GridNode) -> bool,
{
    scratch.begin();

    let start_cell = scratch.index(start);
    scratch.cost[start_cell as usize] = 0;
    scratch.parent[start_cell as usize] = start_cell;
    scratch.stamp[start_cell as usize] = scratch.epoch;
    scratch.frontier.push(Frontier {
        key: Frontier::key(heuristic(start), 0),
        cell: start_cell,
    });

    // One buffer for every expansion in this search.
    let mut neighbours: Vec<(GridNode, u64)> = Vec::with_capacity(16);
    let mut goal_cell = None;

    while let Some(Frontier { key, cell }) = scratch.frontier.pop() {
        let cost = key & 0xffff_ffff;
        let node = scratch.node(cell);
        if success(node) {
            goal_cell = Some(cell);
            break;
        }

        // A cell can sit in the frontier more than once, at different costs.
        // An entry that is dearer than what the cell now holds was superseded
        // while it waited, and expanding it would redo work already done at a
        // lower cost.
        //
        // There is deliberately no closed set here. A closed set is only safe
        // when the heuristic is consistent, and this one is not obviously so:
        // it estimates distance while the edges carry congestion, pad and via
        // penalties that the estimate knows nothing about. Re-expanding a cell
        // that later turns out cheaper is what `pathfinding::astar` does, and
        // adding a closed set changed the boards this router produces - two
        // fixtures moved outside their measured noise bands.
        if cost > scratch.cost[cell as usize] {
            continue;
        }

        neighbours.clear();
        successors(node, &mut neighbours);
        for &(next, step) in &neighbours {
            let next_cell = scratch.index(next);
            let next_cost = cost + step;
            if scratch.visited(next_cell) && scratch.cost[next_cell as usize] <= next_cost {
                continue;
            }
            scratch.cost[next_cell as usize] = next_cost;
            scratch.parent[next_cell as usize] = cell;
            scratch.stamp[next_cell as usize] = scratch.epoch;
            scratch.frontier.push(Frontier {
                key: Frontier::key(next_cost + heuristic(next), next_cost),
                cell: next_cell,
            });
        }
    }

    let mut cell = goal_cell?;
    // Walk the parents back to the start, then reverse: the path is returned
    // in the order it is walked.
    loop {
        scratch.path.push(scratch.node(cell));
        let parent = scratch.parent[cell as usize];
        if parent == cell {
            break;
        }
        cell = parent;
    }
    scratch.path.reverse();
    Some(&scratch.path)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A grid where every step costs one and nothing is blocked.
    fn open_grid(scratch: &mut GridSearchScratch, start: GridNode, end: GridNode) -> Vec<GridNode> {
        let (w, h) = (scratch.width, scratch.height);
        let path = astar_grid(
            scratch,
            start,
            |node, out| {
                for (dx, dy) in [(0i32, 1i32), (1, 0), (0, -1), (-1, 0)] {
                    let nx = node.0 as i32 + dx;
                    let ny = node.1 as i32 + dy;
                    if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                        continue;
                    }
                    out.push(((nx as u16, ny as u16, node.2), 1));
                }
            },
            |node| {
                let dx = (node.0 as i64 - end.0 as i64).unsigned_abs();
                let dy = (node.1 as i64 - end.1 as i64).unsigned_abs();
                dx + dy
            },
            |node| node == end,
        );
        path.expect("an open grid always has a path").to_vec()
    }

    #[test]
    fn it_finds_the_shortest_path_on_an_open_grid() {
        let mut scratch = GridSearchScratch::for_grid(20, 20, 1);
        let path = open_grid(&mut scratch, (0, 0, 0), (5, 7, 0));

        assert_eq!(path.first(), Some(&(0, 0, 0)));
        assert_eq!(path.last(), Some(&(5, 7, 0)));
        // Manhattan distance plus the start cell.
        assert_eq!(path.len(), 5 + 7 + 1);
    }

    #[test]
    fn the_second_search_is_not_poisoned_by_the_first() {
        // The whole point of the epoch: the arrays keep the previous search's
        // costs and parents, and the next search has to read straight past
        // them. A test that runs one search proves nothing about that.
        let mut scratch = GridSearchScratch::for_grid(20, 20, 1);
        let first = open_grid(&mut scratch, (0, 0, 0), (9, 9, 0));
        assert_eq!(first.len(), 19);

        let second = open_grid(&mut scratch, (9, 9, 0), (0, 0, 0));
        assert_eq!(second.first(), Some(&(9, 9, 0)));
        assert_eq!(second.last(), Some(&(0, 0, 0)));
        assert_eq!(second.len(), 19);

        // And a third that shares cells with both.
        let third = open_grid(&mut scratch, (0, 9, 0), (9, 0, 0));
        assert_eq!(third.len(), 19);
    }

    #[test]
    fn a_wall_with_no_door_has_no_path() {
        let mut scratch = GridSearchScratch::for_grid(10, 10, 1);
        let end = (9u16, 5u16, 0u8);
        let path = astar_grid(
            &mut scratch,
            (0, 5, 0),
            |node, out| {
                for (dx, dy) in [(0i32, 1i32), (1, 0), (0, -1), (-1, 0)] {
                    let nx = node.0 as i32 + dx;
                    let ny = node.1 as i32 + dy;
                    if nx < 0 || ny < 0 || nx >= 10 || ny >= 10 {
                        continue;
                    }
                    // Column 5 is solid.
                    if nx == 5 {
                        continue;
                    }
                    out.push(((nx as u16, ny as u16, node.2), 1));
                }
            },
            |_| 0,
            |node| node == end,
        );
        assert!(path.is_none(), "there is no way through a solid column");
    }

    #[test]
    fn the_cheaper_way_round_wins() {
        // Straight line costs 10 a step, the detour costs 1, so the path has
        // to be the long way: a search that ignores costs would take the
        // straight line and this test would see a shorter path.
        let mut scratch = GridSearchScratch::for_grid(10, 10, 1);
        let end = (3u16, 0u16, 0u8);
        let path = astar_grid(
            &mut scratch,
            (0, 0, 0),
            |node, out| {
                for (dx, dy) in [(0i32, 1i32), (1, 0), (0, -1), (-1, 0)] {
                    let nx = node.0 as i32 + dx;
                    let ny = node.1 as i32 + dy;
                    if nx < 0 || ny < 0 || nx >= 10 || ny >= 10 {
                        continue;
                    }
                    let cost = if ny == 0 && nx > 0 && nx < 3 { 10 } else { 1 };
                    out.push(((nx as u16, ny as u16, node.2), cost));
                }
            },
            |_| 0,
            |node| node == end,
        );
        let path = path.expect("there is a way round").to_vec();
        assert!(
            path.iter().any(|node| node.1 > 0),
            "the cheap way leaves row 0: {path:?}"
        );
    }

    #[test]
    fn it_crosses_layers() {
        let mut scratch = GridSearchScratch::for_grid(4, 4, 2);
        let end = (0u16, 0u16, 1u8);
        let path = astar_grid(
            &mut scratch,
            (0, 0, 0),
            |node, out| {
                let other = if node.2 == 0 { 1 } else { 0 };
                out.push(((node.0, node.1, other), 1));
            },
            |_| 0,
            |node| node == end,
        );
        assert_eq!(
            path.map(<[GridNode]>::to_vec),
            Some(vec![(0, 0, 0), (0, 0, 1)])
        );
    }
}
