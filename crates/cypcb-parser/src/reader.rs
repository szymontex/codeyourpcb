//! A reader for `.cypcb` written in Rust, with no C behind it.
//!
//! Step one of the plan in `docs/one-parser.md`. The language is read twice
//! today - a tree-sitter grammar with a generated C parser here, and a
//! hand-written line reader in the viewer's TypeScript, because C does not
//! reach the browser. Two readers means every construct lands twice and drifts
//! in between; the measured cost is in that document.
//!
//! This covers the constructs both current readers already handle: `version`,
//! `board`, `component`, `net` and `trace`. Modules, imports, interfaces,
//! footprints, zones, netclasses, outlines and assertions are step two, and
//! until then this reader is behind the `rust-parser` feature and nothing in
//! the shipping path uses it.
//!
//! It is checked against the parser it will replace rather than against
//! hand-written expectations: `differential.rs` reads every example both ways
//! and compares the ASTs.

use crate::ast::{
    BoardDef, ComponentDef, ComponentKind, Definition, Dimension, Identifier, NetAssignment,
    NetConstraints, NetDef, PinId, PinRef, PositionExpr, RotationExpr, SizeProperty, SourceFile,
    Span, StringLit, TraceDef, TraceDirective, TracePath, TraceVia,
};
use crate::errors::{ParseError, ParseResult};
use crate::lexer::{tokenize, Token, TokenKind};
use cypcb_core::Unit;

/// Read a source file into the AST.
///
/// Returns whatever it could read alongside the errors it hit, the way the
/// tree-sitter parser does: a file with one bad line still shows the rest of
/// the board.
pub fn read(source: &str) -> ParseResult<SourceFile> {
    let tokens = tokenize(source);
    let mut reader = Reader {
        tokens,
        at: 0,
        errors: Vec::new(),
        source,
    };

    let mut version = None;
    let mut definitions = Vec::new();

    while !reader.done() {
        let start = reader.here();
        let Some(word) = reader.peek_ident().map(str::to_string) else {
            reader.unexpected("a definition");
            reader.skip_to_next_definition();
            continue;
        };

        match word.as_str() {
            "version" => {
                reader.bump();
                match reader.number() {
                    Some((value, _)) => version = Some(value as u32),
                    None => reader.unexpected("a version number"),
                }
            }
            "board" => match reader.board(start) {
                Some(def) => definitions.push(Definition::Board(def)),
                None => reader.skip_to_next_definition(),
            },
            "component" => match reader.component(start) {
                Some(def) => definitions.push(Definition::Component(def)),
                None => reader.skip_to_next_definition(),
            },
            "net" => match reader.net(start) {
                Some(def) => definitions.push(Definition::Net(def)),
                None => reader.skip_to_next_definition(),
            },
            "trace" => match reader.trace(start) {
                Some(def) => definitions.push(Definition::Trace(def)),
                None => reader.skip_to_next_definition(),
            },
            _ => {
                // A construct step two adds. Skipping it keeps the reader
                // usable on real files while it is incomplete, and the
                // differential test only compares files it claims to cover.
                reader.skip_to_next_definition();
            }
        }
    }

    let span = Span::new(0, source.len());
    let errors = std::mem::take(&mut reader.errors);
    ParseResult::new(
        SourceFile {
            version,
            definitions,
            span,
        },
        errors,
    )
}

/// The token cursor, and the errors collected on the way through.
struct Reader<'a> {
    tokens: Vec<Token>,
    at: usize,
    errors: Vec<ParseError>,
    source: &'a str,
}

impl<'a> Reader<'a> {
    fn done(&self) -> bool {
        self.at >= self.tokens.len()
    }

    /// Where the cursor is, as a byte offset - the start of the next token, or
    /// the end of the file.
    fn here(&self) -> usize {
        self.tokens
            .get(self.at)
            .map(|t| t.span.start)
            .unwrap_or(self.source.len())
    }

    /// The offset just past the token the cursor last passed.
    fn behind(&self) -> usize {
        if self.at == 0 {
            0
        } else {
            self.tokens[self.at - 1].span.end
        }
    }

    fn peek(&self) -> Option<&TokenKind> {
        self.tokens.get(self.at).map(|t| &t.kind)
    }

    fn peek_ident(&self) -> Option<&str> {
        self.peek().and_then(TokenKind::ident)
    }

    fn bump(&mut self) -> Option<&Token> {
        let token = self.tokens.get(self.at);
        if token.is_some() {
            self.at += 1;
        }
        token
    }

