//! Autocomplete provider for the CodeYourPCB LSP.
//!
//! Provides context-aware completions for:
//! - Footprint names in component definitions
//! - Net names in net blocks and pin references
//! - Component names (refdes) for pin references
//! - Property names inside definition blocks
//! - Layer names after "layer" keyword
//! - Top-level keywords at document root

use cypcb_parser::ast::{ComponentDef, Definition, NetDef, SourceFile};
use cypcb_world::footprint::FootprintLibrary;

use crate::document::{DocumentState, Position};

/// Completion item kind (maps to LSP CompletionItemKind).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionItemKind {
    /// A class/type (footprints)
    Class,
    /// A variable (nets, components)
    Variable,
    /// A property (value, at, rotate)
    Property,
    /// An enum value (layers)
    Enum,
    /// A keyword (board, component, net)
    Keyword,
    /// A snippet with placeholders
    Snippet,
}

/// A completion item to be returned by the LSP.
#[derive(Debug, Clone)]
pub struct CompletionItem {
    /// The text to insert.
    pub label: String,
    /// The kind of completion.
    pub kind: CompletionItemKind,
    /// Optional detail (shown next to label).
    pub detail: Option<String>,
    /// Optional documentation.
    pub documentation: Option<String>,
    /// Optional insert text (if different from label).
    pub insert_text: Option<String>,
    /// Whether insert_text is a snippet (has placeholders).
    pub is_snippet: bool,
}

impl CompletionItem {
    /// Create a simple completion with just label and kind.
    pub fn new(label: impl Into<String>, kind: CompletionItemKind) -> Self {
        CompletionItem {
            label: label.into(),
            kind,
            detail: None,
            documentation: None,
            insert_text: None,
            is_snippet: false,
        }
    }

    /// Add a detail string.
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Add documentation.
    pub fn with_documentation(mut self, doc: impl Into<String>) -> Self {
        self.documentation = Some(doc.into());
        self
    }

    /// Set insert text (for snippets or different text than label).
    pub fn with_insert_text(mut self, text: impl Into<String>, is_snippet: bool) -> Self {
        self.insert_text = Some(text.into());
        self.is_snippet = is_snippet;
        self
    }
}

/// The context where completion was triggered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompletionContext {
    /// Inside a footprint string (after opening quote).
    ComponentFootprint,
    /// In a net context (net definition or pin reference).
    NetName,
    /// Component name for pin reference (before the dot).
    ComponentName,
    /// Property key inside a definition block.
    PropertyKey(PropertyContext),
    /// Layer name (after "layer" keyword).
    LayerName,
    /// At the top level (document root).
    TopLevel,
    /// Unknown context.
    Unknown,
}

/// Sub-context for property completions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropertyContext {
    /// Inside a component definition.
    Component,
    /// Inside a net definition.
    Net,
    /// Inside a board definition.
    Board,
    /// Inside a footprint definition.
    Footprint,
    /// Inside a trace definition.
    Trace,
    /// Inside a zone definition.
    Zone,
    /// Inside a module definition.
    Module,
    /// Inside an interface definition.
    Interface,
}

/// Find the completion context at the given offset.
///
/// The answer comes from the AST where the AST has one, and from the text
/// under the cursor where it cannot. A user asking for completion has usually
/// just typed something incomplete - `component R3 resistor "` with the string
/// still open - and a half-written definition is exactly what the parser
/// cannot return. Reading the AST alone put that cursor "after the last
/// definition" and answered with the top-level keywords, so the editor offered
/// `version`, `board` and `net` inside a footprint string, where accepting one
/// pastes a whole snippet into the string.
pub fn find_completion_context(
    ast: &SourceFile,
    content: &str,
    offset: usize,
) -> CompletionContext {
    let from_ast = context_from_ast(ast, content, offset);
    if matches!(
        from_ast,
        CompletionContext::TopLevel | CompletionContext::Unknown
    ) {
        if let Some(from_text) = context_of_unfinished_text(content, offset) {
            return from_text;
        }
    }
    from_ast
}

