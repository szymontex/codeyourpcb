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
use std::fmt;

use bevy_ecs::prelude::Entity;
use miette::{Diagnostic, LabeledSpan, SourceCode, SourceSpan};

use cypcb_core::{Nm, Point, Rect};
use cypcb_parser::ast::{
    BoardDef, ComponentDef, Definition, FootprintDef, NetDef, PadShape as AstPadShape,
    PinId as AstPinId, SourceFile, Span, ZoneDef, ZoneKind as AstZoneKind,
};

use crate::components::{
    ComponentKind, FootprintRef, Layer, NetConnections, PadShape as EcsPadShape, PinConnection,
    Position, RefDes, Rotation, SourceSpan as EcsSourceSpan, Value, Zone,
    ZoneKind as EcsZoneKind,
};
use crate::footprint::{Footprint, FootprintLibrary, PadDef as FootprintPadDef};
use crate::world::BoardWorld;

/// Semantic errors that can occur during AST to ECS synchronization.
///
/// These errors are distinct from parse errors - they occur when the AST
/// is syntactically valid but has semantic issues like unknown footprints
/// or duplicate reference designators.
#[derive(Debug, Clone)]
pub enum SyncError {
    /// A component references a footprint that doesn't exist in the library.
    UnknownFootprint {
        /// The unknown footprint name.
        name: String,
        /// Source code for miette display.
        src: String,
        /// Source span of the footprint reference.
        span: miette::SourceSpan,
    },

    /// A reference designator is used more than once.
    DuplicateRefDes {
        /// The duplicated refdes.
        refdes: String,
        /// Source code for miette display.
        src: String,
        /// Span of the first definition.
        first: miette::SourceSpan,
        /// Span of the duplicate.
        duplicate: miette::SourceSpan,
    },

    /// A net references a component that doesn't exist.
    UnknownComponent {
        /// The unknown component refdes.
        component: String,
        /// Source code for miette display.
        src: String,
        /// Source span of the component reference.
        span: miette::SourceSpan,
    },
}

impl fmt::Display for SyncError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SyncError::UnknownFootprint { name, .. } => {
                write!(f, "unknown footprint: '{}'", name)
            }
            SyncError::DuplicateRefDes { refdes, .. } => {
                write!(f, "duplicate reference designator: '{}'", refdes)
            }
            SyncError::UnknownComponent { component, .. } => {
                write!(f, "unknown component: '{}'", component)
            }
        }
    }
}

impl std::error::Error for SyncError {}

