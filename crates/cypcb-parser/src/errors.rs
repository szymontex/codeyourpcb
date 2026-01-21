//! Parse error types with miette integration.
//!
//! This module defines the error types produced during parsing, all implementing
//! miette's Diagnostic trait for rich error reporting with source code snippets.
//!
//! # Example Output
//!
//! Errors display with color-coded snippets:
//! ```text
//! error[cypcb::parse::syntax]: Syntax error: expected '}'
//!   --> example.cypcb:5:1
//!   |
//! 5 | board test {
//!   |            ^ expected '}'
//! ```

use miette::{Diagnostic, SourceSpan};
use thiserror::Error;

/// The main parse error type.
///
/// All variants include source code and span information for rich error display.
#[derive(Error, Debug, Diagnostic)]
pub enum ParseError {
    /// A syntax error detected by the Tree-sitter parser.
    #[error("Syntax error: {message}")]
    #[diagnostic(code(cypcb::parse::syntax))]
    Syntax {
        /// Description of the syntax error.
        message: String,
        /// The source code being parsed.
        #[source_code]
        src: String,
        /// Location of the error.
        #[label("here")]
        span: SourceSpan,
    },

    /// An unknown component type was specified.
    #[error("Unknown component type: '{name}'")]
    #[diagnostic(
        code(cypcb::parse::unknown_component),
        help("valid types: resistor, capacitor, inductor, ic, led, connector, diode, transistor, crystal, generic")
    )]
    UnknownComponent {
        /// The unknown component type name.
        name: String,
        /// The source code being parsed.
        #[source_code]
        src: String,
        /// Location of the unknown type.
        #[label("unknown type")]
        span: SourceSpan,
    },

    /// An unknown layer type was specified in stackup.
    #[error("Unknown layer type: '{name}'")]
    #[diagnostic(
        code(cypcb::parse::unknown_layer),
        help("valid types: copper, prepreg, core, mask, silk")
    )]
    UnknownLayerType {
        /// The unknown layer type name.
        name: String,
        /// The source code being parsed.
        #[source_code]
        src: String,
        /// Location of the unknown type.
        #[label("unknown layer type")]
        span: SourceSpan,
    },

    /// An unknown unit was specified for a dimension.
    #[error("Unknown unit: '{name}'")]
    #[diagnostic(
        code(cypcb::parse::unknown_unit),
        help("valid units: mm, mil, in, nm")
    )]
    UnknownUnit {
        /// The unknown unit name.
        name: String,
        /// The source code being parsed.
        #[source_code]
        src: String,
        /// Location of the unknown unit.
        #[label("unknown unit")]
        span: SourceSpan,
    },

    /// A number could not be parsed.
    #[error("Invalid number: '{text}'")]
    #[diagnostic(code(cypcb::parse::invalid_number))]
    InvalidNumber {
        /// The text that could not be parsed as a number.
        text: String,
        /// The source code being parsed.
        #[source_code]
        src: String,
        /// Location of the invalid number.
        #[label("invalid number")]
        span: SourceSpan,
    },

    /// A required node was missing from the parse tree.
    #[error("Missing {expected}")]
    #[diagnostic(code(cypcb::parse::missing))]
    Missing {
        /// What was expected.
        expected: String,
        /// The source code being parsed.
        #[source_code]
        src: String,
        /// Location where the expected element should be.
        #[label("expected {expected}")]
        span: SourceSpan,
    },

    /// A version number is invalid.
    #[error("Invalid version: {message}")]
    #[diagnostic(code(cypcb::parse::invalid_version))]
    InvalidVersion {
        /// Description of the version error.
        message: String,
        /// The source code being parsed.
        #[source_code]
        src: String,
        /// Location of the version.
        #[label("invalid version")]
        span: SourceSpan,
    },

    /// Layer count is invalid.
    #[error("Invalid layer count: {count}")]
    #[diagnostic(
        code(cypcb::parse::invalid_layers),
        help("layer count must be 2, 4, 6, 8, 10, 12, 14, 16, 20, 24, 28, or 32")
    )]
    InvalidLayers {
        /// The invalid layer count.
        count: u32,
        /// The source code being parsed.
        #[source_code]
        src: String,
        /// Location of the layer count.
        #[label("invalid layer count")]
        span: SourceSpan,
    },
}