/// What the unparseable text under the cursor is asking for, if anything.
///
/// Two shapes, both of which the parser refuses and both of which a user is in
/// the middle of typing: a string that is still open, and a block that is not
/// closed yet. Only consulted when the AST placed the cursor nowhere, so a file
/// that parses keeps answering from its own model.
fn context_of_unfinished_text(content: &str, offset: usize) -> Option<CompletionContext> {
    let before = content.get(..offset)?;

    // An odd number of quotes on this line means the cursor is inside one.
    let line_start = before.rfind('\n').map_or(0, |index| index + 1);
    let line = &before[line_start..];
    if line.matches('"').count() % 2 == 1 {
        return match first_word(line) {
            // `component R3 resistor "` - the string being typed is a
            // footprint name. Every other string in the language is a value
            // somebody invents, and a list of guesses is worse than no list.
            Some("component") => Some(CompletionContext::ComponentFootprint),
            _ => None,
        };
    }

    // Otherwise, is the cursor inside a block nobody has closed?
    let opener = innermost_unclosed_block(before)?;
    match first_word(opener) {
        Some("component") => Some(CompletionContext::PropertyKey(PropertyContext::Component)),
        Some("board") => Some(CompletionContext::PropertyKey(PropertyContext::Board)),
        Some("net") => Some(CompletionContext::PropertyKey(PropertyContext::Net)),
        Some("footprint") => Some(CompletionContext::PropertyKey(PropertyContext::Footprint)),
        Some("module") => Some(CompletionContext::PropertyKey(PropertyContext::Module)),
        Some("interface") => Some(CompletionContext::PropertyKey(PropertyContext::Interface)),
        _ => None,
    }
}

/// The first word of a line, which in this language is the keyword.
fn first_word(line: &str) -> Option<&str> {
    line.split_whitespace().next()
}

/// The line that opened the block the cursor is in, or `None` at top level.
///
/// Braces inside strings are skipped - a description may hold one - and the
/// scan runs over the text before the cursor only, so an unclosed block is
/// simply one that never got its `}`.
fn innermost_unclosed_block(before: &str) -> Option<&str> {
    let mut open_lines: Vec<usize> = Vec::new();
    let mut line_start = 0usize;
    let mut in_string = false;

    for (index, ch) in before.char_indices() {
        match ch {
            '"' => in_string = !in_string,
            '\n' if !in_string => line_start = index + 1,
            '{' if !in_string => open_lines.push(line_start),
            '}' if !in_string => {
                open_lines.pop();
            }
            _ => {}
        }
    }

    let start = open_lines.pop()?;
    let line = &before[start..];
    Some(line.split('\n').next().unwrap_or(line))
}

fn context_from_ast(ast: &SourceFile, content: &str, offset: usize) -> CompletionContext {
    // First check if we're at the start of the file or in whitespace at top level
    if ast.definitions.is_empty() {
        return CompletionContext::TopLevel;
    }

    // Check if offset is before first definition
    if let Some(first) = ast.definitions.first() {
        if offset < first.span().start {
            return CompletionContext::TopLevel;
        }
    }

    // Check if offset is after last definition
    if let Some(last) = ast.definitions.last() {
        if offset >= last.span().end {
            return CompletionContext::TopLevel;
        }
    }

    // Check if we're between definitions
    for window in ast.definitions.windows(2) {
        if offset >= window[0].span().end && offset < window[1].span().start {
            return CompletionContext::TopLevel;
        }
    }

    // Find which definition we're in
    for def in &ast.definitions {
        let span = def.span();
        if offset >= span.start && offset < span.end {
            return find_context_in_definition(def, content, offset);
        }
    }

    CompletionContext::TopLevel
}

