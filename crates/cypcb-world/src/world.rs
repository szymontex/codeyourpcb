//! BoardWorld - High-level API for PCB board entities.
//!
//! Provides a clean wrapper around bevy_ecs::World with PCB-specific
//! operations for creating boards, spawning components, and querying.
//!
//! # Example
//!
//! ```
//! use cypcb_world::{BoardWorld, RefDes, Value, Position, Rotation, FootprintRef, NetConnections};
//! use cypcb_core::Nm;
//!
//! // Create a new board world
//! let mut world = BoardWorld::new();
//!
//! // Set up the board
//! let board = world.set_board("MyBoard".to_string(), (Nm::from_mm(100.0), Nm::from_mm(80.0)), 4);
//!
//! // Intern net names
//! let vcc = world.intern_net("VCC");
//! let gnd = world.intern_net("GND");
//!
//! // Spawn components
//! let r1 = world.spawn_component(
//!     RefDes::new("R1"),
//!     Value::new("10k"),
//!     Position::from_mm(10.0, 20.0),
//!     Rotation::ZERO,
//!     FootprintRef::new("0402"),
//!     NetConnections::new(),
//! );
//!
//! // Query the world
//! assert_eq!(world.component_count(), 1);
//! ```

use bevy_ecs::prelude::*;

use cypcb_core::{Nm, Point, Rect};

use crate::components::*;
use crate::registry::NetRegistry;
use crate::spatial::{SpatialEntry, SpatialIndex};

/// High-level wrapper around bevy_ecs::World for PCB board operations.
///
/// BoardWorld provides a clean, type-safe API for:
/// - Creating and configuring the board
/// - Spawning component entities
/// - Interning net names
/// - Spatial queries
/// - Direct ECS access when needed
///
/// # Architecture
///
/// Internally uses bevy_ecs::World with two resources:
/// - [`NetRegistry`]: For interning net names to integer IDs
/// - [`SpatialIndex`]: For efficient region-based queries
///
/// # Example
///
/// ```
/// use cypcb_world::BoardWorld;
/// use cypcb_core::Nm;
///
/// let mut world = BoardWorld::new();
///
/// // Create a 100x80mm 4-layer board
/// world.set_board("MyPCB".to_string(), (Nm::from_mm(100.0), Nm::from_mm(80.0)), 4);
///
/// // Get board info
/// if let Some((size, layers)) = world.board_info() {
///     println!("Board: {}x{}, {} layers", size.width, size.height, layers.count);
/// }
/// ```
pub struct BoardWorld {
    /// The underlying bevy_ecs World.
    world: World,
    /// Entity ID of the board (if set).
    board_entity: Option<Entity>,
}

impl BoardWorld {
    /// Create a new empty BoardWorld.
    ///
    /// Initializes the ECS world with:
    /// - Empty [`SpatialIndex`] resource
    /// - Empty [`NetRegistry`] resource
    ///
    /// # Example
    ///
    /// ```
    /// use cypcb_world::BoardWorld;
    ///
    /// let world = BoardWorld::new();
    /// assert!(world.is_empty());
    /// ```
    pub fn new() -> Self {
        let mut world = World::new();
        world.insert_resource(SpatialIndex::new());
        world.insert_resource(NetRegistry::new());
        BoardWorld {
            world,
            board_entity: None,
        }
    }

    // ========================================================================
    // Board Management
    // ========================================================================

