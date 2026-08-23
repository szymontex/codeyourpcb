//! Tree-sitter to AST conversion.
//!
//! This module provides the [`CypcbParser`] which uses Tree-sitter to parse
//! source code and convert the resulting CST (Concrete Syntax Tree) into
//! typed AST nodes.
//!
//! # Example
//!
//! ```rust
//! use cypcb_parser::{CypcbParser, parse};
//!
//! let source = r#"
//! version 1
//! board test {
//!     size 30mm x 20mm
//!     layers 2
//! }
//! "#;
//!
//! let mut parser = CypcbParser::new();
//! let result = parser.parse(source);
//!
//! if result.is_ok() {
//!     let ast = result.value;
//!     println!("Parsed {} definitions", ast.definitions.len());
//! } else {
//!     for error in &result.errors {
//!         eprintln!("{:?}", error);
//!     }
//! }
//! ```

use crate::ast::{
    format_pad_number, AssertDef, AssertExpression, AssertOperand, BoardDef, ComparisonOp,
    ComponentDef, ComponentKind, CurrentUnit, CurrentValue, Definition, DiffPairDef, Dimension,
    EdgeConnectorDef, FootprintDef, Identifier, ImplementsClause, ImportDef, InterfaceDef,
    LayerType, ModuleDef, ModuleInstance, NeckDef, NetAssignment, NetClassDef, NetConstraints,
    NetDef, OutlineDef, PadDef, PadShape, PhysicalValue, PinDeclaration, PinId, PinRef,
    PortConnection, PositionExpr, RotationExpr, SilkDef, SizeProperty, SourceFile, Span,
    StackupDef, StackupLayer, StackupSheetDef, StringLit, Tolerance, ToleranceKind, TraceDef,
    TraceDirective, TracePath, TraceVia, ZoneDef, ZoneKind,
};
use crate::errors::{ParseError, ParseResult};
use crate::node_kinds;
use cypcb_core::{PhysicalUnit, Unit};
use tree_sitter::{Node, Parser, Tree};

/// Parser for CodeYourPCB source files.
///
/// Uses Tree-sitter for parsing and converts the resulting CST to typed AST nodes.
/// The parser supports error recovery, collecting errors while continuing to parse.
pub struct CypcbParser {
    parser: Parser,
}

impl CypcbParser {
    /// Create a new parser instance.
    ///
    /// # Panics
    ///
    /// Panics if the Tree-sitter language cannot be loaded.
    pub fn new() -> Self {
        let mut parser = Parser::new();
        parser
            .set_language(&crate::language())
            .expect("Failed to set cypcb language");
        CypcbParser { parser }
    }

    /// Parse source code and return the AST with any errors.
    ///
    /// The parser uses error recovery, so it will return a partial AST
    /// even if there are syntax errors.
    pub fn parse(&mut self, source: &str) -> ParseResult<SourceFile> {
        let tree = match self.parser.parse(source, None) {
            Some(t) => t,
            None => {
                return ParseResult::new(
                    SourceFile {
                        version: None,
                        definitions: Vec::new(),
                        span: Span::new(0, source.len()),
                    },
                    vec![ParseError::syntax(
                        "Failed to parse source",
                        source.to_string(),
                        (0, source.len().min(1)),
                    )],
                );
            }
        };

        let mut errors = Vec::new();
        let ast = self.convert_source_file(source, &tree, &mut errors);
        ParseResult::new(ast, errors)
    }

    /// Convert the root node to a SourceFile AST node.
    fn convert_source_file(
        &self,
        source: &str,
        tree: &Tree,
        errors: &mut Vec<ParseError>,
    ) -> SourceFile {
        let root = tree.root_node();
        let span = span_of(&root);

        // Collect errors from ERROR nodes
        self.collect_errors(source, &root, errors);

        let mut version = None;
        let mut definitions = Vec::new();

        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            match child.kind() {
                node_kinds::VERSION_STATEMENT => {
                    version = self.convert_version(source, &child, errors);
                }
                node_kinds::BOARD_DEFINITION => {
                    if let Some(board) = self.convert_board(source, &child, errors) {
                        definitions.push(Definition::Board(board));
                    }
                }
                node_kinds::COMPONENT_DEFINITION => {
                    if let Some(component) = self.convert_component(source, &child, errors) {
                        definitions.push(Definition::Component(component));
                    }
                }
                node_kinds::NET_DEFINITION => {
                    if let Some(net) = self.convert_net(source, &child, errors) {
                        definitions.push(Definition::Net(net));
                    }
                }
                "footprint_definition" => {
                    if let Some(fp) = self.convert_footprint_definition(source, &child, errors) {
                        definitions.push(Definition::Footprint(fp));
                    }
                }
                "zone_definition" => {
                    if let Some(zone) = self.convert_zone(source, &child, errors) {
                        definitions.push(Definition::Zone(zone));
                    }
                }
                "trace_definition" => {
                    if let Some(trace) = self.convert_trace_definition(source, &child, errors) {
                        definitions.push(Definition::Trace(trace));
                    }
                }
                "module_definition" => {
                    if let Some(module) = self.convert_module_definition(source, &child, errors) {
                        definitions.push(Definition::Module(module));
                    }
                }
                "module_instance" => {
                    if let Some(instance) = self.convert_module_instance(source, &child, errors) {
                        definitions.push(Definition::ModuleInstance(instance));
                    }
                }
                "outline_definition" => {
                    if let Some(outline) = self.convert_outline(source, &child, errors) {
                        definitions.push(Definition::Outline(outline));
                    }
                }
                "netclass_definition" => {
                    if let Some(class) = self.convert_netclass(source, &child, errors) {
                        definitions.push(Definition::NetClass(class));
                    }
                }
                "diffpair_definition" => {
                    if let Some(pair) = self.convert_diffpair_definition(source, &child) {
                        definitions.push(Definition::DiffPair(pair));
                    }
                }
                "interface_definition" => {
                    if let Some(iface) = self.convert_interface_definition(source, &child, errors) {
                        definitions.push(Definition::Interface(iface));
                    }
                }
                "import_statement" => {
                    if let Some(import) = self.convert_import_statement(source, &child, errors) {
                        definitions.push(Definition::Import(import));
                    }
                }
                "assert_statement" => {
                    if let Some(assert_def) = self.convert_assert_statement(source, &child, errors)
                    {
                        definitions.push(Definition::Assert(assert_def));
                    }
                }
                _ => {}
            }
        }