fn find_context_in_definition(def: &Definition, content: &str, offset: usize) -> CompletionContext {
    match def {
        Definition::Component(comp) => find_context_in_component(comp, content, offset),
        Definition::Net(net) => find_context_in_net(net, content, offset),
        Definition::Board(_) => CompletionContext::PropertyKey(PropertyContext::Board),
        Definition::Footprint(_) => CompletionContext::PropertyKey(PropertyContext::Footprint),
        Definition::Zone(_) => find_context_in_zone(content, offset),
        Definition::Trace(_) => find_context_in_trace(content, offset),
        // v2 constructs — completion context not yet implemented
        Definition::Outline(_)
        | Definition::NetClass(_)
        | Definition::ModuleInstance(_)
        | Definition::Module(_)
        | Definition::Interface(_)
        | Definition::Import(_)
        | Definition::Assert(_) => CompletionContext::TopLevel,
    }
}

fn find_context_in_component(
    comp: &ComponentDef,
    content: &str,
    offset: usize,
) -> CompletionContext {
    // Check if we're inside the footprint string
    let fp_span = &comp.footprint.span;
    if offset >= fp_span.start && offset <= fp_span.end {
        // Check if we're after the opening quote
        if let Some(text) = content.get(fp_span.start..offset) {
            if text.starts_with('"') {
                return CompletionContext::ComponentFootprint;
            }
        }
    }

    // Check if we're in a net assignment (right side of =)
    for assign in &comp.net_assignments {
        if offset >= assign.net.span.start && offset <= assign.net.span.end {
            return CompletionContext::NetName;
        }
    }

    // Default to property context inside component
    CompletionContext::PropertyKey(PropertyContext::Component)
}

fn find_context_in_net(net: &NetDef, content: &str, offset: usize) -> CompletionContext {
    // Check if we're on a pin reference
    for conn in &net.connections {
        if offset >= conn.span.start && offset < conn.span.end {
            // Check if we're before or after the dot
            if let Some(text) = content.get(conn.span.start..offset) {
                if !text.contains('.') {
                    return CompletionContext::ComponentName;
                }
            }
        }
    }

    // Check if we're starting a new pin reference (looking for component name)
    // by checking if the preceding text suggests we're about to type a component
    if let Some(before) = content.get(..offset) {
        let trimmed = before.trim_end();
        // After { or after a pin reference
        if trimmed.ends_with('{') || trimmed.ends_with(',') {
            return CompletionContext::ComponentName;
        }
    }

    // Otherwise could be property context
    CompletionContext::PropertyKey(PropertyContext::Net)
}

fn find_context_in_zone(content: &str, offset: usize) -> CompletionContext {
    // Check if we're after "layer" keyword
    if let Some(before) = content.get(..offset) {
        let trimmed = before.trim_end();
        if trimmed.ends_with("layer") {
            return CompletionContext::LayerName;
        }
    }
    CompletionContext::PropertyKey(PropertyContext::Zone)
}

fn find_context_in_trace(content: &str, offset: usize) -> CompletionContext {
    // Check if we're after "layer" keyword
    if let Some(before) = content.get(..offset) {
        let trimmed = before.trim_end();
        if trimmed.ends_with("layer") {
            return CompletionContext::LayerName;
        }
    }
    CompletionContext::PropertyKey(PropertyContext::Trace)
}

/// Get completions at the given position.
pub fn completion_at_position(doc: &DocumentState, position: &Position) -> Vec<CompletionItem> {
    let Some(offset) = doc.position_to_offset(position) else {
        return vec![];
    };

    let Some(ast) = &doc.ast else {
        // No AST, offer top-level completions
        return top_level_completions();
    };

    let context = find_completion_context(ast, &doc.content, offset);

    match context {
        CompletionContext::ComponentFootprint => footprint_completions(),
        CompletionContext::NetName => net_completions(ast),
        CompletionContext::ComponentName => component_completions(ast),
        CompletionContext::PropertyKey(prop_ctx) => property_completions(&prop_ctx),
        CompletionContext::LayerName => layer_completions(),
        CompletionContext::TopLevel => top_level_completions(),
        CompletionContext::Unknown => vec![],
    }
}