    /// Set the board properties, creating or updating the board entity.
    ///
    /// There can only be one board entity. Calling this multiple times
    /// will update the existing board entity.
    ///
    /// # Arguments
    ///
    /// * `name` - Board name/identifier
    /// * `size` - Board dimensions as (width, height) in nanometers
    /// * `layers` - Number of copper layers (2-32)
    ///
    /// # Returns
    ///
    /// The board entity ID.
    ///
    /// # Example
    ///
    /// ```
    /// use cypcb_world::BoardWorld;
    /// use cypcb_core::Nm;
    ///
    /// let mut world = BoardWorld::new();
    /// let board = world.set_board(
    ///     "TestBoard".to_string(),
    ///     (Nm::from_mm(100.0), Nm::from_mm(80.0)),
    ///     4,
    /// );
    /// ```
    pub fn set_board(&mut self, name: String, size: (Nm, Nm), layers: u8) -> Entity {
        let board_size = BoardSize::new(size.0, size.1);
        let layer_stack = LayerStack::new(layers);
        let name_component = crate::components::metadata::Name(name);

        if let Some(entity) = self.board_entity {
            // Update existing board
            let mut entity_mut = self.world.entity_mut(entity);
            entity_mut.insert((Board, board_size, layer_stack, name_component));
            entity
        } else {
            // Create new board
            let entity = self
                .world
                .spawn((Board, board_size, layer_stack, name_component))
                .id();
            self.board_entity = Some(entity);
            entity
        }
    }

    /// Get the board entity ID if one has been set.
    pub fn board_entity(&self) -> Option<Entity> {
        self.board_entity
    }

    /// Get board size and layer stack if a board has been set.
    ///
    /// # Returns
    ///
    /// Tuple of (BoardSize, LayerStack) if board exists, None otherwise.
    pub fn board_info(&self) -> Option<(BoardSize, LayerStack)> {
        let entity = self.board_entity?;
        let size = self.world.get::<BoardSize>(entity)?;
        let layers = self.world.get::<LayerStack>(entity)?;
        Some((*size, *layers))
    }

    /// Get the board name if one has been set.
    pub fn board_name(&self) -> Option<&str> {
        let entity = self.board_entity?;
        self.world
            .get::<crate::components::metadata::Name>(entity)
            .map(|n| n.0.as_str())
    }

    // ========================================================================
    // Component Spawning
    // ========================================================================

    /// Spawn a new component entity.
    ///
    /// # Arguments
    ///
    /// * `refdes` - Reference designator (R1, C1, U1, etc.)
    /// * `value` - Component value (10k, 100nF, etc.)
    /// * `position` - Position on board
    /// * `rotation` - Rotation angle
    /// * `footprint` - Footprint reference
    /// * `nets` - Net connections for each pin
    ///
    /// # Returns
    ///
    /// The spawned entity ID.
    ///
    /// # Example
    ///
    /// ```
    /// use cypcb_world::{BoardWorld, RefDes, Value, Position, Rotation, FootprintRef, NetConnections};
    ///
    /// let mut world = BoardWorld::new();
    /// let r1 = world.spawn_component(
    ///     RefDes::new("R1"),
    ///     Value::new("10k"),
    ///     Position::from_mm(10.0, 20.0),
    ///     Rotation::ZERO,
    ///     FootprintRef::new("0402"),
    ///     NetConnections::new(),
    /// );
    /// ```
    pub fn spawn_component(
        &mut self,
        refdes: RefDes,
        value: Value,
        position: Position,
        rotation: Rotation,
        footprint: FootprintRef,
        nets: NetConnections,
    ) -> Entity {
        self.world
            .spawn((refdes, value, position, rotation, footprint, nets))
            .id()
    }

    /// Spawn a component with a source span for error reporting.
    ///
    /// Same as [`spawn_component`](Self::spawn_component) but also attaches
    /// a [`SourceSpan`] for linking back to source file locations.
    pub fn spawn_component_with_span(
        &mut self,
        refdes: RefDes,
        value: Value,
        position: Position,
        rotation: Rotation,
        footprint: FootprintRef,
        nets: NetConnections,
        span: SourceSpan,
    ) -> Entity {
        self.world
            .spawn((refdes, value, position, rotation, footprint, nets, span))
            .id()
    }

