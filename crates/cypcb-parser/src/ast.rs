//! AST types for CodeYourPCB DSL.
//!
//! This module defines the typed Abstract Syntax Tree (AST) nodes that are
//! produced by converting Tree-sitter's Concrete Syntax Tree (CST).
//!
//! All AST nodes carry [`Span`] information for error reporting and
//! source mapping back to the original code.
//!
//! # Example
//!
//! A typical AST structure for:
//! ```cypcb
//! version 1
//! board test { size 30mm x 20mm }
//! ```
//!
//! Would be:
//! ```rust,ignore
//! SourceFile {
//!     version: Some(1),
//!     definitions: vec![
//!         Definition::Board(BoardDef {
//!             name: Identifier { value: "test", span: ... },
//!             size: Some(SizeProperty { width: ..., height: ... }),
//!             layers: None,
//!             ...
//!         })
//!     ],
//!     span: Span { start: 0, end: 42 },
//! }
//! ```

use cypcb_core::Unit;
use serde::{Deserialize, Serialize};

/// A byte range in the source code.
///
/// Used for error reporting and source mapping. Start and end are byte offsets
/// into the source string (inclusive start, exclusive end).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    /// Start byte offset (inclusive).
    pub start: usize,
    /// End byte offset (exclusive).
    pub end: usize,
}

impl Span {
    /// Create a new span from start and end byte offsets.
    pub const fn new(start: usize, end: usize) -> Self {
        Span { start, end }
    }

    /// Create a span that covers a single point.
    pub const fn point(pos: usize) -> Self {
        Span {
            start: pos,
            end: pos,
        }
    }

    /// Return the length of this span in bytes.
    pub const fn len(&self) -> usize {
        self.end - self.start
    }

    /// Return true if this span is empty.
    pub const fn is_empty(&self) -> bool {
        self.start >= self.end
    }

    /// Merge two spans to create a span covering both.
    pub fn merge(self, other: Span) -> Span {
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }

    /// Convert to a miette SourceSpan.
    pub fn to_miette(self) -> miette::SourceSpan {
        (self.start, self.len()).into()
    }
}

impl From<Span> for miette::SourceSpan {
    fn from(span: Span) -> Self {
        span.to_miette()
    }
}

/// The root AST node representing an entire source file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceFile {
    /// Optional version declaration (e.g., `version 1`).
    pub version: Option<u32>,
    /// All top-level definitions in the file.
    pub definitions: Vec<Definition>,
    /// Span covering the entire file.
    pub span: Span,
}

/// A top-level definition in the source file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Definition {
    /// A board definition.
    Board(BoardDef),
    /// A component definition.
    Component(ComponentDef),
    /// A net definition.
    Net(NetDef),
}

impl Definition {
    /// Get the span of this definition.
    pub fn span(&self) -> Span {
        match self {
            Definition::Board(b) => b.span,
            Definition::Component(c) => c.span,
            Definition::Net(n) => n.span,
        }
    }
}

/// A board definition: `board name { ... }`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardDef {
    /// The board name (identifier).
    pub name: Identifier,
    /// Size property if specified.
    pub size: Option<SizeProperty>,
    /// Number of copper layers (2, 4, etc.).
    pub layers: Option<u8>,
    /// Stackup definition if specified.
    pub stackup: Option<StackupDef>,
    /// Span covering the entire board definition.
    pub span: Span,
}

/// Board size property: `size 30mm x 20mm`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SizeProperty {
    /// Board width.
    pub width: Dimension,
    /// Board height.
    pub height: Dimension,
    /// Span covering the size property.
    pub span: Span,
}

/// Stackup definition containing layer information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackupDef {
    /// List of layers in the stackup.
    pub layers: Vec<StackupLayer>,
    /// Span covering the stackup definition.
    pub span: Span,
}

/// A single layer in a stackup definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackupLayer {
    /// Type of layer (copper, prepreg, etc.).
    pub layer_type: LayerType,
    /// Optional thickness of the layer.
    pub thickness: Option<Dimension>,
    /// Span covering this layer definition.
    pub span: Span,
}

/// Types of layers in a PCB stackup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LayerType {
    /// Copper layer for traces.
    Copper,
    /// Prepreg (pre-impregnated composite fibers).
    Prepreg,
    /// Core material.
    Core,
    /// Solder mask.
    Mask,
    /// Silkscreen.
    Silk,
}