/// Generate footprint name completions.
pub fn footprint_completions() -> Vec<CompletionItem> {
    let lib = FootprintLibrary::new();
    let mut items = Vec::new();

    for (name, fp) in lib.iter() {
        let pad_type = if fp.pads.iter().any(|p| p.drill.is_some()) {
            "THT"
        } else {
            "SMD"
        };

        let detail = format!("{} {}-pin", pad_type, fp.pads.len());

        let doc = format!(
            "Size: {:.2}mm x {:.2}mm\nCourtyard: {:.2}mm x {:.2}mm",
            fp.bounds.width().to_mm(),
            fp.bounds.height().to_mm(),
            fp.courtyard.width().to_mm(),
            fp.courtyard.height().to_mm()
        );

        items.push(
            CompletionItem::new(name, CompletionItemKind::Class)
                .with_detail(detail)
                .with_documentation(doc),
        );
    }

    // Sort by name for consistent ordering
    items.sort_by(|a, b| a.label.cmp(&b.label));
    items
}

/// Generate net name completions from the AST.
pub fn net_completions(ast: &SourceFile) -> Vec<CompletionItem> {
    let mut items = Vec::new();

    for def in &ast.definitions {
        if let Definition::Net(net) = def {
            let conn_count = net.connections.len();
            let detail = format!("Net ({} connections)", conn_count);

            items.push(
                CompletionItem::new(&net.name.value, CompletionItemKind::Variable)
                    .with_detail(detail),
            );
        }
    }

    items
}

/// Generate component name (refdes) completions from the AST.
pub fn component_completions(ast: &SourceFile) -> Vec<CompletionItem> {
    let mut items = Vec::new();

    for def in &ast.definitions {
        if let Definition::Component(comp) = def {
            let kind_str = format!("{:?}", comp.kind).to_lowercase();
            let detail = format!("{} ({})", kind_str, comp.footprint.value);

            items.push(
                CompletionItem::new(&comp.refdes.value, CompletionItemKind::Variable)
                    .with_detail(detail),
            );
        }
    }

    items
}