    /// Spawn an entity with arbitrary components.
    ///
    /// Use this for custom entity types not covered by [`spawn_component`](Self::spawn_component).
    ///
    /// # Example
    ///
    /// ```
    /// use cypcb_world::{BoardWorld, RefDes, Position};
    ///
    /// let mut world = BoardWorld::new();
    /// let entity = world.spawn_entity((
    ///     RefDes::new("J1"),
    ///     Position::from_mm(5.0, 5.0),
    /// ));
    /// ```
    pub fn spawn_entity<B: Bundle>(&mut self, bundle: B) -> Entity {
        self.world.spawn(bundle).id()
    }

    // ========================================================================
    // Net Registry
    // ========================================================================

    /// Intern a net name, returning its unique NetId.
    ///
    /// If the name has already been interned, returns the existing ID.
    ///
    /// # Example
    ///
    /// ```
    /// use cypcb_world::BoardWorld;
    ///
    /// let mut world = BoardWorld::new();
    /// let vcc = world.intern_net("VCC");
    /// let vcc2 = world.intern_net("VCC");
    /// assert_eq!(vcc, vcc2); // Same ID
    /// ```
    pub fn intern_net(&mut self, name: &str) -> NetId {
        self.world.resource_mut::<NetRegistry>().intern(name)
    }

    /// Get the name for a NetId.
    ///
    /// Returns None if the ID is invalid.
    pub fn net_name(&self, id: NetId) -> Option<&str> {
        self.world.resource::<NetRegistry>().name(id)
    }

    /// Look up a NetId by name without interning.
    ///
    /// Returns None if the name has not been interned.
    pub fn get_net(&self, name: &str) -> Option<NetId> {
        self.world.resource::<NetRegistry>().get(name)
    }

    /// Get the number of interned nets.
    pub fn net_count(&self) -> usize {
        self.world.resource::<NetRegistry>().len()
    }

    /// Iterate over all interned nets.
    pub fn nets(&self) -> impl Iterator<Item = (NetId, &str)> {
        self.world.resource::<NetRegistry>().iter()
    }

    // ========================================================================
    // Spatial Index
    // ========================================================================

    /// Get a reference to the spatial index.
    ///
    /// Use this for direct spatial queries, iteration, or DRC checks.
    ///
    /// # Note
    ///
    /// The spatial index must be rebuilt after component changes
    /// using [`rebuild_spatial_index`](Self::rebuild_spatial_index).
    ///
    /// # Example
    ///
    /// ```
    /// use cypcb_world::BoardWorld;
    ///
    /// let world = BoardWorld::new();
    /// let spatial = world.spatial();
    /// assert!(spatial.is_empty());
    /// ```
    pub fn spatial(&self) -> &SpatialIndex {
        self.world.resource::<SpatialIndex>()
    }

    /// Rebuild the spatial index from current component positions.
    ///
    /// Call this after modifying component positions or spawning
    /// components to update the spatial index for efficient queries.
    ///
    /// # Arguments
    ///
    /// * `footprint_bounds` - Function to get bounds for a footprint name.
    ///   Used to calculate entity bounding boxes.
    ///
    /// # Example
    ///
    /// ```
    /// use cypcb_world::BoardWorld;
    /// use cypcb_core::{Nm, Rect, Point};
    ///
    /// let mut world = BoardWorld::new();
    /// // ... spawn components ...
    ///
    /// // Rebuild with simple bounds
    /// world.rebuild_spatial_index(|_| Rect::from_center_size(
    ///     Point::ORIGIN,
    ///     (Nm::from_mm(1.0), Nm::from_mm(1.0)),
    /// ));
    /// ```
    pub fn rebuild_spatial_index<F>(&mut self, footprint_bounds: F)
    where
        F: Fn(&str) -> Rect,
    {
        let mut entries = Vec::new();

        // Query all positioned entities with footprints
        let mut query = self
            .world
            .query::<(Entity, &Position, &FootprintRef)>();

        for (entity, position, footprint) in query.iter(&self.world) {
            let bounds = footprint_bounds(footprint.as_str());
            let pos = position.0;

            // Translate bounds by position
            let min = Point::new(
                Nm(pos.x.0 + bounds.min.x.0),
                Nm(pos.y.0 + bounds.min.y.0),
            );
            let max = Point::new(
                Nm(pos.x.0 + bounds.max.x.0),
                Nm(pos.y.0 + bounds.max.y.0),
            );

            // Default to all layers for now (could be refined with pad layer info)
            let layer_mask = 0xFFFFFFFF;

            entries.push(SpatialEntry::new(entity, min, max, layer_mask));
        }

        self.world.resource_mut::<SpatialIndex>().rebuild(entries);
    }

