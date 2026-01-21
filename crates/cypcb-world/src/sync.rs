//! AST to ECS synchronization.
//!
//! This module bridges the parser and the board model, converting AST nodes
//! into ECS entities with appropriate components. Semantic errors are collected
//! (e.g., unknown footprint, duplicate refdes) with source spans for reporting.
//!
//! # Example
//!
//! ```
//! use cypcb_parser::parse;
//! use cypcb_world::{BoardWorld, sync_ast_to_world};
//! use cypcb_world::footprint::FootprintLibrary;
//!
//! let source = r#"
//! version 1
//! board test {
//!     size 50mm x 30mm
//!     layers 2
//! }
//! component R1 resistor "0402" {
//!     value "10k"
//!     at 10mm, 15mm
//! }
//! "#;
//!
//! let parse_result = parse(source);
//! if parse_result.is_ok() {
//!     let mut world = BoardWorld::new();
//!     let lib = FootprintLibrary::new();
//!     let sync_result = sync_ast_to_world(&parse_result.value, source, &mut world, &lib);
//!
//!     if sync_result.errors.is_empty() {
//!         println!("Synchronized successfully!");
//!     } else {
//!         for error in &sync_result.errors {
//!             eprintln!("{:?}", error);
//!         }
//!     }
//! }
//! ```

use std::collections::HashMap;

use bevy_ecs::prelude::Entity;
use miette::{Diagnostic, SourceSpan};
use thiserror::Error;

use cypcb_core::{Nm, Point, Rect};
use cypcb_parser::ast::{
    BoardDef, ComponentDef, Definition, NetDef, PinId as AstPinId, SourceFile, Span,
};

use crate::components::{
    ComponentKind, FootprintRef, NetConnections, PinConnection, Position, RefDes, Rotation,
    SourceSpan as EcsSourceSpan, Value,
};
use crate::footprint::FootprintLibrary;
use crate::world::BoardWorld;

/// Semantic errors that can occur during AST to ECS synchronization.
///
/// These errors are distinct from parse errors - they occur when the AST
/// is syntactically valid but has semantic issues like unknown footprints
/// or duplicate reference designators.
#[derive(Error, Debug, Diagnostic)]
pub enum SyncError {
    /// A component references a footprint that doesn't exist in the library.
    #[error("unknown footprint: '{name}'")]
    #[diagnostic(
        code(cypcb::sync::unknown_footprint),
        help("add this footprint to the library or use a built-in footprint like '0402', '0603', 'DIP-8'")
    )]
    UnknownFootprint {
        /// The unknown footprint name.
        name: String,
        /// Source code for miette display.
        #[source_code]
        src: String,
        /// Source span of the footprint reference.
        #[label("footprint not found in library")]
        span: SourceSpan,
    },

    /// A reference designator is used more than once.
    #[error("duplicate reference designator: '{refdes}'")]
    #[diagnostic(
        code(cypcb::sync::duplicate_refdes),
        help("each component must have a unique reference designator")
    )]
    DuplicateRefDes {
        /// The duplicated refdes.
        refdes: String,
        /// Source code for miette display.
        #[source_code]
        src: String,
        /// Span of the first definition.
        #[label("first defined here")]
        first: SourceSpan,
        /// Span of the duplicate.
        #[label("duplicate definition")]
        duplicate: SourceSpan,
    },

    /// A net references a component that doesn't exist.
    #[error("unknown component: '{component}'")]
    #[diagnostic(
        code(cypcb::sync::unknown_component),
        help("define the component before referencing it in a net")
    )]
    UnknownComponent {
        /// The unknown component refdes.
        component: String,
        /// Source code for miette display.
        #[source_code]
        src: String,
        /// Source span of the component reference.
        #[label("component not defined")]
        span: SourceSpan,
    },
}