        SourceFile {
            version,
            definitions,
            span,
        }
    }

    /// Recursively collect ERROR nodes and report them.
    fn collect_errors(&self, source: &str, node: &Node, errors: &mut Vec<ParseError>) {
        if node.is_error() {
            let span = span_of(node);
            let text = node_text(source, node);
            errors.push(ParseError::syntax(
                format!(
                    "unexpected token: '{}'",
                    text.chars().take(20).collect::<String>()
                ),
                source.to_string(),
                span.to_miette(),
            ));
        }

        // Check children recursively
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.collect_errors(source, &child, errors);
        }
    }

    /// Convert a version statement node.
    fn convert_version(
        &self,
        source: &str,
        node: &Node,
        errors: &mut Vec<ParseError>,
    ) -> Option<u32> {
        let number_node = get_child_by_field(node, "number")?;
        let text = node_text(source, &number_node);
        match text.parse::<u32>() {
            Ok(v) => {
                if v == 0 {
                    errors.push(ParseError::invalid_version(
                        "version must be at least 1",
                        source.to_string(),
                        span_of(&number_node).to_miette(),
                    ));
                }
                Some(v)
            }
            Err(_) => {
                errors.push(ParseError::invalid_number(
                    text,
                    source.to_string(),
                    span_of(&number_node).to_miette(),
                ));
                None
            }
        }
    }

    /// Convert a board definition node.
    fn convert_board(
        &self,
        source: &str,
        node: &Node,
        errors: &mut Vec<ParseError>,
    ) -> Option<BoardDef> {
        let name_node = get_child_by_field(node, "name")?;
        let name = Identifier::new(node_text(source, &name_node), span_of(&name_node));

        let mut size = None;
        let mut layers = None;
        let mut stackup = None;
        let mut fab = None;

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            // board_property is a choice node, so we need to check both the wrapper
            // and the actual property types
            let property_node = if child.kind() == "board_property" {
                // Get the first named child which is the actual property
                child.named_child(0)
            } else {
                Some(child)
            };

            if let Some(prop) = property_node {
                match prop.kind() {
                    node_kinds::SIZE_PROPERTY => {
                        size = self.convert_size(source, &prop, errors);
                    }
                    node_kinds::LAYERS_PROPERTY => {
                        layers = self.convert_layers(source, &prop, errors);
                    }
                    "stackup_property" => {
                        stackup = self.convert_stackup(source, &prop, errors);
                    }
                    node_kinds::FAB_PROPERTY => {
                        if let Some(name_node) = get_child_by_field(&prop, "name") {
                            fab = Some(Identifier::new(
                                node_text(source, &name_node),
                                span_of(&name_node),
                            ));
                        }
                    }
                    _ => {}
                }
            }
        }

        Some(BoardDef {
            name,
            size,
            layers,
            stackup,
            fab,
            span: span_of(node),
        })
    }

    /// Convert a size property node.
    fn convert_size(
        &self,
        source: &str,
        node: &Node,
        errors: &mut Vec<ParseError>,
    ) -> Option<SizeProperty> {
        let width_node = get_child_by_field(node, "width")?;
        let height_node = get_child_by_field(node, "height")?;

        let width = self.convert_dimension(source, &width_node, errors)?;
        let height = self.convert_dimension(source, &height_node, errors)?;

        Some(SizeProperty {
            width,
            height,
            span: span_of(node),
        })
    }

    /// Convert a layers property node.
    fn convert_layers(
        &self,
        source: &str,
        node: &Node,
        errors: &mut Vec<ParseError>,
    ) -> Option<u8> {
        let count_node = get_child_by_field(node, "count")?;
        let text = node_text(source, &count_node);
        match text.parse::<u32>() {
            Ok(count) => {
                // Validate layer count (must be even, 2-32)
                if !(2..=32).contains(&count) || count % 2 != 0 {
                    errors.push(ParseError::invalid_layers(
                        count,
                        source.to_string(),
                        span_of(&count_node).to_miette(),
                    ));
                }
                Some(count as u8)
            }
            Err(_) => {
                errors.push(ParseError::invalid_number(
                    text,
                    source.to_string(),
                    span_of(&count_node).to_miette(),
                ));
                None
            }
        }
    }

    /// Convert a stackup property node.
    fn convert_stackup(
        &self,
        source: &str,
        node: &Node,
        errors: &mut Vec<ParseError>,
    ) -> Option<StackupDef> {
        let mut layers = Vec::new();
        let mut finish = None;
        let mut edges_plated = false;
        let mut castellated_pads = false;
        let mut edge_connector = None;
        let mut impedance_controlled = false;

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "stackup_layer" => {
                    if let Some(layer) = self.convert_stackup_layer(source, &child, errors) {
                        layers.push(layer);
                    }
                }
                // What the fabricator does to the board rather than what it
                // presses. The flags carry no field: the node being here is
                // the statement, the way `locked` on a trace is.
                "stackup_finish" => {
                    if let Some(text) = get_child_by_field(&child, "finish") {
                        finish = Some(self.convert_string_literal(source, &text));
                    }
                }
                "stackup_edges" => edges_plated = true,
                "stackup_pads" => castellated_pads = true,
                "stackup_connector" => {
                    edge_connector = match get_child_by_field(&child, "bevel")
                        .map(|bevel| node_text(source, &bevel).to_string())
                        .as_deref()
                    {
                        Some("bevelled") => Some(EdgeConnectorDef::Bevelled),
                        Some("plain") => Some(EdgeConnectorDef::Plain),
                        _ => edge_connector,
                    };
                }
                "stackup_impedance" => impedance_controlled = true,
                _ => {}
            }
        }

        Some(StackupDef {
            layers,
            finish,
            edges_plated,
            castellated_pads,
            edge_connector,
            impedance_controlled,
            span: span_of(node),
        })
    }

    /// Convert a stackup layer node.
    fn convert_stackup_layer(
        &self,
        source: &str,
        node: &Node,
        errors: &mut Vec<ParseError>,
    ) -> Option<StackupLayer> {
        let type_node = get_child_by_field(node, "layer_type")?;
        let type_text = node_text(source, &type_node);

        let layer_type = match LayerType::from_str(type_text) {
            Some(t) => t,
            None => {
                errors.push(ParseError::unknown_layer_type(
                    type_text,
                    source.to_string(),
                    span_of(&type_node).to_miette(),
                ));
                return None;
            }
        };

        let name =
            get_child_by_field(node, "name").map(|n| self.convert_string_literal(source, &n));

        // A copper layer's thickness may be stated in ounces per square foot,
        // which is how every fab table states it and how nobody's stackup
        // could until now. It is a different node, not a unit on `dimension`,
        // because it is a thickness of copper and of nothing else.
        let thickness = get_child_by_field(node, "thickness").and_then(|n| {
            if n.kind() == "copper_weight" {
                self.convert_copper_weight(source, &n, errors)
            } else {
                self.convert_dimension(source, &n, errors)
            }
        });

        let material =
            get_child_by_field(node, "material").map(|n| self.convert_string_literal(source, &n));

        let color =
            get_child_by_field(node, "color").map(|n| self.convert_string_literal(source, &n));

        // The rest of the sheets in this slot, in the order the design wrote
        // them: `addsublayer` in KiCad's file, `sheet` here.
        let mut sheets = Vec::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() != "stackup_sheet" {
                continue;
            }
            let sheet_number = |field: &str| {
                get_child_by_field(&child, field)
                    .and_then(|n| node_text(source, &n).parse::<f64>().ok())
                    .filter(|value| value.is_finite() && *value > 0.0)
            };
            sheets.push(StackupSheetDef {
                thickness: get_child_by_field(&child, "thickness")
                    .and_then(|n| self.convert_dimension(source, &n, errors)),
                material: get_child_by_field(&child, "material")
                    .map(|n| self.convert_string_literal(source, &n)),
                dk: sheet_number("dk"),
                df: sheet_number("df"),
                span: span_of(&child),
            });
        }

        let number = |field: &str| {
            get_child_by_field(node, field)
                .and_then(|n| node_text(source, &n).parse::<f64>().ok())
                .filter(|value| value.is_finite() && *value > 0.0)
        };

        Some(StackupLayer {
            layer_type,
            name,
            thickness,
            material,
            color,
            sheets,
            dk: number("dk"),
            df: number("df"),
            span: span_of(node),
        })
    }

    /// Convert a component definition node.
    fn convert_component(
        &self,
        source: &str,
        node: &Node,
        errors: &mut Vec<ParseError>,
    ) -> Option<ComponentDef> {
        let refdes_node = get_child_by_field(node, "refdes")?;
        let type_node = get_child_by_field(node, "type")?;
        let footprint_node = get_child_by_field(node, "footprint")?;

        let refdes = Identifier::new(node_text(source, &refdes_node), span_of(&refdes_node));
        let type_text = node_text(source, &type_node);

        let kind = match ComponentKind::from_str(type_text) {
            Some(k) => k,
            None => {
                errors.push(ParseError::unknown_component(
                    type_text,
                    source.to_string(),
                    span_of(&type_node).to_miette(),
                ));
                ComponentKind::Generic
            }
        };

        let footprint = self.convert_string_literal(source, &footprint_node);

        let mut value = None;
        let mut position = None;
        let mut rotation = None;
        let mut net_assignments = Vec::new();
        let mut typed_value = None;
        let mut lcsc = None;
        let mut spec = Vec::new();
        let mut side = None;

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                node_kinds::VALUE_PROPERTY => {
                    if let Some(val_node) = get_child_by_field(&child, "value") {
                        if val_node.kind() == "string" {
                            value = Some(self.convert_string_literal(source, &val_node));
                        } else {
                            // A typed value. Keep the text for anything that
                            // only wants to print it, and the quantity for
                            // anything that wants to check it.
                            let text = node_text(source, &val_node).to_string();
                            value = Some(StringLit::new(text, span_of(&val_node)));
                            typed_value = self.convert_physical_value(source, &val_node, errors);
                        }
                    }
                }
                node_kinds::POSITION_PROPERTY => {
                    position = self.convert_position(source, &child, errors);
                }
                node_kinds::ROTATION_PROPERTY => {
                    rotation = self.convert_rotation(source, &child, errors);
                }
                "spec_property" => {
                    let mut walk = child.walk();
                    for entry in child.children(&mut walk) {
                        if entry.kind() != "spec_entry" {
                            continue;
                        }
                        let name = get_child_by_field(&entry, "name");
                        let value = get_child_by_field(&entry, "value");
                        if let (Some(name), Some(value)) = (name, value) {
                            let name = Identifier::new(
                                node_text(source, &name).to_string(),
                                span_of(&name),
                            );
                            if let Some(value) = self.convert_physical_value(source, &value, errors)
                            {
                                spec.push(crate::ast::SpecEntry {
                                    name,
                                    value,
                                    span: span_of(&entry),
                                });
                            }
                        }
                    }
                }
                "lcsc_property" => {
                    if let Some(part) = get_child_by_field(&child, "part") {
                        lcsc = Some(self.convert_string_literal(source, &part));
                    }
                }
                // `side top` / `side bottom`. The rule is in `grammar.js`; the
                // generated `grammar/src/parser.c` in this repository predates
                // it and cannot be rebuilt without the tree-sitter CLI, so this
                // arm never fires until somebody regenerates it. It is written
                // now rather than left as a hole for the same reason the rule
                // was added to the grammar: the two parsers are supposed to
                // read the same language.
                "side_property" => {
                    if let Some(face) = get_child_by_field(&child, "face") {
                        side = Some(Identifier::new(node_text(source, &face), span_of(&face)));
                    }
                }
                "net_assignment" => {
                    if let Some(assignment) = self.convert_net_assignment(source, &child, errors) {
                        net_assignments.push(assignment);
                    }
                }
                _ => {}
            }
        }

        Some(ComponentDef {
            refdes,
            kind,
            lcsc,
            spec,
            side,
            footprint,
            value,
            typed_value,
            position,
            rotation,
            net_assignments,
            span: span_of(node),
        })
    }

    /// Convert a position property node.
    fn convert_position(
        &self,
        source: &str,
        node: &Node,
        errors: &mut Vec<ParseError>,
    ) -> Option<PositionExpr> {
        let x_node = get_child_by_field(node, "x")?;
        let y_node = get_child_by_field(node, "y")?;

        let x = self.convert_dimension(source, &x_node, errors)?;
        let y = self.convert_dimension(source, &y_node, errors)?;

        Some(PositionExpr {
            x,
            y,
            span: span_of(node),
        })
    }

    /// Convert a rotation property node.
    fn convert_rotation(
        &self,
        source: &str,
        node: &Node,
        errors: &mut Vec<ParseError>,
    ) -> Option<RotationExpr> {
        let angle_node = get_child_by_field(node, "angle")?;
        let text = node_text(source, &angle_node);

        let angle = match text.parse::<f64>() {
            Ok(a) => a,
            Err(_) => {
                errors.push(ParseError::invalid_number(
                    text,
                    source.to_string(),
                    span_of(&angle_node).to_miette(),
                ));
                return None;
            }
        };

        Some(RotationExpr {
            angle,
            span: span_of(node),
        })
    }

    /// Convert a net assignment node.
    fn convert_net_assignment(
        &self,
        source: &str,
        node: &Node,
        _errors: &mut Vec<ParseError>,
    ) -> Option<NetAssignment> {
        let pin_node = get_child_by_field(node, "pin")?;
        let net_node = get_child_by_field(node, "net")?;

        let pin = self.convert_pin_identifier(source, &pin_node);
        let net = net_name_of(source, &net_node);

        Some(NetAssignment {
            pin,
            net,
            span: span_of(node),
        })
    }

    /// Convert a net definition node.
    fn convert_net(
        &self,
        source: &str,
        node: &Node,
        errors: &mut Vec<ParseError>,
    ) -> Option<NetDef> {
        let name_node = get_child_by_field(node, "name")?;
        let name = net_name_of(source, &name_node);

        let mut constraints = None;
        let mut connections = Vec::new();

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "net_constraint_block" => {
                    constraints = self.convert_net_constraints(source, &child, errors);
                }
                "pin_ref_list" => {
                    connections = self.convert_pin_ref_list(source, &child, errors);
                }
                node_kinds::PIN_REF => {
                    if let Some(pin_ref) = self.convert_pin_ref(source, &child, errors) {
                        connections.push(pin_ref);
                    }
                }
                _ => {}
            }
        }

        Some(NetDef {
            name,
            constraints,
            connections,
            span: span_of(node),
        })
    }

    /// Convert a net constraints block.
    fn convert_net_constraints(
        &self,
        source: &str,
        node: &Node,
        errors: &mut Vec<ParseError>,
    ) -> Option<NetConstraints> {
        let mut width = None;
        let mut clearance = None;
        let mut current = None;
        let mut impedance_ohms = None;
        let mut neck = None;

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            // net_constraint is a choice node wrapping width_constraint, clearance_constraint, or current_constraint
            let constraint_node = if child.kind() == "net_constraint" {
                child.named_child(0)
            } else {
                Some(child)
            };

            if let Some(constraint) = constraint_node {
                match constraint.kind() {
                    "width_constraint" => {
                        if let Some(val_node) = get_child_by_field(&constraint, "value") {
                            width = self.convert_dimension(source, &val_node, errors);
                        }
                    }
                    "clearance_constraint" => {
                        if let Some(val_node) = get_child_by_field(&constraint, "value") {
                            clearance = self.convert_dimension(source, &val_node, errors);
                        }
                    }
                    "current_constraint" => {
                        if let Some(val_node) = get_child_by_field(&constraint, "value") {
                            current = self.convert_current_value(source, &val_node, errors);
                        }
                    }
                    "impedance_constraint" => {
                        if let Some(val_node) = get_child_by_field(&constraint, "value") {
                            impedance_ohms = node_text(source, &val_node)
                                .parse::<f64>()
                                .ok()
                                .filter(|value| value.is_finite() && *value > 0.0);
                        }
                    }
                    "neck_constraint" => {
                        let width_node = get_child_by_field(&constraint, "width");
                        let length_node = get_child_by_field(&constraint, "length");
                        if let (Some(width_node), Some(length_node)) = (width_node, length_node) {
                            let width = self.convert_dimension(source, &width_node, errors);
                            let length = self.convert_dimension(source, &length_node, errors);
                            if let (Some(width), Some(length)) = (width, length) {
                                neck = Some(NeckDef {
                                    width,
                                    length,
                                    span: span_of(&constraint),
                                });
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        Some(NetConstraints {
            width,
            clearance,
            current,
            impedance_ohms,
            neck,
            span: span_of(node),
        })
    }

    /// Convert a current value node (e.g., "500mA" or "2A").
    fn convert_current_value(
        &self,
        source: &str,
        node: &Node,
        errors: &mut Vec<ParseError>,
    ) -> Option<CurrentValue> {
        let amount_node = get_child_by_field(node, "amount")?;
        let text = node_text(source, &amount_node);

        let value = match text.parse::<f64>() {
            Ok(v) => v,
            Err(_) => {
                errors.push(ParseError::invalid_number(
                    text,
                    source.to_string(),
                    span_of(&amount_node).to_miette(),
                ));
                return None;
            }
        };

        let unit_node = get_child_by_field(node, "unit")?;
        let unit_text = node_text(source, &unit_node);

        let unit = match CurrentUnit::from_str(unit_text) {
            Some(u) => u,
            None => {
                errors.push(ParseError::unknown_unit(
                    unit_text,
                    source.to_string(),
                    span_of(&unit_node).to_miette(),
                ));
                return None;
            }
        };

        Some(CurrentValue::new(value, unit, span_of(node)))
    }

    /// Convert a pin reference list.
    fn convert_pin_ref_list(
        &self,
        source: &str,
        node: &Node,
        errors: &mut Vec<ParseError>,
    ) -> Vec<PinRef> {
        let mut refs = Vec::new();

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == node_kinds::PIN_REF {
                if let Some(pin_ref) = self.convert_pin_ref(source, &child, errors) {
                    refs.push(pin_ref);
                }
            }
        }

        refs
    }

    /// Convert a pin reference node.
    fn convert_pin_ref(
        &self,
        source: &str,
        node: &Node,
        _errors: &mut Vec<ParseError>,
    ) -> Option<PinRef> {
        let component_node = get_child_by_field(node, "component")?;
        let pin_node = get_child_by_field(node, "pin")?;

        let component =
            Identifier::new(node_text(source, &component_node), span_of(&component_node));
        let pin = self.convert_pin_identifier(source, &pin_node);

        Some(PinRef {
            component,
            pin,
            span: span_of(node),
        })
    }

    /// Convert a pin identifier (number or name).
    fn convert_pin_identifier(&self, source: &str, node: &Node) -> PinId {
        let text = node_text(source, node);
        // Try to parse as number first
        if let Ok(n) = text.parse::<u32>() {
            PinId::Number(n)
        } else {
            PinId::Name(text.to_string())
        }
    }

    /// Convert a `1oz` copper weight into the thickness it names.
    ///
    /// The conversion is `cypcb_core`'s `NM_PER_OZ`, which the IPC-2221 trace
    /// width calculation reads as well - one number in one place, because two
    /// copies is how the thickness a trace is priced on drifts from the
    /// thickness the board is built with.
    fn convert_copper_weight(
        &self,
        source: &str,
        node: &Node,
        errors: &mut Vec<ParseError>,
    ) -> Option<Dimension> {
        let value_node = get_child_by_field(node, "value")?;
        let text = node_text(source, &value_node);
        match text.parse::<f64>() {
            Ok(value) => Some(Dimension::new(value, Unit::Oz, span_of(node))),
            Err(_) => {
                errors.push(ParseError::invalid_number(
                    text,
                    source.to_string(),
                    span_of(&value_node).to_miette(),
                ));
                None
            }
        }
    }

    /// Convert a dimension node.
    fn convert_dimension(
        &self,
        source: &str,
        node: &Node,
        errors: &mut Vec<ParseError>,
    ) -> Option<Dimension> {
        // Check for negative sign
        let is_negative = get_child_by_field(node, "sign").is_some();

        let value_node = get_child_by_field(node, "value")?;
        let text = node_text(source, &value_node);

        let value = match text.parse::<f64>() {
            Ok(v) => {
                if is_negative {
                    -v
                } else {
                    v
                }
            }
            Err(_) => {
                errors.push(ParseError::invalid_number(
                    text,
                    source.to_string(),
                    span_of(&value_node).to_miette(),
                ));
                return None;
            }
        };

        let unit_present = get_child_by_field(node, "unit").is_some();
        let unit = if let Some(unit_node) = get_child_by_field(node, "unit") {
            let unit_text = node_text(source, &unit_node);
            match unit_text.parse::<Unit>() {
                Ok(u) => u,
                Err(_) => {
                    errors.push(ParseError::unknown_unit(
                        unit_text,
                        source.to_string(),
                        span_of(&unit_node).to_miette(),
                    ));
                    Unit::Mm // Default to mm
                }
            }
        } else {
            Unit::Mm // Default unit
        };

        Some(if unit_present {
            Dimension::new(value, unit, span_of(node))
        } else {
            Dimension::implied_mm(value, span_of(node))
        })
    }

    /// Convert a string literal node (extracts value without quotes).
    fn convert_string_literal(&self, source: &str, node: &Node) -> StringLit {
        let full_text = node_text(source, node);
        // Strip quotes
        let value = full_text.trim_matches('"').to_string();
        StringLit::new(value, span_of(node))
    }

    /// Convert a footprint definition node.
    fn convert_footprint_definition(
        &self,
        source: &str,
        node: &Node,
        errors: &mut Vec<ParseError>,
    ) -> Option<FootprintDef> {
        let name_node = get_child_by_field(node, "name")?;
        let name = Identifier::new(node_text(source, &name_node), span_of(&name_node));

        let mut description: Option<String> = None;
        let mut pads: Vec<PadDef> = Vec::new();
        let mut courtyard: Option<(Dimension, Dimension)> = None;
        let mut silk: Vec<SilkDef> = Vec::new();

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            // footprint_property is a choice node
            let property_node = if child.kind() == "footprint_property" {
                child.named_child(0)
            } else {
                Some(child)
            };

            if let Some(prop) = property_node {
                match prop.kind() {
                    "description_property" => {
                        if let Some(text_node) = get_child_by_field(&prop, "text") {
                            let lit = self.convert_string_literal(source, &text_node);
                            description = Some(lit.value);
                        }
                    }
                    "pad_definition" => {
                        if let Some(pad) = self.convert_pad_definition(source, &prop, errors) {
                            pads.push(pad);
                        }
                    }
                    "courtyard_property" => {
                        let width = get_child_by_field(&prop, "width")
                            .and_then(|n| self.convert_dimension(source, &n, errors));
                        let height = get_child_by_field(&prop, "height")
                            .and_then(|n| self.convert_dimension(source, &n, errors));

                        if let (Some(w), Some(h)) = (width, height) {
                            courtyard = Some((w, h));
                        }
                    }
                    "silk_line" => {
                        let dim = |field: &str, errors: &mut Vec<ParseError>| {
                            get_child_by_field(&prop, field)
                                .and_then(|n| self.convert_dimension(source, &n, errors))
                        };
                        let x1 = dim("x1", errors);
                        let y1 = dim("y1", errors);
                        let x2 = dim("x2", errors);
                        let y2 = dim("y2", errors);
                        let width = dim("width", errors);
                        if let (Some(x1), Some(y1), Some(x2), Some(y2)) = (x1, y1, x2, y2) {
                            silk.push(SilkDef::Line {
                                start: (x1, y1),
                                end: (x2, y2),
                                width,
                                span: span_of(&prop),
                            });
                        }
                    }
                    "silk_circle" => {
                        let dim = |field: &str, errors: &mut Vec<ParseError>| {
                            get_child_by_field(&prop, field)
                                .and_then(|n| self.convert_dimension(source, &n, errors))
                        };
                        let cx = dim("cx", errors);
                        let cy = dim("cy", errors);
                        let radius = dim("radius", errors);
                        let width = dim("width", errors);
                        if let (Some(cx), Some(cy), Some(radius)) = (cx, cy, radius) {
                            silk.push(SilkDef::Circle {
                                centre: (cx, cy),
                                radius,
                                width,
                                span: span_of(&prop),
                            });
                        }
                    }
                    _ => {}
                }
            }
        }

        Some(FootprintDef {
            name,
            description,
            pads,
            courtyard,
            silk,
            span: span_of(node),
        })
    }

    /// Convert a pad definition node.
    fn convert_pad_definition(
        &self,
        source: &str,
        node: &Node,
        errors: &mut Vec<ParseError>,
    ) -> Option<PadDef> {
        let number_node = get_child_by_field(node, "number")?;
        let number_text = node_text(source, &number_node);
        // Three forms reach here: a bare number, a bare identifier, and a
        // quoted string. Only the first needs converting, and it keeps the
        // form it was written in - a pad called 1 must not become "1.0", or a
        // net's `R1.1` stops finding the pad it names.
        let number = if number_node.kind() == "string" {
            number_text.trim_matches('"').to_string()
        } else if let Ok(value) = number_text.parse::<f64>() {
            format_pad_number(value)
        } else {
            number_text.to_string()
        };

        let shape_node = get_child_by_field(node, "shape")?;
        let shape_text = node_text(source, &shape_node);
        let shape = match PadShape::from_str(shape_text) {
            Some(s) => s,
            None => {
                errors.push(ParseError::syntax(
                    format!("unknown pad shape: '{}'", shape_text),
                    source.to_string(),
                    span_of(&shape_node).to_miette(),
                ));
                return None;
            }
        };

        let x = get_child_by_field(node, "x")
            .and_then(|n| self.convert_dimension(source, &n, errors))?;
        let y = get_child_by_field(node, "y")
            .and_then(|n| self.convert_dimension(source, &n, errors))?;
        let width = get_child_by_field(node, "width")
            .and_then(|n| self.convert_dimension(source, &n, errors))?;
        let height = get_child_by_field(node, "height")
            .and_then(|n| self.convert_dimension(source, &n, errors))?;

        // Optional drill spec. One dimension is a round hole; two are a slot,
        // milled along its length.
        let drill_spec = get_child_by_field(node, "drill");
        let drill = drill_spec
            .as_ref()
            .and_then(|spec| get_child_by_field(spec, "width"))
            .and_then(|n| self.convert_dimension(source, &n, errors));
        let drill_height = drill_spec
            .as_ref()
            .and_then(|spec| get_child_by_field(spec, "height"))
            .and_then(|n| self.convert_dimension(source, &n, errors));

        Some(PadDef {
            number,
            shape,
            x,
            y,
            width,
            height,
            drill,
            drill_height,
            span: span_of(node),
        })
    }

    /// Convert a zone definition node.
    fn convert_zone(
        &self,
        source: &str,
        node: &Node,
        errors: &mut Vec<ParseError>,
    ) -> Option<ZoneDef> {
        let kind_node = get_child_by_field(node, "kind")?;
        let kind_text = node_text(source, &kind_node);
        let kind = ZoneKind::from_str(kind_text)?;

        let name = get_child_by_field(node, "name")
            .map(|n| Identifier::new(node_text(source, &n), span_of(&n)));

        let mut bounds: Option<(Dimension, Dimension, Dimension, Dimension)> = None;
        let mut layer: Option<String> = None;
        let mut net: Option<Identifier> = None;

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            let property_node = if child.kind() == "zone_property" {
                child.named_child(0)
            } else {
                Some(child)
            };

            if let Some(prop) = property_node {
                match prop.kind() {
                    "zone_bounds" => {
                        let min_x = get_child_by_field(&prop, "min_x")
                            .and_then(|n| self.convert_dimension(source, &n, errors));
                        let min_y = get_child_by_field(&prop, "min_y")
                            .and_then(|n| self.convert_dimension(source, &n, errors));
                        let max_x = get_child_by_field(&prop, "max_x")
                            .and_then(|n| self.convert_dimension(source, &n, errors));
                        let max_y = get_child_by_field(&prop, "max_y")
                            .and_then(|n| self.convert_dimension(source, &n, errors));

                        if let (Some(x1), Some(y1), Some(x2), Some(y2)) =
                            (min_x, min_y, max_x, max_y)
                        {
                            bounds = Some((x1, y1, x2, y2));
                        }
                    }
                    "zone_layer" => {
                        if let Some(layer_node) = get_child_by_field(&prop, "name") {
                            layer = Some(node_text(source, &layer_node).to_string());
                        }
                    }
                    "zone_net" => {
                        if let Some(net_node) = get_child_by_field(&prop, "net") {
                            // Through `net_name_of`, which takes the quotes off
                            // a quoted name. Reading the node text raw would
                            // give a net called `"VBUS+"` with the quotation
                            // marks in the name.
                            net = Some(net_name_of(source, &net_node));
                        }
                    }
                    _ => {}
                }
            }
        }

        // Bounds are required
        let bounds = bounds?;

        Some(ZoneDef {
            kind,
            name,
            bounds,
            layer,
            net,
            span: span_of(node),
        })
    }

    /// Convert a trace definition node.
    fn convert_trace_definition(
        &self,
        source: &str,
        node: &Node,
        errors: &mut Vec<ParseError>,
    ) -> Option<TraceDef> {
        let net_node = get_child_by_field(node, "net")?;
        let net = net_name_of(source, &net_node);

        let mut from: Option<PinRef> = None;
        let mut to: Option<PinRef> = None;
        let mut waypoints: Vec<PositionExpr> = Vec::new();
        let mut layer: Option<String> = None;
        let mut width: Option<Dimension> = None;
        let mut locked = false;
        let mut neck = None;
        let mut directives: Vec<TraceDirective> = Vec::new();

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "trace_from" => {
                    if let Some(pin_node) = get_child_by_field(&child, "pin") {
                        from = self.convert_pin_ref(source, &pin_node, errors);
                    }
                }
                "trace_to" => {
                    if let Some(pin_node) = get_child_by_field(&child, "pin") {
                        to = self.convert_pin_ref(source, &pin_node, errors);
                    }
                }
                "trace_via" => {
                    let x = get_child_by_field(&child, "x")
                        .and_then(|n| self.convert_dimension(source, &n, errors));
                    let y = get_child_by_field(&child, "y")
                        .and_then(|n| self.convert_dimension(source, &n, errors));
                    let drill = get_child_by_field(&child, "drill")
                        .and_then(|n| self.convert_dimension(source, &n, errors));
                    let layers = match (
                        get_child_by_field(&child, "start_layer"),
                        get_child_by_field(&child, "end_layer"),
                    ) {
                        (Some(start), Some(end)) => Some((
                            node_text(source, &start).to_string(),
                            node_text(source, &end).to_string(),
                        )),
                        _ => None,
                    };

                    if let (Some(x), Some(y)) = (x, y) {
                        let position = PositionExpr {
                            x: x.clone(),
                            y: y.clone(),
                            span: span_of(&child),
                        };
                        // Add to waypoints for backward compat (logical mode)
                        waypoints.push(PositionExpr {
                            x,
                            y,
                            span: span_of(&child),
                        });
                        // Also add as directive (geometric mode)
                        directives.push(TraceDirective::Via(TraceVia {
                            position,
                            drill,
                            layers,
                            span: span_of(&child),
                        }));
                    }
                }
                "trace_path" => {
                    let mut points: Vec<PositionExpr> = Vec::new();
                    let mut path_cursor = child.walk();
                    for path_child in child.children(&mut path_cursor) {
                        if path_child.kind() == "path_point" {
                            let x = get_child_by_field(&path_child, "x")
                                .and_then(|n| self.convert_dimension(source, &n, errors));
                            let y = get_child_by_field(&path_child, "y")
                                .and_then(|n| self.convert_dimension(source, &n, errors));
                            if let (Some(x), Some(y)) = (x, y) {
                                points.push(PositionExpr {
                                    x,
                                    y,
                                    span: span_of(&path_child),
                                });
                            }
                        }
                    }
                    if !points.is_empty() {
                        directives.push(TraceDirective::Path(TracePath {
                            points,
                            span: span_of(&child),
                        }));
                    }
                }
                "trace_layer" => {
                    if let Some(name_node) = get_child_by_field(&child, "name") {
                        let name = node_text(source, &name_node).to_string();
                        layer = Some(name.clone());
                        // Also add as directive (geometric mode)
                        directives.push(TraceDirective::Layer(name));
                    }
                }
                "trace_width" => {
                    if let Some(val_node) = get_child_by_field(&child, "value") {
                        width = self.convert_dimension(source, &val_node, errors);
                    }
                }
                "trace_neck" => {
                    let width = get_child_by_field(&child, "width")
                        .and_then(|n| self.convert_dimension(source, &n, errors));
                    let length = get_child_by_field(&child, "length")
                        .and_then(|n| self.convert_dimension(source, &n, errors));
                    if let Some((width, length)) = width.zip(length) {
                        neck = Some(crate::ast::NeckDef {
                            width,
                            length,
                            span: span_of(&child),
                        });
                    }
                }
                "trace_locked" => {
                    locked = true;
                }
                _ => {}
            }
        }

        Some(TraceDef {
            net,
            from,
            to,
            waypoints,
            layer,
            width,
            locked,
            neck,
            directives,
            span: span_of(node),
        })
    }

    // ========================================================================
    // DSL v2 converters
    // ========================================================================

    /// Convert an import statement node.
    fn convert_import_statement(
        &self,
        source: &str,
        node: &Node,
        _errors: &mut Vec<ParseError>,
    ) -> Option<ImportDef> {
        let path_node = get_child_by_field(node, "path")?;
        let path = self.convert_string_literal(source, &path_node);

        let mut names = Vec::new();
        if let Some(names_node) = get_child_by_field(node, "names") {
            let mut cursor = names_node.walk();
            for child in names_node.children(&mut cursor) {
                if child.kind() == "identifier" {
                    names.push(Identifier::new(node_text(source, &child), span_of(&child)));
                }
            }
        }

        Some(ImportDef {
            names,
            path,
            span: span_of(node),
        })
    }

    /// Convert a board outline node.
    fn convert_outline(
        &self,
        source: &str,
        node: &Node,
        errors: &mut Vec<ParseError>,
    ) -> Option<OutlineDef> {
        let mut points = Vec::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() != "outline_point" {
                continue;
            }
            let (Some(x_node), Some(y_node)) = (
                get_child_by_field(&child, "x"),
                get_child_by_field(&child, "y"),
            ) else {
                continue;
            };
            let (Some(x), Some(y)) = (
                self.convert_dimension(source, &x_node, errors),
                self.convert_dimension(source, &y_node, errors),
            ) else {
                continue;
            };
            points.push((x, y));
        }

        Some(OutlineDef {
            points,
            span: span_of(node),
        })
    }

    /// Convert a net class node.
    fn convert_netclass(
        &self,
        source: &str,
        node: &Node,
        errors: &mut Vec<ParseError>,
    ) -> Option<NetClassDef> {
        let name_node = get_child_by_field(node, "name")?;
        let name = Identifier::new(node_text(source, &name_node), span_of(&name_node));

        let mut constraints = None;
        let mut members = Vec::new();

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "net_constraint_block" => {
                    constraints = self.convert_net_constraints(source, &child, errors);
                }
                // Members are `net_name` nodes and the class name is a plain
                // identifier, so the two no longer need telling apart by span
                // - this used to walk every identifier under the block and
                // skip whichever one matched the name it had already taken.
                "net_name" => {
                    members.push(net_name_of(source, &child));
                }
                _ => {}
            }
        }

        Some(NetClassDef {
            name,
            constraints,
            members,
            span: span_of(node),
        })
    }

    /// Convert a module instantiation node.
    fn convert_module_instance(
        &self,
        source: &str,
        node: &Node,
        errors: &mut Vec<ParseError>,
    ) -> Option<ModuleInstance> {
        let module = get_child_by_field(node, "module")?;
        let name = get_child_by_field(node, "name")?;

        let mut ports = Vec::new();
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            if child.kind() != "port_connection" {
                continue;
            }
            let (Some(pin), Some(net)) = (
                get_child_by_field(&child, "pin"),
                get_child_by_field(&child, "net"),
            ) else {
                continue;
            };
            ports.push(PortConnection {
                pin: Identifier::new(node_text(source, &pin), span_of(&pin)),
                net: Identifier::new(node_text(source, &net), span_of(&net)),
                span: span_of(&child),
            });
        }

        Some(ModuleInstance {
            module: Identifier::new(node_text(source, &module), span_of(&module)),
            name: Identifier::new(node_text(source, &name), span_of(&name)),
            position: get_child_by_field(node, "position")
                .and_then(|n| self.convert_position(source, &n, errors)),
            rotation: get_child_by_field(node, "rotation")
                .and_then(|n| self.convert_rotation(source, &n, errors)),
            ports,
            span: span_of(node),
        })
    }

    /// Convert a module definition node.
    fn convert_module_definition(
        &self,
        source: &str,
        node: &Node,
        errors: &mut Vec<ParseError>,
    ) -> Option<ModuleDef> {
        let name_node = get_child_by_field(node, "name")?;
        let name = Identifier::new(node_text(source, &name_node), span_of(&name_node));

        let mut definitions = Vec::new();
        let mut pins = Vec::new();
        let mut implements = Vec::new();

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "component_definition" => {
                    if let Some(comp) = self.convert_component(source, &child, errors) {
                        definitions.push(Definition::Component(comp));
                    }
                }
                "implements_clause" => {
                    if let Some(clause) = self.convert_implements_clause(source, &child) {
                        implements.push(clause);
                    }
                }
                "net_definition" => {
                    if let Some(net) = self.convert_net(source, &child, errors) {
                        definitions.push(Definition::Net(net));
                    }
                }
                "pin_declaration" => {
                    if let Some(pin) = self.convert_pin_declaration(source, &child, errors) {
                        pins.push(pin);
                    }
                }
                "assert_statement" => {
                    if let Some(assert_def) = self.convert_assert_statement(source, &child, errors)
                    {
                        definitions.push(Definition::Assert(assert_def));
                    }
                }
                "module_instance" => {
                    if let Some(instance) = self.convert_module_instance(source, &child, errors) {
                        definitions.push(Definition::ModuleInstance(instance));
                    }
                }
                _ => {}
            }
        }

        Some(ModuleDef {
            name,
            definitions,
            pins,
            implements,
            span: span_of(node),
        })
    }

    /// Convert a `diffpair Name { P N }` definition.
    fn convert_diffpair_definition(&self, source: &str, node: &Node) -> Option<DiffPairDef> {
        let name_node = get_child_by_field(node, "name")?;
        let positive = get_child_by_field(node, "positive")?;
        let negative = get_child_by_field(node, "negative")?;
        Some(DiffPairDef {
            name: Identifier::new(node_text(source, &name_node), span_of(&name_node)),
            positive: net_name_of(source, &positive),
            negative: net_name_of(source, &negative),
            span: span_of(node),
        })
    }

    /// Convert an `implements Name` clause inside a module.
    fn convert_implements_clause(&self, source: &str, node: &Node) -> Option<ImplementsClause> {
        let name_node = get_child_by_field(node, "interface")?;
        Some(ImplementsClause {
            interface: Identifier::new(node_text(source, &name_node), span_of(&name_node)),
            span: span_of(node),
        })
    }

    /// Convert an interface definition node.
    fn convert_interface_definition(
        &self,
        source: &str,
        node: &Node,
        errors: &mut Vec<ParseError>,
    ) -> Option<InterfaceDef> {
        let name_node = get_child_by_field(node, "name")?;
        let name = Identifier::new(node_text(source, &name_node), span_of(&name_node));

        let mut pins = Vec::new();

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "pin_declaration" {
                if let Some(pin) = self.convert_pin_declaration(source, &child, errors) {
                    pins.push(pin);
                }
            }
        }

        Some(InterfaceDef {
            name,
            pins,
            span: span_of(node),
        })
    }

    /// Convert a pin declaration node.
    fn convert_pin_declaration(
        &self,
        source: &str,
        node: &Node,
        _errors: &mut Vec<ParseError>,
    ) -> Option<PinDeclaration> {
        let name_node = get_child_by_field(node, "name")?;
        let name = Identifier::new(node_text(source, &name_node), span_of(&name_node));

        Some(PinDeclaration {
            name,
            span: span_of(node),
        })
    }

    /// Convert an assert statement node.
    fn convert_assert_statement(
        &self,
        source: &str,
        node: &Node,
        errors: &mut Vec<ParseError>,
    ) -> Option<AssertDef> {
        let expr_node = get_child_by_field(node, "expression")?;
        let expression = self.convert_assert_expression(source, &expr_node, errors)?;

        Some(AssertDef {
            expression,
            span: span_of(node),
        })
    }

    /// Convert an assert expression node.
    fn convert_assert_expression(
        &self,
        source: &str,
        node: &Node,
        errors: &mut Vec<ParseError>,
    ) -> Option<AssertExpression> {
        match node.kind() {
            "assert_comparison" => {
                let left_node = get_child_by_field(node, "left")?;
                let op_node = get_child_by_field(node, "op")?;
                let right_node = get_child_by_field(node, "right")?;

                let left = self.convert_assert_operand(source, &left_node, errors)?;
                let op_text = node_text(source, &op_node);
                let op = ComparisonOp::from_str(op_text).unwrap_or_else(|| {
                    errors.push(ParseError::invalid_assert(
                        format!("unknown comparison operator: '{}'", op_text),
                        source.to_string(),
                        span_of(&op_node).to_miette(),
                    ));
                    ComparisonOp::Eq
                });
                let right = self.convert_assert_operand(source, &right_node, errors)?;

                Some(AssertExpression::Comparison {
                    left,
                    op,
                    right,
                    span: span_of(node),
                })
            }
            "assert_within" => {
                let left_node = get_child_by_field(node, "left")?;
                let target_node = get_child_by_field(node, "target")?;

                let left = self.convert_assert_operand(source, &left_node, errors)?;
                let target = self.convert_physical_value(source, &target_node, errors)?;

                Some(AssertExpression::Within {
                    left,
                    target,
                    span: span_of(node),
                })
            }
            "assert_expression" => {
                // The assert_expression is a choice node, get the actual child
                if let Some(child) = node.named_child(0) {
                    self.convert_assert_expression(source, &child, errors)
                } else {
                    None
                }
            }
            _ => {
                errors.push(ParseError::invalid_assert(
                    format!("unexpected assert expression kind: '{}'", node.kind()),
                    source.to_string(),
                    span_of(node).to_miette(),
                ));
                None
            }
        }
    }

    /// Convert an assert operand node.
    fn convert_assert_operand(
        &self,
        source: &str,
        node: &Node,
        errors: &mut Vec<ParseError>,
    ) -> Option<AssertOperand> {
        // assert_operand is a choice node — inspect the actual child
        let actual = if node.kind() == "assert_operand" {
            node.named_child(0)?
        } else {
            *node
        };

        match actual.kind() {
            "qualified_name" => {
                let mut parts = Vec::new();
                let mut cursor = actual.walk();
                for child in actual.children(&mut cursor) {
                    if child.kind() == "identifier" {
                        parts.push(node_text(source, &child).to_string());
                    }
                }
                Some(AssertOperand::QualifiedName {
                    parts,
                    span: span_of(&actual),
                })
            }
            "physical_value" => {
                let pv = self.convert_physical_value(source, &actual, errors)?;
                Some(AssertOperand::Physical(pv))
            }
            "dimension" => {
                let dim = self.convert_dimension(source, &actual, errors)?;
                Some(AssertOperand::Dimension(dim))
            }
            "number" => {
                let text = node_text(source, &actual);
                let value = text.parse::<f64>().ok()?;
                Some(AssertOperand::Number {
                    value,
                    span: span_of(&actual),
                })
            }
            _ => {
                errors.push(ParseError::invalid_assert(
                    format!("unexpected operand kind: '{}'", actual.kind()),
                    source.to_string(),
                    span_of(&actual).to_miette(),
                ));
                None
            }
        }
    }

    /// Convert a physical value node (e.g., `10kohm`, `3.3V +/- 5%`).
    fn convert_physical_value(
        &self,
        source: &str,
        node: &Node,
        errors: &mut Vec<ParseError>,
    ) -> Option<PhysicalValue> {
        let value_node = get_child_by_field(node, "value")?;
        let text = node_text(source, &value_node);
        let value = match text.parse::<f64>() {
            Ok(v) => v,
            Err(_) => {
                errors.push(ParseError::invalid_number(
                    text,
                    source.to_string(),
                    span_of(&value_node).to_miette(),
                ));
                return None;
            }
        };

        let unit_node = get_child_by_field(node, "unit")?;
        let unit_text = node_text(source, &unit_node);
        let unit = match unit_text.parse::<PhysicalUnit>() {
            Ok(u) => u,
            Err(_) => {
                errors.push(ParseError::invalid_physical_unit(
                    unit_text,
                    source.to_string(),
                    span_of(&unit_node).to_miette(),
                ));
                return None;
            }
        };

        let tolerance = get_child_by_field(node, "tolerance")
            .and_then(|tol_node| self.convert_tolerance(source, &tol_node, errors));

        Some(PhysicalValue {
            value,
            unit,
            tolerance,
            span: span_of(node),
        })
    }

    /// Convert a tolerance node.
    fn convert_tolerance(
        &self,
        source: &str,
        node: &Node,
        errors: &mut Vec<ParseError>,
    ) -> Option<Tolerance> {
        // tolerance is a choice of tolerance_plus_minus or tolerance_range
        let actual = if node.kind() == "tolerance" {
            node.named_child(0)?
        } else {
            *node
        };

        match actual.kind() {
            "tolerance_plus_minus" => {
                let val_node = get_child_by_field(&actual, "value")?;
                let text = node_text(source, &val_node);
                let tol_value = match text.parse::<f64>() {
                    Ok(v) => v,
                    Err(_) => {
                        errors.push(ParseError::invalid_number(
                            text,
                            source.to_string(),
                            span_of(&val_node).to_miette(),
                        ));
                        return None;
                    }
                };

                let kind_node = get_child_by_field(&actual, "kind")?;
                let kind_text = node_text(source, &kind_node);

                let kind = if kind_text == "%" {
                    ToleranceKind::Percentage { value: tol_value }
                } else {
                    // It's a physical_unit — resolve to typed PhysicalUnit
                    let tol_unit = match kind_text.parse::<PhysicalUnit>() {
                        Ok(u) => u,
                        Err(_) => {
                            errors.push(ParseError::invalid_physical_unit(
                                kind_text,
                                source.to_string(),
                                span_of(&kind_node).to_miette(),
                            ));
                            return None;
                        }
                    };
                    ToleranceKind::Absolute(Box::new(PhysicalValue {
                        value: tol_value,
                        unit: tol_unit,
                        tolerance: None,
                        span: Span::new(val_node.start_byte(), kind_node.end_byte()),
                    }))
                };

                Some(Tolerance {
                    kind,
                    span: span_of(&actual),
                })
            }
            "tolerance_range" => {
                // The `upper` field wraps seq(number, physical_unit).
                // Both children have the field name "upper", so iterate
                // all children of the tolerance_range to find them.
                let mut upper_value = 0.0;
                let mut upper_unit: Option<PhysicalUnit> = None;
                let mut upper_span = span_of(&actual);
                let mut cursor = actual.walk();
                for child in actual.children(&mut cursor) {
                    match child.kind() {
                        "number" => {
                            let text = node_text(source, &child);
                            upper_value = text.parse::<f64>().unwrap_or(0.0);
                            upper_span = Span::new(child.start_byte(), upper_span.end);
                        }
                        "physical_unit" => {
                            let unit_text = node_text(source, &child);
                            match unit_text.parse::<PhysicalUnit>() {
                                Ok(u) => {
                                    upper_unit = Some(u);
                                    upper_span = Span::new(upper_span.start, child.end_byte());
                                }
                                Err(_) => {
                                    errors.push(ParseError::invalid_physical_unit(
                                        unit_text,
                                        source.to_string(),
                                        span_of(&child).to_miette(),
                                    ));
                                    return None;
                                }
                            }
                        }
                        _ => {}
                    }
                }

                let upper_unit = upper_unit?;

                Some(Tolerance {
                    kind: ToleranceKind::Range(Box::new(PhysicalValue {
                        value: upper_value,
                        unit: upper_unit,
                        tolerance: None,
                        span: upper_span,
                    })),
                    span: span_of(&actual),
                })
            }
            _ => {
                errors.push(ParseError::invalid_tolerance(
                    format!("unexpected tolerance kind: '{}'", actual.kind()),
                    source.to_string(),
                    span_of(&actual).to_miette(),
                ));
                None
            }
        }
    }
}

