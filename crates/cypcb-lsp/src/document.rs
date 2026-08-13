//! Document state management for the LSP server.
//!
//! Tracks open documents, their content, parsed ASTs, and board worlds.

use std::path::PathBuf;

use cypcb_drc::DrcViolation;
use cypcb_parser::ast::SourceFile;
use cypcb_parser::{ImportError, ParseError};
use cypcb_world::{BoardWorld, SyncError};

/// Position in a document (LSP-style, 0-indexed).
#[derive(Debug, Clone, Copy, Default)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

/// The filesystem path a document URI names, when it names one.
///
/// Editors send `file:///home/x/board.cypcb`. Tests and the wasm host send
/// things like `test://file`, which are not paths - those documents simply
/// cannot resolve an import, and saying so with `None` is better than
/// inventing a directory to resolve against.
fn path_of(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    // `file:///path` on unix, `file://host/path` is not something an editor
    // sends for a local file.
    Some(PathBuf::from(rest))
}

/// State of an open document.
///
/// Holds the document content, parsed AST, and optionally a built BoardWorld.
/// The board world is lazily constructed when needed for DRC.
pub struct DocumentState {
    /// Document content.
    pub content: String,
    /// Document version (incremented on each change).
    pub version: i32,
    /// Parsed AST (if parsing succeeded or recovered).
    pub ast: Option<SourceFile>,
    /// Built board world (lazy, for DRC).
    pub world: Option<BoardWorld>,
    /// Parse errors encountered.
    pub parse_errors: Vec<ParseError>,
    /// DRC violations from the last check.
    pub drc_violations: Vec<DrcViolation>,
    /// Semantic errors from building the board model.
    ///
    /// Collected and thrown away until now: the editor was told about parse
    /// errors and DRC violations only, so an unknown footprint, a duplicate
    /// refdes or a module pin left unconnected - every one of which makes
    /// `cypcb check` exit 1 - drew no squiggle at all.
    pub sync_errors: Vec<SyncError>,
    /// Imports that could not be resolved.
    pub import_errors: Vec<ImportError>,
    /// Where this document lives, when it lives anywhere.
    ///
    /// `import "lib/blocks.cypcb"` resolves against the importing file's own
    /// directory, so a document that does not know its own path cannot follow
    /// one. This used to be `DocumentState::new(_uri, ...)` - the URI arrived
    /// and was dropped - which is why a design split across files came up
    /// empty in the editor and checked fine on the command line.
    pub path: Option<PathBuf>,
}

impl DocumentState {
    /// Create a new document state.
    pub fn new(uri: String, content: String, version: i32) -> Self {
        DocumentState {
            content,
            version,
            ast: None,
            world: None,
            parse_errors: Vec::new(),
            drc_violations: Vec::new(),
            sync_errors: Vec::new(),
            import_errors: Vec::new(),
            path: path_of(&uri),
        }
    }

    /// Update the document content and clear cached state.
    pub fn update(&mut self, content: String, version: i32) {
        self.content = content;
        self.version = version;
        self.ast = None;
        self.world = None;
        self.parse_errors.clear();
        self.drc_violations.clear();
        self.sync_errors.clear();
        self.import_errors.clear();
    }

    /// Parse the document content and update AST and errors.
    pub fn parse(&mut self) {
        use cypcb_parser::parse;

        let result = parse(&self.content);
        self.ast = Some(result.value);
        self.parse_errors = result.errors;
    }

    /// Build the board world from the AST and run DRC.
    ///
    /// Returns true if the world was built successfully.
    pub fn build_world(&mut self) -> bool {
        use cypcb_drc::{run_drc, PresetRules};
        use cypcb_world::footprint::FootprintLibrary;
        use cypcb_world::sync::sync_ast_to_world;

        let Some(ast) = &self.ast else {
            self.world = None;
            self.drc_violations.clear();
            self.sync_errors.clear();
            self.import_errors.clear();
            return false;
        };

        // What this file imports, resolved against its own directory, exactly
        // as every CLI command resolves it. The resolved AST is used to build
        // the model and never stored: hover, go-to-definition and completion
        // read `self.ast`, whose spans point into this document's text, and an
        // imported definition's span points into another file.
        self.import_errors.clear();
        let resolved = match &self.path {
            Some(path) => cypcb_parser::resolve_imports(ast, path, &mut self.import_errors),
            None => ast.clone(),
        };

        let mut world = BoardWorld::new();
        let mut library = FootprintLibrary::new();
        let sync_result = sync_ast_to_world(&resolved, &self.content, &mut world, &mut library);
        self.sync_errors = sync_result.errors.clone();

        // Run DRC against the fab the board named, which is the same question
        // `cypcb check` and the browser both ask. This was `DesignRules::default()`
        // - JLCPCB - on every document, so a board written `fab oshpark` was
        // underlined in the editor against a table it was never meant for.
        //
        // A name this tool does not have falls back rather than failing, the way
        // the viewer does: a language server that stops reporting anything
        // because one word is wrong is worse than one checking against the
        // default. Unlike the viewer, nothing here says so yet - recorded.
        let preset = world
            .fab()
            .and_then(cypcb_drc::Preset::from_name)
            .unwrap_or(cypcb_drc::Preset::JlcpcbStandard2Layer);
        let rules = preset.rules();
        let drc_result = run_drc(&mut world, &rules);
        self.drc_violations = drc_result.violations;

        self.world = Some(world);
        sync_result.is_ok()
    }

