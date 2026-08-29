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
//!             fab: None,
//!             ...
//!         })
//!     ],
//!     span: Span { start: 0, end: 42 },
//! }
//! ```

use cypcb_core::{PhysicalUnit, Unit};
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

impl SourceFile {
    /// The board this file describes, if it describes one.
    ///
    /// A file holds at most one board block in practice and the readers do not
    /// enforce it, so this answers with the first - which is what every caller
    /// asking "what size is it" or "which fab is it for" already meant.
    pub fn board(&self) -> Option<&BoardDef> {
        self.definitions
            .iter()
            .find_map(|definition| match definition {
                Definition::Board(board) => Some(board),
                _ => None,
            })
    }
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
    /// A custom footprint definition.
    Footprint(FootprintDef),
    /// A zone definition (keepout or copper pour).
    Zone(ZoneDef),
    /// A manual trace definition.
    Trace(TraceDef),
    /// A module definition (v2).
    Module(ModuleDef),
    /// A module instantiation (v2).
    ModuleInstance(ModuleInstance),
    /// A net class: one rule stated for a group of nets.
    NetClass(NetClassDef),
    /// The board's outline, when it is not a rectangle.
    Outline(OutlineDef),
    /// Words a person put on the board's legend.
    Text(TextDef),
    /// A measurement between two points, for the drawing rather than the board.
    Dimension(DimensionDef),
    /// An interface definition (v2).
    Interface(InterfaceDef),
    /// A differential pair.
    DiffPair(DiffPairDef),
    /// An import statement (v2).
    Import(ImportDef),
    /// An assert statement (v2).
    Assert(AssertDef),
}

impl Definition {
    /// Get the span of this definition.
    pub fn span(&self) -> Span {
        match self {
            Definition::Board(b) => b.span,
            Definition::Component(c) => c.span,
            Definition::Net(n) => n.span,
            Definition::Footprint(f) => f.span,
            Definition::Zone(z) => z.span,
            Definition::Trace(t) => t.span,
            Definition::Module(m) => m.span,
            Definition::ModuleInstance(i) => i.span,
            Definition::NetClass(c) => c.span,
            Definition::Outline(o) => o.span,
            Definition::Text(t) => t.span,
            Definition::Dimension(d) => d.span,
            Definition::Interface(i) => i.span,
            Definition::DiffPair(d) => d.span,
            Definition::Import(i) => i.span,
            Definition::Assert(a) => a.span,
        }
    }
}

/// A measurement: `dimension { from 0mm, 0mm to 30mm, 0mm offset 2mm }`.
///
/// What a drawing states so a person can check the board against it. Not
/// silkscreen: a dimension printed on copper would put `30.000mm` on the
/// finished product, which is why KiCad keeps them on a documentation layer.
/// This draws them in the SVG plot and nowhere else.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DimensionDef {
    /// One end of what is being measured.
    pub from: (Dimension, Dimension),
    /// The other end.
    pub to: (Dimension, Dimension),
    /// How far the dimension line sits from the thing it measures.
    pub offset: Option<Dimension>,
    /// Source span.
    pub span: Span,
}

/// Words on the board: `text "REV B" { at 5mm, 2mm layer top height 1.5mm }`.
///
/// The legend already carries every part's designator, drawn from the same
/// stroke font. This is for what a design wants to say that is not a
/// designator: a revision, a polarity mark's label, a warning beside a
/// connector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextDef {
    /// What it says.
    pub content: String,
    /// Where the middle of it sits.
    pub at: (Dimension, Dimension),
    /// Which silkscreen it is printed on. `None` means the top.
    pub layer: Option<String>,
    /// How tall the letters are. `None` takes the legend's own height.
    pub height: Option<Dimension>,
    /// Source span.
    pub span: Span,
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
    /// The fabricator this board is for: `fab jlcpcb`.
    ///
    /// A name, not a validated preset - this crate reads the language and has
    /// no table of fabs to check it against. Whoever resolves it to a rule set
    /// says so when the name is not one they know.
    pub fab: Option<Identifier>,
    /// Fillet the joins where tracks meet pads: `teardrops`.
    ///
    /// `None` is a board that did not say. A board that says the word without
    /// a block asks for the ordinary fillet, which is why the ratios inside
    /// are themselves optional.
    pub teardrops: Option<TeardropsProperty>,
    /// Span covering the entire board definition.
    pub span: Span,
}

