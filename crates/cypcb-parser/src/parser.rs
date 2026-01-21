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
    BoardDef, ComponentDef, ComponentKind, Definition, Dimension, Identifier, LayerType,
    NetAssignment, NetConstraints, NetDef, PinId, PinRef, PositionExpr, RotationExpr, SizeProperty,
    SourceFile, Span, StackupDef, StackupLayer, StringLit, ZoneDef, ZoneKind,
};
use crate::errors::{ParseError, ParseResult};
use crate::node_kinds;
use cypcb_core::Unit;
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
                "zone_definition" => {
                    if let Some(zone) = self.convert_zone(source, &child, errors) {
                        definitions.push(Definition::Zone(zone));
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
                format!("unexpected token: '{}'", text.chars().take(20).collect::<String>()),
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
    fn convert_version(&self, source: &str, node: &Node, errors: &mut Vec<ParseError>) -> Option<u32> {
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
    fn convert_board(&self, source: &str, node: &Node, errors: &mut Vec<ParseError>) -> Option<BoardDef> {
        let name_node = get_child_by_field(node, "name")?;
        let name = Identifier::new(node_text(source, &name_node), span_of(&name_node));

        let mut size = None;
        let mut layers = None;
        let mut stackup = None;

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
                    _ => {}
                }
            }
        }

        Some(BoardDef {
            name,
            size,
            layers,
            stackup,
            span: span_of(node),
        })
    }

    /// Convert a size property node.
    fn convert_size(&self, source: &str, node: &Node, errors: &mut Vec<ParseError>) -> Option<SizeProperty> {
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
    fn convert_layers(&self, source: &str, node: &Node, errors: &mut Vec<ParseError>) -> Option<u8> {
        let count_node = get_child_by_field(node, "count")?;
        let text = node_text(source, &count_node);
        match text.parse::<u32>() {
            Ok(count) => {
                // Validate layer count (must be even, 2-32)
                if count < 2 || count > 32 || count % 2 != 0 {
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
    fn convert_stackup(&self, source: &str, node: &Node, errors: &mut Vec<ParseError>) -> Option<StackupDef> {
        let mut layers = Vec::new();

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "stackup_layer" {
                if let Some(layer) = self.convert_stackup_layer(source, &child, errors) {
                    layers.push(layer);
                }
            }
        }

        Some(StackupDef {
            layers,
            span: span_of(node),
        })
    }

    /// Convert a stackup layer node.
    fn convert_stackup_layer(&self, source: &str, node: &Node, errors: &mut Vec<ParseError>) -> Option<StackupLayer> {
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

        let thickness = get_child_by_field(node, "thickness")
            .and_then(|n| self.convert_dimension(source, &n, errors));

        Some(StackupLayer {
            layer_type,
            thickness,
            span: span_of(node),
        })
    }

    /// Convert a component definition node.
    fn convert_component(&self, source: &str, node: &Node, errors: &mut Vec<ParseError>) -> Option<ComponentDef> {
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

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                node_kinds::VALUE_PROPERTY => {
                    if let Some(val_node) = get_child_by_field(&child, "value") {
                        value = Some(self.convert_string_literal(source, &val_node));
                    }
                }
                node_kinds::POSITION_PROPERTY => {
                    position = self.convert_position(source, &child, errors);
                }
                node_kinds::ROTATION_PROPERTY => {
                    rotation = self.convert_rotation(source, &child, errors);
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
            footprint,
            value,
            position,
            rotation,
            net_assignments,
            span: span_of(node),
        })
    }

    /// Convert a position property node.
    fn convert_position(&self, source: &str, node: &Node, errors: &mut Vec<ParseError>) -> Option<PositionExpr> {
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
    fn convert_rotation(&self, source: &str, node: &Node, errors: &mut Vec<ParseError>) -> Option<RotationExpr> {
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
    fn convert_net_assignment(&self, source: &str, node: &Node, errors: &mut Vec<ParseError>) -> Option<NetAssignment> {
        let pin_node = get_child_by_field(node, "pin")?;
        let net_node = get_child_by_field(node, "net")?;

        let pin = self.convert_pin_identifier(source, &pin_node);
        let net = Identifier::new(node_text(source, &net_node), span_of(&net_node));

        Some(NetAssignment {
            pin,
            net,
            span: span_of(node),
        })
    }

    /// Convert a net definition node.
    fn convert_net(&self, source: &str, node: &Node, errors: &mut Vec<ParseError>) -> Option<NetDef> {
        let name_node = get_child_by_field(node, "name")?;
        let name = Identifier::new(node_text(source, &name_node), span_of(&name_node));

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
    fn convert_net_constraints(&self, source: &str, node: &Node, errors: &mut Vec<ParseError>) -> Option<NetConstraints> {
        let mut width = None;
        let mut clearance = None;

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            // net_constraint is a choice node wrapping width_constraint or clearance_constraint
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
                    _ => {}
                }
            }
        }

        Some(NetConstraints {
            width,
            clearance,
            span: span_of(node),
        })
    }

    /// Convert a pin reference list.
    fn convert_pin_ref_list(&self, source: &str, node: &Node, errors: &mut Vec<ParseError>) -> Vec<PinRef> {
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
    fn convert_pin_ref(&self, source: &str, node: &Node, _errors: &mut Vec<ParseError>) -> Option<PinRef> {
        let component_node = get_child_by_field(node, "component")?;
        let pin_node = get_child_by_field(node, "pin")?;

        let component = Identifier::new(node_text(source, &component_node), span_of(&component_node));
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

    /// Convert a dimension node.
    fn convert_dimension(&self, source: &str, node: &Node, errors: &mut Vec<ParseError>) -> Option<Dimension> {
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

        Some(Dimension::new(value, unit, span_of(node)))
    }

    /// Convert a string literal node (extracts value without quotes).
    fn convert_string_literal(&self, source: &str, node: &Node) -> StringLit {
        let full_text = node_text(source, node);
        // Strip quotes
        let value = full_text.trim_matches('"').to_string();
        StringLit::new(value, span_of(node))
    }

    /// Convert a zone definition node.
    fn convert_zone(&self, source: &str, node: &Node, errors: &mut Vec<ParseError>) -> Option<ZoneDef> {
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

                        if let (Some(x1), Some(y1), Some(x2), Some(y2)) = (min_x, min_y, max_x, max_y) {
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
                            net = Some(Identifier::new(
                                node_text(source, &net_node),
                                span_of(&net_node),
                            ));
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
            assert_eq!(comp.value.as_ref().map(|v| &v.value), Some(&"10k".to_string()));

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
            let constraints = net.constraints.as_ref().expect("constraints should be present");
            let width = constraints.width.as_ref().expect("width should be present");
            assert!((width.value - 0.5).abs() < 0.001);
            let clearance = constraints.clearance.as_ref().expect("clearance should be present");
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
        assert!(result.has_errors(), "expected errors: got AST: {:?}", result.value);
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
        assert!(result.has_errors(), "expected syntax error for unknown type");
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
        assert!(json.contains("\"version\": 1") || json.contains("\"version\":1"), "expected version:1 in {}", json);
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
            "resistor", "capacitor", "inductor", "ic", "led",
            "connector", "diode", "transistor", "crystal", "generic"
        ];

        for comp_type in types {
            let source = format!(
                r#"component X1 {} "fp" {{ at 0mm, 0mm }}"#,
                comp_type
            );
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
            assert_eq!(zone.name.as_ref().map(|n| &n.value), Some(&"antenna_clearance".to_string()));
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
            assert_eq!(zone.name.as_ref().map(|n| &n.value), Some(&"gnd_pour".to_string()));
            assert_eq!(zone.layer.as_deref(), Some("bottom"));
            assert_eq!(zone.net.as_ref().map(|n| &n.value), Some(&"GND".to_string()));
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
}