    /// Convert a byte offset to a Position.
    pub fn offset_to_position(&self, offset: usize) -> Position {
        let mut line = 0u32;
        let mut col = 0u32;
        let mut current_offset = 0usize;

        for ch in self.content.chars() {
            if current_offset >= offset {
                break;
            }
            if ch == '\n' {
                line += 1;
                col = 0;
            } else {
                col += 1;
            }
            current_offset += ch.len_utf8();
        }

        Position {
            line,
            character: col,
        }
    }

    /// Convert a Position to a byte offset.
    pub fn position_to_offset(&self, position: &Position) -> Option<usize> {
        let mut current_line = 0u32;
        let mut current_col = 0u32;
        let mut offset = 0usize;

        for ch in self.content.chars() {
            if current_line == position.line && current_col == position.character {
                return Some(offset);
            }
            if ch == '\n' {
                if current_line == position.line {
                    return Some(offset);
                }
                current_line += 1;
                current_col = 0;
            } else {
                current_col += 1;
            }
            offset += ch.len_utf8();
        }

        if current_line == position.line && current_col == position.character {
            return Some(offset);
        }

        if current_line == position.line {
            return Some(self.content.len());
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two traces 0.14mm apart: JLCPCB images that gap and OSHPark does not.
    ///
    /// A discriminator rather than a guess - `cypcb check --preset jlcpcb`
    /// reports no clearance violation on this board and `--preset oshpark`
    /// reports one, so which table the server used is readable off the count.
    fn two_traces_a_hair_apart(fab_line: &str) -> String {
        format!(
            "version 1\n\n\
             board t {{\n    size 20mm x 20mm\n    layers 2\n{fab_line}}}\n\n\
             component R1 resistor \"0402\" {{\n    value \"10k\"\n    at 5mm, 5mm\n}}\n\n\
             component R2 resistor \"0402\" {{\n    value \"10k\"\n    at 15mm, 5mm\n}}\n\n\
             net A {{\n    R1.1\n    R2.1\n}}\n\n\
             net B {{\n    R1.2\n    R2.2\n}}\n\n\
             trace A {{\n    layer Top\n    width 0.127mm\n    path 5mm,10mm -> 15mm,10mm\n}}\n\n\
             trace B {{\n    layer Top\n    width 0.127mm\n    path 5mm,10.267mm -> 15mm,10.267mm\n}}\n"
        )
    }

    /// How many clearance violations the server would underline.
    fn clearance_violations(source: String) -> usize {
        let mut doc = DocumentState::new("test://file".into(), source, 1);
        // `new` stores the text and nothing else - `test_document_state_new`
        // asserts `ast.is_none()` right after it. Without this the world is
        // empty, every count is zero, and the two zero-expecting cases below
        // pass while proving nothing.
        doc.parse();
        assert!(doc.parse_errors.is_empty(), "{:?}", doc.parse_errors);
        assert!(doc.build_world(), "{:?}", doc.sync_errors);
        doc.drc_violations
            .iter()
            .filter(|violation| violation.kind.to_string() == "clearance")
            .count()
    }

    /// The server checked every document against JLCPCB whatever the board
    /// said, so a design written for OSHPark was underlined - or not - against
    /// a table it was never meant for.
    #[test]
    fn the_server_checks_against_the_fab_the_board_named() {
        assert_eq!(
            clearance_violations(two_traces_a_hair_apart("    fab oshpark\n")),
            1,
            "OSHPark does not image a 0.14mm gap and the board asked for OSHPark"
        );
        assert_eq!(
            clearance_violations(two_traces_a_hair_apart("")),
            0,
            "JLCPCB does image it, and a board naming no fab is checked against JLCPCB"
        );
        assert_eq!(
            clearance_violations(two_traces_a_hair_apart("    fab jlpcb\n")),
            0,
            "a name this tool does not have falls back rather than reporting nothing at all"
        );
    }

    #[test]
    fn test_document_state_new() {
        let doc = DocumentState::new("test://file".into(), "version 1".into(), 1);

        assert_eq!(doc.content, "version 1");
        assert_eq!(doc.version, 1);
        assert!(doc.ast.is_none());
        assert!(doc.world.is_none());
        assert!(doc.parse_errors.is_empty());
    }

    #[test]
    fn test_document_update() {
        let mut doc = DocumentState::new("test://file".into(), "version 1".into(), 1);
        doc.update("version 2".into(), 2);

        assert_eq!(doc.content, "version 2");
        assert_eq!(doc.version, 2);
    }

    #[test]
    fn test_offset_to_position_simple() {
        let doc = DocumentState::new("test://file".into(), "hello\nworld".into(), 1);

        let pos = doc.offset_to_position(0);
        assert_eq!(pos.line, 0);
        assert_eq!(pos.character, 0);

        let pos = doc.offset_to_position(3);
        assert_eq!(pos.line, 0);
        assert_eq!(pos.character, 3);

        let pos = doc.offset_to_position(6);
        assert_eq!(pos.line, 1);
        assert_eq!(pos.character, 0);

        let pos = doc.offset_to_position(9);
        assert_eq!(pos.line, 1);
        assert_eq!(pos.character, 3);
    }

    #[test]
    fn test_position_to_offset_simple() {
        let doc = DocumentState::new("test://file".into(), "hello\nworld".into(), 1);

        let offset = doc.position_to_offset(&Position {
            line: 0,
            character: 0,
        });
        assert_eq!(offset, Some(0));

        let offset = doc.position_to_offset(&Position {
            line: 0,
            character: 3,
        });
        assert_eq!(offset, Some(3));

        let offset = doc.position_to_offset(&Position {
            line: 1,
            character: 0,
        });
        assert_eq!(offset, Some(6));

        let offset = doc.position_to_offset(&Position {
            line: 1,
            character: 3,
        });
        assert_eq!(offset, Some(9));
    }
}