impl Default for CypcbParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper: get the text of a node from the source.
fn node_text<'a>(source: &'a str, node: &Node) -> &'a str {
    &source[node.start_byte()..node.end_byte()]
}

/// Helper: get a child node by field name.
/// A net's name as the model holds it, whichever way the design spelled it.
///
/// `net_name` wraps either an identifier or a string, so the text of the node
/// itself carries the quotes when a design wrote `net "VBUS+"`. Stripping them
/// here keeps every caller writing the same two lines it wrote before.
fn net_name_of(source: &str, node: &Node) -> Identifier {
    let text = node_text(source, node);
    let unquoted = if text.starts_with('"') && text.ends_with('"') && text.len() >= 2 {
        &text[1..text.len() - 1]
    } else {
        text
    };
    Identifier::new(unquoted, span_of(node))
}

fn get_child_by_field<'a>(node: &'a Node, name: &str) -> Option<Node<'a>> {
    node.child_by_field_name(name)
}

/// Helper: convert a tree-sitter node to our Span type.
fn span_of(node: &Node) -> Span {
    Span::new(node.start_byte(), node.end_byte())
}

/// Convenience function to parse source code.
///
/// # Example
///
/// ```rust
/// use cypcb_parser::parse;
///
/// let result = parse("version 1\nboard test { size 10mm x 10mm }");
/// assert!(result.is_ok());
/// ```
pub fn parse(source: &str) -> ParseResult<SourceFile> {
    let mut parser = CypcbParser::new();
    parser.parse(source)
}

