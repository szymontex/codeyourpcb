//! Document state management for the LSP server.
//!
//! Tracks open documents, their content, parsed ASTs, and board worlds.

use cypcb_parser::ast::SourceFile;
use cypcb_parser::ParseError;
use cypcb_world::BoardWorld;

/// Position in a document (LSP-style, 0-indexed).
#[derive(Debug, Clone, Copy, Default)]
pub struct Position {
    pub line: u32,
    pub character: u32,
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
}

impl DocumentState {
    /// Create a new document state.
    pub fn new(content: String, version: i32) -> Self {
        DocumentState {
            content,
            version,
            ast: None,
            world: None,
            parse_errors: Vec::new(),
        }
    }

    /// Update the document content and clear cached state.
    pub fn update(&mut self, content: String, version: i32) {
        self.content = content;
        self.version = version;
        self.ast = None;
        self.world = None;
        self.parse_errors.clear();
    }

    /// Parse the document content and update AST and errors.
    pub fn parse(&mut self) {
        use cypcb_parser::parse;

        let result = parse(&self.content);
        self.ast = Some(result.value);
        self.parse_errors = result.errors;
    }

    /// Build the board world from the AST.
    ///
    /// Returns true if the world was built successfully.
    pub fn build_world(&mut self) -> bool {
        use cypcb_world::footprint::FootprintLibrary;
        use cypcb_world::sync::sync_ast_to_world;

        if let Some(ast) = &self.ast {
            let mut world = BoardWorld::new();
            let library = FootprintLibrary::new();
            let sync_result = sync_ast_to_world(ast, &self.content, &mut world, &library);

            self.world = Some(world);
            sync_result.is_ok()
        } else {
            self.world = None;
            false
        }
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

        Position { line, character: col }
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

    #[test]
    fn test_document_state_new() {
        let doc = DocumentState::new("version 1".into(), 1);

        assert_eq!(doc.content, "version 1");
        assert_eq!(doc.version, 1);
        assert!(doc.ast.is_none());
        assert!(doc.world.is_none());
        assert!(doc.parse_errors.is_empty());
    }

    #[test]
    fn test_document_update() {
        let mut doc = DocumentState::new("version 1".into(), 1);
        doc.update("version 2".into(), 2);

        assert_eq!(doc.content, "version 2");
        assert_eq!(doc.version, 2);
    }

    #[test]
    fn test_offset_to_position_simple() {
        let doc = DocumentState::new("hello\nworld".into(), 1);

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
        let doc = DocumentState::new("hello\nworld".into(), 1);

        let offset = doc.position_to_offset(&Position { line: 0, character: 0 });
        assert_eq!(offset, Some(0));

        let offset = doc.position_to_offset(&Position { line: 0, character: 3 });
        assert_eq!(offset, Some(3));

        let offset = doc.position_to_offset(&Position { line: 1, character: 0 });
        assert_eq!(offset, Some(6));

        let offset = doc.position_to_offset(&Position { line: 1, character: 3 });
        assert_eq!(offset, Some(9));
    }
}
