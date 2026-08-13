//! DRC and parse error to LSP diagnostic conversion.
//!
//! Converts parse errors and DRC violations into LSP diagnostics for
//! display in the editor (squiggly underlines, problems panel).

use cypcb_drc::{DrcViolation, ViolationKind};
use cypcb_parser::{ImportError, ParseError};
use cypcb_world::SourceSpan as WorldSourceSpan;
use cypcb_world::SyncError;
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

    // 2. Convert semantic errors from building the model
    for error in &doc.sync_errors {
        diagnostics.push(sync_error_to_diagnostic(doc, error));
    }

    // 3. Convert imports that could not be resolved
    for error in &doc.import_errors {
        diagnostics.push(import_error_to_diagnostic(doc, error));
    }

    // 4. A fab name this tool does not have. The board is still checked -
    //    against JLCPCB - so this is a warning rather than an error, and it
    //    sits on the word that caused it because that is where the fix goes.
    if let Some(unknown) = &doc.fab_fallback {
        let available: Vec<&str> = cypcb_drc::Preset::all()
            .iter()
            .map(|preset| preset.name())
            .collect();
        let start = doc.offset_to_position(unknown.span.0);
        let end = doc.offset_to_position(unknown.span.1);
        diagnostics.push(Diagnostic {
            start_line: start.line,
            start_col: start.character,
            end_line: end.line,
            end_col: end.character,
            severity: "warning",
            code: "cypcb::unknown-fab".to_string(),
            source: "cypcb",
            message: format!(
                "The board asks for fab '{}', which is not a preset this tool has. \
                 Checking against jlcpcb instead. Available presets: {}",
                unknown.named,
                available.join(", ")
            ),
        });
    }

    // 5. Convert DRC violations (run during build_world)
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

/// A semantic error, placed where it happened.
///
/// Read through miette's `Diagnostic` rather than matched variant by variant:
/// every `SyncError` already carries the code, the message and the span the
/// command line prints, and the editor should show the same words as `cypcb
/// check` rather than a second wording that drifts from it.
fn sync_error_to_diagnostic(doc: &DocumentState, error: &SyncError) -> Diagnostic {
    use miette::Diagnostic as _;

    let code = error
        .code()
        .map(|code| code.to_string())
        .unwrap_or_else(|| "sync".to_string());

    // The first label is the one the message is about. An error from an
    // imported file carries that file's offsets, which mean nothing here, so
    // anything past the end of this document is reported at its start rather
    // than at a position invented for it.
    let span = error
        .labels()
        .and_then(|mut labels| labels.next())
        .map(|label| SourceSpan::new(label.offset().into(), label.len()))
        .filter(|span| span.offset() + span.len() <= doc.content.len())
        .unwrap_or_else(|| SourceSpan::new(0.into(), 0));

    let (start_line, start_col, end_line, end_col) = span_to_positions(doc, &span);

    Diagnostic {
        start_line,
        start_col,
        end_line,
        end_col,
        severity: "error",
        code,
        source: "cypcb-world",
        message: error.to_string(),
    }
}

/// An import that could not be resolved, placed on the statement that wrote it.
fn import_error_to_diagnostic(doc: &DocumentState, error: &ImportError) -> Diagnostic {
    use cypcb_parser::ast::Definition;

    // `ImportError` names the path as it was written, which is enough to find
    // the statement it came from and underline that rather than the file's
    // first character.
    let wanted = error.path();
    let span = doc
        .ast
        .as_ref()
        .and_then(|ast| {
            ast.definitions.iter().find_map(|def| match def {
                Definition::Import(import) if import.path.value == wanted => Some(import.span),
                _ => None,
            })
        })
        .map(|span| SourceSpan::new(span.start.into(), span.end - span.start))
        .unwrap_or_else(|| SourceSpan::new(0.into(), 0));

    let (start_line, start_col, end_line, end_col) = span_to_positions(doc, &span);

    Diagnostic {
        start_line,
        start_col,
        end_line,
        end_col,
        severity: "error",
        code: "import".to_string(),
        source: "cypcb-parser",
        message: error.to_string(),
    }
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

    /// A board named a fab, the server did not have it, and nothing was said.
    ///
    /// The CLI refuses an unknown fab outright and the viewer draws the board
    /// with a diagnostic on the word. The server checked against JLCPCB and
    /// underlined nothing - on the surface where somebody is most likely to be
    /// typing the name in the first place.
    #[test]
    fn an_unknown_fab_is_underlined_where_it_was_written() {
        let source =
            "version 1\n\nboard b {\n    size 20mm x 20mm\n    layers 2\n    fab jlpcb\n}\n";
        let doc = make_doc(source);
        let diagnostics = run_diagnostics(&doc);

        let unknown: Vec<&Diagnostic> = diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == "cypcb::unknown-fab")
            .collect();
        assert_eq!(unknown.len(), 1, "{diagnostics:?}");

        let diagnostic = unknown[0];
        assert_eq!(diagnostic.severity, "warning", "the board is still checked");
        assert!(
            diagnostic.message.contains("fab 'jlpcb'")
                && diagnostic.message.contains("oshpark_2layer"),
            "{}",
            diagnostic.message
        );

        // Line 5, zero-based: the `fab jlpcb` line, not the top of the file.
        assert_eq!(diagnostic.start_line, 5, "{diagnostic:?}");
        let line = source.lines().nth(5).expect("the file has six lines");
        let column = line.find("jlpcb").expect("the name is on that line") as u32;
        assert_eq!(
            diagnostic.start_col, column,
            "the squiggle belongs under the name, not under the keyword"
        );
    }

    /// A fab the tool does have says nothing at all.
    #[test]
    fn a_fab_the_tool_has_is_not_reported() {
        let doc = make_doc(
            "version 1\n\nboard b {\n    size 20mm x 20mm\n    layers 2\n    fab oshpark\n}\n",
        );
        assert!(
            !run_diagnostics(&doc)
                .iter()
                .any(|diagnostic| diagnostic.code == "cypcb::unknown-fab"),
            "oshpark is a preset this tool has"
        );
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
