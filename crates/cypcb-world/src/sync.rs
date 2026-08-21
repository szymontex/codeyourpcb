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
//!     let mut lib = FootprintLibrary::new();
//!     let sync_result = sync_ast_to_world(&parse_result.value, source, &mut world, &mut lib);
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

use std::collections::{HashMap, HashSet};
use std::fmt;

use bevy_ecs::prelude::Entity;
use miette::{Diagnostic, LabeledSpan, SourceCode, SourceSpan};

use cypcb_core::{Nm, Point, Rect};
use cypcb_parser::ast::{
    BoardDef, ComponentDef, Definition, FootprintDef, NetDef, PadShape as AstPadShape,
    PinId as AstPinId, SourceFile, Span, TraceDef, TraceDirective, ZoneDef,
    ZoneKind as AstZoneKind,
};

use crate::components::{
    trace::{Trace, TraceSegment, TraceSource, Via},
    ComponentKind, FootprintRef, Layer, NetConnections, PadShape as EcsPadShape, PinConnection,
    Position, RefDes, Rotation, SourceSpan as EcsSourceSpan, Stackup, StackupLayer,
    StackupLayerKind, Value, Zone, ZoneKind as EcsZoneKind,
};
use crate::footprint::{Footprint, FootprintLibrary, PadDef as FootprintPadDef};
use crate::world::BoardWorld;

/// Stroke a silkscreen shape gets when the design does not state one.
///
/// Matches the exporter's default line width, so a legend written without a
/// width prints as the same ink the exporter would have drawn anyway.
const DEFAULT_SILK_WIDTH: Nm = Nm(150_000);

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

    /// A net names a pin the component's footprint does not have.
    ///
    /// Silent until 2026-08-08: `net SIG { R1.3 }` on a two-pad part stored a
    /// connection to pin 3, the ratsnest had one end and nothing to route, and
    /// the only thing reported was that R1.1 and R1.2 were unconnected - which
    /// reads as the design's fault rather than a typo. The connection the user
    /// asked for simply did not exist.
    UnknownPin {
        /// The component whose footprint was consulted.
        component: String,
        /// The pin the net asked for.
        pin: String,
        /// The pins the footprint does have, in order.
        available: Vec<String>,
        /// Source code for miette display.
        src: String,
        /// Source span of the pin reference.
        span: miette::SourceSpan,
    },

    /// A trace references an invalid pin.
    InvalidTracePin {
        /// The trace net name.
        net: String,
        /// The component refdes.
        component: String,
        /// The pin name/number.
        pin: String,
        /// Source code for miette display.
        src: String,
        /// Source span of the pin reference.
        span: miette::SourceSpan,
    },

    /// A trace references a net that doesn't exist.
    MissingNet {
        /// The unknown net name.
        net: String,
        /// Source code for miette display.
        src: String,
        /// Source span of the net reference.
        span: miette::SourceSpan,
    },

    /// A `use` names a module that was never defined.
    UnknownModule {
        /// The module name.
        name: String,
        /// Source code for miette display.
        src: String,
        /// Source span of the module reference.
        span: miette::SourceSpan,
    },

    /// An instance leaves one of its module's pins unconnected.
    UnconnectedModulePin {
        /// The instance name.
        instance: String,
        /// The pin the module exposes.
        pin: String,
        /// Source code for miette display.
        src: String,
        /// Source span of the instantiation.
        span: miette::SourceSpan,
    },

    /// A module claims an interface nobody defined.
    UnknownInterface {
        /// The module making the claim.
        module: String,
        /// The interface it named.
        interface: String,
        /// The interfaces the file does define, in order.
        available: Vec<String>,
        /// Source code for miette display.
        src: String,
        /// Source span of the `implements` clause.
        span: miette::SourceSpan,
    },

    /// A module claims an interface and does not expose all of its pins.
    ///
    /// This is the whole point of the construct: `interface I2C { pin SDA
    /// pin SCL }` is a contract, and a module that says `implements I2C`
    /// without an `SDA` cannot be wired to an I2C bus. Before this the
    /// interface parsed, was stored in the AST and read by nothing.
    InterfaceNotSatisfied {
        /// The module making the claim.
        module: String,
        /// The interface it claims.
        interface: String,
        /// Pins the interface declares and the module does not expose.
        missing: Vec<String>,
        /// Source code for miette display.
        src: String,
        /// Source span of the `implements` clause.
        span: miette::SourceSpan,
        /// Source span of the interface definition.
        declaration: miette::SourceSpan,
    },

    /// A module instantiates itself, directly or through others.
    ModuleCycle {
        /// The chain of modules, from the outermost to the repeat.
        chain: String,
        /// Source code for miette display.
        src: String,
        /// Source span of the instantiation that closes the loop.
        span: miette::SourceSpan,
    },

    /// A trace references an unknown layer.
    UnknownLayer {
        /// The unknown layer name.
        layer: String,
        /// Source code for miette display.
        src: String,
        /// Source span of the layer reference.
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
            SyncError::InvalidTracePin {
                net,
                component,
                pin,
                ..
            } => {
                write!(f, "trace '{}': invalid pin {}.{}", net, component, pin)
            }
            SyncError::UnknownPin {
                component,
                pin,
                available,
                ..
            } => {
                write!(
                    f,
                    "component '{component}' has no pin '{pin}'. It has: {}",
                    available.join(", ")
                )
            }
            SyncError::MissingNet { net, .. } => {
                write!(f, "trace references undefined net: '{}'", net)
            }
            SyncError::UnknownModule { name, .. } => {
                write!(f, "unknown module: '{}'", name)
            }
            SyncError::UnconnectedModulePin { instance, pin, .. } => {
                write!(
                    f,
                    "instance '{}' leaves pin '{}' unconnected",
                    instance, pin
                )
            }
            SyncError::UnknownInterface {
                module,
                interface,
                available,
                ..
            } => {
                write!(
                    f,
                    "module '{module}' implements '{interface}', which is not defined. Defined: {}",
                    if available.is_empty() {
                        "none".to_string()
                    } else {
                        available.join(", ")
                    }
                )
            }
            SyncError::InterfaceNotSatisfied {
                module,
                interface,
                missing,
                ..
            } => {
                write!(
                    f,
                    "module '{module}' implements '{interface}' without pin{} {}",
                    if missing.len() == 1 { "" } else { "s" },
                    missing.join(", ")
                )
            }
            SyncError::ModuleCycle { chain, .. } => {
                write!(f, "module instantiates itself: {}", chain)
            }
            SyncError::UnknownLayer { layer, .. } => {
                write!(f, "unknown layer: '{}'", layer)
            }
        }
    }
}

impl std::error::Error for SyncError {}

impl Diagnostic for SyncError {
    fn code<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        match self {
            SyncError::UnknownFootprint { .. } => Some(Box::new("cypcb::sync::unknown_footprint")),
            SyncError::DuplicateRefDes { .. } => Some(Box::new("cypcb::sync::duplicate_refdes")),
            SyncError::UnknownComponent { .. } => Some(Box::new("cypcb::sync::unknown_component")),
            SyncError::InvalidTracePin { .. } => Some(Box::new("cypcb::sync::invalid_trace_pin")),
            SyncError::UnknownPin { .. } => Some(Box::new("cypcb::sync::unknown_pin")),
            SyncError::MissingNet { .. } => Some(Box::new("cypcb::sync::missing_net")),
            SyncError::UnknownModule { .. } => Some(Box::new("cypcb::sync::unknown_module")),
            SyncError::UnconnectedModulePin { .. } => {
                Some(Box::new("cypcb::sync::unconnected_module_pin"))
            }
            SyncError::UnknownInterface { .. } => Some(Box::new("cypcb::sync::unknown_interface")),
            SyncError::InterfaceNotSatisfied { .. } => {
                Some(Box::new("cypcb::sync::interface_not_satisfied"))
            }
            SyncError::ModuleCycle { .. } => Some(Box::new("cypcb::sync::module_cycle")),
            SyncError::UnknownLayer { .. } => Some(Box::new("cypcb::sync::unknown_layer")),
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
            SyncError::UnknownPin { available, .. } => Some(Box::new(format!(
                "use one of the pins the footprint declares: {}",
                available.join(", ")
            ))),
            SyncError::InvalidTracePin { .. } => {
                Some(Box::new("ensure the component and pin exist before defining a trace"))
            }
            SyncError::MissingNet { .. } => {
                Some(Box::new("define the net before creating manual traces for it"))
            }
            SyncError::UnknownModule { .. } => {
                Some(Box::new("define the module before instantiating it with `use`"))
            }
            SyncError::UnconnectedModulePin { .. } => Some(Box::new(
                "give every pin the module declares a net: `use M as N { PIN = net }`",
            )),
            SyncError::UnknownInterface { available, .. } => Some(Box::new(format!(
                "define the interface before claiming it, or name one that exists: {}",
                if available.is_empty() {
                    "this file defines none".to_string()
                } else {
                    available.join(", ")
                }
            ))),
            SyncError::InterfaceNotSatisfied { missing, .. } => Some(Box::new(format!(
                "add `pin {}` to the module, or drop the claim",
                missing.join("`, `pin ")
            ))),
            SyncError::ModuleCycle { .. } => Some(Box::new(
                "a module cannot contain itself; break the loop or inline one of the blocks",
            )),
            SyncError::UnknownLayer { .. } => {
                Some(Box::new("use a valid layer name: Top, Bottom, Inner1, Inner2, etc."))
            }
        }
    }

    fn source_code(&self) -> Option<&dyn SourceCode> {
        match self {
            SyncError::UnknownFootprint { src, .. } => Some(src),
            SyncError::DuplicateRefDes { src, .. } => Some(src),
            SyncError::UnknownComponent { src, .. } => Some(src),
            SyncError::UnknownPin { src, .. } => Some(src),
            SyncError::InvalidTracePin { src, .. } => Some(src),
            SyncError::MissingNet { src, .. } => Some(src),
            SyncError::UnknownModule { src, .. } => Some(src),
            SyncError::UnconnectedModulePin { src, .. } => Some(src),
            SyncError::UnknownInterface { src, .. } => Some(src),
            SyncError::InterfaceNotSatisfied { src, .. } => Some(src),
            SyncError::ModuleCycle { src, .. } => Some(src),
            SyncError::UnknownLayer { src, .. } => Some(src),
        }
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        match self {
            SyncError::UnknownFootprint { span, .. } => {
                Some(Box::new(std::iter::once(LabeledSpan::new_with_span(
                    Some("footprint not found in library".to_string()),
                    *span,
                ))))
            }
            SyncError::DuplicateRefDes {
                first, duplicate, ..
            } => Some(Box::new(
                vec![
                    LabeledSpan::new_with_span(Some("first defined here".to_string()), *first),
                    LabeledSpan::new_with_span(
                        Some("duplicate definition".to_string()),
                        *duplicate,
                    ),
                ]
                .into_iter(),
            )),
            SyncError::UnknownComponent { span, .. } => Some(Box::new(std::iter::once(
                LabeledSpan::new_with_span(Some("component not defined".to_string()), *span),
            ))),
            SyncError::UnknownPin {
                span, available, ..
            } => Some(Box::new(std::iter::once(LabeledSpan::new_with_span(
                Some(format!("this footprint has {} pins", available.len())),
                *span,
            )))),
            SyncError::InvalidTracePin { span, .. } => Some(Box::new(std::iter::once(
                LabeledSpan::new_with_span(Some("invalid pin reference".to_string()), *span),
            ))),
            SyncError::MissingNet { span, .. } => Some(Box::new(std::iter::once(
                LabeledSpan::new_with_span(Some("net not defined".to_string()), *span),
            ))),
            SyncError::UnknownModule { span, .. } => {
                Some(Box::new(std::iter::once(LabeledSpan::new_with_span(
                    Some("no module with this name is defined".to_string()),
                    *span,
                ))))
            }
            SyncError::UnconnectedModulePin { span, pin, .. } => {
                Some(Box::new(std::iter::once(LabeledSpan::new_with_span(
                    Some(format!("pin '{pin}' is given no net here")),
                    *span,
                ))))
            }
            SyncError::UnknownInterface { span, .. } => Some(Box::new(std::iter::once(
                LabeledSpan::new_with_span(Some("no interface with this name".to_string()), *span),
            ))),
            SyncError::InterfaceNotSatisfied {
                span,
                declaration,
                missing,
                ..
            } => Some(Box::new(
                vec![
                    LabeledSpan::new_with_span(
                        Some(format!("this promises {}", missing.join(", "))),
                        *span,
                    ),
                    LabeledSpan::new_with_span(Some("declared here".to_string()), *declaration),
                ]
                .into_iter(),
            )),
            SyncError::ModuleCycle { span, .. } => Some(Box::new(std::iter::once(
                LabeledSpan::new_with_span(Some("this closes the loop".to_string()), *span),
            ))),
            SyncError::UnknownLayer { span, .. } => Some(Box::new(std::iter::once(
                LabeledSpan::new_with_span(Some("unknown layer".to_string()), *span),
            ))),
        }
    }
}

