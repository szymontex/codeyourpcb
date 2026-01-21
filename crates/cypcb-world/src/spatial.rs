//! Spatial indexing for efficient region queries.
//!
//! Uses an R*-tree to enable O(log n) queries for:
//! - Finding all entities in a rectangular region
//! - Finding entities at a specific point
//! - Finding entities that might overlap with a given entity
//!
//! This is essential for:
//! - DRC (Design Rule Check) - finding nearby objects to check clearances
//! - Rendering - only drawing visible objects
//! - Selection - finding objects under cursor

use bevy_ecs::prelude::*;
use rstar::{RTree, RTreeObject, AABB};

use cypcb_core::Point;

/// An entry in the spatial index.
///
/// Associates an entity with its bounding box and layer information.
/// The layer mask allows filtering queries by copper layer.
#[derive(Debug, Clone)]
pub struct SpatialEntry {
    /// The ECS entity this entry represents.
    pub entity: Entity,
    /// The axis-aligned bounding box in nanometers.
    pub envelope: AABB<[i64; 2]>,
    /// Bit mask of layers this entity occupies (bit 0 = layer 0, etc.).
    pub layer_mask: u32,
}

impl SpatialEntry {
    /// Create a new spatial entry.
    ///
    /// # Arguments
    ///
    /// * `entity` - The ECS entity
    /// * `min` - Minimum corner of bounding box
    /// * `max` - Maximum corner of bounding box
    /// * `layer_mask` - Bit mask of occupied layers
    ///
    /// # Examples
    ///
    /// ```
    /// use bevy_ecs::prelude::*;
    /// use cypcb_world::SpatialEntry;
    /// use cypcb_core::Point;
    ///
    /// // Create an entry for a component on top layer only
    /// let entry = SpatialEntry::new(
    ///     Entity::from_raw(0),
    ///     Point::from_mm(0.0, 0.0),
    ///     Point::from_mm(10.0, 10.0),
    ///     0b01,  // Top layer only
    /// );
    /// ```
    pub fn new(entity: Entity, min: Point, max: Point, layer_mask: u32) -> Self {
        SpatialEntry {
            entity,
            envelope: AABB::from_corners([min.x.0, min.y.0], [max.x.0, max.y.0]),
            layer_mask,
        }
    }

    /// Create an entry from raw coordinates.
    pub fn from_raw(entity: Entity, min_x: i64, min_y: i64, max_x: i64, max_y: i64, layer_mask: u32) -> Self {
        SpatialEntry {
            entity,
            envelope: AABB::from_corners([min_x, min_y], [max_x, max_y]),
            layer_mask,
        }
    }

    /// Check if this entry overlaps with a specific layer.
    #[inline]
    pub fn on_layer(&self, layer: u8) -> bool {
        self.layer_mask & (1 << layer) != 0
    }

    /// Check if this entry shares any layers with the given mask.
    #[inline]
    pub fn layers_overlap(&self, mask: u32) -> bool {
        self.layer_mask & mask != 0
    }
}

impl RTreeObject for SpatialEntry {
    type Envelope = AABB<[i64; 2]>;

    fn envelope(&self) -> Self::Envelope {
        self.envelope
    }
}

/// R*-tree spatial index for efficient region queries.
///
/// Wraps an rstar `RTree` to provide O(log n) spatial queries.
/// The index is rebuilt after batch updates rather than supporting
/// incremental insertion, which is more efficient for the typical
/// "parse file, build world" workflow.
///
/// # Examples
///
/// ```
/// use bevy_ecs::prelude::*;
/// use cypcb_world::{SpatialIndex, SpatialEntry};
/// use cypcb_core::Point;
///
/// let mut index = SpatialIndex::new();
///
/// // Create some entries
/// let entries = vec![
///     SpatialEntry::new(
///         Entity::from_raw(0),
///         Point::from_mm(0.0, 0.0),
///         Point::from_mm(10.0, 10.0),
///         0b01,
///     ),
///     SpatialEntry::new(
///         Entity::from_raw(1),
///         Point::from_mm(5.0, 5.0),
///         Point::from_mm(15.0, 15.0),
///         0b01,
///     ),
/// ];
///
/// // Bulk load for efficiency
/// index.rebuild(entries);
///
/// // Query a region
/// let found: Vec<_> = index.query_region(
///     Point::from_mm(4.0, 4.0),
///     Point::from_mm(6.0, 6.0),
/// ).collect();
/// assert_eq!(found.len(), 2); // Both entries overlap
/// ```
#[derive(Resource, Default)]
pub struct SpatialIndex {
    tree: RTree<SpatialEntry>,
}