/// Generate property key completions based on context.
pub fn property_completions(context: &PropertyContext) -> Vec<CompletionItem> {
    match context {
        PropertyContext::Component => vec![
            CompletionItem::new("value", CompletionItemKind::Property)
                .with_detail("Component value")
                .with_insert_text("value \"$1\"", true),
            CompletionItem::new("at", CompletionItemKind::Property)
                .with_detail("Position (x, y)")
                .with_insert_text("at ${1:0}mm, ${2:0}mm", true),
            CompletionItem::new("rotate", CompletionItemKind::Property)
                .with_detail("Rotation angle")
                .with_insert_text("rotate ${1:0}", true),
        ],
        PropertyContext::Net => vec![
            CompletionItem::new("width", CompletionItemKind::Property)
                .with_detail("Trace width constraint")
                .with_insert_text("width ${1:0.25}mm", true),
            CompletionItem::new("clearance", CompletionItemKind::Property)
                .with_detail("Clearance constraint")
                .with_insert_text("clearance ${1:0.15}mm", true),
            CompletionItem::new("current", CompletionItemKind::Property)
                .with_detail("Current carrying requirement")
                .with_insert_text("current ${1:500}mA", true),
        ],
        PropertyContext::Board => vec![
            CompletionItem::new("size", CompletionItemKind::Property)
                .with_detail("Board dimensions")
                .with_insert_text("size ${1:50}mm x ${2:30}mm", true),
            CompletionItem::new("layers", CompletionItemKind::Property)
                .with_detail("Number of copper layers")
                .with_insert_text("layers ${1:2}", true),
            CompletionItem::new("stackup", CompletionItemKind::Property)
                .with_detail("Layer stackup definition"),
        ],
        PropertyContext::Footprint => vec![
            CompletionItem::new("description", CompletionItemKind::Property)
                .with_detail("Footprint description")
                .with_insert_text("description \"$1\"", true),
            CompletionItem::new("pad", CompletionItemKind::Property)
                .with_detail("Pad definition")
                .with_insert_text(
                    "pad ${1:1} rect at ${2:0}mm, ${3:0}mm size ${4:1}mm x ${5:0.5}mm",
                    true,
                ),
            CompletionItem::new("courtyard", CompletionItemKind::Property)
                .with_detail("Courtyard dimensions")
                .with_insert_text("courtyard ${1:2}mm x ${2:1}mm", true),
        ],
        PropertyContext::Trace => vec![
            CompletionItem::new("from", CompletionItemKind::Property)
                .with_detail("Starting pin")
                .with_insert_text("from ${1:R1}.${2:1}", true),
            CompletionItem::new("to", CompletionItemKind::Property)
                .with_detail("Ending pin")
                .with_insert_text("to ${1:C1}.${2:1}", true),
            CompletionItem::new("via", CompletionItemKind::Property)
                .with_detail("Via waypoint")
                .with_insert_text("via ${1:5}mm, ${2:5}mm", true),
            CompletionItem::new("layer", CompletionItemKind::Property).with_detail("Copper layer"),
            CompletionItem::new("width", CompletionItemKind::Property)
                .with_detail("Trace width")
                .with_insert_text("width ${1:0.25}mm", true),
            CompletionItem::new("locked", CompletionItemKind::Property)
                .with_detail("Prevent autorouter modification"),
        ],
        PropertyContext::Module => vec![
            CompletionItem::new("pin", CompletionItemKind::Property)
                .with_detail("Pin the module exposes")
                .with_insert_text("pin ${1:NAME}", true),
            CompletionItem::new("implements", CompletionItemKind::Property)
                .with_detail("Interface this module satisfies")
                .with_insert_text("implements ${1:I2C}", true),
            CompletionItem::new("component", CompletionItemKind::Property)
                .with_detail("Component inside the module")
                .with_insert_text(
                    "component ${1:R1} ${2:resistor} \"${3:0402}\" {\n    $0\n}",
                    true,
                ),
            CompletionItem::new("net", CompletionItemKind::Property)
                .with_detail("Net inside the module")
                .with_insert_text("net ${1:NAME} {\n    $0\n}", true),
            CompletionItem::new("use", CompletionItemKind::Property)
                .with_detail("Instantiate another module")
                .with_insert_text("use ${1:Module} as ${2:NAME} {\n    $0\n}", true),
        ],
        PropertyContext::Interface => {
            vec![CompletionItem::new("pin", CompletionItemKind::Property)
                .with_detail("Pin the interface declares")
                .with_insert_text("pin ${1:NAME}", true)]
        }
        PropertyContext::Zone => vec![
            CompletionItem::new("bounds", CompletionItemKind::Property)
                .with_detail("Zone bounds")
                .with_insert_text("bounds ${1:0}mm, ${2:0}mm to ${3:10}mm, ${4:10}mm", true),
            CompletionItem::new("layer", CompletionItemKind::Property)
                .with_detail("Layer for zone"),
            CompletionItem::new("net", CompletionItemKind::Property)
                .with_detail("Net for copper pour")
                .with_insert_text("net ${1:GND}", true),
        ],
    }
}

/// Generate layer name completions.
pub fn layer_completions() -> Vec<CompletionItem> {
    vec![
        CompletionItem::new("Top", CompletionItemKind::Enum).with_detail("Top copper layer"),
        CompletionItem::new("Bottom", CompletionItemKind::Enum).with_detail("Bottom copper layer"),
        CompletionItem::new("Inner1", CompletionItemKind::Enum).with_detail("Inner copper layer 1"),
        CompletionItem::new("Inner2", CompletionItemKind::Enum).with_detail("Inner copper layer 2"),
        CompletionItem::new("Inner3", CompletionItemKind::Enum).with_detail("Inner copper layer 3"),
        CompletionItem::new("Inner4", CompletionItemKind::Enum).with_detail("Inner copper layer 4"),
        CompletionItem::new("all", CompletionItemKind::Enum).with_detail("All layers"),
    ]
}