/// Result of AST to ECS synchronization.
///
/// Something the board did not say, and what was assumed instead.
///
/// Warnings were `String` until 2026-08-08 - a sentence with no line, printed
/// after the errors that had just learned to point at one. A warning nobody can
/// locate is a warning nobody acts on.
#[derive(Debug, Clone, thiserror::Error)]
#[error("{message}")]
pub struct SyncWarning {
    /// What was assumed.
    pub message: String,
    /// What to write instead, when there is something to write.
    pub help: Option<String>,
    /// Source code for miette display.
    pub src: String,
    /// The span the assumption was made at.
    pub span: miette::SourceSpan,
}

impl miette::Diagnostic for SyncWarning {
    fn severity(&self) -> Option<miette::Severity> {
        Some(miette::Severity::Warning)
    }

    fn source_code(&self) -> Option<&dyn SourceCode> {
        Some(&self.src)
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        Some(Box::new(std::iter::once(LabeledSpan::new_with_span(
            Some("assumed here".to_string()),
            self.span,
        ))))
    }

    fn help<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        self.help
            .as_ref()
            .map(|h| Box::new(h) as Box<dyn fmt::Display>)
    }
}

/// Contains any errors and warnings that occurred during the process.
/// The synchronization continues even when errors occur, producing
/// a partial world that can still be useful for error reporting.
#[derive(Debug, Default)]
pub struct SyncResult {
    /// Semantic errors encountered during sync.
    pub errors: Vec<SyncError>,
    /// Non-fatal warnings.
    pub warnings: Vec<SyncWarning>,
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
/// * `footprint_lib` - Library of available footprints. Footprints defined in the
///   source are registered into it, so callers that need pad geometry afterwards
///   (export, rendering) see them. Footprints from a previous sync are dropped
///   first, so the library always matches the source it was last synced with.
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
/// let mut lib = FootprintLibrary::new();
///
/// let result = sync_ast_to_world(&parse_result.value, source, &mut world, &mut lib);
/// assert!(result.is_ok());
/// ```
pub fn sync_ast_to_world(
    ast: &SourceFile,
    source: &str,
    world: &mut BoardWorld,
    footprint_lib: &mut FootprintLibrary,
) -> SyncResult {
    let mut result = SyncResult::new();

    // Phase 0: Register custom footprints BEFORE component sync so they are
    // available when components reference them. Registering into the caller's
    // library (rather than a local clone) is what lets export and rendering
    // resolve them afterwards.
    // Hold each module to the interfaces it claims. Done before expansion,
    // because the modules are still there to read and because a module nobody
    // instantiates is bound by its promises too.
    check_interface_contracts(ast, source, &mut result);

    // Expand module instances first, so every later pass sees plain
    // components and nets and none of them needs to know modules exist.
    let expanded = expand_module_instances(ast, source, &mut result);
    // Then fold `pin.1 = VCC` into the nets it names, so the rest of this
    // function only ever reads `net` blocks.
    let expanded = fold_pin_assignments_into_nets(expanded);
    let definitions = &expanded;

    // How many copper layers the board declares, read before the footprints
    // because a drilled pad exists on every one of them.
    //
    // A through-hole pad used to be given `[TopCopper, BottomCopper]` whatever
    // the board said, so on a four-layer board it did not exist on In1 or In2.
    // A trace on an inner layer could not reach it, the checker could not see
    // its copper there, and `examples/four-layer.cypcb` - whose whole subject
    // is inner copper - shipped a trace on Inner1 joining two pads it could
    // not touch, with `cypcb check` reporting all four pins unreached.
    let copper_layers = definitions
        .iter()
        .find_map(|def| match def {
            Definition::Board(board) => board.layers,
            _ => None,
        })
        .unwrap_or(2);

    footprint_lib.clear_design();
    for def in definitions {
        if let Definition::Footprint(fp_def) = def {
            footprint_lib.register_design(convert_footprint_def(fp_def, copper_layers));
        }
    }

    // A part on the bottom of the board is the same footprint flipped over, and
    // the flip is registered once here rather than performed by each consumer.
    // The checker, the four Gerber writers, the drill file, the renderer and
    // the pick-and-place list all place a footprint themselves; a mirror
    // written six times is a board whose copper and solder mask disagree about
    // which side a part is on.
    for def in definitions {
        let Definition::Component(comp) = def else {
            continue;
        };
        if comp.side.as_ref().map(|face| face.value.as_str()) != Some("bottom") {
            continue;
        }
        let Some(footprint) = footprint_lib.get(&comp.footprint.value) else {
            continue;
        };
        let flipped = crate::footprint::mirrored_to_bottom(footprint);
        if footprint_lib.get(&flipped.name).is_none() {
            footprint_lib.register_design(flipped);
        }
    }

    // Net classes first: a class states a rule for a group, and a net that
    // states something itself overwrites only the field it states. Applying
    // classes first is what makes that precedence work regardless of the order
    // the two appear in the file.
    for def in definitions {
        if let Definition::NetClass(class) = def {
            sync_netclass(class, world);
        }
    }

    // Claims the design makes about itself. Collected here rather than acted
    // on: an assertion is about the finished board, so the checker evaluates
    // it once everything is placed.
    world.set_diff_pairs(
        definitions
            .iter()
            .filter_map(|def| match def {
                Definition::DiffPair(pair) => Some(pair.clone()),
                _ => None,
            })
            .collect(),
    );

    world.set_assertions(
        definitions
            .iter()
            .filter_map(|def| match def {
                Definition::Assert(assert) => Some(assert.clone()),
                _ => None,
            })
            .collect(),
    );

    // Track reference designators for duplicate detection
    // Maps refdes string to (span, entity)
    let mut refdes_spans: HashMap<String, Span> = HashMap::new();

    // Track component entities for net resolution
    let mut component_entities: HashMap<String, Entity> = HashMap::new();

    // Process definitions in order (footprints already handled above)
    for def in definitions {
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
                sync_net(
                    net,
                    source,
                    world,
                    footprint_lib,
                    &component_entities,
                    &mut result,
                );
            }
            Definition::Zone(zone) => {
                sync_zone(zone, world, &mut result);
            }
            Definition::Trace(trace) => {
                sync_trace(
                    trace,
                    source,
                    world,
                    footprint_lib,
                    &component_entities,
                    &mut result,
                );
            }
            Definition::Footprint(_) => {
                // Already handled in Phase 0 above
            }
            Definition::NetClass(_) => {
                // Already applied above, before any net could overwrite it.
            }
            Definition::Outline(_) => {
                // Already applied above, once the board existed.
            }
            Definition::ModuleInstance(_) => {
                // Already replaced by expand_module_instances above; an
                // instance never reaches this point.
            }
            // v2 constructs — not yet wired to ECS, parsed and stored in AST only
            Definition::Module(_)
            | Definition::Interface(_)
            | Definition::Import(_)
            | Definition::Assert(_) => {}
            // Rides onto the model beside the assertions, below: a pair is a
            // claim about two nets, checked once the copper is in place.
            Definition::DiffPair(_) => {}
        }
    }

    // Publish the effective table on the world so consumers that only get a
    // &BoardWorld - every DRC rule - resolve the same footprints this sync used.
    world.set_footprints(footprint_lib.clone());

    // Rebuild spatial index after all entities are added (including traces/vias)
    world.rebuild_spatial_index_from_library(footprint_lib);
    // The board's outline, once the board itself exists: the loop above is
    // what creates it, so this cannot run before it.
    for def in definitions {
        if let Definition::Outline(outline) = def {
            sync_outline(outline, world);
        }
    }

    result
}

/// Synchronize a board definition to the world.
fn sync_board(board: &BoardDef, source: &str, world: &mut BoardWorld, result: &mut SyncResult) {
    let at_the_board = span_to_source_span(&board.name.span);

    // Extract size, defaulting if not specified
    let (width, height) = if let Some(size) = &board.size {
        // A bare number is millimetres - the grammar's rule, and not one that
        // is going to change. It is still an assumption, and this is the one
        // place where getting it wrong resizes the whole board: somebody
        // thinking in mils who writes `size 800 x 600` asks for 800mm.
        for dimension in [&size.width, &size.height] {
            if !dimension.unit_written {
                result.warnings.push(SyncWarning {
                    message: format!(
                        "board size {} has no unit, read as {}mm",
                        dimension.value, dimension.value
                    ),
                    help: Some(format!(
                        "write `{}mm` to say so, or `{}mil` if that is what you meant",
                        dimension.value, dimension.value
                    )),
                    src: source.to_string(),
                    span: span_to_source_span(&dimension.span),
                });
            }
        }
        (size.width.to_nm(), size.height.to_nm())
    } else {
        result.warnings.push(SyncWarning {
            message: "board has no size, defaulting to 100mm x 100mm".into(),
            help: Some("state one with `size 50mm x 40mm`".into()),
            src: source.to_string(),
            span: at_the_board,
        });
        (Nm::from_mm(100.0), Nm::from_mm(100.0))
    };

    // Extract layer count, defaulting to 2
    let layers = board.layers.unwrap_or_else(|| {
        result.warnings.push(SyncWarning {
            message: "board has no layer count, defaulting to 2 layers".into(),
            help: Some("state one with `layers 2`".into()),
            src: source.to_string(),
            span: at_the_board,
        });
        2
    });

    world.set_board(board.name.value.clone(), (width, height), layers);

    // The fabricator, as the design wrote it. Not checked against a table of
    // fabs here - this crate has none - so a name nobody recognises reaches the
    // caller that resolves it, which is the one that can name the alternatives.
    if let Some(fab) = &board.fab {
        world.set_fab(crate::components::Fab(fab.value.clone()));
    }

    // A stackup is what the design expects to be built, and both parsers have
    // read it since the board block started refusing what it does not
    // recognise. Nothing consumed it until now, so a stackup that contradicted
    // the layer count beside it was a fabrication order nobody checked.
    if let Some(stackup) = &board.stackup {
        world.set_stackup(Stackup {
            layers: stackup
                .layers
                .iter()
                .map(|layer| StackupLayer {
                    kind: stackup_kind(layer.layer_type),
                    name: layer.name.as_ref().map(|n| n.value.clone()),
                    thickness: layer.thickness.as_ref().map(|d| d.to_nm()),
                    material: layer.material.as_ref().map(|m| m.value.clone()),
                    dk_x1000: layer.dk.map(|dk| (dk * 1_000.0).round() as u32),
                    df_x1000000: layer.df.map(|df| (df * 1_000_000.0).round() as u32),
                })
                .collect(),
        });
    }
}

