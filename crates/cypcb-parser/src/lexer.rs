//! Tokens for the Rust reader.
//!
//! Step one of `docs/one-parser.md`: the language is read twice today, and the
//! reader that survives is the one that can be carried into a browser. C
//! cannot, so this is the tokenizer the Rust reader runs on.
//!
//! It is deliberately small. The DSL is blocks of `keyword value` lines, so
//! the token set is identifiers, numbers, strings, punctuation and the two
//! comment forms - no operator precedence, no nesting beyond braces.

use crate::ast::Span;

/// One token, with the bytes it came from.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    /// What it is.
    pub kind: TokenKind,
    /// Where it is, as byte offsets into the source.
    pub span: Span,
}

/// The kinds of token this language has.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    /// A bare word: keyword, name, unit, layer.
    Ident(String),
    /// A number, with the text it was written as so `2` and `2.0` stay apart.
    Number(f64),
    /// A quoted string, without its quotes.
    Str(String),
    /// `{`
    LBrace,
    /// `}`
    RBrace,
    /// `[`
    LBracket,
    /// `]`
    RBracket,
    /// `,`
    Comma,
    /// `.`
    Dot,
    /// `=`
    Equals,
    /// `->`
    Arrow,
    /// Anything the language does not use, kept so the reader can complain
    /// about it with a position rather than skipping it.
    Unknown(char),
}

impl TokenKind {
    /// The word this token carries, if it is one.
    pub fn ident(&self) -> Option<&str> {
        match self {
            TokenKind::Ident(word) => Some(word.as_str()),
            _ => None,
        }
    }
}

/// Turn source into tokens.
///
/// Comments and whitespace are dropped. Nothing here fails: an unexpected
/// character becomes `Unknown` and the reader decides what to say about it,
/// which keeps error messages in one place.
pub fn tokenize(source: &str) -> Vec<Token> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0usize;

    while i < bytes.len() {
        let start = i;
        let c = bytes[i] as char;

        // Whitespace
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }

        // Comments: `// to end of line` and `/* to the closing marker */`
        if c == '/' && i + 1 < bytes.len() {
            match bytes[i + 1] as char {
                '/' => {
                    while i < bytes.len() && bytes[i] != b'\n' {
                        i += 1;
                    }
                    continue;
                }
                '*' => {
                    i += 2;
                    while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                        i += 1;
                    }
                    i = (i + 2).min(bytes.len());
                    continue;
                }
                _ => {}
            }
        }

        // Words: a letter or underscore, then letters, digits and underscores.
        if c.is_ascii_alphabetic() || c == '_' {
            while i < bytes.len() {
                let ch = bytes[i] as char;
                if ch.is_ascii_alphanumeric() || ch == '_' {
                    i += 1;
                } else {
                    break;
                }
            }
            tokens.push(Token {
                kind: TokenKind::Ident(source[start..i].to_string()),
                span: Span::new(start, i),
            });
            continue;
        }

        // Numbers, including a leading minus and one decimal point.
        if c.is_ascii_digit() || (c == '-' && i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit()) {
            i += 1;
            let mut seen_dot = false;
            while i < bytes.len() {
                let ch = bytes[i] as char;
                if ch.is_ascii_digit() {
                    i += 1;
                } else if ch == '.' && !seen_dot && i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit()
                {
                    seen_dot = true;
                    i += 1;
                } else {
                    break;
                }
            }
            let text = &source[start..i];
            tokens.push(Token {
                kind: TokenKind::Number(text.parse().unwrap_or(0.0)),
                span: Span::new(start, i),
            });
            continue;
        }

        // Strings. An unterminated one ends at the end of the file rather than
        // swallowing the reader: the parser reports it with this span.
        if c == '"' {
            i += 1;
            let text_start = i;
            while i < bytes.len() && bytes[i] != b'"' {
                i += 1;
            }
            let text = source[text_start..i.min(source.len())].to_string();
            if i < bytes.len() {
                i += 1;
            }
            tokens.push(Token {
                kind: TokenKind::Str(text),
                span: Span::new(start, i),
            });
            continue;
        }

        // `->`, before the single-character punctuation below.
        if c == '-' && i + 1 < bytes.len() && bytes[i + 1] == b'>' {
            i += 2;
            tokens.push(Token {
                kind: TokenKind::Arrow,
                span: Span::new(start, i),
            });
            continue;
        }

        let kind = match c {
            '{' => TokenKind::LBrace,
            '}' => TokenKind::RBrace,
            '[' => TokenKind::LBracket,
            ']' => TokenKind::RBracket,
            ',' => TokenKind::Comma,
            '.' => TokenKind::Dot,
            '=' => TokenKind::Equals,
            other => TokenKind::Unknown(other),
        };
        i += 1;
        tokens.push(Token {
            kind,
            span: Span::new(start, i),
        });
    }

    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(source: &str) -> Vec<TokenKind> {
        tokenize(source).into_iter().map(|t| t.kind).collect()
    }

    #[test]
    fn a_board_line_becomes_the_words_and_braces_it_is_written_from() {
        assert_eq!(
            kinds("board t { layers 2 }"),
            vec![
                TokenKind::Ident("board".into()),
                TokenKind::Ident("t".into()),
                TokenKind::LBrace,
                TokenKind::Ident("layers".into()),
                TokenKind::Number(2.0),
                TokenKind::RBrace,
            ]
        );
    }

    #[test]
    fn a_dimension_is_a_number_beside_its_unit() {
        // The unit is a word, not part of the number, so `10mm` and `10 mm`
        // are the same two tokens and the reader does not have to care.
        assert_eq!(
            kinds("10.5mm"),
            vec![TokenKind::Number(10.5), TokenKind::Ident("mm".into())]
        );
        assert_eq!(kinds("10.5mm"), kinds("10.5 mm"));
    }

    #[test]
    fn both_comment_forms_disappear() {
        assert_eq!(kinds("// gone\nlayers 2"), kinds("layers 2"));
        assert_eq!(kinds("/* gone\nstill gone */ layers 2"), kinds("layers 2"));
        // A `/` that starts nothing is not a comment.
        assert_eq!(kinds("/"), vec![TokenKind::Unknown('/')]);
    }

    #[test]
    fn a_path_arrow_is_one_token_and_a_negative_number_is_another() {
        assert_eq!(
            kinds("-2 -> 3"),
            vec![
                TokenKind::Number(-2.0),
                TokenKind::Arrow,
                TokenKind::Number(3.0),
            ]
        );
    }

    #[test]
    fn a_string_keeps_its_spaces_and_loses_its_quotes() {
        assert_eq!(kinds("\"10 k\""), vec![TokenKind::Str("10 k".into())]);
    }

    #[test]
    fn spans_point_back_at_the_bytes_they_came_from() {
        let source = "board plate";
        let tokens = tokenize(source);
        assert_eq!(&source[tokens[0].span.start..tokens[0].span.end], "board");
        assert_eq!(&source[tokens[1].span.start..tokens[1].span.end], "plate");
    }

    #[test]
    fn an_unterminated_string_ends_at_the_end_of_the_file() {
        // Rather than looping, so the reader can report it.
        let tokens = tokenize("\"open");
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, TokenKind::Str("open".into()));
    }
}