/// What `teardrops` asks for: `teardrops { length 0.5 width 0.9 }`.
///
/// Both ratios are fractions of the pad's size, the way KiCad states them.
/// Absent means the house's ordinary figure rather than zero.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeardropsProperty {
    /// How far the fillet runs along the track.
    pub length: Option<f64>,
    /// How wide it is where it leaves the pad.
    pub width: Option<f64>,
    /// Span covering the property.
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
    /// The surface finish, as written: `finish "ENIG"`.
    ///
    /// A fabricator's word, held quoted for the reason `material` is: this
    /// language has no table of finishes to check one against.
    pub finish: Option<StringLit>,
    /// `edges plated`: copper on the routed outline.
    pub edges_plated: bool,
    /// `pads castellated`: holes cut in half by the outline, plated.
    pub castellated_pads: bool,
    /// `connector plain` or `connector bevelled`: a gold-finger edge.
    pub edge_connector: Option<EdgeConnectorDef>,
    /// `impedance controlled`: the fabricator holds the dielectric to the
    /// stackup rather than pressing to a total thickness.
    pub impedance_controlled: bool,
    /// `drill Top to Inner2`: the drill spans this build makes.
    ///
    /// Layer names as written; the reader that builds the world resolves them,
    /// so a name this language does not have is reported there with the rest.
    pub drill_pairs: Vec<(String, String)>,
    /// Span covering the stackup definition.
    pub span: Span,
}

/// Whether a stated edge connector is bevelled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EdgeConnectorDef {
    /// `connector plain`.
    Plain,
    /// `connector bevelled`.
    Bevelled,
}

/// A single layer in a stackup definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackupLayer {
    /// Type of layer (copper, prepreg, etc.).
    pub layer_type: LayerType,
    /// What the fabricator calls this layer, when the design says.
    ///
    /// Quoted, because the canonical names carry a dot - `F.Cu`, `In1.Cu`,
    /// `B.Mask` - which no identifier in this language may.
    pub name: Option<StringLit>,
    /// Optional thickness of the layer.
    pub thickness: Option<Dimension>,
    /// What the layer is made of: `material "FR4"`.
    ///
    /// A name, not a validated material - this crate reads the language and
    /// has no table of laminates to check it against.
    pub material: Option<StringLit>,
    /// What colour the fabricator is asked to make this layer: `color "Red"`.
    ///
    /// Mask and silkscreen carry one; copper and prepreg are the colour they
    /// are. Held as written, like `material`.
    pub color: Option<StringLit>,
    /// The sheets after this one, when the slot is pressed from several.
    ///
    /// `prepreg 0.1mm sheet 0.0668mm sheet 0.0668mm` - a fabricator hits a
    /// target thickness with the sheets they stock. KiCad calls each extra one
    /// `addsublayer`.
    pub sheets: Vec<StackupSheetDef>,
    /// The dielectric constant, as written: `dk 4.5`.
    ///
    /// KiCad calls this `epsilon_r` and Altium calls it `Dk`; the datasheet a
    /// designer reads it off calls it `Dk`, which is why the language does.
    pub dk: Option<f64>,
    /// The loss tangent, as written: `df 0.02`.
    ///
    /// `loss_tangent` to KiCad, `Df` to Altium and to the datasheet.
    pub df: Option<f64>,
    /// Where the layer stops, when it does not run the whole panel.
    ///
    /// `None` is a layer pressed across the whole board, which is every layer
    /// of a rigid build and most layers of a rigid-flex one.
    pub coverage: Option<LayerCoverageDef>,
    /// Span covering this layer definition.
    pub span: Span,
}

/// Which side of a named area a stackup layer is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverageSense {
    /// `covers bend` - the layer is there and nowhere else.
    Covers,
    /// `outside bend` - the layer is everywhere but there.
    Outside,
}

/// `covers bend` or `outside bend` on a stackup layer.
///
/// A rigid-flex build is not one stack: a stiffener cannot run through the
/// ribbon it is bonded on to stiffen, and a coverlay often stops before the
/// rigid ends. This is how a design says so, against an area it has already
/// named.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerCoverageDef {
    /// Whether the layer is only there, or everywhere but there.
    pub sense: CoverageSense,
    /// The name of the area, as the design spelled it.
    pub region: Identifier,
    /// Source span of the clause.
    pub span: Span,
}

/// One more sheet of laminate in a dielectric slot: `sheet 0.0668mm`.
///
/// The same four things a layer states about its own first sheet. No kind: a
/// slot is prepreg or core, and a sheet of it is that by construction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackupSheetDef {
    /// How thick this sheet is.
    pub thickness: Option<Dimension>,
    /// What it is made of.
    pub material: Option<StringLit>,
    /// The dielectric constant.
    pub dk: Option<f64>,
    /// The loss tangent.
    pub df: Option<f64>,
    /// Source span.
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
    /// Solder paste, deposited through a stencil at assembly.
    ///
    /// Not something a fabricator presses, and deliberately still here: KiCad
    /// puts a paste layer in its own stackup, so a board imported without a
    /// word for one would describe a different build than the file it came
    /// from.
    Paste,
    /// Coverlay: the film that covers copper where the board bends.
    Coverlay,
    /// Stiffener: material bonded under a flexible section to hold it rigid.
    Stiffener,
}