/// Generate top-level completions (keywords and snippets).
pub fn top_level_completions() -> Vec<CompletionItem> {
    vec![
        CompletionItem::new("version", CompletionItemKind::Keyword)
            .with_detail("Version declaration")
            .with_insert_text("version 1", false),
        CompletionItem::new("board", CompletionItemKind::Keyword)
            .with_detail("Board definition")
            .with_insert_text("board ${1:my_board} {\n    size ${2:50}mm x ${3:30}mm\n    layers ${4:2}\n}", true),
        CompletionItem::new("component", CompletionItemKind::Keyword)
            .with_detail("Component definition")
            .with_insert_text("component ${1:R1} resistor \"${2:0402}\" {\n    value \"${3:10k}\"\n    at ${4:10}mm, ${5:10}mm\n}", true),
        CompletionItem::new("net", CompletionItemKind::Keyword)
            .with_detail("Net definition")
            .with_insert_text("net ${1:VCC} {\n    ${2:R1.1}\n}", true),
        CompletionItem::new("footprint", CompletionItemKind::Keyword)
            .with_detail("Custom footprint definition")
            .with_insert_text("footprint ${1:MY_FP} {\n    pad 1 rect at ${2:0}mm, ${3:0}mm size ${4:1}mm x ${5:0.5}mm\n}", true),
        CompletionItem::new("trace", CompletionItemKind::Keyword)
            .with_detail("Manual trace definition")
            .with_insert_text("trace ${1:VCC} {\n    from ${2:R1}.${3:1}\n    to ${4:C1}.${5:1}\n}", true),
        CompletionItem::new("keepout", CompletionItemKind::Keyword)
            .with_detail("Keepout zone")
            .with_insert_text("keepout ${1:no_copper} {\n    bounds ${2:0}mm, ${3:0}mm to ${4:10}mm, ${5:10}mm\n}", true),
        CompletionItem::new("zone", CompletionItemKind::Keyword)
            .with_detail("Copper pour zone")
            .with_insert_text("zone ${1:gnd_pour} {\n    bounds ${2:0}mm, ${3:0}mm to ${4:50}mm, ${5:30}mm\n    net GND\n}", true),
        // v2 keywords
        CompletionItem::new("module", CompletionItemKind::Keyword)
            .with_detail("Module definition (v2)")
            .with_documentation("Defines a reusable module with components, nets, and exposed pins.\n\nSyntax:\n```\nmodule Name {\n    component ...\n    net ...\n    pin PinName\n}\n```")
            .with_insert_text("module ${1:Name} {\n    $0\n}", true),
        CompletionItem::new("interface", CompletionItemKind::Keyword)
            .with_detail("Interface definition (v2)")
            .with_documentation("Defines a named set of pins for typed connections between modules.\n\nSyntax:\n```\ninterface Name {\n    pin Pin1\n    pin Pin2\n}\n```")
            .with_insert_text("interface ${1:Name} {\n    pin ${2:Pin1}\n    pin ${3:Pin2}\n}", true),
        CompletionItem::new("import", CompletionItemKind::Keyword)
            .with_detail("Import statement (v2)")
            .with_documentation("Import definitions from another .cypcb file.\n\nSyntax:\n```\nimport \"path/to/file.cypcb\"\nimport Name from \"path/to/file.cypcb\"\nimport A, B from \"path/to/file.cypcb\"\n```")
            .with_insert_text("import ${1:Name} from \"${2:path.cypcb}\"", true),
        CompletionItem::new("assert", CompletionItemKind::Keyword)
            .with_detail("Constraint assertion (v2)")
            .with_documentation("Assert a constraint on a component or board property.\n\nSyntax:\n```\nassert R1.value >= 10kohm\nassert R1.value within 10kohm +/- 5%\nassert C1.value within 100nF to 220nF\n```")
            .with_insert_text("assert ${1:R1.value} ${2|>=,<=,==,within|} ${3:10kohm}", true),
    ]
}