#[cfg(test)]
mod tests {

    #[test]
    fn an_outline_block_parses_into_points() {
        let source = "version 1\n\nboard b {\n    size 40mm x 40mm\n    layers 2\n}\n\n\
                      outline {\n    point 0mm, 0mm\n    point 40mm, 0mm\n    point 40mm, 20mm\n\
                      \x20   point 20mm, 20mm\n    point 20mm, 40mm\n    point 0mm, 40mm\n}\n";
        let parsed = parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);

        let outline = parsed
            .value
            .definitions
            .iter()
            .find_map(|d| match d {
                Definition::Outline(outline) => Some(outline),
                _ => None,
            })
            .expect("the outline block has to reach the AST");

        assert_eq!(outline.points.len(), 6);
        assert_eq!(outline.points[2].0.to_nm(), cypcb_core::Nm::from_mm(40.0));
        assert_eq!(outline.points[2].1.to_nm(), cypcb_core::Nm::from_mm(20.0));
    }
    use super::*;

    #[test]
    fn test_parse_board() {
        let source = r#"
version 1

board test {
    size 100mm x 50mm
    layers 2
}
"#;
        let mut parser = CypcbParser::new();
        let result = parser.parse(source);
        assert!(result.is_ok(), "errors: {:?}", result.errors);

        let ast = result.value;
        assert_eq!(ast.version, Some(1));
        assert_eq!(ast.definitions.len(), 1);

        if let Definition::Board(board) = &ast.definitions[0] {
            assert_eq!(board.name.value, "test");
            let size = board.size.as_ref().expect("size should be present");
            assert!((size.width.value - 100.0).abs() < 0.001);
            assert_eq!(size.width.unit, Unit::Mm);
            assert!((size.height.value - 50.0).abs() < 0.001);
            assert_eq!(board.layers, Some(2));
        } else {
            panic!("expected board definition");
        }
    }

    #[test]
    fn test_parse_component() {
        let source = r#"
component R1 resistor "0402" {
    value "10k"
    at 10mm, 8mm
    rotate 90
}
"#;
        let result = parse(source);
        assert!(result.is_ok(), "errors: {:?}", result.errors);

        let ast = result.value;
        assert_eq!(ast.definitions.len(), 1);

        if let Definition::Component(comp) = &ast.definitions[0] {
            assert_eq!(comp.refdes.value, "R1");
            assert_eq!(comp.kind, ComponentKind::Resistor);
            assert_eq!(comp.footprint.value, "0402");
            assert_eq!(
                comp.value.as_ref().map(|v| &v.value),
                Some(&"10k".to_string())
            );

            let pos = comp.position.as_ref().expect("position should be present");
            assert!((pos.x.value - 10.0).abs() < 0.001);
            assert!((pos.y.value - 8.0).abs() < 0.001);

            let rot = comp.rotation.as_ref().expect("rotation should be present");
            assert!((rot.angle - 90.0).abs() < 0.001);
        } else {
            panic!("expected component definition");
        }
    }

    #[test]
    fn test_parse_net() {
        let source = r#"
net VCC {
    J1.1
    R1.1
    U1.VCC
}
"#;
        let result = parse(source);
        assert!(result.is_ok(), "errors: {:?}", result.errors);

        let ast = result.value;
        assert_eq!(ast.definitions.len(), 1);

        if let Definition::Net(net) = &ast.definitions[0] {
            assert_eq!(net.name.value, "VCC");
            assert_eq!(net.connections.len(), 3);

            assert_eq!(net.connections[0].component.value, "J1");
            assert!(matches!(net.connections[0].pin, PinId::Number(1)));

            assert_eq!(net.connections[1].component.value, "R1");
            assert!(matches!(net.connections[1].pin, PinId::Number(1)));

            assert_eq!(net.connections[2].component.value, "U1");
            if let PinId::Name(name) = &net.connections[2].pin {
                assert_eq!(name, "VCC");
            } else {
                panic!("expected named pin");
            }
        } else {
            panic!("expected net definition");
        }
    }

    #[test]
    fn test_parse_net_with_constraints() {
        // Grammar uses space-separated constraints, not comma-separated
        let source = r#"
net POWER [width 0.5mm clearance 0.3mm] {
    J1.1
    U1.VIN
}
"#;
        let result = parse(source);
        assert!(result.is_ok(), "errors: {:?}", result.errors);

        let ast = result.value;
        if let Definition::Net(net) = &ast.definitions[0] {
            assert_eq!(net.name.value, "POWER");
            let constraints = net
                .constraints
                .as_ref()
                .expect("constraints should be present");
            let width = constraints.width.as_ref().expect("width should be present");
            assert!((width.value - 0.5).abs() < 0.001);
            let clearance = constraints
                .clearance
                .as_ref()
                .expect("clearance should be present");
            assert!((clearance.value - 0.3).abs() < 0.001);
        } else {
            panic!("expected net definition");
        }
    }

    #[test]
    fn test_error_recovery() {
        // Invalid syntax: wrong token in component definition
        let source = r#"
board test {
    size 10mm x 10mm
}
component R1 resistor { }
"#;
        let result = parse(source);

        // The component is missing the required footprint string
        // Tree-sitter will mark this as an error
        assert!(
            result.has_errors(),
            "expected errors: got AST: {:?}",
            result.value
        );
    }

    #[test]
    fn test_span_accuracy() {
        let source = "version 1";
        let result = parse(source);

        assert!(result.is_ok());
        let ast = result.value;

        // The entire source should be covered
        assert_eq!(ast.span.start, 0);
        assert_eq!(ast.span.end, source.len());
    }

    #[test]
    fn test_syntax_error_unknown_type() {
        // Note: The grammar has a fixed set of component types (resistor, capacitor, etc.)
        // An unknown type like "badtype" will cause a Tree-sitter syntax error,
        // not an UnknownComponent error from our converter.
        let source = r#"
component X1 badtype "0402" {
    at 0mm, 0mm
}
"#;
        let result = parse(source);

        // Should have a syntax error because "badtype" isn't a valid component_type
        assert!(
            result.has_errors(),
            "expected syntax error for unknown type"
        );
    }

    #[test]
    fn test_complete_example() {
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

component J1 connector "pin_header_1x2" {
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
        let result = parse(source);
        assert!(result.is_ok(), "errors: {:?}", result.errors);

        let ast = result.value;
        assert_eq!(ast.version, Some(1));
        // 1 board + 3 components + 3 nets = 7 definitions
        assert_eq!(ast.definitions.len(), 7);

        // Verify JSON serialization
        let json = serde_json::to_string_pretty(&ast).expect("should serialize");
        println!("JSON output:\n{}", json);
        assert!(
            json.contains("\"version\": 1") || json.contains("\"version\":1"),
            "expected version:1 in {}",
            json
        );
        assert!(json.contains("blink"), "expected blink in {}", json);
    }

    #[test]
    fn test_multiple_errors() {
        let source = r#"
board test { size 10mm x 10mm layers -1 }
component X1 badtype "fp" { at 0mm, 0mm }
"#;
        let result = parse(source);

        // Should have multiple errors
        assert!(result.has_errors());
        // Parsing should still produce partial results
        assert!(!result.value.definitions.is_empty());
    }

    #[test]
    fn test_default_units() {
        let source = r#"
board test {
    size 100 x 50
}
"#;
        let result = parse(source);
        // Unitless dimensions default to mm
        if let Definition::Board(board) = &result.value.definitions[0] {
            let size = board.size.as_ref().expect("size should be present");
            assert_eq!(size.width.unit, Unit::Mm);
            assert_eq!(size.height.unit, Unit::Mm);
        }
    }

    #[test]
    fn test_all_units() {
        let source = r#"
board test {
    size 100mm x 50mm
}
component R1 resistor "0402" {
    at 50mil, 25mil
}
"#;
        let result = parse(source);
        assert!(result.is_ok(), "errors: {:?}", result.errors);

        if let Definition::Board(board) = &result.value.definitions[0] {
            let size = board.size.as_ref().unwrap();
            assert_eq!(size.width.unit, Unit::Mm);
        }

        if let Definition::Component(comp) = &result.value.definitions[1] {
            let pos = comp.position.as_ref().unwrap();
            assert_eq!(pos.x.unit, Unit::Mil);
            assert_eq!(pos.y.unit, Unit::Mil);
        }
    }

    #[test]
    fn test_all_component_types() {
        let types = [
            "resistor",
            "capacitor",
            "inductor",
            "ic",
            "led",
            "connector",
            "diode",
            "transistor",
            "crystal",
            "generic",
        ];

        for comp_type in types {
            let source = format!(r#"component X1 {} "fp" {{ at 0mm, 0mm }}"#, comp_type);
            let result = parse(&source);
            assert!(
                result.is_ok(),
                "failed to parse component type '{}': {:?}",
                comp_type,
                result.errors
            );
        }
    }

    #[test]
    fn test_pin_ref_numeric_and_named() {
        let source = r#"
net TEST {
    U1.1
    U1.VCC
    U1.123
    U1.PIN_A
}
"#;
        let result = parse(source);
        assert!(result.is_ok(), "errors: {:?}", result.errors);

        if let Definition::Net(net) = &result.value.definitions[0] {
            assert_eq!(net.connections.len(), 4);

            // Pin 1 (numeric)
            assert!(matches!(net.connections[0].pin, PinId::Number(1)));

            // VCC (named)
            if let PinId::Name(name) = &net.connections[1].pin {
                assert_eq!(name, "VCC");
            } else {
                panic!("expected named pin VCC");
            }

            // 123 (numeric)
            assert!(matches!(net.connections[2].pin, PinId::Number(123)));

            // PIN_A (named)
            if let PinId::Name(name) = &net.connections[3].pin {
                assert_eq!(name, "PIN_A");
            } else {
                panic!("expected named pin PIN_A");
            }
        }
    }

    #[test]
    fn test_version_only() {
        let source = "version 1";
        let result = parse(source);
        assert!(result.is_ok());
        assert_eq!(result.value.version, Some(1));
        assert!(result.value.definitions.is_empty());
    }

    #[test]
    fn test_no_version() {
        let source = r#"
board test {
    size 10mm x 10mm
}
"#;
        let result = parse(source);
        assert!(result.is_ok());
        assert_eq!(result.value.version, None);
        assert_eq!(result.value.definitions.len(), 1);
    }

    #[test]
    fn test_decimal_dimensions() {
        let source = r#"
board test {
    size 25.4mm x 12.7mm
}
component R1 resistor "0402" {
    at 1.5mm, 0.75mm
    rotate 45.5
}
"#;
        let result = parse(source);
        assert!(result.is_ok(), "errors: {:?}", result.errors);

        if let Definition::Board(board) = &result.value.definitions[0] {
            let size = board.size.as_ref().unwrap();
            assert!((size.width.value - 25.4).abs() < 0.001);
            assert!((size.height.value - 12.7).abs() < 0.001);
        }

        if let Definition::Component(comp) = &result.value.definitions[1] {
            let pos = comp.position.as_ref().unwrap();
            assert!((pos.x.value - 1.5).abs() < 0.001);
            assert!((pos.y.value - 0.75).abs() < 0.001);

            let rot = comp.rotation.as_ref().unwrap();
            assert!((rot.angle - 45.5).abs() < 0.001);
        }
    }

    #[test]
    fn test_comments_preserved() {
        let source = r#"
// This is a comment
version 1

/* Block comment */
board test {
    size 10mm x 10mm // inline comment
}
"#;
        let result = parse(source);
        assert!(result.is_ok(), "errors: {:?}", result.errors);
        assert_eq!(result.value.version, Some(1));
    }

    #[test]
    fn a_pour_can_be_poured_to_a_quoted_net() {
        // `zone_net` took `$.identifier` and takes `net_name` now, so the two
        // readers agree about a net called `VBUS+`. This is the tree-sitter
        // side of that: the node it reads is a `net_name` wrapping a string,
        // and reading its text raw would put the quotation marks in the name.
        let source = r#"
zone power {
    bounds 1mm, 1mm to 39mm, 19mm
    layer top
    net "VBUS+"
}
"#;
        let result = parse(source);
        assert!(result.is_ok(), "errors: {:?}", result.errors);

        let Definition::Zone(zone) = &result.value.definitions[0] else {
            panic!("expected zone definition");
        };
        assert_eq!(zone.kind, crate::ast::ZoneKind::CopperPour);
        assert_eq!(
            zone.net.as_ref().map(|n| n.value.as_str()),
            Some("VBUS+"),
            "the name comes back without its quotes"
        );
    }

    #[test]
    fn test_parse_keepout_zone() {
        let source = r#"
keepout antenna_clearance {
    bounds 10mm, 10mm to 20mm, 20mm
    layer top
}
"#;
        let result = parse(source);
        assert!(result.is_ok(), "errors: {:?}", result.errors);

        let ast = result.value;
        assert_eq!(ast.definitions.len(), 1);

        if let Definition::Zone(zone) = &ast.definitions[0] {
            assert_eq!(zone.kind, crate::ast::ZoneKind::Keepout);
            assert_eq!(
                zone.name.as_ref().map(|n| &n.value),
                Some(&"antenna_clearance".to_string())
            );
            assert!((zone.bounds.0.value - 10.0).abs() < 0.001); // min_x
            assert!((zone.bounds.1.value - 10.0).abs() < 0.001); // min_y
            assert!((zone.bounds.2.value - 20.0).abs() < 0.001); // max_x
            assert!((zone.bounds.3.value - 20.0).abs() < 0.001); // max_y
            assert_eq!(zone.layer.as_deref(), Some("top"));
            assert!(zone.net.is_none());
        } else {
            panic!("expected zone definition");
        }
    }

    #[test]
    fn test_parse_copper_pour_zone() {
        let source = r#"
zone gnd_pour {
    bounds 0mm, 0mm to 50mm, 50mm
    layer bottom
    net GND
}
"#;
        let result = parse(source);
        assert!(result.is_ok(), "errors: {:?}", result.errors);

        let ast = result.value;
        assert_eq!(ast.definitions.len(), 1);

        if let Definition::Zone(zone) = &ast.definitions[0] {
            assert_eq!(zone.kind, crate::ast::ZoneKind::CopperPour);
            assert_eq!(
                zone.name.as_ref().map(|n| &n.value),
                Some(&"gnd_pour".to_string())
            );
            assert_eq!(zone.layer.as_deref(), Some("bottom"));
            assert_eq!(
                zone.net.as_ref().map(|n| &n.value),
                Some(&"GND".to_string())
            );
        } else {
            panic!("expected zone definition");
        }
    }

    #[test]
    fn test_parse_keepout_all_layers() {
        let source = r#"
keepout mechanical_clearance {
    bounds 5mm, 5mm to 10mm, 10mm
    layer all
}
"#;
        let result = parse(source);
        assert!(result.is_ok(), "errors: {:?}", result.errors);

        if let Definition::Zone(zone) = &result.value.definitions[0] {
            assert_eq!(zone.layer.as_deref(), Some("all"));
        } else {
            panic!("expected zone definition");
        }
    }

    #[test]
    fn test_parse_anonymous_keepout() {
        let source = r#"
keepout {
    bounds 0mm, 0mm to 5mm, 5mm
}
"#;
        let result = parse(source);
        assert!(result.is_ok(), "errors: {:?}", result.errors);

        if let Definition::Zone(zone) = &result.value.definitions[0] {
            assert_eq!(zone.kind, crate::ast::ZoneKind::Keepout);
            assert!(zone.name.is_none());
            assert!(zone.layer.is_none()); // Defaults to all layers
        } else {
            panic!("expected zone definition");
        }
    }

    #[test]
    fn test_parse_footprint_definition() {
        let source = r#"
footprint MY_PKG {
    description "Test package"
    pad 1 rect at 0mm, 0mm size 1mm x 0.5mm
    pad 2 rect at 2mm, 0mm size 1mm x 0.5mm
    courtyard 4mm x 2mm
}
"#;
        let result = parse(source);
        assert!(result.is_ok(), "errors: {:?}", result.errors);

        let ast = result.value;
        assert_eq!(ast.definitions.len(), 1);

        if let Definition::Footprint(fp) = &ast.definitions[0] {
            assert_eq!(fp.name.value, "MY_PKG");
            assert_eq!(fp.description, Some("Test package".to_string()));
            assert_eq!(fp.pads.len(), 2);

            // Check pad 1
            let pad1 = &fp.pads[0];
            // A pad's name is a string since `pad <name>` shipped: a USB-C
            // receptacle calls one A1. These two lines still compared it with
            // an integer, so this whole test target had not compiled since -
            // which is why nothing in the tree-sitter reader was under test.
            assert_eq!(pad1.number, "1");
            assert_eq!(pad1.shape, PadShape::Rect);
            assert!((pad1.x.value - 0.0).abs() < 0.001);
            assert!((pad1.y.value - 0.0).abs() < 0.001);
            assert!((pad1.width.value - 1.0).abs() < 0.001);
            assert!((pad1.height.value - 0.5).abs() < 0.001);
            assert!(pad1.drill.is_none());

            // Check pad 2
            let pad2 = &fp.pads[1];
            assert_eq!(pad2.number, "2");
            assert!((pad2.x.value - 2.0).abs() < 0.001);

            // Check courtyard
            let (cy_w, cy_h) = fp.courtyard.as_ref().expect("courtyard should be present");
            assert!((cy_w.value - 4.0).abs() < 0.001);
            assert!((cy_h.value - 2.0).abs() < 0.001);
        } else {
            panic!("expected footprint definition");
        }
    }

    #[test]
    fn test_parse_footprint_with_drill() {
        let source = r#"
footprint THT_2PIN {
    pad 1 circle at 0mm, 0mm size 1.8mm x 1.8mm drill 1.0mm
    pad 2 circle at 2.54mm, 0mm size 1.8mm x 1.8mm drill 1.0mm
}
"#;
        let result = parse(source);
        assert!(result.is_ok(), "errors: {:?}", result.errors);

        if let Definition::Footprint(fp) = &result.value.definitions[0] {
            assert_eq!(fp.name.value, "THT_2PIN");
            assert_eq!(fp.pads.len(), 2);
            assert!(fp.description.is_none());
            assert!(fp.courtyard.is_none());

            // Check pad with drill
            let pad1 = &fp.pads[0];
            assert_eq!(pad1.shape, PadShape::Circle);
            let drill = pad1.drill.as_ref().expect("drill should be present");
            assert!((drill.value - 1.0).abs() < 0.001);
            assert_eq!(drill.unit, Unit::Mm);
        } else {
            panic!("expected footprint definition");
        }
    }

    #[test]
    fn test_parse_footprint_all_pad_shapes() {
        let source = r#"
footprint ALL_SHAPES {
    pad 1 rect at 0mm, 0mm size 1mm x 1mm
    pad 2 circle at 2mm, 0mm size 1mm x 1mm
    pad 3 roundrect at 4mm, 0mm size 1mm x 1mm
    pad 4 oblong at 6mm, 0mm size 1mm x 2mm
}
"#;
        let result = parse(source);
        assert!(result.is_ok(), "errors: {:?}", result.errors);

        if let Definition::Footprint(fp) = &result.value.definitions[0] {
            assert_eq!(fp.pads.len(), 4);
            assert_eq!(fp.pads[0].shape, PadShape::Rect);
            assert_eq!(fp.pads[1].shape, PadShape::Circle);
            assert_eq!(fp.pads[2].shape, PadShape::RoundRect);
            assert_eq!(fp.pads[3].shape, PadShape::Oblong);
        } else {
            panic!("expected footprint definition");
        }
    }

    #[test]
    fn test_parse_net_with_current_constraint() {
        let source = r#"
net POWER [width 0.5mm clearance 0.3mm current 500mA] {
    J1.1
    U1.VIN
}
"#;
        let result = parse(source);
        assert!(result.is_ok(), "errors: {:?}", result.errors);

        if let Definition::Net(net) = &result.value.definitions[0] {
            assert_eq!(net.name.value, "POWER");
            let constraints = net
                .constraints
                .as_ref()
                .expect("constraints should be present");

            // Check width
            let width = constraints.width.as_ref().expect("width should be present");
            assert!((width.value - 0.5).abs() < 0.001);

            // Check clearance
            let clearance = constraints
                .clearance
                .as_ref()
                .expect("clearance should be present");
            assert!((clearance.value - 0.3).abs() < 0.001);

            // Check current
            let current = constraints
                .current
                .as_ref()
                .expect("current should be present");
            assert!((current.value - 500.0).abs() < 0.001);
            assert_eq!(current.unit, crate::ast::CurrentUnit::Milliamps);
            assert!((current.to_milliamps() - 500.0).abs() < 0.001);
        } else {
            panic!("expected net definition");
        }
    }

    #[test]
    fn test_parse_net_with_current_in_amps() {
        let source = r#"
net HIGH_POWER [current 2A] {
    J1.1
}
"#;
        let result = parse(source);
        assert!(result.is_ok(), "errors: {:?}", result.errors);

        if let Definition::Net(net) = &result.value.definitions[0] {
            let constraints = net
                .constraints
                .as_ref()
                .expect("constraints should be present");
            let current = constraints
                .current
                .as_ref()
                .expect("current should be present");
            assert!((current.value - 2.0).abs() < 0.001);
            assert_eq!(current.unit, crate::ast::CurrentUnit::Amps);
            assert!((current.to_amps() - 2.0).abs() < 0.001);
            assert!((current.to_milliamps() - 2000.0).abs() < 0.001);
        } else {
            panic!("expected net definition");
        }
    }

    #[test]
    fn test_parse_simple_trace() {
        let source = r#"
trace VCC {
    from R1.1
    to C1.1
    layer Top
    width 0.3mm
}
"#;
        let result = parse(source);
        assert!(result.is_ok(), "errors: {:?}", result.errors);

        let ast = result.value;
        assert_eq!(ast.definitions.len(), 1);

        if let Definition::Trace(trace) = &ast.definitions[0] {
            assert_eq!(trace.net.value, "VCC");

            // Check from pin
            let from = trace.from.as_ref().expect("from should be present");
            assert_eq!(from.component.value, "R1");
            assert!(matches!(from.pin, PinId::Number(1)));

            // Check to pin
            let to = trace.to.as_ref().expect("to should be present");
            assert_eq!(to.component.value, "C1");
            assert!(matches!(to.pin, PinId::Number(1)));

            // Check layer
            assert_eq!(trace.layer.as_deref(), Some("Top"));

            // Check width
            let width = trace.width.as_ref().expect("width should be present");
            assert!((width.value - 0.3).abs() < 0.001);

            // Check not locked by default
            assert!(!trace.locked);

            // No waypoints
            assert!(trace.waypoints.is_empty());
        } else {
            panic!("expected trace definition");
        }
    }

    #[test]
    fn test_parse_trace_with_waypoints() {
        let source = r#"
trace GND {
    from R1.2
    to LED1.cathode
    via 5mm, 8mm
    via 10mm, 8mm
    layer Bottom
}
"#;
        let result = parse(source);
        assert!(result.is_ok(), "errors: {:?}", result.errors);

        if let Definition::Trace(trace) = &result.value.definitions[0] {
            assert_eq!(trace.net.value, "GND");
            assert_eq!(trace.waypoints.len(), 2);

            // First waypoint
            let wp1 = &trace.waypoints[0];
            assert!((wp1.x.value - 5.0).abs() < 0.001);
            assert!((wp1.y.value - 8.0).abs() < 0.001);

            // Second waypoint
            let wp2 = &trace.waypoints[1];
            assert!((wp2.x.value - 10.0).abs() < 0.001);
            assert!((wp2.y.value - 8.0).abs() < 0.001);

            assert_eq!(trace.layer.as_deref(), Some("Bottom"));
        } else {
            panic!("expected trace definition");
        }
    }

    #[test]
    fn test_parse_locked_trace() {
        let source = r#"
trace SENSITIVE {
    from U1.1
    to U2.1
    locked
}
"#;
        let result = parse(source);
        assert!(result.is_ok(), "errors: {:?}", result.errors);

        if let Definition::Trace(trace) = &result.value.definitions[0] {
            assert_eq!(trace.net.value, "SENSITIVE");
            assert!(trace.locked);
        } else {
            panic!("expected trace definition");
        }
    }

    #[test]
    fn test_parse_trace_named_pins() {
        let source = r#"
trace LED_SIGNAL {
    from R1.2
    to LED1.anode
    layer Top
}
"#;
        let result = parse(source);
        assert!(result.is_ok(), "errors: {:?}", result.errors);

        if let Definition::Trace(trace) = &result.value.definitions[0] {
            let to = trace.to.as_ref().expect("to should be present");
            if let PinId::Name(name) = &to.pin {
                assert_eq!(name, "anode");
            } else {
                panic!("expected named pin");
            }
        } else {
            panic!("expected trace definition");
        }
    }

    #[test]
    fn test_parse_trace_minimal() {
        let source = r#"
trace NET1 {
}
"#;
        let result = parse(source);
        assert!(result.is_ok(), "errors: {:?}", result.errors);

        if let Definition::Trace(trace) = &result.value.definitions[0] {
            assert_eq!(trace.net.value, "NET1");
            assert!(trace.from.is_none());
            assert!(trace.to.is_none());
            assert!(trace.waypoints.is_empty());
            assert!(trace.layer.is_none());
            assert!(trace.width.is_none());
            assert!(!trace.locked);
        } else {
            panic!("expected trace definition");
        }
    }

    // ========================================================================
    // DSL v2 forward tests
    // ========================================================================

    #[test]
    fn test_parse_import_bare() {
        let source = r#"import "std/interfaces.cypcb""#;
        let result = parse(source);
        assert!(result.is_ok(), "errors: {:?}", result.errors);

        if let Definition::Import(imp) = &result.value.definitions[0] {
            assert!(imp.names.is_empty(), "bare import should have no names");
            assert_eq!(imp.path.value, "std/interfaces.cypcb");
        } else {
            panic!(
                "expected import definition, got {:?}",
                result.value.definitions[0]
            );
        }
    }

    #[test]
    fn test_parse_import_single_name() {
        let source = r#"import I2C from "std/interfaces.cypcb""#;
        let result = parse(source);
        assert!(result.is_ok(), "errors: {:?}", result.errors);

        if let Definition::Import(imp) = &result.value.definitions[0] {
            assert_eq!(imp.names.len(), 1);
            assert_eq!(imp.names[0].value, "I2C");
            assert_eq!(imp.path.value, "std/interfaces.cypcb");
        } else {
            panic!("expected import definition");
        }
    }

    #[test]
    fn test_parse_import_multiple_names() {
        let source = r#"import I2C, SPI, UART from "std/interfaces.cypcb""#;
        let result = parse(source);
        assert!(result.is_ok(), "errors: {:?}", result.errors);

        if let Definition::Import(imp) = &result.value.definitions[0] {
            assert_eq!(imp.names.len(), 3);
            assert_eq!(imp.names[0].value, "I2C");
            assert_eq!(imp.names[1].value, "SPI");
            assert_eq!(imp.names[2].value, "UART");
        } else {
            panic!("expected import definition");
        }
    }

    #[test]
    fn test_parse_module_with_components() {
        let source = r#"
module PowerSupply {
    component U1 ic "SOT-23" {
        value "LDO-3V3"
        at 0mm, 0mm
    }
    component C1 capacitor "0402" {
        value "100nF"
    }

    pin VIN
    pin VOUT
    pin GND

    net input {
        U1.1
        C1.1
    }
}
"#;
        let result = parse(source);
        assert!(result.is_ok(), "errors: {:?}", result.errors);

        if let Definition::Module(module) = &result.value.definitions[0] {
            assert_eq!(module.name.value, "PowerSupply");
            assert_eq!(module.pins.len(), 3);
            assert_eq!(module.pins[0].name.value, "VIN");
            assert_eq!(module.pins[1].name.value, "VOUT");
            assert_eq!(module.pins[2].name.value, "GND");

            // 2 components + 1 net = 3 definitions
            assert_eq!(module.definitions.len(), 3);
            assert!(matches!(module.definitions[0], Definition::Component(_)));
            assert!(matches!(module.definitions[1], Definition::Component(_)));
            assert!(matches!(module.definitions[2], Definition::Net(_)));
        } else {
            panic!("expected module definition");
        }
    }

    #[test]
    fn test_parse_interface() {
        let source = r#"
interface I2C {
    pin SDA
    pin SCL
}
"#;
        let result = parse(source);
        assert!(result.is_ok(), "errors: {:?}", result.errors);

        if let Definition::Interface(iface) = &result.value.definitions[0] {
            assert_eq!(iface.name.value, "I2C");
            assert_eq!(iface.pins.len(), 2);
            assert_eq!(iface.pins[0].name.value, "SDA");
            assert_eq!(iface.pins[1].name.value, "SCL");
        } else {
            panic!("expected interface definition");
        }
    }

    #[test]
    fn test_parse_interface_power() {
        let source = r#"
interface Power {
    pin VCC
    pin GND
}
"#;
        let result = parse(source);
        assert!(result.is_ok(), "errors: {:?}", result.errors);

        if let Definition::Interface(iface) = &result.value.definitions[0] {
            assert_eq!(iface.name.value, "Power");
            assert_eq!(iface.pins.len(), 2);
        } else {
            panic!("expected interface definition");
        }
    }

    #[test]
    fn test_parse_assert_comparison_ge() {
        let source = r#"assert R1.value >= 10kohm"#;
        let result = parse(source);
        assert!(result.is_ok(), "errors: {:?}", result.errors);

        if let Definition::Assert(assert_def) = &result.value.definitions[0] {
            if let AssertExpression::Comparison {
                left, op, right, ..
            } = &assert_def.expression
            {
                assert_eq!(*op, ComparisonOp::Ge);
                if let AssertOperand::QualifiedName { parts, .. } = left {
                    assert_eq!(parts, &["R1", "value"]);
                } else {
                    panic!("expected qualified name, got {:?}", left);
                }
                if let AssertOperand::Physical(pv) = right {
                    assert!((pv.value - 10.0).abs() < 0.001);
                    assert_eq!(pv.unit, PhysicalUnit::KiloOhm);
                } else {
                    panic!("expected physical value, got {:?}", right);
                }
            } else {
                panic!("expected comparison");
            }
        } else {
            panic!("expected assert definition");
        }
    }

    #[test]
    fn test_parse_assert_comparison_eq() {
        let source = r#"assert board.layers == 4"#;
        let result = parse(source);
        assert!(result.is_ok(), "errors: {:?}", result.errors);

        if let Definition::Assert(assert_def) = &result.value.definitions[0] {
            if let AssertExpression::Comparison {
                left, op, right, ..
            } = &assert_def.expression
            {
                assert_eq!(*op, ComparisonOp::Eq);
                if let AssertOperand::QualifiedName { parts, .. } = left {
                    assert_eq!(parts, &["board", "layers"]);
                } else {
                    panic!("expected qualified name");
                }
                if let AssertOperand::Number { value, .. } = right {
                    assert!((value - 4.0).abs() < 0.001);
                } else {
                    panic!("expected number, got {:?}", right);
                }
            } else {
                panic!("expected comparison");
            }
        } else {
            panic!("expected assert definition");
        }
    }

    #[test]
    fn test_parse_assert_within_percentage() {
        let source = r#"assert R1.value within 10kohm +/- 5%"#;
        let result = parse(source);
        assert!(result.is_ok(), "errors: {:?}", result.errors);

        if let Definition::Assert(assert_def) = &result.value.definitions[0] {
            if let AssertExpression::Within { left, target, .. } = &assert_def.expression {
                if let AssertOperand::QualifiedName { parts, .. } = left {
                    assert_eq!(parts, &["R1", "value"]);
                } else {
                    panic!("expected qualified name");
                }
                assert!((target.value - 10.0).abs() < 0.001);
                assert_eq!(target.unit, PhysicalUnit::KiloOhm);
                let tol = target.tolerance.as_ref().expect("should have tolerance");
                if let ToleranceKind::Percentage { value } = &tol.kind {
                    assert!((value - 5.0).abs() < 0.001);
                } else {
                    panic!("expected percentage tolerance, got {:?}", tol.kind);
                }
            } else {
                panic!("expected within expression");
            }
        } else {
            panic!("expected assert definition");
        }
    }

    #[test]
    fn test_parse_assert_within_absolute() {
        let source = r#"assert U1.output within 3.3V +/- 0.1V"#;
        let result = parse(source);
        assert!(result.is_ok(), "errors: {:?}", result.errors);

        if let Definition::Assert(assert_def) = &result.value.definitions[0] {
            if let AssertExpression::Within { target, .. } = &assert_def.expression {
                assert!((target.value - 3.3).abs() < 0.001);
                assert_eq!(target.unit, PhysicalUnit::Volt);
                let tol = target.tolerance.as_ref().expect("should have tolerance");
                if let ToleranceKind::Absolute(abs) = &tol.kind {
                    assert!((abs.value - 0.1).abs() < 0.001);
                    assert_eq!(abs.unit, PhysicalUnit::Volt);
                } else {
                    panic!("expected absolute tolerance, got {:?}", tol.kind);
                }
            } else {
                panic!("expected within expression");
            }
        } else {
            panic!("expected assert definition");
        }
    }

    #[test]
    fn test_parse_assert_within_range() {
        let source = r#"assert C1.value within 100nF to 220nF"#;
        let result = parse(source);
        assert!(result.is_ok(), "errors: {:?}", result.errors);

        if let Definition::Assert(assert_def) = &result.value.definitions[0] {
            if let AssertExpression::Within { target, .. } = &assert_def.expression {
                assert!((target.value - 100.0).abs() < 0.001);
                assert_eq!(target.unit, PhysicalUnit::NanoFarad);
                let tol = target.tolerance.as_ref().expect("should have tolerance");
                if let ToleranceKind::Range(upper) = &tol.kind {
                    assert!((upper.value - 220.0).abs() < 0.001);
                    assert_eq!(upper.unit, PhysicalUnit::NanoFarad);
                } else {
                    panic!("expected range tolerance, got {:?}", tol.kind);
                }
            } else {
                panic!("expected within expression");
            }
        } else {
            panic!("expected assert definition");
        }
    }

    #[test]
    fn test_parse_physical_value_resistance() {
        for unit in &["ohm", "kohm", "Mohm"] {
            let source = format!(r#"component R1 resistor "0402" {{ value 10{} }}"#, unit);
            let result = parse(&source);
            assert!(result.is_ok(), "failed for {}: {:?}", unit, result.errors);

            let Definition::Component(comp) = &result.value.definitions[0] else {
                panic!("expected component definition for unit {}", unit);
            };
            // value_property accepts a physical_value node; the converter keeps
            // the raw text so the component value stays a plain string.
            let value = comp
                .value
                .as_ref()
                .unwrap_or_else(|| panic!("value missing for unit {}", unit));
            assert_eq!(value.value, format!("10{}", unit));
        }
    }

    #[test]
    fn test_parse_physical_value_capacitance() {
        for unit in &["pF", "nF", "uF", "mF"] {
            let source = format!(r#"component C1 capacitor "0402" {{ value 100{} }}"#, unit);
            let result = parse(&source);
            assert!(result.is_ok(), "failed for {}: {:?}", unit, result.errors);
        }
    }

    #[test]
    fn test_parse_physical_value_inductance() {
        for unit in &["nH", "uH", "mH", "H"] {
            let source = format!(r#"component L1 inductor "0402" {{ value 10{} }}"#, unit);
            let result = parse(&source);
            assert!(result.is_ok(), "failed for {}: {:?}", unit, result.errors);
        }
    }

    #[test]
    fn test_parse_physical_value_voltage() {
        for unit in &["mV", "V", "kV"] {
            let source = format!("assert U1.output >= 3.3{}", unit);
            let result = parse(&source);
            assert!(result.is_ok(), "failed for {}: {:?}", unit, result.errors);
        }
    }

    #[test]
    fn test_parse_physical_value_frequency() {
        for unit in &["Hz", "kHz", "MHz", "GHz"] {
            let source = format!("assert Y1.freq >= 16{}", unit);
            let result = parse(&source);
            assert!(result.is_ok(), "failed for {}: {:?}", unit, result.errors);
        }
    }

    #[test]
    fn test_parse_physical_value_power() {
        for unit in &["mW", "W"] {
            let source = format!("assert U1.power <= 500{}", unit);
            let result = parse(&source);
            assert!(result.is_ok(), "failed for {}: {:?}", unit, result.errors);
        }
    }

    #[test]
    fn test_parse_value_property_string_still_works() {
        // Backward compat: string values still work
        let source = r#"
component R1 resistor "0402" {
    value "10k"
}
"#;
        let result = parse(source);
        assert!(result.is_ok(), "errors: {:?}", result.errors);
        if let Definition::Component(comp) = &result.value.definitions[0] {
            assert_eq!(comp.value.as_ref().unwrap().value, "10k");
        } else {
            panic!("expected component");
        }
    }

    #[test]
    fn test_parse_module_with_assert() {
        let source = r#"
module PowerReg {
    component U1 ic "SOT-23" {
        value "LDO"
    }
    pin VIN
    pin VOUT
    assert U1.output >= 3.3V
}
"#;
        let result = parse(source);
        assert!(result.is_ok(), "errors: {:?}", result.errors);

        if let Definition::Module(module) = &result.value.definitions[0] {
            assert_eq!(module.name.value, "PowerReg");
            assert_eq!(module.pins.len(), 2);
            // 1 component + 1 assert = 2 definitions
            assert_eq!(module.definitions.len(), 2);
            assert!(matches!(module.definitions[0], Definition::Component(_)));
            assert!(matches!(module.definitions[1], Definition::Assert(_)));
        } else {
            panic!("expected module definition");
        }
    }

    #[test]
    fn test_parse_mixed_v1_v2() {
        // v1 and v2 constructs should coexist
        let source = r#"
version 2

import I2C from "std/interfaces.cypcb"

board test {
    size 30mm x 20mm
    layers 2
}

interface Power {
    pin VCC
    pin GND
}

component R1 resistor "0402" {
    value 10kohm
    at 10mm, 8mm
}

module LDO {
    component U1 ic "SOT-23" { value "LDO" }
    pin VIN
    pin VOUT
}

net VCC {
    R1.1
}

assert R1.value within 10kohm +/- 5%
"#;
        let result = parse(source);
        assert!(result.is_ok(), "errors: {:?}", result.errors);

        let ast = result.value;
        assert_eq!(ast.version, Some(2));
        // import + board + interface + component + module + net + assert = 7
        assert_eq!(ast.definitions.len(), 7);
        assert!(matches!(ast.definitions[0], Definition::Import(_)));
        assert!(matches!(ast.definitions[1], Definition::Board(_)));
        assert!(matches!(ast.definitions[2], Definition::Interface(_)));
        assert!(matches!(ast.definitions[3], Definition::Component(_)));
        assert!(matches!(ast.definitions[4], Definition::Module(_)));
        assert!(matches!(ast.definitions[5], Definition::Net(_)));
        assert!(matches!(ast.definitions[6], Definition::Assert(_)));
    }

    #[test]
    fn test_parse_all_comparison_operators() {
        let ops = &["==", "!=", ">=", "<=", ">", "<"];
        let expected = &[
            ComparisonOp::Eq,
            ComparisonOp::Ne,
            ComparisonOp::Ge,
            ComparisonOp::Le,
            ComparisonOp::Gt,
            ComparisonOp::Lt,
        ];

        for (op_str, expected_op) in ops.iter().zip(expected.iter()) {
            let source = format!("assert R1.value {} 10kohm", op_str);
            let result = parse(&source);
            assert!(result.is_ok(), "failed for {}: {:?}", op_str, result.errors);

            if let Definition::Assert(assert_def) = &result.value.definitions[0] {
                if let AssertExpression::Comparison { op, .. } = &assert_def.expression {
                    assert_eq!(op, expected_op, "wrong op for {}", op_str);
                } else {
                    panic!("expected comparison for {}", op_str);
                }
            }
        }
    }

    #[test]
    fn test_backward_compat_all_example_files() {
        // Parse all 10 existing .cypcb example files and assert zero parse errors.
        // This test must pass before and after every grammar change.
        let examples_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("examples");

        let mut files_tested = 0;
        for entry in std::fs::read_dir(&examples_dir).expect("examples dir should exist") {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "cypcb") {
                let filename = path.file_name().unwrap().to_string_lossy().to_string();
                let source = std::fs::read_to_string(&path)
                    .unwrap_or_else(|e| panic!("failed to read {}: {}", filename, e));

                let result = parse(&source);

                // Some files are intentionally invalid (invalid.cypcb, unknown_keyword.cypcb)
                // We just check they don't panic — they may have errors.
                if !filename.contains("invalid") && !filename.contains("unknown") {
                    assert!(
                        result.is_ok(),
                        "backward compat failed for {}: {:?}",
                        filename,
                        result.errors
                    );
                }
                files_tested += 1;
            }
        }

        assert!(
            files_tested >= 10,
            "expected at least 10 example files, found {}",
            files_tested
        );
    }

    #[test]
    fn test_v2_modules_example() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("examples/v2-modules.cypcb");
        let source = std::fs::read_to_string(&path).expect("v2-modules.cypcb should exist");
        let result = parse(&source);
        assert!(
            result.is_ok(),
            "v2-modules.cypcb errors: {:?}",
            result.errors
        );

        let ast = &result.value;
        // Should contain: 2 modules + 1 board
        let modules: Vec<_> = ast
            .definitions
            .iter()
            .filter(|d| matches!(d, Definition::Module(_)))
            .collect();
        assert_eq!(modules.len(), 2, "expected 2 modules");

        if let Definition::Module(m) = modules[0] {
            assert_eq!(m.name.value, "PowerSupply");
            assert_eq!(m.pins.len(), 3, "PowerSupply should have 3 pins");
        }

        if let Definition::Module(m) = modules[1] {
            assert_eq!(m.name.value, "LedDriver");
            assert_eq!(m.pins.len(), 2, "LedDriver should have 2 pins");
        }
    }

    #[test]
    fn test_v2_interfaces_example() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("examples/v2-interfaces.cypcb");
        let source = std::fs::read_to_string(&path).expect("v2-interfaces.cypcb should exist");
        let result = parse(&source);
        assert!(
            result.is_ok(),
            "v2-interfaces.cypcb errors: {:?}",
            result.errors
        );

        let ast = &result.value;
        let interfaces: Vec<_> = ast
            .definitions
            .iter()
            .filter(|d| matches!(d, Definition::Interface(_)))
            .collect();
        assert_eq!(
            interfaces.len(),
            4,
            "expected 4 interfaces (I2C, SPI, Power, UART)"
        );

        let modules: Vec<_> = ast
            .definitions
            .iter()
            .filter(|d| matches!(d, Definition::Module(_)))
            .collect();
        assert_eq!(modules.len(), 2, "expected 2 modules");
    }

    #[test]
    fn test_v2_constraints_example() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("examples/v2-constraints.cypcb");
        let source = std::fs::read_to_string(&path).expect("v2-constraints.cypcb should exist");
        let result = parse(&source);
        assert!(
            result.is_ok(),
            "v2-constraints.cypcb errors: {:?}",
            result.errors
        );

        let ast = &result.value;
        let asserts: Vec<_> = ast
            .definitions
            .iter()
            .filter(|d| matches!(d, Definition::Assert(_)))
            .collect();
        assert!(
            asserts.len() >= 5,
            "expected at least 5 assert statements, found {}",
            asserts.len()
        );

        // Verify physical value components parsed
        let comps: Vec<_> = ast
            .definitions
            .iter()
            .filter(|d| matches!(d, Definition::Component(_)))
            .collect();
        assert_eq!(comps.len(), 6, "expected 6 components");
    }
}