impl LayerType {
    /// Parse a layer type from a string.
    #[allow(clippy::should_implement_trait)] // Returns Option, not Result — different semantics than FromStr
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "copper" => Some(LayerType::Copper),
            "prepreg" => Some(LayerType::Prepreg),
            "core" => Some(LayerType::Core),
            "mask" => Some(LayerType::Mask),
            "silk" => Some(LayerType::Silk),
            "paste" => Some(LayerType::Paste),
            "coverlay" => Some(LayerType::Coverlay),
            "stiffener" => Some(LayerType::Stiffener),
            _ => None,
        }
    }
}

/// One line of a component's `spec` block: `output 3.3V`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecEntry {
    /// What the design called it.
    pub name: Identifier,
    /// The quantity it stated.
    pub value: PhysicalValue,
    /// Source span.
    pub span: Span,
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
    ///
    /// Always populated when a value is given, including for a typed one, so
    /// anything that just wants to print it keeps working.
    pub value: Option<StringLit>,
    /// The value as a typed quantity, when the design wrote one: `value 10kohm`
    /// rather than `value "10k"`.
    ///
    /// A string cannot be checked - "10k" is ten kilohms only if you already
    /// know the part is a resistor - so the type is kept rather than flattened.
    pub typed_value: Option<PhysicalValue>,
    /// The catalogue part to buy, when the design names one: `lcsc "C7593"`.
    ///
    /// A footprint says what the pads look like; this says which part goes on
    /// them. It is what an assembly house is given, so it rides through to the
    /// bill of materials.
    #[serde(default)]
    pub lcsc: Option<StringLit>,
    /// Facts about the part that only its datasheet knows: `spec { output 3.3V }`.
    ///
    /// Free names on purpose. The component block itself refuses a property it
    /// does not know, which is right for the ones the language defines; this
    /// is where a design states something it has no keyword for, so an
    /// `assert` has something to read.
    #[serde(default)]
    pub spec: Vec<SpecEntry>,
    /// Which face of the board the part is soldered to, when the design says.
    ///
    /// `side bottom` on a component. Absent means the top, except where the
    /// footprint itself is bottom-only - the world decides that, because it is
    /// the thing holding the footprints.
    #[serde(default)]
    pub side: Option<Identifier>,
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
    #[allow(clippy::should_implement_trait)] // Returns Option, not Result — different semantics than FromStr
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

/// A piece of silkscreen artwork inside a footprint definition.
///
/// Coordinates are relative to the footprint origin, like a pad's. A width of
/// `None` means the exporter's default stroke.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SilkDef {
    /// `silk line X1, Y1 to X2, Y2 [width W]`.
    Line {
        /// Start.
        start: (Dimension, Dimension),
        /// End.
        end: (Dimension, Dimension),
        /// Stroke width, if stated.
        width: Option<Dimension>,
        /// Source span.
        span: Span,
    },
    /// `silk circle CX, CY radius R [width W]`.
    Circle {
        /// Centre.
        centre: (Dimension, Dimension),
        /// Radius.
        radius: Dimension,
        /// Stroke width, if stated.
        width: Option<Dimension>,
        /// Source span.
        span: Span,
    },
}

/// The board's outline: `outline { point 0mm, 0mm  point 40mm, 0mm ... }`.
///
/// A ring of points, closed implicitly. A board without one is the rectangle
/// its size describes, which cannot say cutout, slot or chamfer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutlineDef {
    /// The ring, in order.
    pub points: Vec<(Dimension, Dimension)>,
    /// Source span.
    pub span: Span,
}

/// A net class: `netclass Power [width 0.5mm] { VCC GND }`.
///
/// States a rule once for a group of nets. A net that states something itself
/// keeps its own answer; the class fills in what the net left unsaid.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetClassDef {
    /// Class name, for reading and for error messages.
    pub name: Identifier,
    /// What the class requires of its members.
    pub constraints: Option<NetConstraints>,
    /// The nets in the class.
    pub members: Vec<Identifier>,
    /// Source span.
    pub span: Span,
}

/// Net constraints: `[width 0.3mm, clearance 0.2mm, current 500mA]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetConstraints {
    /// Trace width constraint.
    pub width: Option<Dimension>,
    /// Clearance constraint.
    pub clearance: Option<Dimension>,
    /// Current carrying requirement (for IPC-2221 calculation).
    pub current: Option<CurrentValue>,
    /// Target characteristic impedance in ohms, as written: `impedance 90ohm`.
    ///
    /// A plain number rather than a value type, because there is one unit and
    /// the grammar makes writing it compulsory.
    pub impedance_ohms: Option<f64>,
    /// How narrow copper on this net may get on a pad approach, and how far.
    ///
    /// The same `NeckDef` a `trace` block carries, because it is the same
    /// statement about the same copper - said once for the net instead of on
    /// each trace of it.
    pub neck: Option<NeckDef>,
    /// Span covering the constraint block.
    pub span: Span,
}

