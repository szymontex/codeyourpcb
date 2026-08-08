//! DRC and parse error to LSP diagnostic conversion.
//!
//! Converts parse errors and DRC violations into LSP diagnostics for
//! display in the editor (squiggly underlines, problems panel).

use cypcb_drc::{DrcViolation, ViolationKind};
use cypcb_parser::ParseError;
use cypcb_world::SourceSpan as WorldSourceSpan;
use miette::SourceSpan;

use crate::document::DocumentState;

/// LSP-style diagnostic information.
///
/// This is a simplified version that doesn't depend on tower-lsp types,
/// making it usable both with and without the server feature.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    /// Start line (0-indexed).
    pub start_line: u32,
    /// Start column (0-indexed).
    pub start_col: u32,
    /// End line (0-indexed).
    pub end_line: u32,
    /// End column (0-indexed).
    pub end_col: u32,
    /// Severity: "error", "warning", "info", "hint".
    pub severity: &'static str,
    /// Error code.
    pub code: String,
    /// Source identifier.
    pub source: &'static str,
    /// Human-readable message.
    pub message: String,
}

/// Maximum number of diagnostics to report per file.
const MAX_DIAGNOSTICS: usize = 100;

/// Run diagnostics on a document and return diagnostics.
pub fn run_diagnostics(doc: &DocumentState) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    // 1. Convert parse errors
    for error in &doc.parse_errors {
        if let Some(diag) = parse_error_to_diagnostic(doc, error) {
            diagnostics.push(diag);
        }
    }

    // 2. Convert DRC violations (run during build_world)
    for violation in &doc.drc_violations {
        if let Some(diag) = violation_to_diagnostic(doc, violation) {
            diagnostics.push(diag);
        }
    }

    // Cap diagnostics
    if diagnostics.len() > MAX_DIAGNOSTICS {
        let overflow = diagnostics.len() - MAX_DIAGNOSTICS;
        diagnostics.truncate(MAX_DIAGNOSTICS);

        diagnostics.push(Diagnostic {
            start_line: 0,
            start_col: 0,
            end_line: 0,
            end_col: 0,
            severity: "info",
            code: "cypcb::overflow".to_string(),
            source: "cypcb",
            message: format!("... and {} more diagnostics (truncated)", overflow),
        });
    }

    diagnostics
}

fn parse_error_to_diagnostic(doc: &DocumentState, error: &ParseError) -> Option<Diagnostic> {
    let (message, span) = match error {
        ParseError::Syntax { message, span, .. } => (message.clone(), span),
        ParseError::UnknownComponent { name, span, .. } => {
            (format!("Unknown component type: '{}'", name), span)
        }
        ParseError::UnknownLayerType { name, span, .. } => {
            (format!("Unknown layer type: '{}'", name), span)
        }
        ParseError::UnknownUnit { name, span, .. } => (format!("Unknown unit: '{}'", name), span),
        ParseError::InvalidNumber { text, span, .. } => {
            (format!("Invalid number: '{}'", text), span)
        }
        ParseError::Missing { expected, span, .. } => (format!("Missing {}", expected), span),
        ParseError::InvalidVersion { message, span, .. } => {
            (format!("Invalid version: {}", message), span)
        }
        ParseError::InvalidLayers { count, span, .. } => {
            (format!("Invalid layer count: {}", count), span)
        }
        ParseError::InvalidModule { message, span, .. } => {
            (format!("Invalid module: {}", message), span)
        }
        ParseError::InvalidInterface { message, span, .. } => {
            (format!("Invalid interface: {}", message), span)
        }
        ParseError::InvalidImport { message, span, .. } => {
            (format!("Invalid import: {}", message), span)
        }
        ParseError::InvalidAssert { message, span, .. } => {
            (format!("Invalid assert: {}", message), span)
        }
        ParseError::InvalidPhysicalUnit { name, span, .. } => {
            (format!("Invalid physical unit: '{}'", name), span)
        }
        ParseError::InvalidTolerance { message, span, .. } => {
            (format!("Invalid tolerance: {}", message), span)
        }
        ParseError::UnknownProperty {
            block,
            found,
            known,
            span,
            ..
        } => (
            format!("`{block}` has no property `{found}`. It takes: {known}"),
            span,
        ),
    };

    let (start_line, start_col, end_line, end_col) = span_to_positions(doc, span);

    Some(Diagnostic {
        start_line,
        start_col,
        end_line,
        end_col,
        severity: "error",
        code: error_code(error),
        source: "cypcb-parser",
        message,
    })
}

fn error_code(error: &ParseError) -> String {
    match error {
        ParseError::Syntax { .. } => "syntax",
        ParseError::UnknownComponent { .. } => "unknown-component",
        ParseError::UnknownLayerType { .. } => "unknown-layer",
        ParseError::UnknownUnit { .. } => "unknown-unit",
        ParseError::InvalidNumber { .. } => "invalid-number",
        ParseError::Missing { .. } => "missing",
        ParseError::InvalidVersion { .. } => "invalid-version",
        ParseError::InvalidLayers { .. } => "invalid-layers",
        ParseError::InvalidModule { .. } => "invalid-module",
        ParseError::InvalidInterface { .. } => "invalid-interface",
        ParseError::InvalidImport { .. } => "invalid-import",
        ParseError::InvalidAssert { .. } => "invalid-assert",
        ParseError::InvalidPhysicalUnit { .. } => "invalid-physical-unit",
        ParseError::InvalidTolerance { .. } => "invalid-tolerance",
        ParseError::UnknownProperty { .. } => "unknown-property",
    }
    .to_string()
}