/// The world's word for a parsed stackup layer.
fn stackup_kind(layer_type: cypcb_parser::ast::LayerType) -> StackupLayerKind {
    use cypcb_parser::ast::LayerType;
    match layer_type {
        LayerType::Copper => StackupLayerKind::Copper,
        LayerType::Prepreg => StackupLayerKind::Prepreg,
        LayerType::Core => StackupLayerKind::Core,
        LayerType::Mask => StackupLayerKind::Mask,
        LayerType::Silk => StackupLayerKind::Silk,
        LayerType::Paste => StackupLayerKind::Paste,
    }
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

    // A part on the bottom is placed as the flipped copy registered above. The
    // error above names what the designer wrote, not the derived entry, because
    // `0402@bottom` is not a footprint anybody asked for by name.
    let on_bottom = comp.side.as_ref().map(|face| face.value.as_str()) == Some("bottom");
    let placed_footprint = if on_bottom {
        crate::footprint::bottom_name(footprint_name)
    } else {
        footprint_name.clone()
    };

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
    let ecs_span = EcsSourceSpan::new(comp.span.start, comp.span.end);

    // Spawn the component entity
    let entity = world.spawn_component_with_span(
        RefDes::new(&refdes_str),
        value,
        position,
        rotation,
        FootprintRef::new(&placed_footprint),
        NetConnections::new(),
        ecs_span,
    );

    // Add component kind
    world.ecs_mut().entity_mut(entity).insert(kind);

    // The part to buy, when the design names one. It reaches the bill of
    // materials from here; nothing else on the board needs it.
    if let Some(part) = &comp.lcsc {
        world
            .ecs_mut()
            .entity_mut(entity)
            .insert(crate::components::LcscPart(part.value.clone()));
    }

    // A value written as a quantity stays one. Without this the checker sees
    // only the label and cannot tell ten kilohms from ten microfarads.
    if let Some(typed) = &comp.typed_value {
        world
            .ecs_mut()
            .entity_mut(entity)
            .insert(crate::components::TypedValue {
                value: typed.value,
                unit: typed.unit,
            });
    }

    // What the design said about the part itself. Kept as written: nothing
    // derives anything from these, and a checker that reads one says which
    // part stated it.
    if !comp.spec.is_empty() {
        let entries = comp
            .spec
            .iter()
            .map(|entry| {
                (
                    entry.name.value.clone(),
                    crate::components::TypedValue {
                        value: entry.value.value,
                        unit: entry.value.unit,
                    },
                )
            })
            .collect();
        world
            .ecs_mut()
            .entity_mut(entity)
            .insert(crate::components::PartSpec { entries });
    }

    // Which face the part sits on. The design says with `side bottom`, and
    // where it says nothing the answer is derived from the footprint's copper -
    // a footprint whose pads are bottom-only is a bottom-side part. Storing the
    // answer means every rule and every exporter reads the same one instead of
    // each deriving its own.
    let side = match comp.side.as_ref().map(|face| face.value.as_str()) {
        Some("bottom") => crate::components::Side::Bottom,
        Some("top") => crate::components::Side::Top,
        _ => footprint_lib
            .get(footprint_name)
            .map(side_of_footprint)
            .unwrap_or_default(),
    };
    world.ecs_mut().entity_mut(entity).insert(side);

    // Track for net resolution
    component_entities.insert(refdes_str, entity);
}

/// Synchronize a net definition to the world.
/// Normalize logical pin names to physical pin numbers.
///
/// Standard electronic components use conventional logical names (A/K for diodes,
/// +/- for polar caps) but footprints number pads as 1/2. This maps them so the
/// DSN network section matches the library section.
fn normalize_pin_name(name: &str) -> String {
    match name.to_lowercase().as_str() {
        // Diode / LED: anode=1, cathode=2
        "a" | "anode" => "1".to_string(),
        "k" | "ka" | "cathode" => "2".to_string(),
        // Polar capacitor / power: positive=1, negative=2
        "+" | "pos" | "positive" | "p" => "1".to_string(),
        "-" | "neg" | "negative" | "n" => "2".to_string(),
        // Transistor BJT: base=1, collector=2, emitter=3
        "b" | "base" => "1".to_string(),
        "c" | "collector" => "2".to_string(),
        "e" | "emitter" => "3".to_string(),
        // Pass through unchanged (already numeric or unknown logical name)
        _ => name.to_string(),
    }
}