/// Current value with unit: `500mA` or `2A`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrentValue {
    /// Numeric value.
    pub value: f64,
    /// Unit of current measurement.
    pub unit: CurrentUnit,
    /// Span covering the current value.
    pub span: Span,
}

impl CurrentValue {
    /// Create a new current value.
    pub fn new(value: f64, unit: CurrentUnit, span: Span) -> Self {
        CurrentValue { value, unit, span }
    }

    /// Convert to milliamps.
    pub fn to_milliamps(&self) -> f64 {
        match self.unit {
            CurrentUnit::Milliamps => self.value,
            CurrentUnit::Amps => self.value * 1000.0,
        }
    }

    /// Convert to amps.
    pub fn to_amps(&self) -> f64 {
        match self.unit {
            CurrentUnit::Milliamps => self.value / 1000.0,
            CurrentUnit::Amps => self.value,
        }
    }
}

impl std::fmt::Display for CurrentValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", self.value, self.unit)
    }
}

/// Unit of current measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CurrentUnit {
    /// Milliamps (mA).
    Milliamps,
    /// Amps (A).
    Amps,
}

impl CurrentUnit {
    /// Parse a current unit from a string.
    #[allow(clippy::should_implement_trait)] // Returns Option, not Result — different semantics than FromStr
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "mA" => Some(CurrentUnit::Milliamps),
            "A" => Some(CurrentUnit::Amps),
            _ => None,
        }
    }
}

impl std::fmt::Display for CurrentUnit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CurrentUnit::Milliamps => write!(f, "mA"),
            CurrentUnit::Amps => write!(f, "A"),
        }
    }
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

/// A pad written as a bare number, as the string the model stores.
///
/// `1` has to come back as `"1"` and not `"1.0"`: a footprint's pads are
/// matched against a net's pin references by string, so a trailing `.0` would
/// stop `R1.1` finding the pad it names. A number that really does carry a
/// fraction keeps it, because refusing to write down what somebody typed is
/// worse than an odd-looking pad name.
pub fn format_pad_number(value: f64) -> String {
    if value.fract() == 0.0 && value.is_finite() {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    }
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
    /// Whether the source wrote the unit, or the grammar supplied it.
    ///
    /// A bare number is millimetres, which is the grammar's rule and is not
    /// going to change - but it is an assumption, and the tool should be able
    /// to say when it made one. Somebody thinking in mils who writes `200`
    /// gets 200mm.
    pub unit_written: bool,
    /// Span covering the dimension.
    pub span: Span,
}

impl Dimension {
    /// Create a new dimension, with the unit the source wrote.
    pub fn new(value: f64, unit: Unit, span: Span) -> Self {
        Dimension {
            value,
            unit,
            unit_written: true,
            span,
        }
    }

    /// A bare number, which the grammar reads as millimetres.
    pub fn implied_mm(value: f64, span: Span) -> Self {
        Dimension {
            value,
            unit: Unit::Mm,
            unit_written: false,
            span,
        }
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

/// Pad shape for footprint definitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PadShape {
    /// Rectangular pad.
    Rect,
    /// Circular pad.
    Circle,
    /// Rounded rectangle pad.
    RoundRect,
    /// Oblong (stadium) pad.
    Oblong,
}

impl PadShape {
    /// Parse a pad shape from a string.
    #[allow(clippy::should_implement_trait)] // Returns Option, not Result — different semantics than FromStr
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "rect" => Some(PadShape::Rect),
            "circle" => Some(PadShape::Circle),
            "roundrect" => Some(PadShape::RoundRect),
            "oblong" => Some(PadShape::Oblong),
            _ => None,
        }
    }
}

/// A pad definition within a footprint.
///
/// # Example DSL
///
/// ```cypcb
/// pad 1 rect at -2.7mm, -1.905mm size 1.5mm x 0.6mm
/// pad 2 circle at 0mm, 0mm size 1.8mm x 1.8mm drill 1.0mm
/// pad 3 oblong at 0mm, 3mm size 3.2mm x 1.8mm drill 2.4mm x 1.0mm
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PadDef {
    /// Pad name: `1`, `A1`, `S1`, whatever the datasheet calls it.
    ///
    /// A string, because a pad's name is a name and not a count. A USB-C
    /// receptacle names its pads A1 and B4, a BGA by row and column, an edge
    /// connector whatever it likes. `cypcb-world` has modelled this as a
    /// `String` since it was written - its own doc comment names `"A1"` and
    /// `"VCC"` - and only the parser insisted on a number, which is what kept
    /// most boards worth importing out of this language.
    pub number: String,
    /// Pad shape.
    pub shape: PadShape,
    /// X position relative to footprint origin.
    pub x: Dimension,
    /// Y position relative to footprint origin.
    pub y: Dimension,
    /// Pad width.
    pub width: Dimension,
    /// Pad height.
    pub height: Dimension,
    /// Optional drill diameter for through-hole pads.
    ///
    /// For a slot this is the first of the two written dimensions, which is
    /// its width in the pad's own frame - the same order `size W x H` uses.
    pub drill: Option<Dimension>,
    /// The hole's second dimension, when the design wrote one.
    ///
    /// `drill 0.9mm` is a round hole and leaves this `None`; `drill 2.4mm x
    /// 1.0mm` is a slot, milled along its length rather than drilled. A USB
    /// connector, a barrel jack and a latching header all hold themselves to
    /// the board through one, and until this existed a design written in this
    /// language could not describe the hole its own parts need - slots reached
    /// the model, the drill file, the KiCad file and the screen, from KiCad
    /// imports only.
    pub drill_height: Option<Dimension>,
    /// Source span.
    pub span: Span,
}