impl ParseError {
    /// Create a syntax error.
    pub fn syntax(message: impl Into<String>, src: impl Into<String>, span: impl Into<SourceSpan>) -> Self {
        ParseError::Syntax {
            message: message.into(),
            src: src.into(),
            span: span.into(),
        }
    }

    /// Create an unknown component error.
    pub fn unknown_component(name: impl Into<String>, src: impl Into<String>, span: impl Into<SourceSpan>) -> Self {
        ParseError::UnknownComponent {
            name: name.into(),
            src: src.into(),
            span: span.into(),
        }
    }

    /// Create an unknown layer type error.
    pub fn unknown_layer_type(name: impl Into<String>, src: impl Into<String>, span: impl Into<SourceSpan>) -> Self {
        ParseError::UnknownLayerType {
            name: name.into(),
            src: src.into(),
            span: span.into(),
        }
    }

    /// Create an unknown unit error.
    pub fn unknown_unit(name: impl Into<String>, src: impl Into<String>, span: impl Into<SourceSpan>) -> Self {
        ParseError::UnknownUnit {
            name: name.into(),
            src: src.into(),
            span: span.into(),
        }
    }

    /// Create an invalid number error.
    pub fn invalid_number(text: impl Into<String>, src: impl Into<String>, span: impl Into<SourceSpan>) -> Self {
        ParseError::InvalidNumber {
            text: text.into(),
            src: src.into(),
            span: span.into(),
        }
    }

    /// Create a missing element error.
    pub fn missing(expected: impl Into<String>, src: impl Into<String>, span: impl Into<SourceSpan>) -> Self {
        ParseError::Missing {
            expected: expected.into(),
            src: src.into(),
            span: span.into(),
        }
    }

    /// Create an invalid version error.
    pub fn invalid_version(message: impl Into<String>, src: impl Into<String>, span: impl Into<SourceSpan>) -> Self {
        ParseError::InvalidVersion {
            message: message.into(),
            src: src.into(),
            span: span.into(),
        }
    }

    /// Create an invalid layers error.
    pub fn invalid_layers(count: u32, src: impl Into<String>, span: impl Into<SourceSpan>) -> Self {
        ParseError::InvalidLayers {
            count,
            src: src.into(),
            span: span.into(),
        }
    }
}

/// Result of parsing with collected errors.
///
/// Even when there are errors, the parser tries to recover and produce
/// a partial AST. This allows tools to provide feedback on as much of
/// the source as possible.
#[derive(Debug)]
pub struct ParseResult<T> {
    /// The parsed value, may be partial if there were errors.
    pub value: T,
    /// Errors encountered during parsing.
    pub errors: Vec<ParseError>,
}

impl<T> ParseResult<T> {
    /// Create a new parse result.
    pub fn new(value: T, errors: Vec<ParseError>) -> Self {
        ParseResult { value, errors }
    }

    /// Create a successful result with no errors.
    pub fn ok(value: T) -> Self {
        ParseResult {
            value,
            errors: Vec::new(),
        }
    }

    /// Check if parsing succeeded without errors.
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }

    /// Check if parsing had errors.
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Convert to a standard Result, failing if there were any errors.
    pub fn into_result(self) -> Result<T, Vec<ParseError>> {
        if self.errors.is_empty() {
            Ok(self.value)
        } else {
            Err(self.errors)
        }
    }

    /// Map the value.
    pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> ParseResult<U> {
        ParseResult {
            value: f(self.value),
            errors: self.errors,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_syntax_error_display() {
        let err = ParseError::syntax("expected '}'", "board test {", (11, 1));
        assert!(err.to_string().contains("Syntax error"));
        assert!(err.to_string().contains("expected '}'"));
    }

    #[test]
    fn test_unknown_component_display() {
        let err = ParseError::unknown_component("foobar", "component R1 foobar", (13, 6));
        assert!(err.to_string().contains("Unknown component type"));
        assert!(err.to_string().contains("foobar"));
    }

    #[test]
    fn test_parse_result_ok() {
        let result: ParseResult<i32> = ParseResult::ok(42);
        assert!(result.is_ok());
        assert!(!result.has_errors());
        assert_eq!(result.into_result().unwrap(), 42);
    }

    #[test]
    fn test_parse_result_with_errors() {
        let errors = vec![ParseError::syntax("test", "", (0, 0))];
        let result: ParseResult<i32> = ParseResult::new(42, errors);
        assert!(!result.is_ok());
        assert!(result.has_errors());
        assert!(result.into_result().is_err());
    }
}
