//! CodeYourPCB World
//!
//! ECS-based board model using bevy_ecs.
//! Provides the runtime representation of PCB designs.
//!
//! # Architecture
//!
//! This crate uses an Entity Component System (ECS) architecture from bevy_ecs.
//! PCB elements are represented as entities composed of small, focused components.
//!
//! ## Why ECS?
//!
//! - **Composition over inheritance**: Combine components flexibly
//! - **Cache-friendly**: Contiguous memory layout for fast iteration
//! - **Parallel queries**: bevy_ecs supports parallel iteration
//! - **Decoupled systems**: Components and systems are independent
//!
//! # Components
//!
//! Components are organized into categories:
//!
//! - **Position**: [`Position`], [`Rotation`] - spatial placement
//! - **Electrical**: [`NetId`], [`RefDes`], [`Value`], [`NetConnections`] - electrical properties
//! - **Physical**: [`Layer`], [`FootprintRef`], [`Pad`], [`PadShape`] - physical properties
//! - **Board**: [`Board`], [`BoardSize`], [`LayerStack`] - board-level properties
//! - **Metadata**: [`SourceSpan`], [`ComponentKind`] - source tracking
//!
//! # Entity Examples
//!
//! A resistor entity might have:
//! ```text
//! Entity: R1
//!   - RefDes("R1")
//!   - Value("10k")
//!   - Position at (10mm, 20mm)
//!   - Rotation at 90 degrees
//!   - FootprintRef("0402")
//!   - NetConnections { "1" -> VCC, "2" -> GND }
//!   - SourceSpan pointing to source file
//! ```
//!
//! A board entity might have:
//! ```text
//! Entity: Board
//!   - Board (marker)
//!   - BoardSize(100mm x 80mm)
//!   - LayerStack(4 layers)
//! ```
//!
//! # Example Usage
//!
//! ```
//! use bevy_ecs::prelude::*;
//! use cypcb_world::*;
//! use cypcb_core::Point;
//!
//! // Create a world
//! let mut world = World::new();
//!
//! // Spawn a board entity
//! world.spawn((
//!     Board,
//!     BoardSize::from_mm(100.0, 80.0),
//!     LayerStack::new(2),
//! ));
//!
//! // Spawn a resistor entity
//! let vcc = NetId::new(0);
//! let gnd = NetId::new(1);
//!
//! let mut nets = NetConnections::new();
//! nets.add(PinConnection::new("1", vcc));
//! nets.add(PinConnection::new("2", gnd));
//!
//! world.spawn((
//!     RefDes::new("R1"),
//!     Value::new("10k"),
//!     Position::from_mm(10.0, 20.0),
//!     Rotation::from_degrees(90.0),
//!     FootprintRef::new("0402"),
//!     nets,
//! ));
//!
//! // Query all components with position
//! let mut query = world.query::<(&RefDes, &Position)>();
//! for (refdes, pos) in query.iter(&world) {
//!     println!("{} at {}", refdes, pos);
//! }
//! ```

pub mod components;
pub mod footprint;
pub mod registry;
pub mod spatial;
pub mod sync;
pub mod world;

// Re-export all component types at crate root for convenience
pub use components::{
    // Board components
    Board,
    BoardSize,
    LayerStack,
    // Electrical components
    NetConnections,
    NetId,
    PinConnection,
    RefDes,
    Value,
    // Metadata components
    ComponentKind,
    SourceSpan,
    // Physical components
    FootprintRef,
    Layer,
    Pad,
    PadShape,
    // Position components
    Position,
    Rotation,
};

// Re-export registry and spatial types
pub use registry::NetRegistry;
pub use spatial::{SpatialEntry, SpatialIndex};

// Re-export BoardWorld
pub use world::BoardWorld;

// Re-export Entity for convenience
pub use bevy_ecs::entity::Entity;

// Re-export sync functionality
pub use sync::{sync_ast_to_world, SyncError, SyncResult};