    /// Query entities in a rectangular region.
    ///
    /// Returns all entities whose bounding boxes intersect the given bounds.
    ///
    /// # Note
    ///
    /// The spatial index must be rebuilt after component changes
    /// using [`rebuild_spatial_index`](Self::rebuild_spatial_index).
    pub fn query_region(&self, bounds: Rect) -> Vec<Entity> {
        self.world
            .resource::<SpatialIndex>()
            .query_region(bounds.min, bounds.max)
            .collect()
    }

    /// Query entities at a specific point.
    ///
    /// Returns all entities whose bounding boxes contain the point.
    pub fn query_point(&self, point: Point) -> Vec<Entity> {
        self.world
            .resource::<SpatialIndex>()
            .query_point(point)
            .collect()
    }

    /// Query entities in a region, filtered by layer.
    ///
    /// # Arguments
    ///
    /// * `bounds` - The query region
    /// * `layer_mask` - Bit mask of layers to include
    pub fn query_region_on_layers(&self, bounds: Rect, layer_mask: u32) -> Vec<Entity> {
        self.world
            .resource::<SpatialIndex>()
            .query_region_on_layers(bounds.min, bounds.max, layer_mask)
            .collect()
    }

    // ========================================================================
    // Entity Queries
    // ========================================================================

    /// Get the number of component entities (excluding board).
    pub fn component_count(&mut self) -> usize {
        let mut query = self.world.query::<&RefDes>();
        query.iter(&self.world).count()
    }

    /// Check if the world is empty (no entities).
    pub fn is_empty(&self) -> bool {
        self.world.entities().len() == 0
    }

    /// Get a component from an entity.
    ///
    /// # Example
    ///
    /// ```
    /// use cypcb_world::{BoardWorld, RefDes, Value, Position, Rotation, FootprintRef, NetConnections};
    ///
    /// let mut world = BoardWorld::new();
    /// let r1 = world.spawn_component(
    ///     RefDes::new("R1"),
    ///     Value::new("10k"),
    ///     Position::from_mm(10.0, 20.0),
    ///     Rotation::ZERO,
    ///     FootprintRef::new("0402"),
    ///     NetConnections::new(),
    /// );
    ///
    /// let refdes = world.get::<RefDes>(r1);
    /// assert_eq!(refdes.map(|r| r.as_str()), Some("R1"));
    /// ```
    pub fn get<T: Component>(&self, entity: Entity) -> Option<&T> {
        self.world.get::<T>(entity)
    }