impl SpatialIndex {
    /// Create an empty spatial index.
    #[inline]
    pub fn new() -> Self {
        SpatialIndex {
            tree: RTree::new(),
        }
    }

    /// Query all entities whose bounding boxes intersect the given region.
    ///
    /// Returns an iterator over entities (not entries) for convenience.
    ///
    /// # Arguments
    ///
    /// * `min` - Minimum corner of query region
    /// * `max` - Maximum corner of query region
    ///
    /// # Examples
    ///
    /// ```
    /// use bevy_ecs::prelude::*;
    /// use cypcb_world::{SpatialIndex, SpatialEntry};
    /// use cypcb_core::Point;
    ///
    /// let mut index = SpatialIndex::new();
    /// index.rebuild(vec![
    ///     SpatialEntry::new(Entity::from_raw(0), Point::from_mm(0.0, 0.0), Point::from_mm(10.0, 10.0), 1),
    /// ]);
    ///
    /// let found: Vec<_> = index.query_region(Point::from_mm(5.0, 5.0), Point::from_mm(15.0, 15.0)).collect();
    /// assert_eq!(found.len(), 1);
    /// ```
    pub fn query_region(&self, min: Point, max: Point) -> impl Iterator<Item = Entity> + '_ {
        let envelope = AABB::from_corners([min.x.0, min.y.0], [max.x.0, max.y.0]);
        self.tree
            .locate_in_envelope_intersecting(&envelope)
            .map(|e| e.entity)
    }

    /// Query all entries (with full metadata) in a region.
    ///
    /// Use this when you need layer information or bounding boxes.
    pub fn query_region_entries(&self, min: Point, max: Point) -> impl Iterator<Item = &SpatialEntry> {
        let envelope = AABB::from_corners([min.x.0, min.y.0], [max.x.0, max.y.0]);
        self.tree.locate_in_envelope_intersecting(&envelope)
    }

    /// Query entities at a specific point.
    ///
    /// Returns all entities whose bounding box contains the point.
    pub fn query_point(&self, point: Point) -> impl Iterator<Item = Entity> + '_ {
        let p = [point.x.0, point.y.0];
        let envelope = AABB::from_point(p);
        self.tree
            .locate_in_envelope_intersecting(&envelope)
            .map(|e| e.entity)
    }

    /// Query entities at a point, filtered by layer.
    ///
    /// # Arguments
    ///
    /// * `point` - The query point
    /// * `layer_mask` - Bit mask of layers to include
    pub fn query_point_on_layers(
        &self,
        point: Point,
        layer_mask: u32,
    ) -> impl Iterator<Item = Entity> + '_ {
        let p = [point.x.0, point.y.0];
        let envelope = AABB::from_point(p);
        self.tree
            .locate_in_envelope_intersecting(&envelope)
            .filter(move |e| e.layers_overlap(layer_mask))
            .map(|e| e.entity)
    }

    /// Query entities in a region, filtered by layer.
    ///
    /// # Arguments
    ///
    /// * `min` - Minimum corner of query region
    /// * `max` - Maximum corner of query region
    /// * `layer_mask` - Bit mask of layers to include
    pub fn query_region_on_layers(
        &self,
        min: Point,
        max: Point,
        layer_mask: u32,
    ) -> impl Iterator<Item = Entity> + '_ {
        let envelope = AABB::from_corners([min.x.0, min.y.0], [max.x.0, max.y.0]);
        self.tree
            .locate_in_envelope_intersecting(&envelope)
            .filter(move |e| e.layers_overlap(layer_mask))
            .map(|e| e.entity)
    }

    /// Rebuild the spatial index with new entries.
    ///
    /// Uses bulk loading for optimal tree structure.
    /// This is more efficient than incremental insertion for large batches.
    ///
    /// # Arguments
    ///
    /// * `entries` - Vector of spatial entries to index
    pub fn rebuild(&mut self, entries: Vec<SpatialEntry>) {
        self.tree = RTree::bulk_load(entries);
    }

    /// Clear the spatial index.
    pub fn clear(&mut self) {
        self.tree = RTree::new();
    }

    /// Check if the index is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.tree.size() == 0
    }

    /// Get the number of entries in the index.
    #[inline]
    pub fn len(&self) -> usize {
        self.tree.size()
    }

    /// Iterate over all entries in the index.
    pub fn iter(&self) -> impl Iterator<Item = &SpatialEntry> {
        self.tree.iter()
    }
}