/// Generate physical unit completions for value contexts.
pub fn physical_unit_completions() -> Vec<CompletionItem> {
    vec![
        // Resistance
        CompletionItem::new("ohm", CompletionItemKind::Enum).with_detail("Ohms (Ω)"),
        CompletionItem::new("kohm", CompletionItemKind::Enum).with_detail("Kilohms (kΩ)"),
        CompletionItem::new("Mohm", CompletionItemKind::Enum).with_detail("Megohms (MΩ)"),
        // Capacitance
        CompletionItem::new("pF", CompletionItemKind::Enum).with_detail("Picofarads"),
        CompletionItem::new("nF", CompletionItemKind::Enum).with_detail("Nanofarads"),
        CompletionItem::new("uF", CompletionItemKind::Enum).with_detail("Microfarads"),
        CompletionItem::new("mF", CompletionItemKind::Enum).with_detail("Millifarads"),
        // Inductance
        CompletionItem::new("nH", CompletionItemKind::Enum).with_detail("Nanohenries"),
        CompletionItem::new("uH", CompletionItemKind::Enum).with_detail("Microhenries"),
        CompletionItem::new("mH", CompletionItemKind::Enum).with_detail("Millihenries"),
        CompletionItem::new("H", CompletionItemKind::Enum).with_detail("Henries"),
        // Voltage
        CompletionItem::new("mV", CompletionItemKind::Enum).with_detail("Millivolts"),
        CompletionItem::new("V", CompletionItemKind::Enum).with_detail("Volts"),
        CompletionItem::new("kV", CompletionItemKind::Enum).with_detail("Kilovolts"),
        // Current
        CompletionItem::new("uA", CompletionItemKind::Enum).with_detail("Microamps"),
        CompletionItem::new("mA", CompletionItemKind::Enum).with_detail("Milliamps"),
        CompletionItem::new("A", CompletionItemKind::Enum).with_detail("Amps"),
        // Frequency
        CompletionItem::new("Hz", CompletionItemKind::Enum).with_detail("Hertz"),
        CompletionItem::new("kHz", CompletionItemKind::Enum).with_detail("Kilohertz"),
        CompletionItem::new("MHz", CompletionItemKind::Enum).with_detail("Megahertz"),
        CompletionItem::new("GHz", CompletionItemKind::Enum).with_detail("Gigahertz"),
        // Power
        CompletionItem::new("mW", CompletionItemKind::Enum).with_detail("Milliwatts"),
        CompletionItem::new("W", CompletionItemKind::Enum).with_detail("Watts"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_doc(content: &str) -> DocumentState {
        let mut doc = DocumentState::new("test://file".into(), content.to_string(), 1);
        doc.parse();
        doc
    }

    #[test]
    fn test_footprint_completions() {
        let items = footprint_completions();

        // Should have built-in footprints
        assert!(!items.is_empty());

        // Check for specific footprints
        let names: Vec<_> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(names.contains(&"0402"));
        assert!(names.contains(&"0603"));
        assert!(names.contains(&"SOIC-8"));
    }

    #[test]
    fn test_completion_in_footprint_string() {
        let doc = make_doc(r#"component R1 resistor "" {}"#);

        // Position inside the empty quotes (offset 23 is between the quotes)
        let pos = Position {
            line: 0,
            character: 23,
        };
        let items = completion_at_position(&doc, &pos);

        // Should get footprint completions
        let has_footprints = items.iter().any(|i| i.kind == CompletionItemKind::Class);
        assert!(
            has_footprints,
            "Expected footprint completions inside string"
        );
    }

    #[test]
    fn test_net_completions() {
        let doc = make_doc(
            r#"
net VCC { R1.1 }
net GND { R1.2, C1.1 }
"#,
        );

        let ast = doc.ast.as_ref().unwrap();
        let items = net_completions(ast);

        assert_eq!(items.len(), 2);
        let names: Vec<_> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(names.contains(&"VCC"));
        assert!(names.contains(&"GND"));
    }

    #[test]
    fn test_component_completions() {
        let doc = make_doc(
            r#"
component R1 resistor "0402" {}
component C1 capacitor "0603" {}
"#,
        );

        let ast = doc.ast.as_ref().unwrap();
        let items = component_completions(ast);

        assert_eq!(items.len(), 2);
        let names: Vec<_> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(names.contains(&"R1"));
        assert!(names.contains(&"C1"));
    }

    #[test]
    fn test_top_level_completions() {
        let items = top_level_completions();

        let labels: Vec<_> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"board"));
        assert!(labels.contains(&"component"));
        assert!(labels.contains(&"net"));
        assert!(labels.contains(&"version"));
    }

    #[test]
    fn test_completion_at_document_start() {
        let doc = make_doc("");

        let pos = Position {
            line: 0,
            character: 0,
        };
        let items = completion_at_position(&doc, &pos);

        // Should get top-level completions
        let has_keywords = items.iter().any(|i| i.kind == CompletionItemKind::Keyword);
        assert!(
            has_keywords,
            "Expected keyword completions at empty document"
        );
    }

    #[test]
    fn test_layer_completions() {
        let items = layer_completions();

        let labels: Vec<_> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"Top"));
        assert!(labels.contains(&"Bottom"));
        assert!(labels.contains(&"Inner1"));
    }

    #[test]
    fn test_property_completions_component() {
        let items = property_completions(&PropertyContext::Component);

        let labels: Vec<_> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"value"));
        assert!(labels.contains(&"at"));
        assert!(labels.contains(&"rotate"));
    }

    #[test]
    fn test_property_completions_net() {
        let items = property_completions(&PropertyContext::Net);

        let labels: Vec<_> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"width"));
        assert!(labels.contains(&"clearance"));
        assert!(labels.contains(&"current"));
    }

    #[test]
    fn test_top_level_completions_v2_keywords() {
        let items = top_level_completions();

        let labels: Vec<_> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"module"), "Should have module completion");
        assert!(
            labels.contains(&"interface"),
            "Should have interface completion"
        );
        assert!(labels.contains(&"import"), "Should have import completion");
        assert!(labels.contains(&"assert"), "Should have assert completion");
    }

    #[test]
    fn test_v2_completions_have_snippets() {
        let items = top_level_completions();

        let module = items.iter().find(|i| i.label == "module").unwrap();
        assert!(module.is_snippet, "module should be a snippet");
        assert!(module.insert_text.as_ref().unwrap().contains("module"));

        let iface = items.iter().find(|i| i.label == "interface").unwrap();
        assert!(iface.is_snippet, "interface should be a snippet");

        let import = items.iter().find(|i| i.label == "import").unwrap();
        assert!(import.is_snippet, "import should be a snippet");
        assert!(import.insert_text.as_ref().unwrap().contains("from"));
    }

    #[test]
    fn test_physical_unit_completions() {
        let items = physical_unit_completions();

        let labels: Vec<_> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"kohm"), "Should have kohm");
        assert!(labels.contains(&"nF"), "Should have nF");
        assert!(labels.contains(&"V"), "Should have V");
        assert!(labels.contains(&"MHz"), "Should have MHz");
        assert!(labels.contains(&"mA"), "Should have mA");
        assert!(labels.contains(&"W"), "Should have W");
        // Verify count matches the 23 PhysicalUnit variants
        assert_eq!(items.len(), 23, "Should have all 23 physical unit suffixes");
    }
}
