//! Go-to-definition provider for the CodeYourPCB LSP.
//!
//! Provides navigation from references to their definitions:
//! - Pin references (R1.1) -> component definition
//! - Net names in assignments -> net definition
//! - Custom footprint names -> footprint definition

use cypcb_parser::ast::{Definition, SourceFile, Span};

use crate::document::{DocumentState, Position};

/// The kind of definition being looked up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefinitionKind {
    /// A component (e.g., R1, C1, U1).
    Component,
    /// A net (e.g., VCC, GND).
    Net,
    /// A custom footprint.
    Footprint,
}

/// A location in the source document.
#[derive(Debug, Clone)]
pub struct Location {
    /// Start line (0-indexed).
    pub start_line: u32,
    /// Start column (0-indexed).
    pub start_col: u32,
    /// End line (0-indexed).
    pub end_line: u32,
    /// End column (0-indexed).
    pub end_col: u32,
}

impl Location {
    /// Create a location from a span in the document.
    pub fn from_span(doc: &DocumentState, span: &Span) -> Self {
        let start = doc.offset_to_position(span.start);
        let end = doc.offset_to_position(span.end);
        Location {
            start_line: start.line,
            start_col: start.character,
            end_line: end.line,
            end_col: end.character,
        }
    }
}

/// Go to the definition of the symbol at the given position.
///
/// Returns the location of the definition if found, or None if:
/// - The cursor is not on a navigable symbol
/// - The symbol is a built-in (no source definition)
/// - The symbol is undefined
pub fn goto_definition(doc: &DocumentState, position: &Position) -> Option<Location> {
    let offset = doc.position_to_offset(position)?;
    let ast = doc.ast.as_ref()?;

    // Find what's at this position
    for def in &ast.definitions {
        match def {
            Definition::Component(comp) => {
                // Check net assignments - navigate to net definition
                for assign in &comp.net_assignments {
                    if offset >= assign.net.span.start && offset < assign.net.span.end {
                        // Navigate to net definition
                        return find_definition_location(ast, &assign.net.value, DefinitionKind::Net)
                            .map(|span| Location::from_span(doc, &span));
                    }
                }

                // Check footprint - navigate to custom footprint definition
                let fp_span = &comp.footprint.span;
                // Check if we're in the footprint string content (not just the span)
                if offset >= fp_span.start && offset < fp_span.end {
                    return find_definition_location(ast, &comp.footprint.value, DefinitionKind::Footprint)
                        .map(|span| Location::from_span(doc, &span));
                }
            }
            Definition::Net(net) => {
                // Check pin references - navigate to component definition
                for conn in &net.connections {
                    if offset >= conn.span.start && offset < conn.span.end {
                        // Navigate to component definition
                        return find_component_location(ast, &conn.component.value)
                            .map(|span| Location::from_span(doc, &span));
                    }
                }
            }
            Definition::Trace(trace) => {
                // Check from pin reference
                if let Some(from) = &trace.from {
                    if offset >= from.span.start && offset < from.span.end {
                        return find_component_location(ast, &from.component.value)
                            .map(|span| Location::from_span(doc, &span));
                    }
                }

                // Check to pin reference
                if let Some(to) = &trace.to {
                    if offset >= to.span.start && offset < to.span.end {
                        return find_component_location(ast, &to.component.value)
                            .map(|span| Location::from_span(doc, &span));
                    }
                }

                // Check net name
                if offset >= trace.net.span.start && offset < trace.net.span.end {
                    return find_definition_location(ast, &trace.net.value, DefinitionKind::Net)
                        .map(|span| Location::from_span(doc, &span));
                }
            }
            Definition::Zone(zone) => {
                // Check net name in copper pour
                if let Some(net) = &zone.net {
                    if offset >= net.span.start && offset < net.span.end {
                        return find_definition_location(ast, &net.value, DefinitionKind::Net)
                            .map(|span| Location::from_span(doc, &span));
                    }
                }
            }
            _ => {}
        }
    }

    None
}

/// Find the location of a component definition by refdes.
fn find_component_location(ast: &SourceFile, refdes: &str) -> Option<Span> {
    for def in &ast.definitions {
        if let Definition::Component(comp) = def {
            if comp.refdes.value == refdes {
                return Some(comp.span);
            }
        }
    }
    None
}