/// Result of AST to ECS synchronization.
///
/// Contains any errors and warnings that occurred during the process.
/// The synchronization continues even when errors occur, producing
/// a partial world that can still be useful for error reporting.
#[derive(Debug, Default)]
pub struct SyncResult {
    /// Semantic errors encountered during sync.
    pub errors: Vec<SyncError>,
    /// Non-fatal warnings.
    pub warnings: Vec<String>,
}

impl SyncResult {
    /// Create a new empty result.
    pub fn new() -> Self {
        SyncResult {
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// Check if sync completed without errors.
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }

    /// Check if there were any errors.
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

/// Synchronize an AST to a BoardWorld.
///
/// Processes the AST definitions in order:
/// 1. Board definitions set up the board entity
/// 2. Component definitions spawn component entities
/// 3. Net definitions connect components via interned net IDs
///
/// # Arguments
///
/// * `ast` - The parsed AST source file
/// * `source` - The original source code (for error spans)
/// * `world` - The BoardWorld to populate
/// * `footprint_lib` - Library of available footprints
///
/// # Returns
///
/// A `SyncResult` containing any errors or warnings.
///
/// # Example
///
/// ```
/// use cypcb_parser::parse;
/// use cypcb_world::{BoardWorld, sync_ast_to_world};
/// use cypcb_world::footprint::FootprintLibrary;
///
/// let source = "version 1\nboard test { size 10mm x 10mm }";
/// let parse_result = parse(source);
/// let mut world = BoardWorld::new();
/// let lib = FootprintLibrary::new();
///
/// let result = sync_ast_to_world(&parse_result.value, source, &mut world, &lib);
/// assert!(result.is_ok());
/// ```
pub fn sync_ast_to_world(
    ast: &SourceFile,
    source: &str,
    world: &mut BoardWorld,
    footprint_lib: &FootprintLibrary,
) -> SyncResult {
    let mut result = SyncResult::new();

    // Track reference designators for duplicate detection
    // Maps refdes string to (span, entity)
    let mut refdes_spans: HashMap<String, Span> = HashMap::new();

    // Track component entities for net resolution
    let mut component_entities: HashMap<String, Entity> = HashMap::new();

    // Process definitions in order
    for def in &ast.definitions {
        match def {
            Definition::Board(board) => {
                sync_board(board, source, world, &mut result);
            }
            Definition::Component(comp) => {
                sync_component(
                    comp,
                    source,
                    world,
                    footprint_lib,
                    &mut refdes_spans,
                    &mut component_entities,
                    &mut result,
                );
            }
            Definition::Net(net) => {
                sync_net(net, source, world, &component_entities, &mut result);
            }
        }
    }

    // Rebuild spatial index after all entities are added
    world.rebuild_spatial_index(|name| {
        footprint_lib
            .get(name)
            .map(|fp| fp.courtyard)
            .unwrap_or_else(|| {
                // Default 1mm x 1mm bounds for unknown footprints
                Rect::from_center_size(Point::ORIGIN, (Nm::from_mm(1.0), Nm::from_mm(1.0)))
            })
    });

    result
}

/// Synchronize a board definition to the world.
fn sync_board(board: &BoardDef, _source: &str, world: &mut BoardWorld, result: &mut SyncResult) {
    // Extract size, defaulting if not specified
    let (width, height) = if let Some(size) = &board.size {
        (size.width.to_nm(), size.height.to_nm())
    } else {
        result
            .warnings
            .push("Board has no size, defaulting to 100mm x 100mm".into());
        (Nm::from_mm(100.0), Nm::from_mm(100.0))
    };

    // Extract layer count, defaulting to 2
    let layers = board.layers.unwrap_or_else(|| {
        result
            .warnings
            .push("Board has no layer count, defaulting to 2 layers".into());
        2
    });

    world.set_board(board.name.value.clone(), (width, height), layers);
}

/// Synchronize a component definition to the world.
fn sync_component(
    comp: &ComponentDef,
    source: &str,
    world: &mut BoardWorld,
    footprint_lib: &FootprintLibrary,
    refdes_spans: &mut HashMap<String, Span>,
    component_entities: &mut HashMap<String, Entity>,
    result: &mut SyncResult,
) {
    let refdes_str = comp.refdes.value.clone();

    // Check for duplicate refdes
    if let Some(first_span) = refdes_spans.get(&refdes_str) {
        result.errors.push(SyncError::DuplicateRefDes {
            refdes: refdes_str.clone(),
            src: source.to_string(),
            first: span_to_source_span(first_span),
            duplicate: span_to_source_span(&comp.refdes.span),
        });
        // Continue anyway to collect more errors
    } else {
        refdes_spans.insert(refdes_str.clone(), comp.refdes.span);
    }

    // Check footprint exists
    let footprint_name = &comp.footprint.value;
    if !footprint_lib.contains(footprint_name) {
        result.errors.push(SyncError::UnknownFootprint {
            name: footprint_name.clone(),
            src: source.to_string(),
            span: span_to_source_span(&comp.footprint.span),
        });
        // Continue anyway - entity will be created but may not render correctly
    }

    // Convert position (default to origin if not specified)
    let position = if let Some(pos) = &comp.position {
        Position(Point::new(pos.x.to_nm(), pos.y.to_nm()))
    } else {
        Position::from_mm(0.0, 0.0)
    };

    // Convert rotation (default to 0 if not specified)
    let rotation = if let Some(rot) = &comp.rotation {
        Rotation::from_degrees(rot.angle)
    } else {
        Rotation::ZERO
    };

    // Convert value (default to empty string if not specified)
    let value = Value::new(comp.value.as_ref().map(|v| v.value.as_str()).unwrap_or(""));

    // Convert component kind from AST to ECS
    let kind = ast_kind_to_ecs_kind(comp.kind);

    // Create source span component
    let ecs_span = EcsSourceSpan::new(comp.span.start, comp.span.end, 1, 1);

    // Spawn the component entity
    let entity = world.spawn_component_with_span(
        RefDes::new(&refdes_str),
        value,
        position,
        rotation,
        FootprintRef::new(footprint_name),
        NetConnections::new(),
        ecs_span,
    );

    // Add component kind
    world.ecs_mut().entity_mut(entity).insert(kind);

    // Track for net resolution
    component_entities.insert(refdes_str, entity);
}

/// Synchronize a net definition to the world.
fn sync_net(
    net: &NetDef,
    source: &str,
    world: &mut BoardWorld,
    component_entities: &HashMap<String, Entity>,
    result: &mut SyncResult,
) {
    // Intern the net name
    let net_id = world.intern_net(&net.name.value);

    // Process each pin reference in the net
    for pin_ref in &net.connections {
        let comp_name = &pin_ref.component.value;

        // Look up component entity
        if let Some(&entity) = component_entities.get(comp_name) {
            // Convert pin ID to string
            let pin_str = match &pin_ref.pin {
                AstPinId::Number(n) => n.to_string(),
                AstPinId::Name(s) => s.clone(),
            };

            // Get or create NetConnections component
            let ecs = world.ecs_mut();
            if let Some(mut connections) = ecs.get_mut::<NetConnections>(entity) {
                connections.add(PinConnection::new(pin_str, net_id));
            } else {
                // Component exists but no NetConnections - this shouldn't happen
                // since we add empty NetConnections when spawning
                let mut new_connections = NetConnections::new();
                new_connections.add(PinConnection::new(pin_str, net_id));
                ecs.entity_mut(entity).insert(new_connections);
            }
        } else {
            result.errors.push(SyncError::UnknownComponent {
                component: comp_name.clone(),
                src: source.to_string(),
                span: span_to_source_span(&pin_ref.component.span),
            });
        }
    }
}

/// Convert AST Span to miette SourceSpan.
fn span_to_source_span(span: &Span) -> SourceSpan {
    span.to_miette()
}

/// Convert AST ComponentKind to ECS ComponentKind.
fn ast_kind_to_ecs_kind(kind: cypcb_parser::ast::ComponentKind) -> ComponentKind {
    match kind {
        cypcb_parser::ast::ComponentKind::Resistor => ComponentKind::Resistor,
        cypcb_parser::ast::ComponentKind::Capacitor => ComponentKind::Capacitor,
        cypcb_parser::ast::ComponentKind::Inductor => ComponentKind::Inductor,
        cypcb_parser::ast::ComponentKind::Ic => ComponentKind::IC,
        cypcb_parser::ast::ComponentKind::Led => ComponentKind::LED,
        cypcb_parser::ast::ComponentKind::Connector => ComponentKind::Connector,
        cypcb_parser::ast::ComponentKind::Diode => ComponentKind::Diode,
        cypcb_parser::ast::ComponentKind::Transistor => ComponentKind::Transistor,
        cypcb_parser::ast::ComponentKind::Crystal => ComponentKind::Crystal,
        cypcb_parser::ast::ComponentKind::Generic => ComponentKind::Generic,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cypcb_parser::parse;

    #[test]
    fn test_sync_simple_board() {
        let source = r#"
version 1
board test {
    size 50mm x 30mm
    layers 2
}
"#;
        let parse_result = parse(source);
        assert!(parse_result.is_ok(), "parse errors: {:?}", parse_result.errors);

        let mut world = BoardWorld::new();
        let lib = FootprintLibrary::new();

        let result = sync_ast_to_world(&parse_result.value, source, &mut world, &lib);

        assert!(result.is_ok(), "sync errors: {:?}", result.errors);
        assert_eq!(world.board_name(), Some("test"));

        let (size, layers) = world.board_info().unwrap();
        assert_eq!(size.width, Nm::from_mm(50.0));
        assert_eq!(size.height, Nm::from_mm(30.0));
        assert_eq!(layers.count, 2);
    }

    #[test]
    fn test_sync_component() {
        let source = r#"
version 1
board test {
    size 50mm x 30mm
    layers 2
}
component R1 resistor "0402" {
    value "10k"
    at 10mm, 15mm
    rotate 90
}
"#;
        let parse_result = parse(source);
        assert!(parse_result.is_ok(), "parse errors: {:?}", parse_result.errors);

        let mut world = BoardWorld::new();
        let lib = FootprintLibrary::new();

        let result = sync_ast_to_world(&parse_result.value, source, &mut world, &lib);

        assert!(result.is_ok(), "sync errors: {:?}", result.errors);
        assert_eq!(world.component_count(), 1);

        let entity = world.find_by_refdes("R1").expect("R1 should exist");
        let refdes = world.get::<RefDes>(entity).unwrap();
        assert_eq!(refdes.as_str(), "R1");

        let value = world.get::<Value>(entity).unwrap();
        assert_eq!(value.as_str(), "10k");

        let pos = world.get::<Position>(entity).unwrap();
        assert_eq!(pos.0.x, Nm::from_mm(10.0));
        assert_eq!(pos.0.y, Nm::from_mm(15.0));

        let rot = world.get::<Rotation>(entity).unwrap();
        assert_eq!(rot.to_degrees(), 90.0);

        let kind = world.get::<ComponentKind>(entity).unwrap();
        assert_eq!(*kind, ComponentKind::Resistor);
    }

    #[test]
    fn test_sync_net() {
        let source = r#"
version 1
board test { size 50mm x 30mm layers 2 }
component R1 resistor "0402" { at 10mm, 10mm }
component R2 resistor "0402" { at 20mm, 10mm }
net VCC {
    R1.1
    R2.1
}
"#;
        let parse_result = parse(source);
        assert!(parse_result.is_ok(), "parse errors: {:?}", parse_result.errors);

        let mut world = BoardWorld::new();
        let lib = FootprintLibrary::new();

        let result = sync_ast_to_world(&parse_result.value, source, &mut world, &lib);

        assert!(result.is_ok(), "sync errors: {:?}", result.errors);

        // Check net was interned
        let vcc = world.get_net("VCC").expect("VCC should be interned");

        // Check R1 has connection
        let r1 = world.find_by_refdes("R1").unwrap();
        let r1_conns = world.get::<NetConnections>(r1).unwrap();
        assert!(r1_conns.contains_net(vcc));
        assert_eq!(r1_conns.pin_net("1"), Some(vcc));

        // Check R2 has connection
        let r2 = world.find_by_refdes("R2").unwrap();
        let r2_conns = world.get::<NetConnections>(r2).unwrap();
        assert!(r2_conns.contains_net(vcc));
        assert_eq!(r2_conns.pin_net("1"), Some(vcc));
    }

    #[test]
    fn test_sync_unknown_footprint() {
        let source = r#"
version 1
board test { size 50mm x 30mm }
component R1 resistor "UNKNOWN_FOOTPRINT" {
    at 10mm, 10mm
}
"#;
        let parse_result = parse(source);
        assert!(parse_result.is_ok());

        let mut world = BoardWorld::new();
        let lib = FootprintLibrary::new();

        let result = sync_ast_to_world(&parse_result.value, source, &mut world, &lib);

        assert!(result.has_errors());
        assert_eq!(result.errors.len(), 1);
        assert!(matches!(result.errors[0], SyncError::UnknownFootprint { .. }));

        // Component should still be created
        assert_eq!(world.component_count(), 1);
    }

    #[test]
    fn test_sync_duplicate_refdes() {
        let source = r#"
version 1
board test { size 50mm x 30mm }
component R1 resistor "0402" { at 10mm, 10mm }
component R1 resistor "0402" { at 20mm, 20mm }
"#;
        let parse_result = parse(source);
        assert!(parse_result.is_ok());

        let mut world = BoardWorld::new();
        let lib = FootprintLibrary::new();

        let result = sync_ast_to_world(&parse_result.value, source, &mut world, &lib);

        assert!(result.has_errors());
        assert_eq!(result.errors.len(), 1);
        assert!(matches!(result.errors[0], SyncError::DuplicateRefDes { .. }));

        // Both components are created (error doesn't stop sync)
        assert_eq!(world.component_count(), 2);
    }

    #[test]
    fn test_sync_unknown_component_in_net() {
        let source = r#"
version 1
board test { size 50mm x 30mm }
component R1 resistor "0402" { at 10mm, 10mm }
net VCC {
    R1.1
    R999.1
}
"#;
        let parse_result = parse(source);
        assert!(parse_result.is_ok());

        let mut world = BoardWorld::new();
        let lib = FootprintLibrary::new();

        let result = sync_ast_to_world(&parse_result.value, source, &mut world, &lib);

        assert!(result.has_errors());
        assert_eq!(result.errors.len(), 1);
        assert!(matches!(result.errors[0], SyncError::UnknownComponent { .. }));
    }

    #[test]
    fn test_sync_named_pin() {
        let source = r#"
version 1
board test { size 50mm x 30mm }
component LED1 led "0603" { at 10mm, 10mm }
net ANODE {
    LED1.anode
}
"#;
        let parse_result = parse(source);
        assert!(parse_result.is_ok(), "parse errors: {:?}", parse_result.errors);

        let mut world = BoardWorld::new();
        let lib = FootprintLibrary::new();

        let result = sync_ast_to_world(&parse_result.value, source, &mut world, &lib);

        assert!(result.is_ok(), "sync errors: {:?}", result.errors);

        let led = world.find_by_refdes("LED1").unwrap();
        let conns = world.get::<NetConnections>(led).unwrap();
        let anode_net = world.get_net("ANODE").unwrap();
        assert_eq!(conns.pin_net("anode"), Some(anode_net));
    }

    #[test]
    fn test_sync_complete_example() {
        let source = r#"
// LED blink circuit
version 1

board blink {
    size 30mm x 20mm
    layers 2
}

component R1 resistor "0402" {
    value "330"
    at 10mm, 8mm
}

component LED1 led "0603" {
    at 15mm, 8mm
}

component J1 connector "PIN-HDR-1x2" {
    at 5mm, 8mm
}

net VCC {
    J1.1
    R1.1
}

net GND {
    J1.2
    LED1.cathode
}

net LED_SIGNAL {
    R1.2
    LED1.anode
}
"#;
        let parse_result = parse(source);
        assert!(parse_result.is_ok(), "parse errors: {:?}", parse_result.errors);

        let mut world = BoardWorld::new();
        let lib = FootprintLibrary::new();

        let result = sync_ast_to_world(&parse_result.value, source, &mut world, &lib);

        assert!(result.is_ok(), "sync errors: {:?}", result.errors);
        assert_eq!(world.board_name(), Some("blink"));
        assert_eq!(world.component_count(), 3);
        assert_eq!(world.net_count(), 3);

        // Verify net connections
        let r1 = world.find_by_refdes("R1").unwrap();
        let r1_conns = world.get::<NetConnections>(r1).unwrap();
        assert_eq!(r1_conns.len(), 2); // Pin 1 -> VCC, Pin 2 -> LED_SIGNAL
    }

    #[test]
    fn test_sync_board_defaults() {
        let source = r#"
board minimal {
}
"#;
        let parse_result = parse(source);
        assert!(parse_result.is_ok());

        let mut world = BoardWorld::new();
        let lib = FootprintLibrary::new();

        let result = sync_ast_to_world(&parse_result.value, source, &mut world, &lib);

        // Should have warnings about defaults
        assert!(!result.warnings.is_empty());
        assert!(result.is_ok()); // No errors though

        // Check defaults were applied
        let (size, layers) = world.board_info().unwrap();
        assert_eq!(size.width, Nm::from_mm(100.0));
        assert_eq!(size.height, Nm::from_mm(100.0));
        assert_eq!(layers.count, 2);
    }

    #[test]
    fn test_sync_multiple_nets_same_component() {
        let source = r#"
version 1
board test { size 50mm x 30mm }
component R1 resistor "0402" { at 10mm, 10mm }
net NET_A { R1.1 }
net NET_B { R1.2 }
"#;
        let parse_result = parse(source);
        assert!(parse_result.is_ok());

        let mut world = BoardWorld::new();
        let lib = FootprintLibrary::new();

        let result = sync_ast_to_world(&parse_result.value, source, &mut world, &lib);
        assert!(result.is_ok());

        let r1 = world.find_by_refdes("R1").unwrap();
        let conns = world.get::<NetConnections>(r1).unwrap();
        assert_eq!(conns.len(), 2);

        let net_a = world.get_net("NET_A").unwrap();
        let net_b = world.get_net("NET_B").unwrap();
        assert!(conns.contains_net(net_a));
        assert!(conns.contains_net(net_b));
    }

    #[test]
    fn test_source_span_preserved() {
        let source = r#"
board test { size 50mm x 30mm }
component R1 resistor "0402" { at 10mm, 10mm }
"#;
        let parse_result = parse(source);
        assert!(parse_result.is_ok());

        let mut world = BoardWorld::new();
        let lib = FootprintLibrary::new();

        let result = sync_ast_to_world(&parse_result.value, source, &mut world, &lib);
        assert!(result.is_ok());

        let r1 = world.find_by_refdes("R1").unwrap();
        let span = world.get::<EcsSourceSpan>(r1).expect("should have source span");
        assert!(span.start_byte > 0); // Not at start of file
        assert!(span.end_byte > span.start_byte);
    }
}