/// A custom footprint definition.
///
/// # Example DSL
///
/// ```cypcb
/// footprint MY_SOIC_8 {
///     description "Custom SOIC-8 with thermal pad"
///     pad 1 rect at -2.7mm, -1.905mm size 1.5mm x 0.6mm
///     pad 2 rect at -2.7mm, -0.635mm size 1.5mm x 0.6mm
///     courtyard 6mm x 5mm
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FootprintDef {
    /// Footprint name/identifier.
    pub name: Identifier,
    /// Optional description.
    pub description: Option<String>,
    /// Pad definitions.
    pub pads: Vec<PadDef>,
    /// Optional explicit courtyard dimensions (width, height).
    pub courtyard: Option<(Dimension, Dimension)>,
    /// Silkscreen artwork the footprint carries.
    pub silk: Vec<SilkDef>,
    /// Source span.
    pub span: Span,
}

/// Zone type (keepout vs copper pour).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZoneKind {
    /// No copper allowed in this region.
    Keepout,
    /// Copper fill zone (pour) - connected to a net.
    CopperPour,
    /// A flexible region: the part of a rigid-flex board that bends.
    Flex,
    /// A named area and nothing else.
    ///
    /// The other three kinds each carry a meaning: a pour is filled, a keepout
    /// is kept clear, a flexible region bends. A rigid end of a rigid-flex
    /// board is none of those and still needs a name, so that a stackup layer
    /// can say `core 1mm covers rigid_left`.
    Region,
}

impl ZoneKind {
    /// Parse a zone kind from a string.
    #[allow(clippy::should_implement_trait)] // Returns Option, not Result — different semantics than FromStr
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "keepout" => Some(ZoneKind::Keepout),
            "zone" => Some(ZoneKind::CopperPour),
            "flex" => Some(ZoneKind::Flex),
            "region" => Some(ZoneKind::Region),
            _ => None,
        }
    }
}

/// A zone definition (keepout or copper pour).
///
/// Zones define rectangular regions with special properties:
/// - Keepouts prevent copper from being placed in the region
/// - Copper pours fill the region with copper connected to a net
///
/// # Example DSL
///
/// ```cypcb
/// keepout antenna_clearance {
///     bounds 10mm, 10mm to 20mm, 20mm
///     layer all
/// }
///
/// zone gnd_pour {
///     bounds 0mm, 0mm to 50mm, 50mm
///     layer bottom
///     net GND
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoneDef {
    /// Zone type (keepout or copper pour).
    pub kind: ZoneKind,
    /// Optional zone name for reference.
    pub name: Option<Identifier>,
    /// Zone bounds (min_x, min_y, max_x, max_y).
    pub bounds: (Dimension, Dimension, Dimension, Dimension),
    /// Layer this zone applies to (None = all layers).
    pub layer: Option<String>,
    /// Net for copper pour zones (keepouts don't have this).
    pub net: Option<Identifier>,
    /// Tie the two sides of this pour together with vias at this pitch.
    ///
    /// `None` is a pour that did not ask. A plane on a two-layer board is two
    /// planes until a field of vias joins them, and where those go is a
    /// decision a design makes rather than a tool.
    pub stitch: Option<Dimension>,
    /// How tightly the board is folded here: `radius 3mm`.
    ///
    /// A fact about the product rather than about the outline - the same
    /// ribbon is folded flat in one case and round a battery in another - so
    /// it is stated where the bend is named. `None` is a design that has not
    /// said, and nothing invents one for it.
    pub radius: Option<Dimension>,
    /// Fill this pour as a mesh rather than as a sheet: `hatch 0.3mm pitch 1mm`.
    ///
    /// The width is the copper and the pitch is centre to centre, so a design
    /// that states one has said nothing about the other - which is why the
    /// grammar takes both or neither.
    pub hatch: Option<HatchDef>,
    /// Source span.
    pub span: Span,
}