    /// Take the next token if it is this punctuation.
    fn eat(&mut self, kind: &TokenKind) -> bool {
        if self.peek() == Some(kind) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    /// Take the next token if it is this word.
    fn eat_word(&mut self, word: &str) -> bool {
        if self.peek_ident() == Some(word) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn identifier(&mut self) -> Option<Identifier> {
        match self.tokens.get(self.at) {
            Some(Token {
                kind: TokenKind::Ident(word),
                span,
            }) => {
                let identifier = Identifier::new(word.clone(), *span);
                self.at += 1;
                Some(identifier)
            }
            _ => None,
        }
    }

    fn string(&mut self) -> Option<StringLit> {
        match self.tokens.get(self.at) {
            Some(Token {
                kind: TokenKind::Str(text),
                span,
            }) => {
                let literal = StringLit::new(text.clone(), *span);
                self.at += 1;
                Some(literal)
            }
            _ => None,
        }
    }

    fn number(&mut self) -> Option<(f64, Span)> {
        match self.tokens.get(self.at) {
            Some(Token {
                kind: TokenKind::Number(value),
                span,
            }) => {
                let pair = (*value, *span);
                self.at += 1;
                Some(pair)
            }
            _ => None,
        }
    }

    /// A number and the unit written beside it. A bare number is millimetres,
    /// which is what the grammar's `unit` rule defaults to.
    fn dimension(&mut self) -> Option<Dimension> {
        let (value, span) = self.number()?;
        let unit = match self.peek_ident() {
            Some("mm") => Some(Unit::Mm),
            Some("mil") => Some(Unit::Mil),
            Some("in") => Some(Unit::Inch),
            Some("nm") => Some(Unit::Nm),
            _ => None,
        };
        match unit {
            Some(unit) => {
                let end = self.tokens[self.at].span.end;
                self.at += 1;
                Some(Dimension::new(value, unit, Span::new(span.start, end)))
            }
            None => Some(Dimension::new(value, Unit::Mm, span)),
        }
    }

    /// `R1.1` or `U1.VCC`.
    fn pin_ref(&mut self) -> Option<PinRef> {
        let start = self.here();
        let component = self.identifier()?;
        if !self.eat(&TokenKind::Dot) {
            return None;
        }
        let pin = self.pin_id()?;
        Some(PinRef {
            component,
            pin,
            span: Span::new(start, self.behind()),
        })
    }

    fn pin_id(&mut self) -> Option<PinId> {
        match self.peek() {
            Some(TokenKind::Number(_)) => {
                let (value, _) = self.number()?;
                Some(PinId::Number(value as u32))
            }
            Some(TokenKind::Ident(_)) => self.identifier().map(|id| PinId::Name(id.value)),
            _ => None,
        }
    }

    fn unexpected(&mut self, wanted: &str) {
        let span = self
            .tokens
            .get(self.at)
            .map(|t| t.span)
            .unwrap_or(Span::new(self.source.len(), self.source.len()));
        self.errors.push(ParseError::Missing {
            expected: wanted.to_string(),
            src: self.source.to_string(),
            span: span.to_miette(),
        });
    }

    /// Walk to the start of the next top-level definition.
    ///
    /// Braces are counted so a bad line inside a block does not make the rest
    /// of that block read as top level.
    fn skip_to_next_definition(&mut self) {
        const STARTERS: &[&str] = &[
            "version",
            "board",
            "component",
            "net",
            "netclass",
            "trace",
            "zone",
            "footprint",
            "module",
            "interface",
            "import",
            "assert",
            "outline",
        ];

        let mut depth = 0i32;
        // Always move at least one token, or a definition this reader does not
        // know would spin here forever.
        if self.at < self.tokens.len() {
            if let TokenKind::LBrace = self.tokens[self.at].kind {
                depth += 1;
            }
            self.at += 1;
        }
        while self.at < self.tokens.len() {
            match &self.tokens[self.at].kind {
                TokenKind::LBrace => depth += 1,
                TokenKind::RBrace => depth -= 1,
                TokenKind::Ident(word) if depth <= 0 && STARTERS.contains(&word.as_str()) => return,
                _ => {}
            }
            self.at += 1;
        }
    }

    /// `board name { size W x H  layers N }`.
    fn board(&mut self, start: usize) -> Option<BoardDef> {
        self.bump(); // `board`
        let name = match self.identifier() {
            Some(name) => name,
            None => {
                self.unexpected("a board name");
                return None;
            }
        };
        if !self.eat(&TokenKind::LBrace) {
            self.unexpected("`{` after the board name");
            return None;
        }

        let mut size = None;
        let mut layers = None;

        while !self.done() && !self.eat(&TokenKind::RBrace) {
            let property_start = self.here();
            match self.peek_ident() {
                Some("size") => {
                    self.bump();
                    let width = self.dimension();
                    let has_x = self.eat_word("x");
                    let height = self.dimension();
                    match (width, has_x, height) {
                        (Some(width), true, Some(height)) => {
                            size = Some(SizeProperty {
                                width,
                                height,
                                span: Span::new(property_start, self.behind()),
                            })
                        }
                        _ => self.unexpected("a size like `30mm x 20mm`"),
                    }
                }
                Some("layers") => {
                    self.bump();
                    match self.number() {
                        Some((value, _)) => layers = Some(value as u8),
                        None => self.unexpected("a layer count"),
                    }
                }
                _ => {
                    // stackup and anything else: step two.
                    self.bump();
                }
            }
        }

        Some(BoardDef {
            name,
            size,
            layers,
            stackup: None,
            span: Span::new(start, self.behind()),
        })
    }

    /// `component R1 resistor "0402" { value "10k"  at 5mm, 10mm  rotate 90 pin.1 = VCC }`.
    fn component(&mut self, start: usize) -> Option<ComponentDef> {
        self.bump(); // `component`
        let refdes = match self.identifier() {
            Some(refdes) => refdes,
            None => {
                self.unexpected("a reference designator");
                return None;
            }
        };

        // The kind is optional in the grammar; a footprint string may follow
        // the refdes directly, in which case the part is generic.
        let kind = match self.peek_ident().and_then(ComponentKind::from_str) {
            Some(kind) => {
                self.bump();
                kind
            }
            None => ComponentKind::Generic,
        };

        let footprint = match self.string() {
            Some(footprint) => footprint,
            None => {
                self.unexpected("a footprint name in quotes");
                return None;
            }
        };

        let mut value = None;
        let mut position = None;
        let mut rotation = None;
        let mut net_assignments = Vec::new();

        if self.eat(&TokenKind::LBrace) {
            while !self.done() && !self.eat(&TokenKind::RBrace) {
                let property_start = self.here();
                match self.peek_ident() {
                    Some("value") => {
                        self.bump();
                        match self.string() {
                            Some(text) => value = Some(text),
                            // `value 10kohm` is a typed value, step two.
                            None => {
                                self.bump();
                            }
                        }
                    }
                    Some("at") => {
                        self.bump();
                        let x = self.dimension();
                        self.eat(&TokenKind::Comma);
                        let y = self.dimension();
                        match (x, y) {
                            (Some(x), Some(y)) => {
                                position = Some(PositionExpr {
                                    x,
                                    y,
                                    span: Span::new(property_start, self.behind()),
                                })
                            }
                            _ => self.unexpected("a position like `at 10mm, 8mm`"),
                        }
                    }
                    Some("rotate") => {
                        self.bump();
                        match self.number() {
                            Some((angle, _)) => {
                                self.eat_word("deg");
                                rotation = Some(RotationExpr {
                                    angle,
                                    span: Span::new(property_start, self.behind()),
                                });
                            }
                            None => self.unexpected("an angle"),
                        }
                    }
                    Some("pin") => {
                        self.bump();
                        self.eat(&TokenKind::Dot);
                        let pin = self.pin_id();
                        let assigned = self.eat(&TokenKind::Equals);
                        let net = self.identifier();
                        match (pin, assigned, net) {
                            (Some(pin), true, Some(net)) => net_assignments.push(NetAssignment {
                                pin,
                                net,
                                span: Span::new(property_start, self.behind()),
                            }),
                            _ => self.unexpected("a net assignment like `pin.1 = VCC`"),
                        }
                    }
                    _ => {
                        self.bump();
                    }
                }
            }
        }

        Some(ComponentDef {
            refdes,
            kind,
            footprint,
            value,
            typed_value: None,
            position,
            rotation,
            net_assignments,
            span: Span::new(start, self.behind()),
        })
    }

    /// `net VCC [width 0.3mm] { R1.1  C1.1 }`.
    fn net(&mut self, start: usize) -> Option<NetDef> {
        self.bump(); // `net`
        let name = match self.identifier() {
            Some(name) => name,
            None => {
                self.unexpected("a net name");
                return None;
            }
        };

        let constraints = self.net_constraints();

        let mut connections = Vec::new();
        if self.eat(&TokenKind::LBrace) {
            while !self.done() && !self.eat(&TokenKind::RBrace) {
                if self.eat(&TokenKind::Comma) {
                    continue;
                }
                match self.pin_ref() {
                    Some(pin) => connections.push(pin),
                    None => {
                        self.unexpected("a pin like `R1.1`");
                        self.bump();
                    }
                }
            }
        }

        Some(NetDef {
            name,
            constraints,
            connections,
            span: Span::new(start, self.behind()),
        })
    }

    /// `[width 0.3mm, clearance 0.2mm, current 500mA]`, when there is one.
    fn net_constraints(&mut self) -> Option<NetConstraints> {
        let start = self.here();
        if !self.eat(&TokenKind::LBracket) {
            return None;
        }

        let mut width = None;
        let mut clearance = None;
        let mut current = None;

        while !self.done() && !self.eat(&TokenKind::RBracket) {
            if self.eat(&TokenKind::Comma) {
                continue;
            }
            match self.peek_ident() {
                Some("width") => {
                    self.bump();
                    width = self.dimension();
                }
                Some("clearance") => {
                    self.bump();
                    clearance = self.dimension();
                }
                Some("current") => {
                    self.bump();
                    let value_start = self.here();
                    match self.number() {
                        Some((value, span)) => {
                            let unit = match self.peek_ident() {
                                Some("A") => {
                                    self.bump();
                                    crate::ast::CurrentUnit::Amps
                                }
                                Some("mA") => {
                                    self.bump();
                                    crate::ast::CurrentUnit::Milliamps
                                }
                                _ => crate::ast::CurrentUnit::Milliamps,
                            };
                            let _ = span;
                            current = Some(crate::ast::CurrentValue::new(
                                value,
                                unit,
                                Span::new(value_start, self.behind()),
                            ));
                        }
                        None => self.unexpected("a current like `500mA`"),
                    }
                }
                _ => {
                    self.bump();
                }
            }
        }

        Some(NetConstraints {
            width,
            clearance,
            current,
            span: Span::new(start, self.behind()),
        })
    }

    /// `trace VCC { from R1.1  to C1.1  layer Top  width 0.3mm  locked }`, and
    /// the geometric form with `path` and `via`.
    fn trace(&mut self, start: usize) -> Option<TraceDef> {
        self.bump(); // `trace`
        let net = match self.identifier() {
            Some(net) => net,
            None => {
                self.unexpected("a net name");
                return None;
            }
        };

        let mut from = None;
        let mut to = None;
        let mut layer = None;
        let mut width = None;
        let mut locked = false;
        let mut directives = Vec::new();

        if self.eat(&TokenKind::LBrace) {
            while !self.done() && !self.eat(&TokenKind::RBrace) {
                let directive_start = self.here();
                match self.peek_ident() {
                    Some("from") => {
                        self.bump();
                        from = self.pin_ref();
                    }
                    Some("to") => {
                        self.bump();
                        to = self.pin_ref();
                    }
                    Some("layer") => {
                        self.bump();
                        match self.identifier() {
                            Some(name) => {
                                layer = Some(name.value.clone());
                                directives.push(TraceDirective::Layer(name.value));
                            }
                            None => self.unexpected("a layer name"),
                        }
                    }
                    Some("width") => {
                        self.bump();
                        width = self.dimension();
                    }
                    Some("locked") => {
                        self.bump();
                        locked = true;
                    }
                    Some("path") => {
                        self.bump();
                        let mut points = Vec::new();
                        loop {
                            let point_start = self.here();
                            let x = self.dimension();
                            self.eat(&TokenKind::Comma);
                            let y = self.dimension();
                            match (x, y) {
                                (Some(x), Some(y)) => points.push(PositionExpr {
                                    x,
                                    y,
                                    span: Span::new(point_start, self.behind()),
                                }),
                                _ => {
                                    self.unexpected("a point like `10mm, 12mm`");
                                    break;
                                }
                            }
                            if !self.eat(&TokenKind::Arrow) {
                                break;
                            }
                        }
                        directives.push(TraceDirective::Path(TracePath {
                            points,
                            span: Span::new(directive_start, self.behind()),
                        }));
                    }
                    Some("via") => {
                        self.bump();
                        let position_start = self.here();
                        let x = self.dimension();
                        self.eat(&TokenKind::Comma);
                        let y = self.dimension();
                        let Some((x, y)) = x.zip(y) else {
                            self.unexpected("a via position");
                            continue;
                        };
                        let position = PositionExpr {
                            x,
                            y,
                            span: Span::new(position_start, self.behind()),
                        };
                        let drill = if self.eat_word("drill") {
                            self.dimension()
                        } else {
                            None
                        };
                        let layers = if self.eat_word("layers") {
                            let first = self.identifier().map(|id| id.value);
                            self.eat_word("to");
                            let second = self.identifier().map(|id| id.value);
                            first.zip(second)
                        } else {
                            None
                        };
                        directives.push(TraceDirective::Via(TraceVia {
                            position,
                            drill,
                            layers,
                            span: Span::new(directive_start, self.behind()),
                        }));
                    }
                    _ => {
                        self.bump();
                    }
                }
            }
        }

        Some(TraceDef {
            net,
            from,
            to,
            waypoints: Vec::new(),
            layer,
            width,
            locked,
            directives,
            span: Span::new(start, self.behind()),
        })
    }
}