    /// Get a mutable reference to a component.
    pub fn get_mut<T: Component>(&mut self, entity: Entity) -> Option<Mut<'_, T>> {
        self.world.get_mut::<T>(entity)
    }

    /// Check if an entity has a specific component.
    pub fn has<T: Component>(&self, entity: Entity) -> bool {
        self.world.get::<T>(entity).is_some()
    }

    /// Find an entity by its reference designator.
    ///
    /// # Example
    ///
    /// ```
    /// use cypcb_world::{BoardWorld, RefDes, Value, Position, Rotation, FootprintRef, NetConnections};
    ///
    /// let mut world = BoardWorld::new();
    /// let r1 = world.spawn_component(
    ///     RefDes::new("R1"),
    ///     Value::new("10k"),
    ///     Position::from_mm(10.0, 20.0),
    ///     Rotation::ZERO,
    ///     FootprintRef::new("0402"),
    ///     NetConnections::new(),
    /// );
    ///
    /// assert_eq!(world.find_by_refdes("R1"), Some(r1));
    /// assert_eq!(world.find_by_refdes("R2"), None);
    /// ```
    pub fn find_by_refdes(&mut self, refdes: &str) -> Option<Entity> {
        let mut query = self.world.query::<(Entity, &RefDes)>();
        query
            .iter(&self.world)
            .find(|(_, r)| r.as_str() == refdes)
            .map(|(e, _)| e)
    }

    /// Iterate over all components (entities with RefDes).
    ///
    /// Returns a vector of (Entity, RefDes clone, Position clone) tuples.
    /// Uses clones to avoid lifetime issues with the query.
    pub fn components(&mut self) -> Vec<(Entity, RefDes, Position)> {
        let mut query = self.world.query::<(Entity, &RefDes, &Position)>();
        query
            .iter(&self.world)
            .map(|(e, r, p)| (e, r.clone(), *p))
            .collect()
    }

    /// Iterate over all zones.
    ///
    /// Returns a vector of (Entity, Zone clone) tuples.
    /// Uses clones to avoid lifetime issues with the query.
    pub fn zones(&mut self) -> Vec<(Entity, Zone)> {
        let mut query = self.world.query::<(Entity, &Zone)>();
        query
            .iter(&self.world)
            .map(|(e, z)| (e, z.clone()))
            .collect()
    }

    // ========================================================================
    // Direct ECS Access
    // ========================================================================

    /// Get direct access to the underlying bevy_ecs World.
    ///
    /// Use this for advanced queries not covered by the BoardWorld API.
    pub fn ecs(&self) -> &World {
        &self.world
    }

    /// Get mutable access to the underlying bevy_ecs World.
    ///
    /// Use this for advanced operations not covered by the BoardWorld API.
    pub fn ecs_mut(&mut self) -> &mut World {
        &mut self.world
    }

    // ========================================================================
    // Serialization Helpers
    // ========================================================================

    /// Clear all entities and reset resources.
    pub fn clear(&mut self) {
        self.world.clear_entities();
        self.world.insert_resource(SpatialIndex::new());
        self.world.insert_resource(NetRegistry::new());
        self.board_entity = None;
    }
}

impl Default for BoardWorld {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_world_is_empty() {
        let mut world = BoardWorld::new();
        assert!(world.is_empty());
        assert_eq!(world.component_count(), 0);
        assert_eq!(world.net_count(), 0);
    }

    #[test]
    fn test_set_board() {
        let mut world = BoardWorld::new();
        let board = world.set_board(
            "TestBoard".to_string(),
            (Nm::from_mm(100.0), Nm::from_mm(80.0)),
            4,
        );

        assert_eq!(world.board_entity(), Some(board));
        assert_eq!(world.board_name(), Some("TestBoard"));

        let (size, layers) = world.board_info().unwrap();
        assert_eq!(size.width, Nm::from_mm(100.0));
        assert_eq!(size.height, Nm::from_mm(80.0));
        assert_eq!(layers.count, 4);
    }

    #[test]
    fn test_spawn_component() {
        let mut world = BoardWorld::new();
        let r1 = world.spawn_component(
            RefDes::new("R1"),
            Value::new("10k"),
            Position::from_mm(10.0, 20.0),
            Rotation::ZERO,
            FootprintRef::new("0402"),
            NetConnections::new(),
        );

        assert_eq!(world.component_count(), 1);
        assert_eq!(world.get::<RefDes>(r1).map(|r| r.as_str()), Some("R1"));
        assert_eq!(world.get::<Value>(r1).map(|v| v.as_str()), Some("10k"));
    }

    #[test]
    fn test_intern_net() {
        let mut world = BoardWorld::new();

        let vcc = world.intern_net("VCC");
        let gnd = world.intern_net("GND");
        let vcc2 = world.intern_net("VCC");

        assert_eq!(vcc, vcc2);
        assert_ne!(vcc, gnd);
        assert_eq!(world.net_name(vcc), Some("VCC"));
        assert_eq!(world.net_count(), 2);
    }