impl std::fmt::Debug for SpatialIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SpatialIndex")
            .field("entries", &self.tree.size())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(id: u32, min_mm: (f64, f64), max_mm: (f64, f64), layers: u32) -> SpatialEntry {
        SpatialEntry::new(
            Entity::from_raw(id),
            Point::from_mm(min_mm.0, min_mm.1),
            Point::from_mm(max_mm.0, max_mm.1),
            layers,
        )
    }

    #[test]
    fn test_empty_index() {
        let index = SpatialIndex::new();
        assert!(index.is_empty());
        assert_eq!(index.len(), 0);
    }

    #[test]
    fn test_rebuild_and_len() {
        let mut index = SpatialIndex::new();
        let entries = vec![
            make_entry(0, (0.0, 0.0), (10.0, 10.0), 1),
            make_entry(1, (20.0, 20.0), (30.0, 30.0), 1),
        ];
        index.rebuild(entries);

        assert!(!index.is_empty());
        assert_eq!(index.len(), 2);
    }

    #[test]
    fn test_query_region_finds_intersecting() {
        let mut index = SpatialIndex::new();
        index.rebuild(vec![
            make_entry(0, (0.0, 0.0), (10.0, 10.0), 1),
            make_entry(1, (5.0, 5.0), (15.0, 15.0), 1),
            make_entry(2, (100.0, 100.0), (110.0, 110.0), 1),
        ]);

        // Query overlapping region - should find entries 0 and 1
        let found: Vec<_> = index
            .query_region(Point::from_mm(4.0, 4.0), Point::from_mm(6.0, 6.0))
            .collect();
        assert_eq!(found.len(), 2);

        // Query region only overlapping entry 2
        let found: Vec<_> = index
            .query_region(Point::from_mm(105.0, 105.0), Point::from_mm(106.0, 106.0))
            .collect();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0], Entity::from_raw(2));
    }

    #[test]
    fn test_query_region_no_match() {
        let mut index = SpatialIndex::new();
        index.rebuild(vec![make_entry(0, (0.0, 0.0), (10.0, 10.0), 1)]);

        let found: Vec<_> = index
            .query_region(Point::from_mm(50.0, 50.0), Point::from_mm(60.0, 60.0))
            .collect();
        assert!(found.is_empty());
    }

    #[test]
    fn test_query_point() {
        let mut index = SpatialIndex::new();
        index.rebuild(vec![
            make_entry(0, (0.0, 0.0), (10.0, 10.0), 1),
            make_entry(1, (5.0, 5.0), (15.0, 15.0), 1),
        ]);

        // Point inside only entry 0
        let found: Vec<_> = index.query_point(Point::from_mm(2.0, 2.0)).collect();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0], Entity::from_raw(0));

        // Point inside both entries
        let found: Vec<_> = index.query_point(Point::from_mm(7.0, 7.0)).collect();
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn test_layer_filtering() {
        let mut index = SpatialIndex::new();
        index.rebuild(vec![
            make_entry(0, (0.0, 0.0), (10.0, 10.0), 0b01), // Top only
            make_entry(1, (0.0, 0.0), (10.0, 10.0), 0b10), // Bottom only
            make_entry(2, (0.0, 0.0), (10.0, 10.0), 0b11), // Both
        ]);

        // Query top layer only
        let found: Vec<_> = index
            .query_region_on_layers(
                Point::from_mm(0.0, 0.0),
                Point::from_mm(10.0, 10.0),
                0b01,
            )
            .collect();
        assert_eq!(found.len(), 2); // Entries 0 and 2

        // Query bottom layer only
        let found: Vec<_> = index
            .query_region_on_layers(
                Point::from_mm(0.0, 0.0),
                Point::from_mm(10.0, 10.0),
                0b10,
            )
            .collect();
        assert_eq!(found.len(), 2); // Entries 1 and 2
    }

    #[test]
    fn test_entry_layer_methods() {
        let entry = make_entry(0, (0.0, 0.0), (10.0, 10.0), 0b101); // Layers 0 and 2

        assert!(entry.on_layer(0));
        assert!(!entry.on_layer(1));
        assert!(entry.on_layer(2));

        assert!(entry.layers_overlap(0b001));
        assert!(!entry.layers_overlap(0b010));
        assert!(entry.layers_overlap(0b111));
    }

    #[test]
    fn test_clear() {
        let mut index = SpatialIndex::new();
        index.rebuild(vec![make_entry(0, (0.0, 0.0), (10.0, 10.0), 1)]);
        assert!(!index.is_empty());

        index.clear();
        assert!(index.is_empty());
    }

    #[test]
    fn test_iter() {
        let mut index = SpatialIndex::new();
        index.rebuild(vec![
            make_entry(0, (0.0, 0.0), (10.0, 10.0), 1),
            make_entry(1, (20.0, 20.0), (30.0, 30.0), 1),
        ]);

        let entries: Vec<_> = index.iter().collect();
        assert_eq!(entries.len(), 2);
    }
}
