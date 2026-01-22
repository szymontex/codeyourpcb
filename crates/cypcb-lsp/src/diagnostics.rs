//! DRC and parse error to LSP diagnostic conversion.
//!
//! Converts parse errors and DRC violations into LSP diagnostics for
//! display in the editor (squiggly underlines, problems panel).

use cypcb_drc::DrcViolation;
use cypcb_parser::ParseError;
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
        ParseError::UnknownUnit { name, span, .. } => {
            (format!("Unknown unit: '{}'", name), span)
        }
        ParseError::InvalidNumber { text, span, .. } => {
            (format!("Invalid number: '{}'", text), span)
        }
        ParseError::Missing { expected, span, .. } => {
            (format!("Missing {}", expected), span)
        }
        ParseError::InvalidVersion { message, span, .. } => {
            (format!("Invalid version: {}", message), span)
        }
        ParseError::InvalidLayers { count, span, .. } => {
            (format!("Invalid layer count: {}", count), span)
        }
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
    }
    .to_string()
}

fn violation_to_diagnostic(doc: &DocumentState, violation: &DrcViolation) -> Option<Diagnostic> {
    let (start_line, start_col, end_line, end_col) = if let Some(span) = &violation.source_span {
        span_to_positions(doc, &SourceSpan::from(*span))
    } else {
        (0, 0, 0, 0)
    };

    Some(Diagnostic {
        start_line,
        start_col,
        end_line,
        end_col,
        severity: "error",
        code: format!("{}", violation.kind),
        source: "cypcb-drc",
        message: violation.message.clone(),
    })
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
    fn test_diagnostic_limit() {
        assert!(MAX_DIAGNOSTICS >= 50);
        assert!(MAX_DIAGNOSTICS <= 200);
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