fn violation_to_diagnostic(doc: &DocumentState, violation: &DrcViolation) -> Option<Diagnostic> {
    let (start_line, start_col, end_line, end_col) = if let Some(span) = &violation.source_span {
        span_to_positions(doc, &SourceSpan::from(*span))
    } else if let Some(span) = declaration_span(doc, violation) {
        span_to_positions(doc, &span)
    } else {
        (0, 0, 0, 0)
    };

    Some(Diagnostic {
        start_line,
        start_col,
        end_line,
        end_col,
        severity: severity_for(violation),
        code: format!("{}", violation.kind),
        source: "cypcb-drc",
        message: violation.message.clone(),
    })
}

/// Where the thing a violation is about was written.
///
/// `DrcViolation::source_span` is `Some` nowhere in `cypcb-drc` - every
/// constructor sets it to `None` - so every design rule violation arrived in
/// the editor at line 0, character 0. A file with twenty parts got twenty
/// squiggles stacked on its first character, none of them pointing at the part
/// they named.
///
/// The board model already knows: `sync_ast_to_world` spawns components and
/// traces with a `SourceSpan`, so the entity the violation names carries the
/// offsets of its own declaration. This reads them back.
fn declaration_span(doc: &DocumentState, violation: &DrcViolation) -> Option<SourceSpan> {
    let world = doc.world.as_ref()?;
    let span = world
        .get::<WorldSourceSpan>(violation.entity)
        .or_else(|| violation.other_entity.and_then(|e| world.get(e)))?;
    Some(SourceSpan::new(
        span.start_byte.into(),
        span.end_byte.saturating_sub(span.start_byte),
    ))
}

/// How loudly the editor should draw a violation.
///
/// Everything the design rule check finds used to be an error, which makes an
/// editor useless while a board is being written: a part exists before its net
/// does, so "Unconnected pin" and "on a net that no copper reaches" are what a
/// board in progress *is*, not what is wrong with it. Those two are warnings.
/// Everything else - copper too close, a hole too small, an assertion the
/// designer wrote and broke - stays an error, because each is a board that
/// cannot be made.
fn severity_for(violation: &DrcViolation) -> &'static str {
    match violation.kind {
        ViolationKind::UnconnectedPin | ViolationKind::UnroutedPin => "warning",
        _ => "error",
    }
}

fn span_to_positions(doc: &DocumentState, span: &SourceSpan) -> (u32, u32, u32, u32) {
    let start = doc.offset_to_position(span.offset());
    let end = doc.offset_to_position(span.offset() + span.len());
    (start.line, start.character, end.line, end.character)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_doc(content: &str) -> DocumentState {
        let mut doc = DocumentState::new("test://file".into(), content.to_string(), 1);
        doc.parse();
        doc.build_world();
        doc
    }

    #[test]
    fn test_clean_document_no_parse_errors() {
        let doc = make_doc(
            r#"
version 1

board test {
    size 30mm x 20mm
    layers 2
}

component R1 resistor "0402" {
    at 10mm, 8mm
}
"#,
        );

        let diagnostics = run_diagnostics(&doc);
        for diag in &diagnostics {
            assert_ne!(diag.source, "cypcb-parser", "Should not have parse errors");
        }
    }

    #[test]
    fn test_parse_error_diagnostic() {
        // Use invalid syntax that should trigger a parse error
        let doc = make_doc("component R1 unknown_type \"bad\" {");

        let diagnostics = run_diagnostics(&doc);
        let parse_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.source == "cypcb-parser")
            .collect();

        // Should have an error for unknown component type
        assert!(!parse_errors.is_empty(), "Should have parse error");
    }

    #[test]
    fn a_flood_of_problems_is_truncated_and_says_so() {
        // This used to assert `MAX_DIAGNOSTICS >= 50` and `<= 200`, which
        // compares two literals and holds whatever the code does. What matters
        // is the behaviour the constant governs: an editor handed thousands of
        // diagnostics stops being usable, so the list is cut and the last entry
        // says how many were dropped. A cut that loses the count silently is
        // worse than no cut.
        // Broken syntax will not do it - the parser collapses a file of
        // garbage into one error, which is how the first version of this test
        // passed while checking nothing. Overlapping parts will: twenty of them
        // in the same spot is 190 pairs for the courtyard rule to complain
        // about.
        let mut source =
            String::from("version 1\n\nboard flood {\n    size 30mm x 20mm\n    layers 2\n}\n\n");
        for i in 0..20 {
            source.push_str(&format!(
                "component R{i} resistor \"0402\" {{\n    value 10kohm\n    at 10mm, 10mm\n}}\n\n"
            ));
        }
        let doc = make_doc(&source);

        let diagnostics = run_diagnostics(&doc);
        assert_eq!(
            diagnostics.len(),
            MAX_DIAGNOSTICS + 1,
            "the list is capped at {MAX_DIAGNOSTICS} plus the note that says so"
        );

        let last = diagnostics.last().expect("the overflow note");
        assert_eq!(last.code, "cypcb::overflow");
        assert!(
            last.message.contains("more diagnostics"),
            "the note has to say how many were dropped: {}",
            last.message
        );
    }

    #[test]
    fn test_drc_violation_diagnostic() {
        // Create a document with components that have unconnected pins
        // This will trigger DRC violations
        let doc = make_doc(
            r#"
version 1

board test {
    size 30mm x 20mm
    layers 2
}

component R1 resistor "0402" {
    at 10mm, 8mm
}
"#,
        );

        let diagnostics = run_diagnostics(&doc);

        // R1 has 2 pins, neither connected to any net -> 2 unconnected pin violations
        let drc_errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.source == "cypcb-drc")
            .collect();

        // Should have unconnected pin errors
        assert!(
            !drc_errors.is_empty(),
            "Should have DRC errors for unconnected pins"
        );
    }
}
