//! ECS components for board entities.
//!
//! This module defines all the components used to represent PCB elements
//! in the Entity Component System. Components are designed to be:
//!
//! - **Composable**: Small, focused components that combine flexibly
//! - **Cache-friendly**: Simple data structures for efficient queries
//! - **Serializable**: All components can be serialized to JSON
//!
//! # Component Categories
//!
//! - **Position**: Spatial placement ([`Position`], [`Rotation`])
//! - **Electrical**: Net and pin connections ([`NetId`], [`RefDes`], [`Value`], [`NetConnections`])
//! - **Physical**: Layers, footprints, pads ([`Layer`], [`FootprintRef`], [`Pad`])
//! - **Board**: Board-level properties ([`Board`], [`BoardSize`], [`LayerStack`])
//! - **Metadata**: Source tracking ([`SourceSpan`], [`ComponentKind`])
//!
//! # Entity Composition Examples
//!
//! A resistor might have:
//! - `RefDes("R1")`
//! - `Value("10k")`
//! - `Position` at (10mm, 20mm)
//! - `Rotation` at 90 degrees
//! - `FootprintRef("0402")`
//! - `NetConnections` mapping pins to nets
//! - `SourceSpan` for error reporting
//!
//! A board entity might have:
//! - `Board` marker
//! - `BoardSize` (100mm x 80mm)
//! - `LayerStack` (2 layers)

mod board;
mod electrical;
pub mod metadata;
mod physical;
mod position;
pub mod trace;
pub mod zone;

// Re-export all components
pub use board::{Board, BoardSize, LayerStack};
pub use electrical::{NetConnections, NetId, PinConnection, RefDes, Value};
pub use metadata::{ComponentKind, Name, SourceSpan};
pub use physical::{FootprintRef, Layer, Pad, PadShape};
pub use position::{Position, Rotation};
pub use zone::{Zone, ZoneKind};