/// `hatch 0.3mm pitch 1mm` on a pour.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HatchDef {
    /// How wide each line of copper is.
    pub width: Dimension,
    /// Centre to centre between lines.
    pub pitch: Dimension,
    /// Source span of the clause.
    pub span: Span,
}

/// A manual trace definition.
///
/// Manual traces allow users to define explicit routing between two pins,
/// optionally with via waypoints. These traces can be locked to prevent
/// the autorouter from modifying them.
///
/// Supports two modes:
/// 1. **Logical** (from/to/waypoints): high-level pin-to-pin routing
/// 2. **Geometric** (directives with path/layer/via): explicit polyline geometry
///
/// When `directives` is non-empty, it takes precedence over from/to/waypoints.
///
/// # Example DSL (logical)
///
/// ```cypcb
/// trace VCC {
///     from R1.1
///     to C1.1
///     via 5mm, 8mm
///     layer Top
///     width 0.4mm
///     locked
/// }
/// ```
///
/// # Example DSL (geometric — path-based persistence)
///
/// ```cypcb
/// trace GND {
///     layer Top
///     width 0.3mm
///     path 5mm,10mm -> 12mm,10mm
///     via 12mm,10mm drill 0.3mm
///     layer Bottom
///     path 12mm,10mm -> 20mm,10mm
///     locked
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceDef {
    /// Net name this trace belongs to.
    pub net: Identifier,
    /// Starting pin reference (logical mode).
    pub from: Option<PinRef>,
    /// Ending pin reference (logical mode).
    pub to: Option<PinRef>,
    /// Via waypoints — positions between from and to (logical mode).
    pub waypoints: Vec<PositionExpr>,
    /// Copper layer (None = use net default or TopCopper).
    pub layer: Option<String>,
    /// Trace width (None = use net constraint or default).
    pub width: Option<Dimension>,
    /// If true, autorouter should not modify this trace.
    pub locked: bool,
    /// How narrow this trace may get on a pad approach, and for how far.
    pub neck: Option<NeckDef>,
    /// Ordered directives for geometric (path-based) traces.
    /// When non-empty, these define the exact trace geometry.
    pub directives: Vec<TraceDirective>,
    /// Source span.
    pub span: Span,
}

/// `neck 0.8mm for 4mm` on a trace.
///
/// Both halves are compulsory. A width with no length is a second width, and
/// the whole point of stating a neck is that its length is bounded - copper
/// too thin for the current is only safe because it is short.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeckDef {
    /// The narrow width.
    pub width: Dimension,
    /// How far the trace may run at it.
    pub length: Dimension,
    /// Source span.
    pub span: Span,
}

/// An ordered directive within a geometric trace definition.
///
/// Directives appear in the order written in the DSL, preserving the
/// interleaving of layer switches, path segments, and vias.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TraceDirective {
    /// Switch current layer: `layer Top`
    Layer(String),
    /// Explicit polyline path: `path X1,Y1 -> X2,Y2 -> ...`
    Path(TracePath),
    /// Via with optional drill size: `via X,Y [drill D]`
    Via(TraceVia),
    /// A curve continuing from where the copper is: `arc centre X,Y sweep 90`
    Arc(TraceArc),
}

/// A curve in a trace: where it turns about, and how far.
///
/// In the DSL: `arc centre 15mm,10mm sweep 90`
///
/// It starts wherever the copper already is - the end of the path or arc
/// before it - so a curve is written as the continuation it is. The sweep is
/// signed: positive turns counter-clockwise.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceArc {
    /// Where the curve begins, when the file says so rather than leaving it to
    /// the copper before it.
    pub start: Option<PositionExpr>,
    /// The centre the copper turns about.
    pub centre: PositionExpr,
    /// How far it turns, in degrees. Negative turns clockwise.
    pub sweep_degrees: f64,
    /// Span covering the arc statement.
    pub span: Span,
}

/// An explicit trace path: a polyline of coordinate pairs.
///
/// In the DSL: `path 10mm,12mm -> 15mm,12mm -> 15mm,8mm`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracePath {
    /// Ordered list of points in the polyline.
    pub points: Vec<PositionExpr>,
    /// Span covering the entire path statement.
    pub span: Span,
}

/// A via waypoint with optional drill diameter.
///
/// In the DSL: `via 12mm,10mm drill 0.3mm layers Top to Inner1`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceVia {
    /// Via position.
    pub position: PositionExpr,
    /// Optional drill diameter.
    pub drill: Option<Dimension>,
    /// The layers the via joins, as the DSL names them.
    ///
    /// `None` means through the board, which is what a via with no stated pair
    /// has always been.
    pub layers: Option<(String, String)>,
    /// Span covering the via statement.
    pub span: Span,
}

// ============================================================================
// DSL v2: Modules, Interfaces, Imports, Asserts, Physical Units
// ============================================================================