impl Diagnostic for SyncError {
    fn code<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        match self {
            SyncError::UnknownFootprint { .. } => {
                Some(Box::new("cypcb::sync::unknown_footprint"))
            }
            SyncError::DuplicateRefDes { .. } => {
                Some(Box::new("cypcb::sync::duplicate_refdes"))
            }
            SyncError::UnknownComponent { .. } => {
                Some(Box::new("cypcb::sync::unknown_component"))
            }
        }
    }

    fn help<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        match self {
            SyncError::UnknownFootprint { .. } => {
                Some(Box::new("add this footprint to the library or use a built-in footprint like '0402', '0603', 'DIP-8'"))
            }
            SyncError::DuplicateRefDes { .. } => {
                Some(Box::new("each component must have a unique reference designator"))
            }
            SyncError::UnknownComponent { .. } => {
                Some(Box::new("define the component before referencing it in a net"))
            }
        }
    }

    fn source_code(&self) -> Option<&dyn SourceCode> {
        match self {
            SyncError::UnknownFootprint { src, .. } => Some(src),
            SyncError::DuplicateRefDes { src, .. } => Some(src),
            SyncError::UnknownComponent { src, .. } => Some(src),
        }
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        match self {
            SyncError::UnknownFootprint { span, .. } => {
                Some(Box::new(std::iter::once(
                    LabeledSpan::new_with_span(Some("footprint not found in library".to_string()), *span)
                )))
            }
            SyncError::DuplicateRefDes { first, duplicate, .. } => {
                Some(Box::new(vec![
                    LabeledSpan::new_with_span(Some("first defined here".to_string()), *first),
                    LabeledSpan::new_with_span(Some("duplicate definition".to_string()), *duplicate),
                ].into_iter()))
            }
            SyncError::UnknownComponent { span, .. } => {
                Some(Box::new(std::iter::once(
                    LabeledSpan::new_with_span(Some("component not defined".to_string()), *span)
                )))
            }
        }
    }
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

    // Clone the library so we can add custom footprints from the AST
    let mut lib = footprint_lib.clone();

    // Phase 0: Register custom footprints BEFORE component sync
    // This ensures custom footprints are available when components reference them
    for def in &ast.definitions {
        if let Definition::Footprint(fp_def) = def {
            let footprint = convert_footprint_def(fp_def);
            lib.register(footprint);
        }
    }

    // Track reference designators for duplicate detection
    // Maps refdes string to (span, entity)
    let mut refdes_spans: HashMap<String, Span> = HashMap::new();

    // Track component entities for net resolution
    let mut component_entities: HashMap<String, Entity> = HashMap::new();

    // Process definitions in order (footprints already handled above)
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
                    &lib, // Use our modified library with custom footprints
                    &mut refdes_spans,
                    &mut component_entities,
                    &mut result,
                );
            }
            Definition::Net(net) => {
                sync_net(net, source, world, &component_entities, &mut result);
            }
            Definition::Zone(zone) => {
                sync_zone(zone, world, &mut result);
            }
            Definition::Footprint(_) => {
                // Already handled in Phase 0 above
            }
        }
    }

    // Rebuild spatial index after all entities are added
    world.rebuild_spatial_index(|name| {
        lib.get(name)
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

/// Synchronize a zone definition to the world.
fn sync_zone(zone_def: &ZoneDef, world: &mut BoardWorld, _result: &mut SyncResult) {
    // Convert bounds to Rect
    let min = Point::new(zone_def.bounds.0.to_nm(), zone_def.bounds.1.to_nm());
    let max = Point::new(zone_def.bounds.2.to_nm(), zone_def.bounds.3.to_nm());
    let bounds = Rect::new(min, max);

    // Convert zone kind
    let kind = match zone_def.kind {
        AstZoneKind::Keepout => EcsZoneKind::Keepout,
        AstZoneKind::CopperPour => EcsZoneKind::CopperPour,
    };

    // Parse layer to layer mask
    let layer_mask = match zone_def.layer.as_deref() {
        Some("top") => 0b01,      // Layer 0 (top copper)
        Some("bottom") => 0b10,   // Layer 1 (bottom copper)
        Some("all") | None => 0xFFFFFFFF, // All layers
        Some(_) => 0xFFFFFFFF,    // Unknown layer defaults to all
    };

    // Create zone component
    let zone = Zone {
        bounds,
        kind,
        layer_mask,
        name: zone_def.name.as_ref().map(|n| n.value.clone()),
    };

    // If copper pour, we would also store the net reference
    // For now, we store it in the name if present
    if zone_def.kind == AstZoneKind::CopperPour {
        if let Some(net) = &zone_def.net {
            // Could intern the net and store it, but for now just note it
            // In the future this would be used for DRC checks
            let _ = net;
        }
    }

    // Spawn zone entity
    world.ecs_mut().spawn(zone);
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

/// Convert an AST FootprintDef to a library Footprint.
fn convert_footprint_def(fp_def: &FootprintDef) -> Footprint {
    let pads: Vec<FootprintPadDef> = fp_def
        .pads
        .iter()
        .map(|p| {
            let is_tht = p.drill.is_some();
            FootprintPadDef {
                number: p.number.to_string(),
                shape: convert_pad_shape(p.shape),
                position: Point::new(p.x.to_nm(), p.y.to_nm()),
                size: (p.width.to_nm(), p.height.to_nm()),
                drill: p.drill.as_ref().map(|d| d.to_nm()),
                layers: if is_tht {
                    // Through-hole pads span both copper layers
                    vec![Layer::TopCopper, Layer::BottomCopper]
                } else {
                    // SMD pads on top copper with paste and mask
                    vec![Layer::TopCopper, Layer::TopPaste, Layer::TopMask]
                },
            }
        })
        .collect();

    // Calculate bounds from pad positions and sizes
    let bounds = calculate_footprint_bounds(&pads);

    // Courtyard: use explicit if provided, otherwise expand bounds by IPC-7351B margin
    let courtyard = fp_def
        .courtyard
        .as_ref()
        .map(|(w, h)| Rect::from_center_size(Point::ORIGIN, (w.to_nm(), h.to_nm())))
        .unwrap_or_else(|| bounds.expand(Nm::from_mm(0.5)));

    Footprint {
        name: fp_def.name.value.clone(),
        description: fp_def.description.clone().unwrap_or_default(),
        pads,
        bounds,
        courtyard,
    }
}

/// Convert AST PadShape to ECS PadShape.
fn convert_pad_shape(shape: AstPadShape) -> EcsPadShape {
    match shape {
        AstPadShape::Rect => EcsPadShape::Rect,
        AstPadShape::Circle => EcsPadShape::Circle,
        AstPadShape::RoundRect => EcsPadShape::RoundRect { corner_ratio: 25 }, // Default 25%
        AstPadShape::Oblong => EcsPadShape::Oblong,
    }
}

/// Calculate the bounding box for a set of pads.
fn calculate_footprint_bounds(pads: &[FootprintPadDef]) -> Rect {
    if pads.is_empty() {
        return Rect::from_center_size(Point::ORIGIN, (Nm::from_mm(1.0), Nm::from_mm(1.0)));
    }

    let mut min_x = Nm(i64::MAX);
    let mut min_y = Nm(i64::MAX);
    let mut max_x = Nm(i64::MIN);
    let mut max_y = Nm(i64::MIN);

    for pad in pads {
        let half_w = Nm(pad.size.0 .0 / 2);
        let half_h = Nm(pad.size.1 .0 / 2);

        let pad_min_x = Nm(pad.position.x.0 - half_w.0);
        let pad_min_y = Nm(pad.position.y.0 - half_h.0);
        let pad_max_x = Nm(pad.position.x.0 + half_w.0);
        let pad_max_y = Nm(pad.position.y.0 + half_h.0);

        min_x = Nm(min_x.0.min(pad_min_x.0));
        min_y = Nm(min_y.0.min(pad_min_y.0));
        max_x = Nm(max_x.0.max(pad_max_x.0));
        max_y = Nm(max_y.0.max(pad_max_y.0));
    }

    Rect::new(Point::new(min_x, min_y), Point::new(max_x, max_y))
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

    #[test]
    fn test_sync_keepout_zone() {
        let source = r#"
version 1
board test { size 50mm x 30mm }
keepout antenna_area {
    bounds 10mm, 10mm to 20mm, 20mm
    layer top
}
"#;
        let parse_result = parse(source);
        assert!(parse_result.is_ok(), "parse errors: {:?}", parse_result.errors);

        let mut world = BoardWorld::new();
        let lib = FootprintLibrary::new();

        let result = sync_ast_to_world(&parse_result.value, source, &mut world, &lib);
        assert!(result.is_ok(), "sync errors: {:?}", result.errors);

        // Query for zone entities
        let mut found_zone = false;
        let mut query = world.ecs_mut().query::<&Zone>();
        for zone in query.iter(world.ecs()) {
            found_zone = true;
            assert!(zone.is_keepout());
            assert_eq!(zone.name.as_deref(), Some("antenna_area"));
            assert_eq!(zone.layer_mask, 0b01); // Top layer only
            assert_eq!(zone.bounds.min.x, Nm::from_mm(10.0));
            assert_eq!(zone.bounds.min.y, Nm::from_mm(10.0));
            assert_eq!(zone.bounds.max.x, Nm::from_mm(20.0));
            assert_eq!(zone.bounds.max.y, Nm::from_mm(20.0));
        }
        assert!(found_zone, "zone entity should be created");
    }

    #[test]
    fn test_sync_copper_pour_zone() {
        let source = r#"
version 1
board test { size 50mm x 30mm }
zone gnd_pour {
    bounds 0mm, 0mm to 50mm, 30mm
    layer bottom
    net GND
}
"#;
        let parse_result = parse(source);
        assert!(parse_result.is_ok(), "parse errors: {:?}", parse_result.errors);

        let mut world = BoardWorld::new();
        let lib = FootprintLibrary::new();

        let result = sync_ast_to_world(&parse_result.value, source, &mut world, &lib);
        assert!(result.is_ok(), "sync errors: {:?}", result.errors);

        // Query for zone entities
        let mut found_zone = false;
        let mut query = world.ecs_mut().query::<&Zone>();
        for zone in query.iter(world.ecs()) {
            found_zone = true;
            assert!(zone.is_copper_pour());
            assert_eq!(zone.name.as_deref(), Some("gnd_pour"));
            assert_eq!(zone.layer_mask, 0b10); // Bottom layer only
        }
        assert!(found_zone, "zone entity should be created");
    }

    #[test]
    fn test_sync_zone_all_layers() {
        let source = r#"
version 1
board test { size 50mm x 30mm }
keepout restricted {
    bounds 5mm, 5mm to 15mm, 15mm
    layer all
}
"#;
        let parse_result = parse(source);
        assert!(parse_result.is_ok(), "parse errors: {:?}", parse_result.errors);

        let mut world = BoardWorld::new();
        let lib = FootprintLibrary::new();

        let result = sync_ast_to_world(&parse_result.value, source, &mut world, &lib);
        assert!(result.is_ok(), "sync errors: {:?}", result.errors);

        // Query for zone entities
        let mut query = world.ecs_mut().query::<&Zone>();
        for zone in query.iter(world.ecs()) {
            assert_eq!(zone.layer_mask, 0xFFFFFFFF); // All layers
        }
    }

    #[test]
    fn test_custom_footprint_registration() {
        let source = r#"
version 1

footprint CUSTOM_2PIN {
    description "Custom 2-pin"
    pad 1 rect at -1mm, 0mm size 0.5mm x 0.5mm
    pad 2 rect at 1mm, 0mm size 0.5mm x 0.5mm
}

board test { size 20mm x 20mm }

component R1 resistor "CUSTOM_2PIN" {
    at 10mm, 10mm
}
"#;
        let parse_result = parse(source);
        assert!(parse_result.is_ok(), "parse errors: {:?}", parse_result.errors);

        let mut world = BoardWorld::new();
        let lib = FootprintLibrary::new();

        // CUSTOM_2PIN is not in the built-in library
        assert!(lib.get("CUSTOM_2PIN").is_none());

        // But sync should still succeed because we register custom footprints first
        let result = sync_ast_to_world(&parse_result.value, source, &mut world, &lib);
        assert!(result.is_ok(), "sync errors: {:?}", result.errors);

        // Component should be synced
        assert_eq!(world.component_count(), 1);
        let r1 = world.find_by_refdes("R1").expect("R1 should exist");
        let fp_ref = world.get::<FootprintRef>(r1).unwrap();
        assert_eq!(fp_ref.as_str(), "CUSTOM_2PIN");
    }

    #[test]
    fn test_custom_footprint_with_tht_pads() {
        let source = r#"
version 1

footprint MY_DIP8 {
    description "Custom DIP-8"
    pad 1 circle at -3.81mm, 3.81mm size 1.8mm x 1.8mm drill 1.0mm
    pad 2 circle at -3.81mm, 1.27mm size 1.8mm x 1.8mm drill 1.0mm
    pad 3 circle at -3.81mm, -1.27mm size 1.8mm x 1.8mm drill 1.0mm
    pad 4 circle at -3.81mm, -3.81mm size 1.8mm x 1.8mm drill 1.0mm
    pad 5 circle at 3.81mm, -3.81mm size 1.8mm x 1.8mm drill 1.0mm
    pad 6 circle at 3.81mm, -1.27mm size 1.8mm x 1.8mm drill 1.0mm
    pad 7 circle at 3.81mm, 1.27mm size 1.8mm x 1.8mm drill 1.0mm
    pad 8 circle at 3.81mm, 3.81mm size 1.8mm x 1.8mm drill 1.0mm
    courtyard 10mm x 10mm
}

board test { size 30mm x 30mm }

component U1 ic "MY_DIP8" {
    at 15mm, 15mm
}
"#;
        let parse_result = parse(source);
        assert!(parse_result.is_ok(), "parse errors: {:?}", parse_result.errors);

        let mut world = BoardWorld::new();
        let lib = FootprintLibrary::new();

        let result = sync_ast_to_world(&parse_result.value, source, &mut world, &lib);
        assert!(result.is_ok(), "sync errors: {:?}", result.errors);

        // Component should be synced
        assert_eq!(world.component_count(), 1);
    }
}