impl LayerType {
    /// Parse a layer type from a string.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "copper" => Some(LayerType::Copper),
            "prepreg" => Some(LayerType::Prepreg),
            "core" => Some(LayerType::Core),
            "mask" => Some(LayerType::Mask),
            "silk" => Some(LayerType::Silk),
            _ => None,
        }
    }
}

/// A component definition: `component R1 resistor "0402" { ... }`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentDef {
    /// Component reference designator (R1, C1, U1, etc.).
    pub refdes: Identifier,
    /// Type of component.
    pub kind: ComponentKind,
    /// Footprint name (e.g., "0402", "SOIC-8").
    pub footprint: StringLit,
    /// Component value if specified (e.g., "10k", "100nF").
    pub value: Option<StringLit>,
    /// Position if specified.
    pub position: Option<PositionExpr>,
    /// Rotation in degrees if specified.
    pub rotation: Option<RotationExpr>,
    /// Inline net assignments (pin.1 = VCC).
    pub net_assignments: Vec<NetAssignment>,
    /// Span covering the entire component definition.
    pub span: Span,
}

/// Types of electronic components.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ComponentKind {
    /// Resistor (R prefix).
    Resistor,
    /// Capacitor (C prefix).
    Capacitor,
    /// Inductor (L prefix).
    Inductor,
    /// Integrated circuit (U prefix).
    Ic,
    /// LED (D or LED prefix).
    Led,
    /// Connector (J prefix).
    Connector,
    /// Diode (D prefix).
    Diode,
    /// Transistor (Q prefix).
    Transistor,
    /// Crystal oscillator (Y prefix).
    Crystal,
    /// Generic component.
    Generic,
}

impl ComponentKind {
    /// Parse a component kind from a string.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "resistor" => Some(ComponentKind::Resistor),
            "capacitor" => Some(ComponentKind::Capacitor),
            "inductor" => Some(ComponentKind::Inductor),
            "ic" => Some(ComponentKind::Ic),
            "led" => Some(ComponentKind::Led),
            "connector" => Some(ComponentKind::Connector),
            "diode" => Some(ComponentKind::Diode),
            "transistor" => Some(ComponentKind::Transistor),
            "crystal" => Some(ComponentKind::Crystal),
            "generic" => Some(ComponentKind::Generic),
            _ => None,
        }
    }

    /// Get the expected reference designator prefix for this component kind.
    pub fn refdes_prefix(&self) -> &'static str {
        match self {
            ComponentKind::Resistor => "R",
            ComponentKind::Capacitor => "C",
            ComponentKind::Inductor => "L",
            ComponentKind::Ic => "U",
            ComponentKind::Led => "D",
            ComponentKind::Connector => "J",
            ComponentKind::Diode => "D",
            ComponentKind::Transistor => "Q",
            ComponentKind::Crystal => "Y",
            ComponentKind::Generic => "X",
        }
    }
}

/// Position expression: `at 10mm, 8mm`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionExpr {
    /// X coordinate.
    pub x: Dimension,
    /// Y coordinate.
    pub y: Dimension,
    /// Span covering the position expression.
    pub span: Span,
}

/// Rotation expression: `rotate 90` or `rotate 90deg`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotationExpr {
    /// Rotation angle in degrees.
    pub angle: f64,
    /// Span covering the rotation expression.
    pub span: Span,
}

/// Inline net assignment: `pin.1 = VCC`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetAssignment {
    /// Pin identifier (number or name).
    pub pin: PinId,
    /// Net name to assign.
    pub net: Identifier,
    /// Span covering the assignment.
    pub span: Span,
}

/// A net definition: `net VCC { J1.1, R1.1 }`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetDef {
    /// Net name.
    pub name: Identifier,
    /// Optional constraints (width, clearance).
    pub constraints: Option<NetConstraints>,
    /// List of pin references connected to this net.
    pub connections: Vec<PinRef>,
    /// Span covering the entire net definition.
    pub span: Span,
}

/// Net constraints: `[width 0.3mm, clearance 0.2mm]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetConstraints {
    /// Trace width constraint.
    pub width: Option<Dimension>,
    /// Clearance constraint.
    pub clearance: Option<Dimension>,
    /// Span covering the constraint block.
    pub span: Span,
}

/// A pin reference: `J1.1` or `U1.VCC`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinRef {
    /// Component reference designator.
    pub component: Identifier,
    /// Pin identifier (number or name).
    pub pin: PinId,
    /// Span covering the pin reference.
    pub span: Span,
}

/// A pin identifier: number or name.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PinId {
    /// Numeric pin (1, 2, 3, ...).
    Number(u32),
    /// Named pin (VCC, GND, anode, cathode, ...).
    Name(String),
}