/// An import statement: `import "path"` or `import Name from "path"`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportDef {
    /// Imported names (empty = import all).
    pub names: Vec<Identifier>,
    /// Path to the imported file.
    pub path: StringLit,
    /// Source span.
    pub span: Span,
}

/// A module definition: `module Name { ... }`.
///
/// Modules are reusable circuit blocks containing components, nets,
/// pin declarations, and assertions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleDef {
    /// Module name.
    pub name: Identifier,
    /// Component and net definitions inside the module.
    pub definitions: Vec<Definition>,
    /// Exposed pins of the module.
    pub pins: Vec<PinDeclaration>,
    /// Interfaces the module claims to implement.
    ///
    /// Each claim is checked: every pin the interface declares has to be a
    /// pin the module exposes. Empty for a module that claims nothing, which
    /// is every module written before `implements` existed.
    #[serde(default)]
    pub implements: Vec<ImplementsClause>,
    /// Source span.
    pub span: Span,
}

/// A differential pair: `diffpair USB { USB_DP USB_DM }`.
///
/// Two nets that carry one signal between them. What makes them a pair rather
/// than two nets is that they have to stay the same length - the receiver
/// reads the difference, so copper one net runs and the other does not is
/// skew, and skew is what the checker measures.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffPairDef {
    /// Name of the pair.
    pub name: Identifier,
    /// The net carrying the positive half.
    pub positive: Identifier,
    /// The net carrying the negative half.
    pub negative: Identifier,
    /// Make the two halves the same length, rather than only measuring them.
    ///
    /// The checker has reported skew since it was written and could do nothing
    /// about it. A pair that says `match` asks for the short half to be
    /// meandered until the two agree.
    pub match_lengths: bool,
    /// Source span.
    pub span: Span,
}

/// A claim inside a module: `implements I2C`.
///
/// The interface is named, not inlined, so one definition can be held over
/// every module that says it speaks that bus.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImplementsClause {
    /// Name of the interface being claimed.
    pub interface: Identifier,
    /// Source span of the whole clause.
    pub span: Span,
}

/// A module instantiation: `use Divider as DIV1 { IN = VIN, OUT = VOUT }`.
///
/// Placing an instance copies the module's components into the design under
/// the instance's name, and wires each exposed pin to a net the design names.
/// Without this a module is a definition nothing can reach.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleInstance {
    /// Name of the module being instantiated.
    pub module: Identifier,
    /// Name this instance is known by; also the prefix for its components.
    pub name: Identifier,
    /// Where the instance's origin sits on the board. Without one, every
    /// instance of a module lands on top of the last.
    pub position: Option<PositionExpr>,
    /// How far the whole instance is turned, in degrees.
    pub rotation: Option<RotationExpr>,
    /// Which net each of the module's pins connects to.
    pub ports: Vec<PortConnection>,
    /// Source span.
    pub span: Span,
}

/// One `PIN = net` line inside an instantiation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortConnection {
    /// The module pin being connected.
    pub pin: Identifier,
    /// The net in the enclosing design.
    pub net: Identifier,
    /// Source span.
    pub span: Span,
}

/// An interface definition: `interface Name { pin ... }`.
///
/// Interfaces define a set of named pins that can be connected as a group.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterfaceDef {
    /// Interface name.
    pub name: Identifier,
    /// Pin declarations.
    pub pins: Vec<PinDeclaration>,
    /// Source span.
    pub span: Span,
}

/// A pin declaration: `pin Name`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinDeclaration {
    /// Pin name.
    pub name: Identifier,
    /// Source span.
    pub span: Span,
}

/// An assert statement: `assert expression`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssertDef {
    /// The assertion expression.
    pub expression: AssertExpression,
    /// Source span.
    pub span: Span,
}

/// An assertion expression.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AssertExpression {
    /// Comparison: `left op right`.
    Comparison {
        left: AssertOperand,
        op: ComparisonOp,
        right: AssertOperand,
        span: Span,
    },
    /// Within: `left within target`.
    Within {
        left: AssertOperand,
        target: PhysicalValue,
        span: Span,
    },
}

impl AssertExpression {
    /// Get the span of this expression.
    pub fn span(&self) -> Span {
        match self {
            AssertExpression::Comparison { span, .. } => *span,
            AssertExpression::Within { span, .. } => *span,
        }
    }
}

/// An operand in an assert expression.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AssertOperand {
    /// A qualified name like `R1.value`.
    QualifiedName { parts: Vec<String>, span: Span },
    /// A physical value like `10kohm`.
    Physical(PhysicalValue),
    /// A dimension value like `0.3mm`.
    Dimension(Dimension),
    /// A bare number.
    Number { value: f64, span: Span },
}