    #[test]
    fn test_find_by_refdes() {
        let mut world = BoardWorld::new();

        let r1 = world.spawn_component(
            RefDes::new("R1"),
            Value::new("10k"),
            Position::from_mm(10.0, 20.0),
            Rotation::ZERO,
            FootprintRef::new("0402"),
            NetConnections::new(),
        );

        let r2 = world.spawn_component(
            RefDes::new("R2"),
            Value::new("4.7k"),
            Position::from_mm(20.0, 20.0),
            Rotation::ZERO,
            FootprintRef::new("0402"),
            NetConnections::new(),
        );

        assert_eq!(world.find_by_refdes("R1"), Some(r1));
        assert_eq!(world.find_by_refdes("R2"), Some(r2));
        assert_eq!(world.find_by_refdes("R3"), None);
    }

    #[test]
    fn test_spatial_index() {
        let mut world = BoardWorld::new();

        // Spawn some components at known positions
        world.spawn_component(
            RefDes::new("R1"),
            Value::new("10k"),
            Position::from_mm(10.0, 10.0),
            Rotation::ZERO,
            FootprintRef::new("0402"),
            NetConnections::new(),
        );

        world.spawn_component(
            RefDes::new("R2"),
            Value::new("10k"),
            Position::from_mm(50.0, 50.0),
            Rotation::ZERO,
            FootprintRef::new("0402"),
            NetConnections::new(),
        );

        // Rebuild with simple 1mm x 1mm bounds
        world.rebuild_spatial_index(|_| {
            Rect::from_center_size(Point::ORIGIN, (Nm::from_mm(1.0), Nm::from_mm(1.0)))
        });

        // Query near R1
        let found = world.query_region(Rect::from_points(
            Point::from_mm(9.0, 9.0),
            Point::from_mm(11.0, 11.0),
        ));
        assert_eq!(found.len(), 1);

        // Query near R2
        let found = world.query_region(Rect::from_points(
            Point::from_mm(49.0, 49.0),
            Point::from_mm(51.0, 51.0),
        ));
        assert_eq!(found.len(), 1);

        // Query covering both
        let found = world.query_region(Rect::from_points(
            Point::from_mm(0.0, 0.0),
            Point::from_mm(100.0, 100.0),
        ));
        assert_eq!(found.len(), 2);

        // Query empty area
        let found = world.query_region(Rect::from_points(
            Point::from_mm(200.0, 200.0),
            Point::from_mm(300.0, 300.0),
        ));
        assert!(found.is_empty());
    }

    #[test]
    fn test_clear() {
        let mut world = BoardWorld::new();

        world.set_board("Test".to_string(), (Nm::from_mm(100.0), Nm::from_mm(100.0)), 2);
        world.intern_net("VCC");
        world.spawn_component(
            RefDes::new("R1"),
            Value::new("10k"),
            Position::from_mm(10.0, 10.0),
            Rotation::ZERO,
            FootprintRef::new("0402"),
            NetConnections::new(),
        );

        world.clear();

        assert!(world.is_empty());
        assert_eq!(world.board_entity(), None);
        assert_eq!(world.net_count(), 0);
    }

    #[test]
    fn test_components_iterator() {
        let mut world = BoardWorld::new();

        world.spawn_component(
            RefDes::new("R1"),
            Value::new("10k"),
            Position::from_mm(10.0, 10.0),
            Rotation::ZERO,
            FootprintRef::new("0402"),
            NetConnections::new(),
        );

        world.spawn_component(
            RefDes::new("C1"),
            Value::new("100nF"),
            Position::from_mm(20.0, 20.0),
            Rotation::ZERO,
            FootprintRef::new("0402"),
            NetConnections::new(),
        );

        let components = world.components();
        assert_eq!(components.len(), 2);
    }
}