fn sync_net(
    net: &NetDef,
    source: &str,
    world: &mut BoardWorld,
    footprint_lib: &FootprintLibrary,
    component_entities: &HashMap<String, Entity>,
    result: &mut SyncResult,
) {
    // Intern the net name
    let net_id = world.intern_net(&net.name.value);

    // Carry the design's own requirements onto the board model. Without this
    // the checker cannot see a current requirement and the router hands every
    // net the same preset width.
    if let Some(ref constraints) = net.constraints {
        let carried = cypcb_world_net_constraints(constraints);
        if !carried.is_empty() {
            // Merge rather than replace: whatever a net class already put here
            // stays for the fields this net says nothing about.
            //
            // Destructured on purpose. This list used to name three fields and
            // a fourth was added upstream without it; the constraint reached
            // the world through a `netclass` and was dropped on the way
            // through a net's own block, silently, because nothing here had to
            // mention it. Destructuring makes the next field a compile error
            // instead of a missing feature.
            let crate::registry::NetConstraints {
                width,
                clearance,
                current_ma,
                impedance_ohms_x100,
                neck,
            } = carried;
            let mut merged = world.net_constraints(net_id).unwrap_or_default();
            if width.is_some() {
                merged.width = width;
            }
            if clearance.is_some() {
                merged.clearance = clearance;
            }
            if current_ma.is_some() {
                merged.current_ma = current_ma;
            }
            if impedance_ohms_x100.is_some() {
                merged.impedance_ohms_x100 = impedance_ohms_x100;
            }
            if neck.is_some() {
                merged.neck = neck;
            }
            world.set_net_constraints(net_id, merged);
        }
    }

    // Process each pin reference in the net
    for pin_ref in &net.connections {
        let comp_name = &pin_ref.component.value;

        // Look up component entity
        if let Some(&entity) = component_entities.get(comp_name) {
            // Convert pin ID to string, normalizing logical names to physical numbers
            let pin_str = match &pin_ref.pin {
                AstPinId::Number(n) => n.to_string(),
                AstPinId::Name(s) => normalize_pin_name(s),
            };

            // The pin has to be one the part actually has.
            //
            // Only checked when the footprint is known: a part fetched from a
            // supplier has no pads until its fetch lands, and erroring on those
            // would refuse every board using one.
            let footprint_name = world
                .get::<FootprintRef>(entity)
                .map(|f| f.as_str().to_string());
            if let Some(footprint) = footprint_name.and_then(|name| footprint_lib.get(&name)) {
                if !footprint.pads.is_empty()
                    && !footprint.pads.iter().any(|pad| pad.number == pin_str)
                {
                    result.errors.push(SyncError::UnknownPin {
                        component: comp_name.clone(),
                        pin: pin_str.clone(),
                        available: footprint.pads.iter().map(|p| p.number.clone()).collect(),
                        src: source.to_string(),
                        span: span_to_source_span(&pin_ref.component.span),
                    });
                    continue;
                }
            }

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
    // Either spelling. The grammar takes `top` and `Top` in both a zone and a
    // trace now, so the same word cannot be right in one block and an error in
    // the next.
    let layer_mask = match zone_def
        .layer
        .as_deref()
        .map(|name| name.to_ascii_lowercase())
        .as_deref()
    {
        Some("top") => 0b01,              // Layer 0 (top copper)
        Some("bottom") => 0b10,           // Layer 1 (bottom copper)
        Some("all") | None => 0xFFFFFFFF, // All layers
        Some(_) => 0xFFFFFFFF,            // Unknown layer defaults to all
    };

    // A pour's net. The grammar has read this since the beginning and sync
    // dropped it on the floor, which left a copper pour unable to say what it
    // is poured to - so it could be neither filled nor checked, and the pads it
    // swallows looked unconnected.
    let net = match zone_def.kind {
        AstZoneKind::CopperPour => zone_def
            .net
            .as_ref()
            .map(|net| world.intern_net(&net.value)),
        AstZoneKind::Keepout => None,
    };

    let zone = Zone {
        bounds,
        kind,
        layer_mask,
        name: zone_def.name.as_ref().map(|n| n.value.clone()),
        net,
    };

    world.ecs_mut().spawn(zone);
}

/// Synchronize a trace definition to the ECS world.
///
/// Creates a Trace entity with the specified net, endpoints, and waypoints.
fn sync_trace(
    trace_def: &TraceDef,
    source: &str,
    world: &mut BoardWorld,
    library: &FootprintLibrary,
    component_entities: &HashMap<String, Entity>,
    result: &mut SyncResult,
) {
    // Look up the net ID - the net must be defined before the trace
    let net_id = match world.get_net(&trace_def.net.value) {
        Some(id) => id,
        None => {
            result.errors.push(SyncError::MissingNet {
                net: trace_def.net.value.clone(),
                src: source.to_string(),
                span: span_to_source_span(&trace_def.net.span),
            });
            return;
        }
    };

    // Parse the layer
    let layer = if let Some(layer_name) = &trace_def.layer {
        match parse_layer_name(layer_name) {
            Some(l) => l,
            None => {
                result.errors.push(SyncError::UnknownLayer {
                    layer: layer_name.clone(),
                    src: source.to_string(),
                    span: span_to_source_span(&trace_def.span),
                });
                Layer::TopCopper // Default to top copper
            }
        }
    } else {
        Layer::TopCopper // Default layer
    };

    // Parse the width
    let width = trace_def
        .width
        .as_ref()
        .map(|d| d.to_nm())
        .unwrap_or_else(|| Nm::from_mm(0.2)); // Default 0.2mm

    // Get positions for from/to pins (if specified)
    let from_position = if let Some(ref pin_ref) = trace_def.from {
        get_pin_position(
            world,
            library,
            component_entities,
            pin_ref,
            source,
            result,
            &trace_def.net.value,
        )
    } else {
        None
    };

    let to_position = if let Some(ref pin_ref) = trace_def.to {
        get_pin_position(
            world,
            library,
            component_entities,
            pin_ref,
            source,
            result,
            &trace_def.net.value,
        )
    } else {
        None
    };

    // Whether this block describes geometry rather than a pin-to-pin
    // connection.
    //
    // A via counts. The writer emits each via as a block of its own -
    // `trace GND { via 20mm,10mm drill 0.3mm }` - with no path in it, and
    // testing only for a path skipped that block entirely: every via the
    // router placed disappeared the moment the file was read back, silently,
    // taking its layer change with it.
    let has_geometry = trace_def
        .directives
        .iter()
        .any(|d| matches!(d, TraceDirective::Path(_) | TraceDirective::Via(_)));

    if has_geometry {
        // Geometric mode: process ordered directives to create traces and vias
        let mut current_layer = layer;
        let span = EcsSourceSpan::new(trace_def.span.start, trace_def.span.end);

        for directive in &trace_def.directives {
            match directive {
                TraceDirective::Layer(name) => {
                    current_layer = match parse_layer_name(name) {
                        Some(l) => l,
                        None => {
                            result.errors.push(SyncError::UnknownLayer {
                                layer: name.clone(),
                                src: source.to_string(),
                                span: span_to_source_span(&trace_def.span),
                            });
                            current_layer // keep previous
                        }
                    };
                }
                TraceDirective::Path(path) => {
                    let mut segments = Vec::new();
                    let points: Vec<Point> = path
                        .points
                        .iter()
                        .map(|p| Point::new(p.x.to_nm(), p.y.to_nm()))
                        .collect();

                    for window in points.windows(2) {
                        if let [start, end] = window {
                            segments.push(TraceSegment::new(*start, *end));
                        }
                    }

                    if !segments.is_empty() {
                        let mut trace = Trace {
                            segments,
                            width,
                            layer: current_layer,
                            net_id,
                            locked: trace_def.locked,
                            source: TraceSource::Manual,
                        };
                        // A declared neck becomes copper here rather than
                        // staying a note beside it. Until this, a trace synced
                        // from a design ran at one width end to end and the
                        // check that measures the thin stretch had nothing to
                        // read on a board written in this language.
                        let neck = declared_neck(trace_def);
                        if let Some(neck) = neck {
                            trace.apply_neck(neck);
                        }
                        // NetId has to be its own component, not just a field on Trace: DRC's
                        // same-net exemption and its message enrichment both query for
                        // it. The autorouted path learned this already (KNOWLEDGE.md
                        // K012); traces written in the DSL never did, so they collided
                        // with the pads they connect and reported as trace '?'.
                        let entity = world.ecs_mut().spawn((trace, net_id, span)).id();
                        if let Some(neck) = neck {
                            world.ecs_mut().entity_mut(entity).insert(neck);
                        }
                    }
                }
                TraceDirective::Via(via_def) => {
                    let position =
                        Point::new(via_def.position.x.to_nm(), via_def.position.y.to_nm());
                    let drill = via_def
                        .drill
                        .as_ref()
                        .map(|d| d.to_nm())
                        .unwrap_or_else(|| Nm::from_mm(0.3));
                    let outer_diameter = Nm(drill.0 * 2); // 2x drill for annular ring

                    // A via with no stated pair goes through the board.
                    let (start_layer, end_layer) = via_def
                        .layers
                        .as_ref()
                        .and_then(|(start, end)| {
                            Some((parse_layer_name(start)?, parse_layer_name(end)?))
                        })
                        .unwrap_or((Layer::TopCopper, Layer::BottomCopper));

                    let via = Via {
                        position,
                        drill,
                        outer_diameter,
                        start_layer,
                        end_layer,
                        net_id,
                        locked: trace_def.locked,
                    };
                    world.ecs_mut().spawn((via, net_id, span));
                }
            }
        }
    } else {
        // Logical mode: from -> waypoints -> to (existing behavior)
        let mut segments = Vec::new();
        let mut all_points: Vec<Point> = Vec::new();

        if let Some(start) = from_position {
            all_points.push(start);
        }

        // Add waypoints
        for waypoint in &trace_def.waypoints {
            let point = Point::new(waypoint.x.to_nm(), waypoint.y.to_nm());
            all_points.push(point);
        }

        if let Some(end) = to_position {
            all_points.push(end);
        }

        // Create segments from consecutive points
        for window in all_points.windows(2) {
            if let [start, end] = window {
                segments.push(TraceSegment::new(*start, *end));
            }
        }

        // Create the trace entity
        let mut trace = Trace {
            segments,
            width,
            layer,
            net_id,
            locked: trace_def.locked,
            source: TraceSource::Manual,
        };
        let neck = declared_neck(trace_def);
        if let Some(neck) = neck {
            trace.apply_neck(neck);
        }

        // Add source span for error reporting
        let span = EcsSourceSpan::new(trace_def.span.start, trace_def.span.end);

        // Spawn the trace entity
        // NetId has to be its own component, not just a field on Trace: DRC's
        // same-net exemption and its message enrichment both query for
        // it. The autorouted path learned this already (KNOWLEDGE.md
        // K012); traces written in the DSL never did, so they collided
        // with the pads they connect and reported as trace '?'.
        let entity = world.ecs_mut().spawn((trace, net_id, span)).id();
        if let Some(neck) = neck {
            world.ecs_mut().entity_mut(entity).insert(neck);
        }
    }
}

/// The neck a `trace` block declares, in the model's own units.
///
/// One reader for the two places a trace is built - a `path` and a
/// `from`/`to` - because a neck drawn on one shape and not the other is the
/// kind of difference nobody finds until a board is wrong.
fn declared_neck(
    trace_def: &cypcb_parser::ast::TraceDef,
) -> Option<crate::components::trace::TraceNeck> {
    trace_def
        .neck
        .as_ref()
        .map(|neck| crate::components::trace::TraceNeck {
            width: neck.width.to_nm(),
            length: neck.length.to_nm(),
        })
}

/// Helper to get the position of a pin reference.
fn get_pin_position(
    world: &BoardWorld,
    library: &FootprintLibrary,
    component_entities: &HashMap<String, Entity>,
    pin_ref: &cypcb_parser::ast::PinRef,
    source: &str,
    result: &mut SyncResult,
    net_name: &str,
) -> Option<Point> {
    let component_name = &pin_ref.component.value;

    // Look up the component entity
    let entity = match component_entities.get(component_name) {
        Some(e) => *e,
        None => {
            result.errors.push(SyncError::InvalidTracePin {
                net: net_name.to_string(),
                component: component_name.clone(),
                pin: format!("{}", pin_ref.pin),
                src: source.to_string(),
                span: span_to_source_span(&pin_ref.span),
            });
            return None;
        }
    };

    // Get the component's position
    let position = match world.get::<Position>(entity) {
        Some(p) => p.0,
        None => {
            // Component exists but has no position - use origin
            Point::ORIGIN
        }
    };

    // The pad the reference names, turned the way the part is turned. Until
    // 2026-08-07 this returned the component's own position with a comment
    // calling it "a good approximation": a `trace VCC { from R1.1 to C1.1 }`
    // came out as copper between two part centres, touching neither pad, and
    // that copper is what the Gerber carries.
    let pin = match &pin_ref.pin {
        AstPinId::Number(n) => n.to_string(),
        AstPinId::Name(name) => normalize_pin_name(name),
    };

    let pad_offset = world
        .get::<crate::components::FootprintRef>(entity)
        .and_then(|footprint| library.get(footprint.as_str()))
        .and_then(|footprint| {
            footprint
                .pads
                .iter()
                .find(|pad| pad.number == pin)
                .map(|pad| pad.position)
        });

    let Some(offset) = pad_offset else {
        // A footprint nobody registered, or a pin the footprint does not have.
        // The pin is already reported elsewhere when it is wired to a net; a
        // trace endpoint falls back to the part's own position rather than
        // dropping the copper.
        return Some(position);
    };

    let degrees = world
        .get::<crate::components::Rotation>(entity)
        .map(|rotation| rotation.to_degrees())
        .unwrap_or(0.0);
    let (sin, cos) = degrees.to_radians().sin_cos();
    let (px, py) = (offset.x.0 as f64, offset.y.0 as f64);

    Some(Point::new(
        Nm(position.x.0 + (px * cos - py * sin).round() as i64),
        Nm(position.y.0 + (px * sin + py * cos).round() as i64),
    ))
}

/// Parse a layer name string to a Layer enum.
fn parse_layer_name(name: &str) -> Option<Layer> {
    // Case-insensitive, because a zone spelled its layers in lower case and a
    // trace in title case, and a designer moving between the two blocks should
    // not have to remember which.
    match name.to_ascii_lowercase().as_str() {
        "top" => Some(Layer::TopCopper),
        "bottom" => Some(Layer::BottomCopper),
        "inner1" => Some(Layer::Inner(0)),
        "inner2" => Some(Layer::Inner(1)),
        "inner3" => Some(Layer::Inner(2)),
        "inner4" => Some(Layer::Inner(3)),
        "inner5" => Some(Layer::Inner(4)),
        "inner6" => Some(Layer::Inner(5)),
        _ => None,
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

/// Convert an AST FootprintDef to a library Footprint.
fn convert_footprint_def(fp_def: &FootprintDef, copper_layers: u8) -> Footprint {
    let pads: Vec<FootprintPadDef> =
        fp_def
            .pads
            .iter()
            .map(|p| {
                let is_tht = p.drill.is_some();
                FootprintPadDef {
                    number: p.number.to_string(),
                    shape: convert_pad_shape(p.shape),
                    position: Point::new(p.x.to_nm(), p.y.to_nm()),
                    size: (p.width.to_nm(), p.height.to_nm()),
                    // `drill 2.4mm x 1.0mm` is a slot; the narrow dimension is
                    // what every rule about a drill means, so that is what the
                    // model's `drill` carries and the pair goes beside it. A
                    // design that wrote one number has a round hole and no slot.
                    drill: p.drill.as_ref().map(|d| {
                        let width = d.to_nm();
                        match p.drill_height.as_ref().map(|h| h.to_nm()) {
                            Some(height) => width.min(height),
                            None => width,
                        }
                    }),
                    slot: p.drill.as_ref().zip(p.drill_height.as_ref()).and_then(
                        |(width, height)| {
                            let (width, height) = (width.to_nm(), height.to_nm());
                            // A pair that is equal describes a round hole written
                            // the long way, and sending a milling path for a hole
                            // one drill hit makes would be a slower board.
                            (width != height).then_some((width, height))
                        },
                    ),
                    layers: if is_tht {
                        // A drilled hole goes through the whole board, so its
                        // copper is on every copper layer the board has - not just
                        // the outer two.
                        // Zero-based: `Layer::Inner(0)` is the first inner layer,
                        // which `job.rs` names `In1` and the DSL spells `Inner1`.
                        let mut layers = vec![Layer::TopCopper];
                        for inner in 0..copper_layers.saturating_sub(2) {
                            layers.push(Layer::Inner(inner));
                        }
                        if copper_layers > 1 {
                            layers.push(Layer::BottomCopper);
                        }
                        layers
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
        silk: fp_def
            .silk
            .iter()
            .map(|shape| match shape {
                cypcb_parser::ast::SilkDef::Line {
                    start, end, width, ..
                } => crate::footprint::SilkShape::Segment {
                    start: Point::new(start.0.to_nm(), start.1.to_nm()),
                    end: Point::new(end.0.to_nm(), end.1.to_nm()),
                    width: width
                        .as_ref()
                        .map(|w| w.to_nm())
                        .unwrap_or(DEFAULT_SILK_WIDTH),
                },
                cypcb_parser::ast::SilkDef::Circle {
                    centre,
                    radius,
                    width,
                    ..
                } => crate::footprint::SilkShape::Circle {
                    centre: Point::new(centre.0.to_nm(), centre.1.to_nm()),
                    radius: radius.to_nm(),
                    width: width
                        .as_ref()
                        .map(|w| w.to_nm())
                        .unwrap_or(DEFAULT_SILK_WIDTH),
                },
            })
            .collect(),
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

    #[test]
    fn a_net_class_states_a_rule_once_and_a_net_can_still_override_it() {
        let source = r#"version 1

board t {
    size 40mm x 20mm
    layers 2
}

component R1 resistor "0402" {
    value 10kohm
    at 10mm, 10mm
}

component R2 resistor "0402" {
    value 10kohm
    at 20mm, 10mm
}

netclass Power [width 0.5mm clearance 0.3mm] {
    VCC
    GND
}

net VCC [width 0.8mm] {
    R1.1
    R2.1
}

net GND {
    R1.2
    R2.2
}
"#;
        let parsed = cypcb_parser::parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);

        let mut world = BoardWorld::new();
        let mut library = FootprintLibrary::new();
        let result = sync_ast_to_world(&parsed.value, source, &mut world, &mut library);
        assert!(result.errors.is_empty(), "{:?}", result.errors);

        let gnd = world.get_net("GND").expect("GND");
        let vcc = world.get_net("VCC").expect("VCC");

        // GND says nothing, so it takes the class whole.
        let gnd_rules = world.net_constraints(gnd).expect("GND is in a class");
        assert_eq!(gnd_rules.width, Some(Nm::from_mm(0.5)));
        assert_eq!(gnd_rules.clearance, Some(Nm::from_mm(0.3)));

        // VCC states a width, which wins, while the clearance it says nothing
        // about still comes from the class.
        let vcc_rules = world.net_constraints(vcc).expect("VCC is in a class");
        assert_eq!(
            vcc_rules.width,
            Some(Nm::from_mm(0.8)),
            "a net's own statement beats its class"
        );
        assert_eq!(
            vcc_rules.clearance,
            Some(Nm::from_mm(0.3)),
            "and the class still fills in what the net left unsaid"
        );
    }

    #[test]
    fn a_module_instance_becomes_real_components() {
        // Two instances of one block. Each gets its own copy of the parts,
        // under its own name, and its pins are wired to whatever nets the
        // design names - which is what makes a module worth writing.
        let source = r#"version 1

board t {
    size 40mm x 20mm
    layers 2
}

module Divider {
    pin IN
    pin OUT

    component RTOP resistor "0402" {
        value "10k"
        at 5mm, 5mm
    }

    component RBOT resistor "0402" {
        value "10k"
        at 5mm, 10mm
    }

    net IN {
        RTOP.1
    }

    net MID {
        RTOP.2
        RBOT.1
    }

    net OUT {
        RBOT.2
    }
}

use Divider as DIV1 at 10mm, 5mm {
    IN = VIN
    OUT = SENSE_A
}

use Divider as DIV2 at 25mm, 5mm rotate 90 {
    IN = VIN
    OUT = SENSE_B
}
"#;
        let parsed = cypcb_parser::parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);

        let mut world = BoardWorld::new();
        let mut library = FootprintLibrary::new();
        let result = sync_ast_to_world(&parsed.value, source, &mut world, &mut library);
        assert!(result.errors.is_empty(), "{:?}", result.errors);

        let mut refdes: Vec<String> = {
            let ecs = world.ecs_mut();
            let mut query = ecs.query::<&RefDes>();
            query.iter(ecs).map(|r| r.as_str().to_string()).collect()
        };
        refdes.sort();
        assert_eq!(
            refdes,
            vec!["DIV1_RBOT", "DIV1_RTOP", "DIV2_RBOT", "DIV2_RTOP"],
            "each instance brings its own parts, named after it"
        );

        // A pin becomes the design's net; an internal net stays the instance's
        // own, or the two dividers would short their midpoints together.
        assert!(world.get_net("VIN").is_some(), "both instances share VIN");
        assert!(world.get_net("SENSE_A").is_some());
        assert!(world.get_net("SENSE_B").is_some());
        assert!(world.get_net("DIV1_MID").is_some(), "internal net is local");
        assert!(world.get_net("DIV2_MID").is_some());
        assert!(
            world.get_net("MID").is_none(),
            "an unprefixed MID would be one net across both instances"
        );

        // The instance decides where the module's coordinates land. RTOP sits
        // at 5mm, 5mm inside the module, so it belongs at 15mm, 10mm in DIV1.
        let mut placed = |refdes: &str| -> (i64, i64, i32) {
            let ecs = world.ecs_mut();
            let mut query = ecs.query::<(&RefDes, &Position, &Rotation)>();
            let found = query
                .iter(ecs)
                .find(|(r, _, _)| r.as_str() == refdes)
                .unwrap_or_else(|| panic!("{refdes} missing"));
            (found.1 .0.x.raw(), found.1 .0.y.raw(), found.2 .0)
        };

        let (x, y, rot) = placed("DIV1_RTOP");
        assert_eq!((x, y, rot), (15_000_000, 10_000_000, 0));

        // DIV2 is turned a quarter turn about its own origin: 5mm, 5mm becomes
        // -5mm, 5mm before the origin is added, and every part turns with it.
        let (x, y, rot) = placed("DIV2_RTOP");
        assert_eq!((x, y, rot), (20_000_000, 10_000_000, 90_000));

        // Two instances no longer occupy the same square millimetre.
        assert_ne!(placed("DIV1_RBOT"), placed("DIV2_RBOT"));
    }

    #[test]
    fn modules_nest() {
        // A pair block made of two dividers, instantiated once. Names and
        // placements compose through both levels, and a port on the inner
        // instance reaches the design's net through the outer one.
        let source = r#"version 1

board t {
    size 60mm x 40mm
    layers 2
}

module Divider {
    pin IN
    pin OUT

    component R1 resistor "0402" {
        value "10k"
        at 2mm, 0mm
    }

    net IN {
        R1.1
    }

    net OUT {
        R1.2
    }
}

module Pair {
    pin SUPPLY
    pin A
    pin B

    use Divider as LEFT at 0mm, 0mm {
        IN = SUPPLY
        OUT = A
    }

    use Divider as RIGHT at 10mm, 0mm {
        IN = SUPPLY
        OUT = B
    }
}

use Pair as BANK at 20mm, 15mm {
    SUPPLY = VIN
    A = SENSE_A
    B = SENSE_B
}
"#;
        let parsed = cypcb_parser::parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);

        let mut world = BoardWorld::new();
        let mut library = FootprintLibrary::new();
        let result = sync_ast_to_world(&parsed.value, source, &mut world, &mut library);
        assert!(result.errors.is_empty(), "{:?}", result.errors);

        let mut refdes: Vec<String> = {
            let ecs = world.ecs_mut();
            let mut query = ecs.query::<&RefDes>();
            query.iter(ecs).map(|r| r.as_str().to_string()).collect()
        };
        refdes.sort();
        assert_eq!(refdes, vec!["BANK_LEFT_R1", "BANK_RIGHT_R1"]);

        // 20mm (BANK) + 10mm (RIGHT inside Pair) + 2mm (R1 inside Divider).
        let mut placed = |name: &str| -> (i64, i64) {
            let ecs = world.ecs_mut();
            let mut query = ecs.query::<(&RefDes, &Position)>();
            let found = query
                .iter(ecs)
                .find(|(r, _)| r.as_str() == name)
                .unwrap_or_else(|| panic!("{name} missing"));
            (found.1 .0.x.raw(), found.1 .0.y.raw())
        };
        assert_eq!(placed("BANK_LEFT_R1"), (22_000_000, 15_000_000));
        assert_eq!(placed("BANK_RIGHT_R1"), (32_000_000, 15_000_000));

        // A port two levels down reaches the design's own net, and both
        // dividers share the supply.
        assert!(world.get_net("VIN").is_some(), "SUPPLY -> VIN through Pair");
        assert!(world.get_net("SENSE_A").is_some());
        assert!(world.get_net("SENSE_B").is_some());
        assert!(
            world.get_net("BANK_LEFT_IN").is_none(),
            "an inner pin must not become a local net when it is wired through"
        );
    }

    #[test]
    fn a_module_that_contains_itself_is_reported() {
        let source = r#"version 1

board t {
    size 20mm x 20mm
    layers 2
}

module A {
    pin P

    use B as INNER {
        Q = P
    }
}

module B {
    pin Q

    use A as INNER {
        P = Q
    }
}

use A as TOP {
    P = VIN
}
"#;
        let parsed = cypcb_parser::parse(source);
        let mut world = BoardWorld::new();
        let mut library = FootprintLibrary::new();
        let result = sync_ast_to_world(&parsed.value, source, &mut world, &mut library);

        assert!(
            result
                .errors
                .iter()
                .any(|e| matches!(e, SyncError::ModuleCycle { .. })),
            "expansion has to stop and say why: {:?}",
            result.errors
        );
    }

    #[test]
    fn instantiating_something_that_does_not_exist_is_an_error() {
        let source = r#"version 1

board t {
    size 10mm x 10mm
    layers 2
}

use NoSuchThing as X {
    IN = VIN
}
"#;
        let parsed = cypcb_parser::parse(source);
        let mut world = BoardWorld::new();
        let mut library = FootprintLibrary::new();
        let result = sync_ast_to_world(&parsed.value, source, &mut world, &mut library);

        assert!(
            result
                .errors
                .iter()
                .any(|e| matches!(e, SyncError::UnknownModule { .. })),
            "{:?}",
            result.errors
        );
    }

    #[test]
    fn a_pin_left_unconnected_is_reported() {
        let source = r#"version 1

board t {
    size 10mm x 10mm
    layers 2
}

module M {
    pin IN
    pin OUT

    component R1 resistor "0402" {
        value "10k"
        at 5mm, 5mm
    }

    net IN {
        R1.1
    }

    net OUT {
        R1.2
    }
}

use M as A {
    IN = VIN
}
"#;
        let parsed = cypcb_parser::parse(source);
        let mut world = BoardWorld::new();
        let mut library = FootprintLibrary::new();
        let result = sync_ast_to_world(&parsed.value, source, &mut world, &mut library);

        assert!(
            result.errors.iter().any(|e| matches!(
                e,
                SyncError::UnconnectedModulePin { pin, .. } if pin == "OUT"
            )),
            "a dangling pin has to be named, not quietly localised: {:?}",
            result.errors
        );
    }

    /// Two interfaces, one module, and the pins it does and does not expose.
    ///
    /// `module` is the body written between the pins and the closing brace.
    fn sync_module(body: &str) -> SyncResult {
        let source = format!(
            r#"version 1

interface I2C {{
    pin SDA
    pin SCL
}}

interface Power {{
    pin VCC
    pin GND
}}

module Sensor {{
{body}
    component U1 ic "SOIC-8" {{
        value "TMP102"
        at 0mm, 0mm
    }}
}}
"#
        );
        let parsed = cypcb_parser::parse(&source);
        assert!(
            parsed.errors.is_empty(),
            "the fixture has to parse: {:?}",
            parsed.errors
        );
        let mut world = BoardWorld::new();
        let mut library = FootprintLibrary::new();
        sync_ast_to_world(&parsed.value, &source, &mut world, &mut library)
    }

    #[test]
    fn a_module_that_exposes_an_interfaces_pins_satisfies_it() {
        let result = sync_module(
            "    implements I2C
    implements Power
    pin SDA
    pin SCL
    pin VCC
    pin GND
",
        );
        assert!(
            !result.errors.iter().any(|e| matches!(
                e,
                SyncError::InterfaceNotSatisfied { .. } | SyncError::UnknownInterface { .. }
            )),
            "every pin of both interfaces is exposed: {:?}",
            result.errors
        );
    }

    #[test]
    fn a_module_claiming_an_interface_without_its_pins_is_reported() {
        // The whole point of the construct: a module that says `implements
        // I2C` and has no SDA cannot be wired to an I2C bus, and until this
        // existed the claim was a comment the compiler never read.
        let result = sync_module(
            "    implements I2C
    pin SCL
",
        );
        let missing_pins = result.errors.iter().find_map(|e| match e {
            SyncError::InterfaceNotSatisfied {
                module,
                interface,
                missing,
                ..
            } => Some((module.clone(), interface.clone(), missing.clone())),
            _ => None,
        });
        assert_eq!(
            missing_pins,
            Some((
                "Sensor".to_string(),
                "I2C".to_string(),
                vec!["SDA".to_string()]
            )),
            "the missing pin has to be named: {:?}",
            result.errors
        );
    }

    #[test]
    fn a_module_claiming_an_interface_nobody_defined_is_reported() {
        let result = sync_module(
            "    implements SPI
    pin MOSI
",
        );
        let named = result.errors.iter().find_map(|e| match e {
            SyncError::UnknownInterface {
                interface,
                available,
                ..
            } => Some((interface.clone(), available.clone())),
            _ => None,
        });
        assert_eq!(
            named,
            Some((
                "SPI".to_string(),
                vec!["I2C".to_string(), "Power".to_string()]
            )),
            "an undefined interface is an error, and the message says which are defined: {:?}",
            result.errors
        );
    }

    #[test]
    fn a_module_that_claims_nothing_is_left_alone() {
        // Every module written before `implements` existed says nothing, and
        // has to keep synchronising without a word about interfaces.
        let result = sync_module("    pin SDA\n");
        assert!(
            !result.errors.iter().any(|e| matches!(
                e,
                SyncError::InterfaceNotSatisfied { .. } | SyncError::UnknownInterface { .. }
            )),
            "a module making no claim cannot break one: {:?}",
            result.errors
        );
    }

    #[test]
    fn a_trace_written_in_the_dsl_carries_its_net_as_a_component() {
        // DRC's same-net exemption and its message enrichment both query for a
        // NetId component. The autorouted path attaches one; the DSL path did
        // not, so a hand-written trace collided with the pad it connects to and
        // reported as trace '?'.
        let source = r#"version 1

board t {
    size 30mm x 20mm
    layers 2
}

component R1 resistor "0402" {
    value "10k"
    at 10mm, 10mm
}

component R2 resistor "0402" {
    value "10k"
    at 20mm, 10mm
}

net SIG {
    R1.1
    R2.1
}

trace SIG {
    from R1.1
    to R2.1
    layer Top
}
"#;
        let parsed = cypcb_parser::parse(source);
        let mut world = BoardWorld::new();
        let mut library = FootprintLibrary::new();
        sync_ast_to_world(&parsed.value, source, &mut world, &mut library);

        let expected = world.get_net("SIG").expect("net interned");
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<(&Trace, &crate::components::NetId)>();
        let tagged: Vec<_> = query.iter(ecs).collect();

        assert_eq!(
            tagged.len(),
            1,
            "the DSL trace must carry a NetId component"
        );
        assert_eq!(*tagged[0].1, expected);
    }
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
        assert!(
            parse_result.is_ok(),
            "parse errors: {:?}",
            parse_result.errors
        );

        let mut world = BoardWorld::new();
        let mut lib = FootprintLibrary::new();

        let result = sync_ast_to_world(&parse_result.value, source, &mut world, &mut lib);

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
        assert!(
            parse_result.is_ok(),
            "parse errors: {:?}",
            parse_result.errors
        );

        let mut world = BoardWorld::new();
        let mut lib = FootprintLibrary::new();

        let result = sync_ast_to_world(&parse_result.value, source, &mut world, &mut lib);

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
        assert!(
            parse_result.is_ok(),
            "parse errors: {:?}",
            parse_result.errors
        );

        let mut world = BoardWorld::new();
        let mut lib = FootprintLibrary::new();

        let result = sync_ast_to_world(&parse_result.value, source, &mut world, &mut lib);

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
    fn a_pin_assignment_puts_the_pin_on_the_net_it_names() {
        // `pin.1 = VCC` is a rule in the grammar, an arm in the reader, a field
        // on the AST and something the language server reads - and the board
        // model dropped it. Measured on this board before the fold: `cypcb
        // parse` reported `"pins": []` for both parts and `cypcb check` gave
        // four `unconnected-pin` violations for a file that names every
        // connection it has.
        let source = r#"
version 1
board test { size 20mm x 20mm layers 2 }
component R1 resistor "0402" {
    at 5mm, 5mm
    pin.1 = VCC
    pin.2 = OUT
}
component R2 resistor "0402" {
    at 12mm, 5mm
    pin.1 = OUT
    pin.2 = GND
}
"#;
        let parse_result = parse(source);
        assert!(parse_result.is_ok(), "parse: {:?}", parse_result.errors);

        let mut world = BoardWorld::new();
        let mut lib = FootprintLibrary::new();
        let result = sync_ast_to_world(&parse_result.value, source, &mut world, &mut lib);
        assert!(result.is_ok(), "sync errors: {:?}", result.errors);

        let out = world.get_net("OUT").expect("OUT is a net");
        let r1 = world.find_by_refdes("R1").expect("R1 was spawned");
        let r2 = world.find_by_refdes("R2").expect("R2 was spawned");

        assert_eq!(
            world.get::<NetConnections>(r1).unwrap().pin_net("2"),
            Some(out),
            "R1.2 says it is on OUT"
        );
        assert_eq!(
            world.get::<NetConnections>(r2).unwrap().pin_net("1"),
            Some(out),
            "R2.1 says it is on OUT, and it is the same net"
        );
        assert!(
            world.get_net("VCC").is_some() && world.get_net("GND").is_some(),
            "a net named only by an assignment still exists"
        );
    }

    #[test]
    fn a_pin_assignment_joins_the_block_of_the_same_name() {
        // One net, written from both ends: the block says what it is made of
        // and how wide it has to be, the assignment adds a pin to it.
        let source = r#"
version 1
board test { size 20mm x 20mm layers 2 }
component R1 resistor "0402" {
    at 5mm, 5mm
}
component R2 resistor "0402" {
    at 12mm, 5mm
    pin.1 = SIG
}
net SIG [width 0.5mm] {
    R1.2
}
"#;
        let parse_result = parse(source);
        assert!(parse_result.is_ok(), "parse: {:?}", parse_result.errors);

        let mut world = BoardWorld::new();
        let mut lib = FootprintLibrary::new();
        let result = sync_ast_to_world(&parse_result.value, source, &mut world, &mut lib);
        assert!(result.is_ok(), "sync errors: {:?}", result.errors);

        let sig = world.get_net("SIG").expect("SIG is a net");
        let r1 = world.find_by_refdes("R1").unwrap();
        let r2 = world.find_by_refdes("R2").unwrap();
        assert_eq!(
            world.get::<NetConnections>(r1).unwrap().pin_net("2"),
            Some(sig)
        );
        assert_eq!(
            world.get::<NetConnections>(r2).unwrap().pin_net("1"),
            Some(sig)
        );

        let constraints = world
            .net_constraints(sig)
            .expect("the block's constraints survive the merge");
        assert_eq!(
            constraints.width,
            Some(cypcb_core::Nm(500_000)),
            "an assignment says who is connected, never how wide the copper is"
        );
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
        let mut lib = FootprintLibrary::new();

        let result = sync_ast_to_world(&parse_result.value, source, &mut world, &mut lib);

        assert!(result.has_errors());
        assert_eq!(result.errors.len(), 1);
        assert!(matches!(
            result.errors[0],
            SyncError::UnknownFootprint { .. }
        ));

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
        let mut lib = FootprintLibrary::new();

        let result = sync_ast_to_world(&parse_result.value, source, &mut world, &mut lib);

        assert!(result.has_errors());
        assert_eq!(result.errors.len(), 1);
        assert!(matches!(
            result.errors[0],
            SyncError::DuplicateRefDes { .. }
        ));

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
        let mut lib = FootprintLibrary::new();

        let result = sync_ast_to_world(&parse_result.value, source, &mut world, &mut lib);

        assert!(result.has_errors());
        assert_eq!(result.errors.len(), 1);
        assert!(matches!(
            result.errors[0],
            SyncError::UnknownComponent { .. }
        ));
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
        assert!(
            parse_result.is_ok(),
            "parse errors: {:?}",
            parse_result.errors
        );

        let mut world = BoardWorld::new();
        let mut lib = FootprintLibrary::new();

        let result = sync_ast_to_world(&parse_result.value, source, &mut world, &mut lib);

        assert!(result.is_ok(), "sync errors: {:?}", result.errors);

        let led = world.find_by_refdes("LED1").unwrap();
        let conns = world.get::<NetConnections>(led).unwrap();
        let anode_net = world.get_net("ANODE").unwrap();
        // "anode" is normalized to pin "1" by normalize_pin_name during sync
        assert_eq!(conns.pin_net("1"), Some(anode_net));
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
        assert!(
            parse_result.is_ok(),
            "parse errors: {:?}",
            parse_result.errors
        );

        let mut world = BoardWorld::new();
        let mut lib = FootprintLibrary::new();

        let result = sync_ast_to_world(&parse_result.value, source, &mut world, &mut lib);

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
        let mut lib = FootprintLibrary::new();

        let result = sync_ast_to_world(&parse_result.value, source, &mut world, &mut lib);

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
        let mut lib = FootprintLibrary::new();

        let result = sync_ast_to_world(&parse_result.value, source, &mut world, &mut lib);
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
        let mut lib = FootprintLibrary::new();

        let result = sync_ast_to_world(&parse_result.value, source, &mut world, &mut lib);
        assert!(result.is_ok());

        let r1 = world.find_by_refdes("R1").unwrap();
        let span = world
            .get::<EcsSourceSpan>(r1)
            .expect("should have source span");
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
        assert!(
            parse_result.is_ok(),
            "parse errors: {:?}",
            parse_result.errors
        );

        let mut world = BoardWorld::new();
        let mut lib = FootprintLibrary::new();

        let result = sync_ast_to_world(&parse_result.value, source, &mut world, &mut lib);
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
        assert!(
            parse_result.is_ok(),
            "parse errors: {:?}",
            parse_result.errors
        );

        let mut world = BoardWorld::new();
        let mut lib = FootprintLibrary::new();

        let result = sync_ast_to_world(&parse_result.value, source, &mut world, &mut lib);
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
        assert!(
            parse_result.is_ok(),
            "parse errors: {:?}",
            parse_result.errors
        );

        let mut world = BoardWorld::new();
        let mut lib = FootprintLibrary::new();

        let result = sync_ast_to_world(&parse_result.value, source, &mut world, &mut lib);
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
        assert!(
            parse_result.is_ok(),
            "parse errors: {:?}",
            parse_result.errors
        );

        let mut world = BoardWorld::new();
        let mut lib = FootprintLibrary::new();

        // CUSTOM_2PIN is not in the built-in library
        assert!(lib.get("CUSTOM_2PIN").is_none());

        // But sync should still succeed because we register custom footprints first
        let result = sync_ast_to_world(&parse_result.value, source, &mut world, &mut lib);
        assert!(result.is_ok(), "sync errors: {:?}", result.errors);

        // Component should be synced
        assert_eq!(world.component_count(), 1);
        let r1 = world.find_by_refdes("R1").expect("R1 should exist");
        let fp_ref = world.get::<FootprintRef>(r1).unwrap();
        assert_eq!(fp_ref.as_str(), "CUSTOM_2PIN");

        // The caller's library must keep the custom footprint - export and
        // rendering resolve pad geometry through it after sync returns.
        let registered = lib
            .get("CUSTOM_2PIN")
            .expect("custom footprint must be visible to the caller after sync");
        assert_eq!(registered.pads.len(), 2);
    }

    #[test]
    fn test_resync_drops_removed_custom_footprint() {
        let with_footprint = r#"
version 1

footprint TEMP_PART {
    pad 1 rect at 0mm, 0mm size 0.5mm x 0.5mm
}

board test { size 20mm x 20mm }
"#;
        let without_footprint = "version 1\n\nboard test { size 20mm x 20mm }\n";

        let mut world = BoardWorld::new();
        let mut lib = FootprintLibrary::new();

        let first = parse(with_footprint);
        sync_ast_to_world(&first.value, with_footprint, &mut world, &mut lib);
        assert!(lib.contains("TEMP_PART"));

        // Hot reload with the footprint deleted from the source: it must not
        // linger and keep resolving.
        let second = parse(without_footprint);
        sync_ast_to_world(&second.value, without_footprint, &mut world, &mut lib);
        assert!(!lib.contains("TEMP_PART"));
        assert!(lib.contains("0402"), "built-ins must survive a re-sync");
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
        assert!(
            parse_result.is_ok(),
            "parse errors: {:?}",
            parse_result.errors
        );

        let mut world = BoardWorld::new();
        let mut lib = FootprintLibrary::new();

        let result = sync_ast_to_world(&parse_result.value, source, &mut world, &mut lib);
        assert!(result.is_ok(), "sync errors: {:?}", result.errors);

        // Component should be synced
        assert_eq!(world.component_count(), 1);
    }

    #[test]
    fn test_sync_trace_basic() {
        let source = r#"
version 1
board test { size 50mm x 30mm }

component R1 resistor "0402" { at 10mm, 10mm }
component C1 capacitor "0402" { at 20mm, 10mm }

net VCC {
    R1.1
    C1.1
}

trace VCC {
    from R1.1
    to C1.1
    layer Top
    width 0.3mm
}
"#;
        let parse_result = parse(source);
        assert!(
            parse_result.is_ok(),
            "parse errors: {:?}",
            parse_result.errors
        );

        let mut world = BoardWorld::new();
        let mut lib = FootprintLibrary::new();

        let result = sync_ast_to_world(&parse_result.value, source, &mut world, &mut lib);
        assert!(result.is_ok(), "sync errors: {:?}", result.errors);

        // Query for trace entities
        use crate::components::trace::Trace;
        let mut trace_count = 0;
        let mut query = world.ecs_mut().query::<&Trace>();
        for trace in query.iter(world.ecs()) {
            trace_count += 1;
            assert_eq!(trace.layer, Layer::TopCopper);
            assert_eq!(trace.width, Nm::from_mm(0.3));
            assert!(!trace.locked);
            // Should have one segment (from R1 to C1)
            assert_eq!(trace.segments.len(), 1);
        }
        assert_eq!(trace_count, 1, "should have one trace");
    }

    #[test]
    fn test_sync_trace_with_waypoints() {
        let source = r#"
version 1
board test { size 50mm x 30mm }

component R1 resistor "0402" { at 10mm, 10mm }
component C1 capacitor "0402" { at 30mm, 20mm }

net SIG {
    R1.2
    C1.1
}

trace SIG {
    from R1.2
    to C1.1
    via 20mm, 10mm
    via 20mm, 20mm
    layer Bottom
}
"#;
        let parse_result = parse(source);
        assert!(
            parse_result.is_ok(),
            "parse errors: {:?}",
            parse_result.errors
        );

        let mut world = BoardWorld::new();
        let mut lib = FootprintLibrary::new();

        let result = sync_ast_to_world(&parse_result.value, source, &mut world, &mut lib);
        assert!(result.is_ok(), "sync errors: {:?}", result.errors);

        use crate::components::trace::Trace;
        let mut query = world.ecs_mut().query::<&Trace>();
        for trace in query.iter(world.ecs()) {
            assert_eq!(trace.layer, Layer::BottomCopper);
            // From R1 -> via1 -> via2 -> to C1 = 3 segments
            assert_eq!(trace.segments.len(), 3);
        }
    }

    #[test]
    fn test_sync_trace_locked() {
        let source = r#"
version 1
board test { size 50mm x 30mm }

component R1 resistor "0402" { at 10mm, 10mm }
component C1 capacitor "0402" { at 20mm, 10mm }

net VCC { R1.1, C1.1 }

trace VCC {
    from R1.1
    to C1.1
    locked
}
"#;
        let parse_result = parse(source);
        assert!(
            parse_result.is_ok(),
            "parse errors: {:?}",
            parse_result.errors
        );

        let mut world = BoardWorld::new();
        let mut lib = FootprintLibrary::new();

        let result = sync_ast_to_world(&parse_result.value, source, &mut world, &mut lib);
        assert!(result.is_ok(), "sync errors: {:?}", result.errors);

        use crate::components::trace::Trace;
        let mut query = world.ecs_mut().query::<&Trace>();
        for trace in query.iter(world.ecs()) {
            assert!(trace.locked);
        }
    }

    #[test]
    fn test_sync_trace_missing_net() {
        let source = r#"
version 1
board test { size 50mm x 30mm }

component R1 resistor "0402" { at 10mm, 10mm }
component C1 capacitor "0402" { at 20mm, 10mm }

trace UNDEFINED_NET {
    from R1.1
    to C1.1
}
"#;
        let parse_result = parse(source);
        assert!(
            parse_result.is_ok(),
            "parse errors: {:?}",
            parse_result.errors
        );

        let mut world = BoardWorld::new();
        let mut lib = FootprintLibrary::new();

        let result = sync_ast_to_world(&parse_result.value, source, &mut world, &mut lib);

        assert!(result.has_errors());
        assert!(matches!(result.errors[0], SyncError::MissingNet { .. }));
    }

    #[test]
    fn test_sync_trace_invalid_component() {
        let source = r#"
version 1
board test { size 50mm x 30mm }

component R1 resistor "0402" { at 10mm, 10mm }

net VCC { R1.1 }

trace VCC {
    from R1.1
    to UNKNOWN.1
}
"#;
        let parse_result = parse(source);
        assert!(
            parse_result.is_ok(),
            "parse errors: {:?}",
            parse_result.errors
        );

        let mut world = BoardWorld::new();
        let mut lib = FootprintLibrary::new();

        let result = sync_ast_to_world(&parse_result.value, source, &mut world, &mut lib);

        assert!(result.has_errors());
        assert!(matches!(
            result.errors[0],
            SyncError::InvalidTracePin { .. }
        ));
    }
}

/// Convert the parsed constraint block into what the board model stores.
fn cypcb_world_net_constraints(
    constraints: &cypcb_parser::ast::NetConstraints,
) -> crate::registry::NetConstraints {
    crate::registry::NetConstraints {
        width: constraints.width.as_ref().map(|w| w.to_nm()),
        clearance: constraints.clearance.as_ref().map(|c| c.to_nm()),
        current_ma: constraints.current.as_ref().map(|c| c.to_milliamps()),
        impedance_ohms_x100: constraints
            .impedance_ohms
            .map(|ohms| (ohms * 100.0).round() as u32),
        neck: constraints
            .neck
            .as_ref()
            .map(|neck| crate::components::trace::TraceNeck {
                width: neck.width.to_nm(),
                length: neck.length.to_nm(),
            }),
    }
}

/// Which face a footprint's copper puts it on.
///
/// Bottom only when every pad that touches copper touches the bottom and none
/// the top; anything else, including a through-hole part that reaches both, is
/// a top-side part until something says otherwise.
fn side_of_footprint(footprint: &Footprint) -> crate::components::Side {
    let mut top = false;
    let mut bottom = false;
    for pad in &footprint.pads {
        for layer in &pad.layers {
            match layer {
                Layer::TopCopper => top = true,
                Layer::BottomCopper => bottom = true,
                _ => {}
            }
        }
    }
    if bottom && !top {
        crate::components::Side::Bottom
    } else {
        crate::components::Side::Top
    }
}

/// Replace every `use M as N { ... }` with the definitions it stands for.
///
/// A module is a circuit block; an instance is a copy of it under a name. The
/// copy's components take that name as a prefix, so two instances of the same
/// divider are `DIV1_R1` and `DIV2_R1` rather than two `R1`s. A net inside the
/// module is local in the same way - `DIV1_MID` - except where it is named by
/// one of the module's pins, which is the whole point of a pin: `IN = VIN`
/// makes the module's `IN` and the design's `VIN` one net.
///
/// Modules nest. An instance inside a module is expanded with the enclosing
/// instance's name and frame already applied, so `TOP_INNER_R1` sits where the
/// composition of both placements puts it, and a port two levels down still
/// reaches the design's own net.
///
/// Returns the design's own definitions with instances replaced in place, so
/// every pass downstream works on components and nets and needs to know
/// nothing about modules.
/// Turn `pin.1 = VCC` inside a component into a connection on net `VCC`.
///
/// The language has carried this since the beginning: it is a rule in
/// `grammar.js`, an arm in `reader.rs`, a field on `ComponentDef`, and the
/// language server reads it for hover and go-to-definition. The board model
/// did not. A design that declared every connection this way came out with
/// **no nets at all** - measured on a two-resistor board, `"pins": []` in
/// `cypcb parse` and four `unconnected-pin` violations from `cypcb check`,
/// for a file that says what is connected to what on every line.
///
/// Folding here rather than wiring it into the component pass is what keeps
/// one path for nets: pin validation against the footprint, net constraints,
/// classes and the ratsnest all run over `net` blocks, and an assignment is
/// the same statement written at the other end. A net named by both a block
/// and an assignment is one net, and the block's constraints stand - the
/// assignment says who is connected, never how wide the copper is.
///
/// The folded blocks go at the **end** of the definitions, never into the
/// block that shares their name. A net is synchronised in place and a net
/// reaching for a component that has not been synchronised yet is
/// `UnknownComponent`, so merging into a `net` block written above the parts
/// reported the design's own components as undefined. Two blocks naming one
/// net are already one net - the registry interns by name - so appending is
/// both correct and the smaller change.
fn fold_pin_assignments_into_nets(definitions: Vec<Definition>) -> Vec<Definition> {
    // Name -> the connections its assignments contribute, in source order.
    let mut assigned: Vec<(String, Vec<cypcb_parser::ast::PinRef>)> = Vec::new();
    for def in &definitions {
        let Definition::Component(component) = def else {
            continue;
        };
        for assignment in &component.net_assignments {
            let pin_ref = cypcb_parser::ast::PinRef {
                component: component.refdes.clone(),
                pin: assignment.pin.clone(),
                span: assignment.span,
            };
            match assigned
                .iter_mut()
                .find(|(name, _)| *name == assignment.net.value)
            {
                Some((_, refs)) => refs.push(pin_ref),
                None => assigned.push((assignment.net.value.clone(), vec![pin_ref])),
            }
        }
    }

    if assigned.is_empty() {
        return definitions;
    }

    let mut out = definitions;
    out.reserve(assigned.len());

    for (name, connections) in assigned {
        let span = connections
            .first()
            .map(|pin_ref| pin_ref.span)
            .unwrap_or(cypcb_parser::ast::Span::new(0, 0));
        out.push(Definition::Net(cypcb_parser::ast::NetDef {
            name: cypcb_parser::ast::Identifier::new(name, span),
            constraints: None,
            connections,
            span,
        }));
    }

    out
}

/// Hold every module to the interfaces it claims.
///
/// `interface I2C { pin SDA pin SCL }` is a contract and `implements I2C` is a
/// module signing it. Both halves parsed for a long time and nothing read
/// either, so `examples/v2-interfaces.cypcb` could declare four interfaces,
/// two modules whose comments said which pins belonged to which bus, and be
/// wrong about it without a word from the checker.
///
/// Checked over the definitions rather than over the instances, because a
/// module nobody instantiates is exactly the case this catches: a library file
/// of blocks has no board and no `use`, and its promises still have to hold.
fn check_interface_contracts(ast: &SourceFile, source: &str, result: &mut SyncResult) {
    let interfaces: HashMap<&str, &cypcb_parser::ast::InterfaceDef> = ast
        .definitions
        .iter()
        .filter_map(|def| match def {
            Definition::Interface(iface) => Some((iface.name.value.as_str(), iface)),
            _ => None,
        })
        .collect();

    for def in &ast.definitions {
        let Definition::Module(module) = def else {
            continue;
        };

        for claim in &module.implements {
            let Some(interface) = interfaces.get(claim.interface.value.as_str()) else {
                let mut available: Vec<String> =
                    interfaces.keys().map(|name| name.to_string()).collect();
                available.sort();
                result.errors.push(SyncError::UnknownInterface {
                    module: module.name.value.clone(),
                    interface: claim.interface.value.clone(),
                    available,
                    src: source.to_string(),
                    span: span_to_source_span(&claim.span),
                });
                continue;
            };

            let exposed: HashSet<&str> = module
                .pins
                .iter()
                .map(|pin| pin.name.value.as_str())
                .collect();
            let missing: Vec<String> = interface
                .pins
                .iter()
                .filter(|pin| !exposed.contains(pin.name.value.as_str()))
                .map(|pin| pin.name.value.clone())
                .collect();

            if !missing.is_empty() {
                result.errors.push(SyncError::InterfaceNotSatisfied {
                    module: module.name.value.clone(),
                    interface: interface.name.value.clone(),
                    missing,
                    src: source.to_string(),
                    span: span_to_source_span(&claim.span),
                    declaration: span_to_source_span(&interface.name.span),
                });
            }
        }
    }
}

fn expand_module_instances(
    ast: &SourceFile,
    source: &str,
    result: &mut SyncResult,
) -> Vec<Definition> {
    let modules: HashMap<&str, &cypcb_parser::ast::ModuleDef> = ast
        .definitions
        .iter()
        .filter_map(|def| match def {
            Definition::Module(module) => Some((module.name.value.as_str(), module)),
            _ => None,
        })
        .collect();

    let mut out = Vec::with_capacity(ast.definitions.len());
    for def in &ast.definitions {
        let Definition::ModuleInstance(instance) = def else {
            out.push(def.clone());
            continue;
        };

        let mut chain = Vec::new();
        expand_one(
            instance,
            &modules,
            &Frame::identity(),
            "",
            &HashMap::new(),
            &mut chain,
            source,
            &mut out,
            result,
        );
    }

    out
}

/// Where an instance's coordinates sit on the board.
#[derive(Clone, Copy)]
struct Frame {
    origin: (i64, i64),
    angle_deg: f64,
}

impl Frame {
    fn identity() -> Self {
        Frame {
            origin: (0, 0),
            angle_deg: 0.0,
        }
    }

    /// Place a child frame inside this one.
    ///
    /// A nested instance's origin is stated in its parent's coordinates, so it
    /// has to be turned by the parent's angle before being added, and the
    /// angles accumulate.
    fn compose(&self, child: Frame) -> Frame {
        let (sin, cos) = self.angle_deg.to_radians().sin_cos();
        let x = child.origin.0 as f64;
        let y = child.origin.1 as f64;
        Frame {
            origin: (
                self.origin.0 + (x * cos - y * sin).round() as i64,
                self.origin.1 + (x * sin + y * cos).round() as i64,
            ),
            angle_deg: self.angle_deg + child.angle_deg,
        }
    }
}

/// Expand one instance, and anything it instantiates in turn.
///
/// `prefix` is what the enclosing instances have already contributed to every
/// name; `outer_nets` maps a net name in the enclosing scope to the name it
/// finally has, which is how a port two levels down still reaches the design's
/// own net. `chain` is the stack of modules currently being expanded, and is
/// what stops a module that instantiates itself from expanding for ever - no
/// depth limit is needed, because a chain without repeats cannot be longer
/// than the number of modules in the file.
#[allow(clippy::too_many_arguments)] // each argument is a distinct piece of scope
fn expand_one(
    instance: &cypcb_parser::ast::ModuleInstance,
    modules: &HashMap<&str, &cypcb_parser::ast::ModuleDef>,
    outer_frame: &Frame,
    prefix: &str,
    outer_nets: &HashMap<String, String>,
    chain: &mut Vec<String>,
    source: &str,
    out: &mut Vec<Definition>,
    result: &mut SyncResult,
) {
    let module_name = instance.module.value.as_str();

    let Some(module) = modules.get(module_name) else {
        result.errors.push(SyncError::UnknownModule {
            name: instance.module.value.clone(),
            src: source.to_string(),
            span: span_to_source_span(&instance.module.span),
        });
        return;
    };

    if chain.iter().any(|seen| seen == module_name) {
        let mut path = chain.clone();
        path.push(module_name.to_string());
        result.errors.push(SyncError::ModuleCycle {
            chain: path.join(" -> "),
            src: source.to_string(),
            span: span_to_source_span(&instance.span),
        });
        return;
    }
    chain.push(module_name.to_string());

    // Each port names a net in the enclosing scope, which may itself be a
    // port of the instance above. Resolving through `outer_nets` is what makes
    // a pin three levels down land on the design's own net.
    let resolve_outer = |name: &str| -> String {
        outer_nets
            .get(name)
            .cloned()
            .unwrap_or_else(|| name.to_string())
    };
    let ports: HashMap<&str, String> = instance
        .ports
        .iter()
        .map(|port| (port.pin.value.as_str(), resolve_outer(&port.net.value)))
        .collect();

    for pin in &module.pins {
        if !ports.contains_key(pin.name.value.as_str()) {
            result.errors.push(SyncError::UnconnectedModulePin {
                instance: instance.name.value.clone(),
                pin: pin.name.value.clone(),
                src: source.to_string(),
                span: span_to_source_span(&instance.span),
            });
        }
    }

    let scope = if prefix.is_empty() {
        instance.name.value.clone()
    } else {
        format!("{prefix}_{}", instance.name.value)
    };
    let local_name = |name: &str| -> String { format!("{scope}_{name}") };
    let net_name =
        |name: &str| -> String { ports.get(name).cloned().unwrap_or_else(|| local_name(name)) };

    let frame = outer_frame.compose(Frame {
        origin: instance
            .position
            .as_ref()
            .map(|at| (at.x.to_nm().raw(), at.y.to_nm().raw()))
            .unwrap_or((0, 0)),
        angle_deg: instance.rotation.as_ref().map(|r| r.angle).unwrap_or(0.0),
    });

    // What every net inside this module is finally called, for anything it
    // instantiates in turn.
    let inner_nets: HashMap<String, String> = module
        .definitions
        .iter()
        .filter_map(|def| match def {
            Definition::Net(net) => Some((net.name.value.clone(), net_name(&net.name.value))),
            _ => None,
        })
        .chain(
            module
                .pins
                .iter()
                .map(|pin| (pin.name.value.clone(), net_name(&pin.name.value))),
        )
        .collect();

    for inner in &module.definitions {
        match inner {
            Definition::Component(component) => {
                let mut copy = component.clone();
                copy.refdes.value = local_name(&component.refdes.value);
                place_in_instance(&mut copy, frame.origin, frame.angle_deg);
                out.push(Definition::Component(copy));
            }
            Definition::Net(net) => {
                let mut copy = net.clone();
                copy.name.value = net_name(&net.name.value);
                for connection in &mut copy.connections {
                    connection.component.value = local_name(&connection.component.value);
                }
                out.push(Definition::Net(copy));
            }
            Definition::ModuleInstance(nested) => {
                expand_one(
                    nested,
                    modules,
                    &frame,
                    &scope,
                    &inner_nets,
                    chain,
                    source,
                    out,
                    result,
                );
            }
            other => out.push(other.clone()),
        }
    }

    chain.pop();
}

/// Move a module's component into the instance's frame.
///
/// The module's own coordinates are relative to its origin, so a part at
/// `5mm, 5mm` in a module placed at `20mm, 10mm` belongs at `25mm, 15mm`. A
/// turned instance turns its parts about that origin and adds its angle to
/// each one, which is what makes a rotated block behave like the same block.
///
/// Positions are rewritten in millimetres because that is what the DSL's own
/// dimensions carry; the value is exact either way, since everything is held
/// in nanometres underneath.
fn place_in_instance(component: &mut ComponentDef, origin: (i64, i64), angle_deg: f64) {
    use cypcb_parser::ast::{Dimension as AstDimension, RotationExpr};

    if let Some(position) = &mut component.position {
        let x = position.x.to_nm().raw() as f64;
        let y = position.y.to_nm().raw() as f64;
        let (sin, cos) = angle_deg.to_radians().sin_cos();
        let rotated_x = x * cos - y * sin;
        let rotated_y = x * sin + y * cos;

        // A placement this code computed, not one the source wrote - so the
        // unit is stated rather than assumed, and nothing warns about it.
        position.x = AstDimension::new(
            (origin.0 as f64 + rotated_x) / 1_000_000.0,
            cypcb_core::Unit::Mm,
            position.x.span,
        );
        position.y = AstDimension::new(
            (origin.1 as f64 + rotated_y) / 1_000_000.0,
            cypcb_core::Unit::Mm,
            position.y.span,
        );
    }

    if angle_deg != 0.0 {
        let span = component
            .rotation
            .as_ref()
            .map(|r| r.span)
            .unwrap_or(component.span);
        let own = component.rotation.as_ref().map(|r| r.angle).unwrap_or(0.0);
        component.rotation = Some(RotationExpr {
            angle: own + angle_deg,
            span,
        });
    }
}

/// Give every net in a class the rule the class states.
///
/// A net that says nothing takes the class's answer whole. A net that says
/// something keeps its own for that field, which `sync_net` arranges by
/// merging rather than replacing.
fn sync_netclass(class: &cypcb_parser::ast::NetClassDef, world: &mut BoardWorld) {
    let Some(constraints) = &class.constraints else {
        return;
    };
    let carried = cypcb_world_net_constraints(constraints);
    if carried.is_empty() {
        return;
    }

    for member in &class.members {
        let net_id = world.intern_net(&member.value);
        world.set_net_constraints(net_id, carried);
    }
}

/// Put the design's outline on the board.
///
/// A ring of fewer than three points cannot enclose an area; rather than store
/// a line and call it a board, nothing is set and the rectangle stands.
fn sync_outline(outline: &cypcb_parser::ast::OutlineDef, world: &mut BoardWorld) {
    let Some(board) = world.board_entity() else {
        return;
    };
    let points: Vec<Point> = outline
        .points
        .iter()
        .map(|(x, y)| Point::new(x.to_nm(), y.to_nm()))
        .collect();

    if let Some(ring) = crate::components::BoardOutline::new(points) {
        world.ecs_mut().entity_mut(board).insert(ring);
    }
}