/// Find the location of a definition by name and kind.
pub fn find_definition_location(ast: &SourceFile, name: &str, kind: DefinitionKind) -> Option<Span> {
    for def in &ast.definitions {
        match (def, kind) {
            (Definition::Component(comp), DefinitionKind::Component) => {
                if comp.refdes.value == name {
                    return Some(comp.span);
                }
            }
            (Definition::Net(net), DefinitionKind::Net) => {
                if net.name.value == name {
                    return Some(net.span);
                }
            }
            (Definition::Footprint(fp), DefinitionKind::Footprint) => {
                if fp.name.value == name {
                    return Some(fp.span);
                }
            }
            _ => {}
        }
    }
    None
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
    fn test_goto_component_from_pin_ref() {
        let doc = make_doc(r#"
component R1 resistor "0402" {
    at 10mm, 10mm
}
component C1 capacitor "0603" {}

net VCC {
    R1.1
    C1.1
}
"#);

        // Position on "R1" in the pin reference R1.1
        // Net VCC starts at around line 7, pin refs are on line 8 and 9
        let pos = Position { line: 7, character: 4 };
        let loc = goto_definition(&doc, &pos);

        assert!(loc.is_some(), "Expected to find R1 component definition");
        let loc = loc.unwrap();
        // R1 component should be on line 1
        assert_eq!(loc.start_line, 1);
    }

    #[test]
    fn test_goto_net_from_assignment() {
        let doc = make_doc(r#"
net VCC {
    R1.1
}

component R1 resistor "0402" {
    pin.1 = VCC
}
"#);

        // Position on "VCC" in the assignment
        let pos = Position { line: 6, character: 12 };
        let loc = goto_definition(&doc, &pos);

        assert!(loc.is_some(), "Expected to find VCC net definition");
        let loc = loc.unwrap();
        // VCC net should be on line 1
        assert_eq!(loc.start_line, 1);
    }

    #[test]
    fn test_goto_custom_footprint() {
        let doc = make_doc(r#"
footprint MY_FP {
    pad 1 rect at 0mm, 0mm size 1mm x 0.5mm
}

component R1 resistor "MY_FP" {}
"#);

        // Position inside the footprint string "MY_FP"
        // Component is on line 5, footprint string is around character 24
        let pos = Position { line: 5, character: 25 };
        let loc = goto_definition(&doc, &pos);

        assert!(loc.is_some(), "Expected to find MY_FP footprint definition");
        let loc = loc.unwrap();
        // Footprint should be on line 1
        assert_eq!(loc.start_line, 1);
    }

    #[test]
    fn test_goto_builtin_footprint_returns_none() {
        let doc = make_doc(r#"
component R1 resistor "0402" {}
"#);

        // Position inside the footprint string "0402"
        let pos = Position { line: 1, character: 24 };
        let loc = goto_definition(&doc, &pos);

        // Built-in footprints have no definition location
        assert!(loc.is_none(), "Built-in footprints should not have definition location");
    }

    #[test]
    fn test_goto_component_from_trace() {
        let doc = make_doc(r#"
component R1 resistor "0402" {}
component C1 capacitor "0603" {}

trace VCC {
    from R1.1
    to C1.1
}
"#);

        // Position on "R1" in "from R1.1"
        let pos = Position { line: 5, character: 9 };
        let loc = goto_definition(&doc, &pos);

        assert!(loc.is_some(), "Expected to find R1 component from trace");
        let loc = loc.unwrap();
        // R1 component should be on line 1
        assert_eq!(loc.start_line, 1);
    }

    #[test]
    fn test_goto_net_from_trace() {
        let doc = make_doc(r#"
net VCC { R1.1 }

trace VCC {
    from R1.1
}
"#);

        // Position on "VCC" net name in trace definition
        let pos = Position { line: 3, character: 6 };
        let loc = goto_definition(&doc, &pos);

        assert!(loc.is_some(), "Expected to find VCC net from trace");
        let loc = loc.unwrap();
        // VCC net should be on line 1
        assert_eq!(loc.start_line, 1);
    }

    #[test]
    fn test_find_definition_location_component() {
        let doc = make_doc(r#"
component R1 resistor "0402" {}
component C1 capacitor "0603" {}
"#);

        let ast = doc.ast.as_ref().unwrap();
        let span = find_definition_location(ast, "C1", DefinitionKind::Component);
        assert!(span.is_some());
    }

    #[test]
    fn test_find_definition_location_net() {
        let doc = make_doc(r#"
net VCC { R1.1 }
net GND { R1.2 }
"#);

        let ast = doc.ast.as_ref().unwrap();
        let span = find_definition_location(ast, "GND", DefinitionKind::Net);
        assert!(span.is_some());
    }
}