impl AssertOperand {
    /// Get the span of this operand.
    pub fn span(&self) -> Span {
        match self {
            AssertOperand::QualifiedName { span, .. } => *span,
            AssertOperand::Physical(pv) => pv.span,
            AssertOperand::Dimension(d) => d.span,
            AssertOperand::Number { span, .. } => *span,
        }
    }
}

/// Comparison operators for assert expressions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ComparisonOp {
    Eq,
    Ne,
    Ge,
    Le,
    Gt,
    Lt,
}

impl ComparisonOp {
    /// Parse a comparison operator from a string.
    #[allow(clippy::should_implement_trait)] // Returns Option, not Result — different semantics than FromStr
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "==" => Some(ComparisonOp::Eq),
            "!=" => Some(ComparisonOp::Ne),
            ">=" => Some(ComparisonOp::Ge),
            "<=" => Some(ComparisonOp::Le),
            ">" => Some(ComparisonOp::Gt),
            "<" => Some(ComparisonOp::Lt),
            _ => None,
        }
    }
}

impl std::fmt::Display for ComparisonOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ComparisonOp::Eq => write!(f, "=="),
            ComparisonOp::Ne => write!(f, "!="),
            ComparisonOp::Ge => write!(f, ">="),
            ComparisonOp::Le => write!(f, "<="),
            ComparisonOp::Gt => write!(f, ">"),
            ComparisonOp::Lt => write!(f, "<"),
        }
    }
}

/// A physical value with unit and optional tolerance.
///
/// Examples: `10kohm`, `3.3V +/- 5%`, `100nF to 220nF`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhysicalValue {
    /// Numeric value.
    pub value: f64,
    /// Typed physical unit (resistance, capacitance, etc.).
    pub unit: PhysicalUnit,
    /// Optional tolerance.
    pub tolerance: Option<Tolerance>,
    /// Source span.
    pub span: Span,
}

/// Tolerance specification for a physical value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tolerance {
    /// The kind of tolerance.
    pub kind: ToleranceKind,
    /// Source span.
    pub span: Span,
}

/// Kinds of tolerance.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToleranceKind {
    /// Percentage tolerance: `+/- 5%`.
    Percentage { value: f64 },
    /// Absolute tolerance: `+/- 0.1V`.
    Absolute(Box<PhysicalValue>),
    /// Range tolerance: `to 220nF`.
    Range(Box<PhysicalValue>),
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
        assert_eq!(
            ComponentKind::from_str("resistor"),
            Some(ComponentKind::Resistor)
        );
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
                fab: None,
                teardrops: None,
                span: Span::new(0, 50),
            })],
            span: Span::new(0, 100),
        };
        let json = serde_json::to_string(&source_file).expect("serialize");
        assert!(json.contains("\"version\":1"));
        assert!(json.contains("\"type\":\"board\""));
    }

    #[test]
    fn test_zone_kind_parse() {
        assert_eq!(ZoneKind::from_str("keepout"), Some(ZoneKind::Keepout));
        assert_eq!(ZoneKind::from_str("zone"), Some(ZoneKind::CopperPour));
        assert_eq!(ZoneKind::from_str("unknown"), None);
    }

    #[test]
    fn test_pad_shape_parse() {
        assert_eq!(PadShape::from_str("rect"), Some(PadShape::Rect));
        assert_eq!(PadShape::from_str("circle"), Some(PadShape::Circle));
        assert_eq!(PadShape::from_str("roundrect"), Some(PadShape::RoundRect));
        assert_eq!(PadShape::from_str("oblong"), Some(PadShape::Oblong));
        assert_eq!(PadShape::from_str("unknown"), None);
    }

    #[test]
    fn test_current_unit_parse() {
        assert_eq!(CurrentUnit::from_str("mA"), Some(CurrentUnit::Milliamps));
        assert_eq!(CurrentUnit::from_str("A"), Some(CurrentUnit::Amps));
        assert_eq!(CurrentUnit::from_str("unknown"), None);
    }

    #[test]
    fn test_current_value_conversions() {
        let ma_val = CurrentValue::new(500.0, CurrentUnit::Milliamps, Span::new(0, 5));
        assert!((ma_val.to_milliamps() - 500.0).abs() < 0.001);
        assert!((ma_val.to_amps() - 0.5).abs() < 0.001);

        let a_val = CurrentValue::new(2.0, CurrentUnit::Amps, Span::new(0, 2));
        assert!((a_val.to_milliamps() - 2000.0).abs() < 0.001);
        assert!((a_val.to_amps() - 2.0).abs() < 0.001);
    }

    #[test]
    fn test_current_value_display() {
        let ma = CurrentValue::new(500.0, CurrentUnit::Milliamps, Span::new(0, 5));
        assert_eq!(format!("{}", ma), "500mA");

        let a = CurrentValue::new(2.5, CurrentUnit::Amps, Span::new(0, 4));
        assert_eq!(format!("{}", a), "2.5A");
    }
}