impl PinId {
    /// Create a numeric pin ID.
    pub fn number(n: u32) -> Self {
        PinId::Number(n)
    }

    /// Create a named pin ID.
    pub fn name(s: impl Into<String>) -> Self {
        PinId::Name(s.into())
    }
}

impl std::fmt::Display for PinId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PinId::Number(n) => write!(f, "{}", n),
            PinId::Name(s) => write!(f, "{}", s),
        }
    }
}

/// An identifier token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Identifier {
    /// The identifier text.
    pub value: String,
    /// Span of the identifier.
    pub span: Span,
}

impl Identifier {
    /// Create a new identifier.
    pub fn new(value: impl Into<String>, span: Span) -> Self {
        Identifier {
            value: value.into(),
            span,
        }
    }
}

impl std::fmt::Display for Identifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value)
    }
}

/// A string literal token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StringLit {
    /// The string value (without quotes).
    pub value: String,
    /// Span of the entire string literal (including quotes).
    pub span: Span,
}

impl StringLit {
    /// Create a new string literal.
    pub fn new(value: impl Into<String>, span: Span) -> Self {
        StringLit {
            value: value.into(),
            span,
        }
    }
}

impl std::fmt::Display for StringLit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "\"{}\"", self.value)
    }
}

/// A dimension value with unit: `30mm`, `100mil`, `1in`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dimension {
    /// Numeric value.
    pub value: f64,
    /// Unit of measurement.
    pub unit: Unit,
    /// Span covering the dimension.
    pub span: Span,
}

impl Dimension {
    /// Create a new dimension.
    pub fn new(value: f64, unit: Unit, span: Span) -> Self {
        Dimension { value, unit, span }
    }

    /// Convert to nanometers using the core library.
    pub fn to_nm(&self) -> cypcb_core::Nm {
        self.unit.to_nm(self.value)
    }

    /// Convert to a core Dimension (without span).
    pub fn to_core(&self) -> cypcb_core::Dimension {
        cypcb_core::Dimension::new(self.value, self.unit)
    }
}

impl std::fmt::Display for Dimension {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", self.value, self.unit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_span_merge() {
        let a = Span::new(10, 20);
        let b = Span::new(15, 30);
        let merged = a.merge(b);
        assert_eq!(merged.start, 10);
        assert_eq!(merged.end, 30);
    }

    #[test]
    fn test_span_to_miette() {
        let span = Span::new(5, 15);
        let miette_span: miette::SourceSpan = span.into();
        // miette::SourceSpan is (offset, length)
        assert_eq!(miette_span.offset(), 5);
        assert_eq!(miette_span.len(), 10);
    }

    #[test]
    fn test_component_kind_parse() {
        assert_eq!(ComponentKind::from_str("resistor"), Some(ComponentKind::Resistor));
        assert_eq!(ComponentKind::from_str("ic"), Some(ComponentKind::Ic));
        assert_eq!(ComponentKind::from_str("unknown"), None);
    }

    #[test]
    fn test_layer_type_parse() {
        assert_eq!(LayerType::from_str("copper"), Some(LayerType::Copper));
        assert_eq!(LayerType::from_str("prepreg"), Some(LayerType::Prepreg));
        assert_eq!(LayerType::from_str("unknown"), None);
    }

    #[test]
    fn test_pin_id_display() {
        assert_eq!(format!("{}", PinId::Number(1)), "1");
        assert_eq!(format!("{}", PinId::Name("VCC".into())), "VCC");
    }

    #[test]
    fn test_dimension_to_nm() {
        let dim = Dimension::new(10.0, Unit::Mm, Span::new(0, 4));
        assert_eq!(dim.to_nm().0, 10_000_000);
    }

    #[test]
    fn test_ast_serialize() {
        let source_file = SourceFile {
            version: Some(1),
            definitions: vec![Definition::Board(BoardDef {
                name: Identifier::new("test", Span::new(0, 4)),
                size: Some(SizeProperty {
                    width: Dimension::new(30.0, Unit::Mm, Span::new(0, 4)),
                    height: Dimension::new(20.0, Unit::Mm, Span::new(0, 4)),
                    span: Span::new(0, 20),
                }),
                layers: Some(2),
                stackup: None,
                span: Span::new(0, 50),
            })],
            span: Span::new(0, 100),
        };
        let json = serde_json::to_string(&source_file).expect("serialize");
        assert!(json.contains("\"version\":1"));
        assert!(json.contains("\"type\":\"board\""));
    }
}
