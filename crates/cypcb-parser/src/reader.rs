//! A reader for `.cypcb` written in Rust, with no C behind it.
//!
//! Step one of the plan in `docs/one-parser.md`. The language is read twice
//! today - a tree-sitter grammar with a generated C parser here, and a
//! hand-written line reader in the viewer's TypeScript, because C does not
//! reach the browser. Two readers means every construct lands twice and drifts
//! in between; the measured cost is in that document.
//!
//! This covers the whole language: every v1 construct - `version`, `board`,
//! `component`, `net`, `netclass`, `trace`, `footprint`, `outline`, `zone` and
//! `keepout` - and the v2 four: `module` with `use ... as`, `import`,
//! `interface` and `assert`. It sits behind the `rust-parser` feature until
//! its errors match the parser it replaces, so nothing in the shipping path
//! calls it yet.
//!
//! It is checked against the parser it will replace rather than against
//! hand-written expectations: `differential.rs` reads every example both ways
//! and compares the ASTs.

use crate::ast::{
    format_pad_number, AssertDef, AssertExpression, AssertOperand, BoardDef, ComparisonOp,
    ComponentDef, ComponentKind, Definition, DiffPairDef, Dimension, EdgeConnectorDef,
    FootprintDef, Identifier, ImplementsClause, ImportDef, InterfaceDef, LayerType, ModuleDef,
    ModuleInstance, NetAssignment, NetClassDef, NetConstraints, NetDef, OutlineDef, PadDef,
    PadShape, PhysicalValue, PinDeclaration, PinId, PinRef, PortConnection, PositionExpr,
    RotationExpr, SilkDef, SizeProperty, SourceFile, Span, StackupDef, StackupLayer,
    StackupSheetDef, StringLit, Tolerance, ToleranceKind, TraceDef, TraceDirective, TracePath,
    TraceVia, ZoneDef, ZoneKind,
};
use crate::errors::{ParseError, ParseResult};
use crate::lexer::{tokenize, Token, TokenKind};
use cypcb_core::Unit;

/// Every word a `stackup` block takes, for the message a wrong one gets.
const STACKUP_WORDS: &[&str] = &[
    "copper",
    "prepreg",
    "core",
    "mask",
    "silk",
    "paste",
    "finish",
    "edges",
    "pads",
    "connector",
    "impedance",
];

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
            "footprint" => match reader.footprint(start) {
                Some(def) => definitions.push(Definition::Footprint(def)),
                None => reader.skip_to_next_definition(),
            },
            "zone" | "keepout" => match reader.zone(start) {
                Some(def) => definitions.push(Definition::Zone(def)),
                None => reader.skip_to_next_definition(),
            },
            "diffpair" => match reader.diffpair(start) {
                Some(def) => definitions.push(Definition::DiffPair(def)),
                None => reader.skip_to_next_definition(),
            },
            "netclass" => match reader.netclass(start) {
                Some(def) => definitions.push(Definition::NetClass(def)),
                None => reader.skip_to_next_definition(),
            },
            "outline" => match reader.outline(start) {
                Some(def) => definitions.push(Definition::Outline(def)),
                None => reader.skip_to_next_definition(),
            },
            "import" => match reader.import(start) {
                Some(def) => definitions.push(Definition::Import(def)),
                None => reader.skip_to_next_definition(),
            },
            "module" => match reader.module(start) {
                Some(def) => definitions.push(Definition::Module(def)),
                None => reader.skip_to_next_definition(),
            },
            "use" => match reader.module_instance(start) {
                Some(def) => definitions.push(Definition::ModuleInstance(def)),
                None => reader.skip_to_next_definition(),
            },
            "interface" => match reader.interface(start) {
                Some(def) => definitions.push(Definition::Interface(def)),
                None => reader.skip_to_next_definition(),
            },
            "assert" => match reader.assertion(start) {
                Some(def) => definitions.push(Definition::Assert(def)),
                None => reader.skip_to_next_definition(),
            },
            other => {
                // Every construct the language has is handled above, so a word
                // here is one the language does not have. Saying so is the
                // difference between a typo being reported and a part quietly
                // missing from the board: until this was measured against the
                // tree-sitter parser, `frobnicate 3` at the top level parsed
                // clean.
                reader.unexpected(&format!("a definition, not `{other}`"));
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
        self.dimension_with_ounces(false)
    }

    /// The same, where ounces of copper are a thickness this position takes.
    ///
    /// `oz` is a weight per square foot rather than a length, and it is a
    /// thickness of copper and of nothing else - `size 1oz x 2oz` is not a
    /// board. One position takes it: a copper layer in a stackup, which is
    /// where every fab table states it.
    fn copper_thickness(&mut self) -> Option<Dimension> {
        self.dimension_with_ounces(true)
    }

    fn dimension_with_ounces(&mut self, ounces: bool) -> Option<Dimension> {
        let (value, span) = self.number()?;
        let unit = match self.peek_ident() {
            Some("mm") => Some(Unit::Mm),
            Some("um") => Some(Unit::Um),
            Some("mil") => Some(Unit::Mil),
            Some("in") => Some(Unit::Inch),
            Some("nm") => Some(Unit::Nm),
            Some("oz") if ounces => Some(Unit::Oz),
            _ => None,
        };
        match unit {
            Some(unit) => {
                let end = self.tokens[self.at].span.end;
                self.at += 1;
                Some(Dimension::new(value, unit, Span::new(span.start, end)))
            }
            None => Some(Dimension::implied_mm(value, span)),
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

    /// A word inside a block that the block does not have.
    ///
    /// Every block body used to end in `_ => { self.bump(); }`, so `rotat 90`
    /// in a component - one letter short of `rotate` - was read, dropped and
    /// never mentioned: the part came out unrotated and `cypcb check` said the
    /// board was fine. Measured against tree-sitter, which reported all six
    /// blocks this reader stayed quiet about.
    ///
    /// One error per property, not per token: the offending word is named and
    /// the rest of its line is skipped, so `zzz 5 6 7` is one complaint rather
    /// than four.
    fn unknown_property(&mut self, block: &str, known: &[&str]) {
        let Some(token) = self.tokens.get(self.at) else {
            self.unexpected(&format!("a property of `{block}`"));
            return;
        };
        let span = token.span;
        let found = self.source[span.start..span.end].to_string();
        self.errors.push(ParseError::UnknownProperty {
            block: block.to_string(),
            found,
            known: known.join(", "),
            src: self.source.to_string(),
            span: span.to_miette(),
        });
        self.skip_rest_of_line();
    }

    /// Skip to the next line, the closing brace, or the end of the file.
    ///
    /// Recovery for `unknown_property`. A property in this language is a line,
    /// so the next token that starts on a new line is the next thing worth
    /// reading.
    fn skip_rest_of_line(&mut self) {
        let Some(first) = self.tokens.get(self.at) else {
            return;
        };
        let mut previous_end = first.span.end;
        self.at += 1;

        while let Some(token) = self.tokens.get(self.at) {
            if token.kind == TokenKind::RBrace {
                return;
            }
            if self.source[previous_end..token.span.start].contains('\n') {
                return;
            }
            previous_end = token.span.end;
            self.at += 1;
        }
    }

    /// `stackup { copper 0.035mm  prepreg 0.2mm  core 1.2mm }` inside a board.
    ///
    /// Read because the board block refuses what it does not recognise now,
    /// and tree-sitter has always accepted this. What a design says here is
    /// now what the rest of the tool believes: the checker grades the stackup
    /// against the layer count, and the thickness it adds up to is the depth
    /// every plated hole is drilled through, which decides whether the fab
    /// can plate it at all.
    fn stackup(&mut self) -> Option<StackupDef> {
        let start = self.here();
        self.bump(); // `stackup`
        if !self.eat(&TokenKind::LBrace) {
            self.unexpected("`{` after `stackup`");
            return None;
        }

        let mut layers = Vec::new();
        let mut finish = None;
        let mut edges_plated = false;
        let mut castellated_pads = false;
        let mut edge_connector = None;
        let mut impedance_controlled = false;
        while !self.done() && !self.eat(&TokenKind::RBrace) {
            let layer_start = self.here();
            let Some(word) = self.peek_ident().map(str::to_string) else {
                self.unknown_property("stackup", STACKUP_WORDS);
                continue;
            };
            // What the fabricator does to the board, rather than what it
            // presses. Each starts with a word no layer type uses, so this
            // reads them off the front without looking ahead.
            match word.as_str() {
                "finish" => {
                    self.bump();
                    match self.string() {
                        Some(name) => finish = Some(name),
                        None => self.unexpected("a quoted finish like `\"ENIG\"`"),
                    }
                    continue;
                }
                "edges" => {
                    self.bump();
                    if !self.eat_word("plated") {
                        self.unexpected("`plated` after `edges`");
                    }
                    edges_plated = true;
                    continue;
                }
                "pads" => {
                    self.bump();
                    if !self.eat_word("castellated") {
                        self.unexpected("`castellated` after `pads`");
                    }
                    castellated_pads = true;
                    continue;
                }
                "connector" => {
                    self.bump();
                    if self.eat_word("bevelled") {
                        edge_connector = Some(EdgeConnectorDef::Bevelled);
                    } else if self.eat_word("plain") {
                        edge_connector = Some(EdgeConnectorDef::Plain);
                    } else {
                        self.unexpected("`plain` or `bevelled` after `connector`");
                    }
                    continue;
                }
                "impedance" => {
                    self.bump();
                    if !self.eat_word("controlled") {
                        self.unexpected("`controlled` after `impedance`");
                    }
                    impedance_controlled = true;
                    continue;
                }
                _ => {}
            }
            let Some(layer_type) = LayerType::from_str(&word) else {
                self.unknown_property("stackup", STACKUP_WORDS);
                continue;
            };
            self.bump();
            // The layer's own name, when the design gives it one. A string,
            // because a fabricator's canonical names carry a dot.
            let name = match self.peek() {
                Some(TokenKind::Str(_)) => self.string(),
                _ => None,
            };
            // A thickness is optional, and the next line's layer name is not
            // one, so only a number starts it.
            let thickness = match self.peek() {
                Some(TokenKind::Number(_)) if layer_type == LayerType::Copper => {
                    self.copper_thickness()
                }
                Some(TokenKind::Number(_)) => {
                    let read = self.dimension();
                    // Without this the `oz` is left for the loop, which reads
                    // the next word as a property name and answers "`stackup`
                    // has no property `oz`" - true, and not what happened.
                    if self.peek_ident() == Some("oz") {
                        self.unexpected(
                            "a length here: ounces are a weight of copper per square foot, \
                             and only a copper layer is stated in them",
                        );
                        self.bump();
                    }
                    read
                }
                _ => None,
            };
            // Consumed here rather than left to the loop. The loop reads the
            // next word as a layer kind, and `material` is not one, so leaving
            // it would report the design's own syntax as an unknown property.
            let material = if self.eat_word("material") {
                let literal = self.string();
                if literal.is_none() {
                    self.unexpected("a quoted material after `material`");
                }
                literal
            } else {
                None
            };
            // Consumed here for the reason `material` is: the loop reads the
            // next word as a layer kind, and `color` is not one.
            let color = if self.eat_word("color") {
                let literal = self.string();
                if literal.is_none() {
                    self.unexpected("a quoted colour after `color`");
                }
                literal
            } else {
                None
            };
            // The two numbers a dielectric is chosen for. Consumed here for
            // the reason `material` is: the loop reads the next word as a
            // layer kind, and neither of these is one.
            let dk = self.stackup_number("dk");
            let df = self.stackup_number("df");
            // The rest of the sheets in this slot. Consumed here rather than
            // left to the loop for the reason `material` is: `sheet` is not a
            // layer kind, and leaving it would report the design's own syntax
            // as an unknown property.
            let mut sheets = Vec::new();
            while self.eat_word("sheet") {
                let sheet_start = self.behind();
                let thickness = match self.peek() {
                    Some(TokenKind::Number(_)) => self.dimension(),
                    _ => None,
                };
                let material = if self.eat_word("material") {
                    let literal = self.string();
                    if literal.is_none() {
                        self.unexpected("a quoted material after `material`");
                    }
                    literal
                } else {
                    None
                };
                let dk = self.stackup_number("dk");
                let df = self.stackup_number("df");
                sheets.push(StackupSheetDef {
                    thickness,
                    material,
                    dk,
                    df,
                    span: Span::new(sheet_start, self.behind()),
                });
            }
            layers.push(StackupLayer {
                layer_type,
                name,
                thickness,
                material,
                color,
                sheets,
                dk,
                df,
                span: Span::new(layer_start, self.behind()),
            });
        }

        Some(StackupDef {
            layers,
            finish,
            edges_plated,
            castellated_pads,
            edge_connector,
            impedance_controlled,
            span: Span::new(start, self.behind()),
        })
    }

    /// `dk 4.5` or `df 0.02` on a stackup layer, when the next word is that one.
    ///
    /// Neither is a dimension: a dielectric constant has no unit, and writing
    /// one would be a unit this language does not have. Zero and below are
    /// refused rather than stored, because a laminate with no permittivity is
    /// not a laminate, and a stored nonsense number reads later as a
    /// measurement.
    fn stackup_number(&mut self, word: &str) -> Option<f64> {
        if !self.eat_word(word) {
            return None;
        }
        let Some((value, _)) = self.number() else {
            self.unexpected(&format!("a number after `{word}`"));
            return None;
        };
        if !value.is_finite() || value <= 0.0 {
            self.unexpected(&format!("a positive number after `{word}`"));
            return None;
        }
        Some(value)
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
            "use",
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
        let mut stackup = None;
        let mut fab = None;

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
                Some("stackup") => stackup = self.stackup(),
                Some("fab") => {
                    self.bump();
                    match self.identifier() {
                        Some(name) => fab = Some(name),
                        None => self.unexpected("a fabricator name like `jlcpcb`"),
                    }
                }
                _ => self.unknown_property("board", &["size", "layers", "stackup", "fab"]),
            }
        }

        Some(BoardDef {
            name,
            size,
            layers,
            stackup,
            fab,
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
        let mut typed_value = None;
        let mut lcsc = None;
        let mut spec = Vec::new();
        let mut side = None;
        let mut position = None;
        let mut rotation = None;
        let mut net_assignments = Vec::new();

        if self.eat(&TokenKind::LBrace) {
            while !self.done() && !self.eat(&TokenKind::RBrace) {
                let property_start = self.here();
                match self.peek_ident() {
                    Some("spec") => {
                        self.bump();
                        if !self.eat(&TokenKind::LBrace) {
                            self.unexpected("`{` after `spec`");
                            continue;
                        }
                        while !self.done() && !self.eat(&TokenKind::RBrace) {
                            let entry_start = self.here();
                            let Some(name) = self.identifier() else {
                                self.unexpected("a name like `output`");
                                self.bump();
                                continue;
                            };
                            let Some(value) = self.try_physical_value() else {
                                self.unexpected("a quantity like `3.3V` after the name");
                                continue;
                            };
                            spec.push(crate::ast::SpecEntry {
                                name,
                                value,
                                span: Span::new(entry_start, self.behind()),
                            });
                        }
                    }
                    Some("value") => {
                        self.bump();
                        match self.string() {
                            Some(text) => value = Some(text),
                            None => {
                                // `value 10kohm`: a quantity rather than a
                                // label. Both are kept - the typed one so a
                                // rule can check it, and the text so anything
                                // that only prints the value keeps working.
                                match self.try_physical_value() {
                                    Some(quantity) => {
                                        value = Some(StringLit::new(
                                            self.source[quantity.span.start..quantity.span.end]
                                                .to_string(),
                                            quantity.span,
                                        ));
                                        typed_value = Some(quantity);
                                    }
                                    None => self.unexpected("a value in quotes or a quantity"),
                                }
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
                    Some("lcsc") => {
                        self.bump();
                        match self.string() {
                            Some(part) => lcsc = Some(part),
                            None => self.unexpected("a part number in quotes"),
                        }
                    }
                    Some("side") => {
                        self.bump();
                        match self.identifier() {
                            Some(face) => side = Some(face),
                            None => self.unexpected("`top` or `bottom`"),
                        }
                    }
                    Some("pin") => {
                        self.bump();
                        self.eat(&TokenKind::Dot);
                        let pin = self.pin_id();
                        let assigned = self.eat(&TokenKind::Equals);
                        let net = self.net_name();
                        match (pin, assigned, net) {
                            (Some(pin), true, Some(net)) => net_assignments.push(NetAssignment {
                                pin,
                                net,
                                span: Span::new(property_start, self.behind()),
                            }),
                            _ => self.unexpected("a net assignment like `pin.1 = VCC`"),
                        }
                    }
                    _ => self.unknown_property(
                        "component",
                        &["value", "at", "rotate", "side", "lcsc", "pin.<N> = <NET>"],
                    ),
                }
            }
        }

        Some(ComponentDef {
            refdes,
            lcsc,
            spec,
            side,
            kind,
            footprint,
            value,
            typed_value,
            position,
            rotation,
            net_assignments,
            span: Span::new(start, self.behind()),
        })
    }

    /// `net VCC [width 0.3mm] { R1.1  C1.1 }`.
    /// A net's name, however the design chose to spell it.
    ///
    /// A bare identifier, or a quoted string for the names the identifier
    /// rule refuses: `VBUS+`, `3V3`, `D-`. Accepted at every site that names a
    /// net rather than only at the declaration, because a net you can declare
    /// and cannot reference is not a net you can use.
    fn net_name(&mut self) -> Option<Identifier> {
        if let Some(literal) = self.string() {
            return Some(Identifier::new(literal.value, literal.span));
        }
        self.identifier()
    }

    fn net(&mut self, start: usize) -> Option<NetDef> {
        self.bump(); // `net`
        let name = match self.net_name() {
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

    /// `impedance 90ohm` inside a net constraint block.
    ///
    /// The unit is compulsory: a bare number after `impedance` would read like
    /// a width to anyone scanning the line, and every other constraint in this
    /// block carries one. Zero and below are refused rather than stored - a
    /// net with no impedance is not a net, and a nonsense figure kept in the
    /// model reads later as a target somebody chose.
    /// `0.8mm for 4mm`, after a `neck` keyword the caller has already eaten.
    ///
    /// One reader for the two places a neck can be written - a `trace` block
    /// and a net's constraint list - because the statement is the same and a
    /// second copy would drift on the first change to either.
    fn neck_after_keyword(&mut self, start: usize) -> Option<crate::ast::NeckDef> {
        let width = match self.dimension() {
            Some(width) => width,
            None => {
                self.unexpected("a width like `0.8mm` after `neck`");
                return None;
            }
        };
        if !self.eat_word("for") {
            self.unexpected("`for` and a length, as in `neck 0.8mm for 4mm`");
            return None;
        }
        let length = match self.dimension() {
            Some(length) => length,
            None => {
                self.unexpected("a length like `4mm` after `for`");
                return None;
            }
        };
        Some(crate::ast::NeckDef {
            width,
            length,
            span: Span::new(start, self.behind()),
        })
    }

    fn impedance(&mut self) -> Option<f64> {
        let Some((value, _)) = self.number() else {
            self.unexpected("an impedance like `90ohm`");
            return None;
        };
        if !self.eat_word("ohm") {
            self.unexpected("`ohm` after the number, as in `90ohm`");
            return None;
        }
        if !value.is_finite() || value <= 0.0 {
            self.unexpected("a positive impedance");
            return None;
        }
        Some(value)
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
        let mut impedance_ohms = None;
        let mut neck = None;

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
                Some("impedance") => {
                    self.bump();
                    impedance_ohms = self.impedance();
                }
                Some("neck") => {
                    let neck_start = self.here();
                    self.bump();
                    neck = self.neck_after_keyword(neck_start);
                }
                _ => self.unknown_property(
                    "net constraint",
                    &["width", "clearance", "current", "impedance", "neck"],
                ),
            }
        }

        Some(NetConstraints {
            width,
            clearance,
            current,
            impedance_ohms,
            neck,
            span: Span::new(start, self.behind()),
        })
    }

    /// `trace VCC { from R1.1  to C1.1  layer Top  width 0.3mm  locked }`, and
    /// the geometric form with `path` and `via`.
    fn trace(&mut self, start: usize) -> Option<TraceDef> {
        self.bump(); // `trace`
        let net = match self.net_name() {
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
        let mut neck = None;
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
                    Some("neck") => {
                        self.bump();
                        neck = self.neck_after_keyword(directive_start);
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
                    _ => self.unknown_property(
                        "trace",
                        &["from", "to", "path", "layer", "width", "via", "locked"],
                    ),
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
            neck,
            directives,
            span: Span::new(start, self.behind()),
        })
    }

    /// `footprint NAME { description "..."  pad 1 rect at X, Y size W x H [drill D]  courtyard W x H  silk ... }`.
    fn footprint(&mut self, start: usize) -> Option<FootprintDef> {
        self.bump(); // `footprint`
        let name = match self.identifier() {
            Some(name) => name,
            None => {
                self.unexpected("a footprint name");
                return None;
            }
        };
        if !self.eat(&TokenKind::LBrace) {
            self.unexpected("`{` after the footprint name");
            return None;
        }

        let mut description = None;
        let mut pads = Vec::new();
        let mut courtyard = None;
        let mut silk = Vec::new();

        while !self.done() && !self.eat(&TokenKind::RBrace) {
            let property_start = self.here();
            match self.peek_ident() {
                Some("description") => {
                    self.bump();
                    match self.string() {
                        Some(text) => description = Some(text.value),
                        None => self.unexpected("a description in quotes"),
                    }
                }
                Some("courtyard") => {
                    self.bump();
                    let width = self.dimension();
                    self.eat_word("x");
                    let height = self.dimension();
                    match width.zip(height) {
                        Some(pair) => courtyard = Some(pair),
                        None => self.unexpected("a courtyard like `2mm x 1mm`"),
                    }
                }
                Some("pad") => match self.pad(property_start) {
                    Some(pad) => pads.push(pad),
                    None => self.skip_to_next_property(),
                },
                Some("silk") => match self.silk(property_start) {
                    Some(shape) => silk.push(shape),
                    None => self.skip_to_next_property(),
                },
                _ => {
                    self.unknown_property("footprint", &["description", "courtyard", "pad", "silk"])
                }
            }
        }

        Some(FootprintDef {
            name,
            description,
            pads,
            courtyard,
            silk,
            span: Span::new(start, self.behind()),
        })
    }

    /// `pad 1 rect at 0mm, 0mm size 1mm x 1mm [drill 0.3mm [x 0.2mm]]`.
    /// The three ways a design can write what a pad is called.
    ///
    /// Numbers keep the form they were written in: `1` stays `"1"` rather
    /// than becoming `"1.0"`, because a pad called 1 and a pad called 1.0 are
    /// not the same pad and the board model compares these by string.
    fn pad_name(&mut self) -> Option<String> {
        if let Some(literal) = self.string() {
            return Some(literal.value);
        }
        if let Some(word) = self.peek_ident() {
            let owned = word.to_string();
            self.bump();
            return Some(owned);
        }
        let (value, _) = self.number()?;
        Some(format_pad_number(value))
    }

    fn pad(&mut self, start: usize) -> Option<PadDef> {
        self.bump(); // `pad`
                     // A bare number, a bare identifier, or a quoted string - the last of
                     // which is the only way to write a name the identifier rule refuses,
                     // like `A1+` or one that starts with a digit and carries a letter.
        let number = match self.pad_name() {
            Some(name) => name,
            None => {
                self.unexpected("a pad name: 1, A1 or \"S1\"");
                return None;
            }
        };
        let shape = match self.peek_ident().and_then(PadShape::from_str) {
            Some(shape) => {
                self.bump();
                shape
            }
            None => {
                self.unexpected("a pad shape: rect, circle, roundrect or oblong");
                return None;
            }
        };
        if !self.eat_word("at") {
            self.unexpected("`at` before a pad position");
            return None;
        }
        let x = self.dimension()?;
        self.eat(&TokenKind::Comma);
        let y = self.dimension()?;
        if !self.eat_word("size") {
            self.unexpected("`size` after a pad position");
            return None;
        }
        let width = self.dimension()?;
        self.eat_word("x");
        let height = self.dimension()?;
        // `drill 0.9mm` is a round hole. `drill 2.4mm x 1.0mm` is a slot,
        // milled along its length - written the same way `size` is, because it
        // is the same question asked of the hole rather than of the copper.
        let (drill, drill_height) = if self.eat_word("drill") {
            let width = self.dimension();
            let height = if width.is_some() && self.eat_word("x") {
                self.dimension()
            } else {
                None
            };
            (width, height)
        } else {
            (None, None)
        };

        Some(PadDef {
            number,
            shape,
            x,
            y,
            width,
            height,
            drill,
            drill_height,
            span: Span::new(start, self.behind()),
        })
    }

    /// `silk line X1, Y1 to X2, Y2 [width W]` and `silk circle CX, CY radius R [width W]`.
    fn silk(&mut self, start: usize) -> Option<SilkDef> {
        self.bump(); // `silk`
        let kind = self.identifier()?;
        match kind.value.as_str() {
            "line" => {
                let x1 = self.dimension()?;
                self.eat(&TokenKind::Comma);
                let y1 = self.dimension()?;
                if !self.eat_word("to") {
                    self.unexpected("`to` between the ends of a silk line");
                    return None;
                }
                let x2 = self.dimension()?;
                self.eat(&TokenKind::Comma);
                let y2 = self.dimension()?;
                let width = if self.eat_word("width") {
                    self.dimension()
                } else {
                    None
                };
                Some(SilkDef::Line {
                    start: (x1, y1),
                    end: (x2, y2),
                    width,
                    span: Span::new(start, self.behind()),
                })
            }
            "circle" => {
                let cx = self.dimension()?;
                self.eat(&TokenKind::Comma);
                let cy = self.dimension()?;
                if !self.eat_word("radius") {
                    self.unexpected("`radius` after a silk circle's centre");
                    return None;
                }
                let radius = self.dimension()?;
                let width = if self.eat_word("width") {
                    self.dimension()
                } else {
                    None
                };
                Some(SilkDef::Circle {
                    centre: (cx, cy),
                    radius,
                    width,
                    span: Span::new(start, self.behind()),
                })
            }
            _ => {
                self.unexpected("`line` or `circle` after `silk`");
                None
            }
        }
    }

    /// `zone NAME { bounds X1, Y1 to X2, Y2  layer top  net GND }`, and the
    /// `keepout` spelling, which is the same block with no net.
    fn zone(&mut self, start: usize) -> Option<ZoneDef> {
        let kind = match self.peek_ident() {
            Some("keepout") => ZoneKind::Keepout,
            _ => ZoneKind::CopperPour,
        };
        self.bump(); // `zone` or `keepout`

        // The name is optional, so a `{` here means the zone is unnamed.
        let name = if self.peek() == Some(&TokenKind::LBrace) {
            None
        } else {
            self.identifier()
        };

        if !self.eat(&TokenKind::LBrace) {
            self.unexpected("`{` after the zone");
            return None;
        }

        let mut bounds = None;
        let mut layer = None;
        let mut net = None;

        while !self.done() && !self.eat(&TokenKind::RBrace) {
            match self.peek_ident() {
                Some("bounds") => {
                    self.bump();
                    let min_x = self.dimension();
                    self.eat(&TokenKind::Comma);
                    let min_y = self.dimension();
                    let has_to = self.eat_word("to");
                    let max_x = self.dimension();
                    self.eat(&TokenKind::Comma);
                    let max_y = self.dimension();
                    match (min_x, min_y, has_to, max_x, max_y) {
                        (Some(min_x), Some(min_y), true, Some(max_x), Some(max_y)) => {
                            bounds = Some((min_x, min_y, max_x, max_y))
                        }
                        _ => self.unexpected("bounds like `5mm, 5mm to 35mm, 35mm`"),
                    }
                }
                Some("layer") => {
                    self.bump();
                    match self.identifier() {
                        Some(name) => layer = Some(name.value),
                        None => self.unexpected("a layer name"),
                    }
                }
                Some("net") => {
                    self.bump();
                    // The same reader the `net` block uses, so a quoted name
                    // means here what it means there.
                    match self.net_name() {
                        Some(name) => net = Some(name),
                        None => self.unexpected("a net name"),
                    }
                }
                _ => self.unknown_property("zone", &["bounds", "layer", "net"]),
            }
        }

        let Some(bounds) = bounds else {
            self.unexpected("a zone with bounds");
            return None;
        };

        Some(ZoneDef {
            kind,
            name,
            bounds,
            layer,
            net,
            span: Span::new(start, self.behind()),
        })
    }

    /// `diffpair USB { USB_DP USB_DM }`.
    fn diffpair(&mut self, start: usize) -> Option<DiffPairDef> {
        self.bump(); // `diffpair`
        let name = match self.identifier() {
            Some(name) => name,
            None => {
                self.unexpected("a pair name");
                return None;
            }
        };
        if !self.eat(&TokenKind::LBrace) {
            self.unexpected("`{` after the pair name");
            return None;
        }
        let Some(positive) = self.net_name() else {
            self.unexpected("the net carrying the positive half");
            return None;
        };
        let Some(negative) = self.net_name() else {
            self.unexpected("the net carrying the negative half");
            return None;
        };
        if !self.eat(&TokenKind::RBrace) {
            self.unexpected("`}` after the pair's two nets");
            return None;
        }
        Some(DiffPairDef {
            name,
            positive,
            negative,
            span: Span::new(start, self.behind()),
        })
    }

    /// `netclass Power [width 0.5mm] { VCC GND }`.
    ///
    /// The same constraint block a net carries, stated once for a group. A net
    /// that says something itself keeps its own answer; this fills in the rest.
    fn netclass(&mut self, start: usize) -> Option<NetClassDef> {
        self.bump(); // `netclass`
        let name = match self.identifier() {
            Some(name) => name,
            None => {
                self.unexpected("a net class name");
                return None;
            }
        };
        let constraints = self.net_constraints();

        let mut members = Vec::new();
        if self.eat(&TokenKind::LBrace) {
            while !self.done() && !self.eat(&TokenKind::RBrace) {
                if self.eat(&TokenKind::Comma) {
                    continue;
                }
                match self.net_name() {
                    Some(member) => members.push(member),
                    None => {
                        self.unexpected("a net name");
                        self.bump();
                    }
                }
            }
        }

        Some(NetClassDef {
            name,
            constraints,
            members,
            span: Span::new(start, self.behind()),
        })
    }

    /// `outline { point 0mm, 0mm  point 40mm, 0mm ... }`.
    ///
    /// The ring is closed implicitly, which is why the reader does not look
    /// for a repeat of the first point.
    fn outline(&mut self, start: usize) -> Option<OutlineDef> {
        self.bump(); // `outline`
        if !self.eat(&TokenKind::LBrace) {
            self.unexpected("`{` after `outline`");
            return None;
        }

        let mut points = Vec::new();
        while !self.done() && !self.eat(&TokenKind::RBrace) {
            if !self.eat_word("point") {
                self.unexpected("`point` inside an outline");
                self.bump();
                continue;
            }
            let x = self.dimension();
            self.eat(&TokenKind::Comma);
            let y = self.dimension();
            match x.zip(y) {
                Some(point) => points.push(point),
                None => self.unexpected("a point like `10mm, 0mm`"),
            }
        }

        Some(OutlineDef {
            points,
            span: Span::new(start, self.behind()),
        })
    }

    /// `import "path"` and `import Name, Other from "path"`.
    fn import(&mut self, start: usize) -> Option<ImportDef> {
        self.bump(); // `import`

        // A string here means the whole file; a name list means `from` follows.
        if let Some(path) = self.string() {
            return Some(ImportDef {
                names: Vec::new(),
                path,
                span: Span::new(start, self.behind()),
            });
        }

        let mut names = Vec::new();
        loop {
            match self.identifier() {
                Some(name) => names.push(name),
                None => {
                    self.unexpected("a name to import");
                    return None;
                }
            }
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        if !self.eat_word("from") {
            self.unexpected("`from` after the names to import");
            return None;
        }
        let path = match self.string() {
            Some(path) => path,
            None => {
                self.unexpected("a path in quotes");
                return None;
            }
        };

        Some(ImportDef {
            names,
            path,
            span: Span::new(start, self.behind()),
        })
    }

    /// `module Name { component ...  net ...  pin OUT  use Other as X { ... } }`.
    ///
    /// A module body holds the same definitions a design does, plus the pins it
    /// exposes, so this reads the same functions the top level does.
    fn module(&mut self, start: usize) -> Option<ModuleDef> {
        self.bump(); // `module`
        let name = match self.identifier() {
            Some(name) => name,
            None => {
                self.unexpected("a module name");
                return None;
            }
        };
        if !self.eat(&TokenKind::LBrace) {
            self.unexpected("`{` after the module name");
            return None;
        }

        let mut definitions = Vec::new();
        let mut pins = Vec::new();
        let mut implements = Vec::new();

        while !self.done() && !self.eat(&TokenKind::RBrace) {
            let item_start = self.here();
            match self.peek_ident() {
                Some("component") => match self.component(item_start) {
                    Some(def) => definitions.push(Definition::Component(def)),
                    None => self.bump_past_block(),
                },
                Some("net") => match self.net(item_start) {
                    Some(def) => definitions.push(Definition::Net(def)),
                    None => self.bump_past_block(),
                },
                Some("use") => match self.module_instance(item_start) {
                    Some(def) => definitions.push(Definition::ModuleInstance(def)),
                    None => self.bump_past_block(),
                },
                Some("assert") => match self.assertion(item_start) {
                    Some(def) => definitions.push(Definition::Assert(def)),
                    None => self.bump_past_block(),
                },
                Some("pin") => {
                    self.bump();
                    match self.identifier() {
                        Some(name) => pins.push(PinDeclaration {
                            name,
                            span: Span::new(item_start, self.behind()),
                        }),
                        None => self.unexpected("a pin name"),
                    }
                }
                Some("implements") => {
                    self.bump();
                    match self.identifier() {
                        Some(interface) => implements.push(ImplementsClause {
                            interface,
                            span: Span::new(item_start, self.behind()),
                        }),
                        None => self.unexpected("an interface name"),
                    }
                }
                _ => self.unknown_property(
                    "module",
                    &["pin", "implements", "component", "net", "use", "assert"],
                ),
            }
        }

        Some(ModuleDef {
            name,
            definitions,
            pins,
            implements,
            span: Span::new(start, self.behind()),
        })
    }

    /// `use Divider as DIV1 at 10mm, 10mm rotate 90 { IN = VIN, OUT = SENSE }`.
    fn module_instance(&mut self, start: usize) -> Option<ModuleInstance> {
        self.bump(); // `use`
        let module = match self.identifier() {
            Some(module) => module,
            None => {
                self.unexpected("a module name");
                return None;
            }
        };
        if !self.eat_word("as") {
            self.unexpected("`as` and a name for the instance");
            return None;
        }
        let name = match self.identifier() {
            Some(name) => name,
            None => {
                self.unexpected("a name for the instance");
                return None;
            }
        };

        let mut position = None;
        if self.peek_ident() == Some("at") {
            let property_start = self.here();
            self.bump();
            let x = self.dimension();
            self.eat(&TokenKind::Comma);
            let y = self.dimension();
            match x.zip(y) {
                Some((x, y)) => {
                    position = Some(PositionExpr {
                        x,
                        y,
                        span: Span::new(property_start, self.behind()),
                    })
                }
                None => self.unexpected("a position like `at 10mm, 8mm`"),
            }
        }

        let mut rotation = None;
        if self.peek_ident() == Some("rotate") {
            let property_start = self.here();
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

        let mut ports = Vec::new();
        if self.eat(&TokenKind::LBrace) {
            while !self.done() && !self.eat(&TokenKind::RBrace) {
                if self.eat(&TokenKind::Comma) {
                    continue;
                }
                let port_start = self.here();
                let pin = self.identifier();
                let wired = self.eat(&TokenKind::Equals);
                let net = self.identifier();
                match (pin, wired, net) {
                    (Some(pin), true, Some(net)) => ports.push(PortConnection {
                        pin,
                        net,
                        span: Span::new(port_start, self.behind()),
                    }),
                    _ => {
                        self.unexpected("a connection like `IN = VIN`");
                        self.bump();
                    }
                }
            }
        }

        Some(ModuleInstance {
            module,
            name,
            position,
            rotation,
            ports,
            span: Span::new(start, self.behind()),
        })
    }

    /// `interface Name { pin SDA  pin SCL }`.
    fn interface(&mut self, start: usize) -> Option<InterfaceDef> {
        self.bump(); // `interface`
        let name = match self.identifier() {
            Some(name) => name,
            None => {
                self.unexpected("an interface name");
                return None;
            }
        };
        if !self.eat(&TokenKind::LBrace) {
            self.unexpected("`{` after the interface name");
            return None;
        }

        let mut pins = Vec::new();
        while !self.done() && !self.eat(&TokenKind::RBrace) {
            let pin_start = self.here();
            if !self.eat_word("pin") {
                self.unexpected("`pin` inside an interface");
                self.bump();
                continue;
            }
            match self.identifier() {
                Some(name) => pins.push(PinDeclaration {
                    name,
                    span: Span::new(pin_start, self.behind()),
                }),
                None => self.unexpected("a pin name"),
            }
        }

        Some(InterfaceDef {
            name,
            pins,
            span: Span::new(start, self.behind()),
        })
    }

    /// `assert R1.value >= 10kohm` and `assert C1.value within 100nF +/- 5%`.
    fn assertion(&mut self, start: usize) -> Option<AssertDef> {
        self.bump(); // `assert`
        let left = self.assert_operand()?;

        if self.eat_word("within") {
            let target = self.physical_value()?;
            let span = Span::new(start, self.behind());
            return Some(AssertDef {
                expression: AssertExpression::Within {
                    left,
                    target,
                    span: Span::new(start + "assert ".len(), span.end),
                },
                span,
            });
        }

        let op = match self.peek() {
            Some(TokenKind::Op(spelling)) => match ComparisonOp::from_str(spelling) {
                Some(op) => {
                    self.bump();
                    op
                }
                None => {
                    self.unexpected("a comparison like `>=`");
                    return None;
                }
            },
            _ => {
                self.unexpected("a comparison like `>=` or `within`");
                return None;
            }
        };
        let right = self.assert_operand()?;
        let span = Span::new(start, self.behind());

        Some(AssertDef {
            expression: AssertExpression::Comparison {
                left,
                op,
                right,
                span: Span::new(start + "assert ".len(), span.end),
            },
            span,
        })
    }

    /// One side of an assertion: `R1.value`, `10kohm`, `0.3mm` or a number.
    fn assert_operand(&mut self) -> Option<AssertOperand> {
        let start = self.here();
        match self.peek() {
            Some(TokenKind::Ident(_)) => {
                let mut parts = vec![self.identifier()?.value];
                while self.eat(&TokenKind::Dot) {
                    match self.identifier() {
                        Some(part) => parts.push(part.value),
                        None => {
                            self.unexpected("a name after `.`");
                            return None;
                        }
                    }
                }
                Some(AssertOperand::QualifiedName {
                    parts,
                    span: Span::new(start, self.behind()),
                })
            }
            Some(TokenKind::Number(_)) => {
                // A number, then whichever unit follows it - a physical one, a
                // length, or none at all.
                if let Some(value) = self.try_physical_value() {
                    return Some(AssertOperand::Physical(value));
                }
                let (value, span) = self.number()?;
                match self.peek_ident() {
                    Some("mm") | Some("mil") | Some("in") | Some("nm") => {
                        self.at -= 1;
                        self.dimension().map(AssertOperand::Dimension)
                    }
                    _ => Some(AssertOperand::Number { value, span }),
                }
            }
            _ => {
                self.unexpected("a value or a name like `R1.value`");
                None
            }
        }
    }

    /// `10kohm`, optionally with a tolerance.
    fn physical_value(&mut self) -> Option<PhysicalValue> {
        match self.try_physical_value() {
            Some(value) => Some(value),
            None => {
                self.unexpected("a value with an electrical unit, like `10kohm`");
                None
            }
        }
    }

    /// The same, but leaves the cursor alone when the next tokens are not one.
    fn try_physical_value(&mut self) -> Option<PhysicalValue> {
        let mark = self.at;
        let start = self.here();
        let Some((value, _)) = self.number() else {
            self.at = mark;
            return None;
        };
        let Some(unit) = self
            .peek_ident()
            .and_then(|word| word.parse::<cypcb_core::PhysicalUnit>().ok())
        else {
            self.at = mark;
            return None;
        };
        self.bump();

        let tolerance = self.tolerance();

        Some(PhysicalValue {
            value,
            unit,
            tolerance,
            span: Span::new(start, self.behind()),
        })
    }

    /// `+/- 5%`, `+/- 0.1V` or `to 220nF`.
    fn tolerance(&mut self) -> Option<Tolerance> {
        let start = self.here();
        if self.peek() == Some(&TokenKind::Op("+/-".to_string())) {
            self.bump();
            let (value, _) = self.number()?;
            if self.peek() == Some(&TokenKind::Op("%".to_string())) {
                self.bump();
                return Some(Tolerance {
                    kind: ToleranceKind::Percentage { value },
                    span: Span::new(start, self.behind()),
                });
            }
            let unit_start = self.here();
            let unit = self
                .peek_ident()
                .and_then(|word| word.parse::<cypcb_core::PhysicalUnit>().ok())?;
            self.bump();
            return Some(Tolerance {
                kind: ToleranceKind::Absolute(Box::new(PhysicalValue {
                    value,
                    unit,
                    tolerance: None,
                    span: Span::new(unit_start, self.behind()),
                })),
                span: Span::new(start, self.behind()),
            });
        }

        if self.peek_ident() == Some("to") {
            let mark = self.at;
            self.bump();
            match self.try_physical_value() {
                Some(upper) => {
                    return Some(Tolerance {
                        kind: ToleranceKind::Range(Box::new(upper)),
                        span: Span::new(start, self.behind()),
                    })
                }
                None => {
                    self.at = mark;
                    return None;
                }
            }
        }

        None
    }

    /// Step over a definition inside a block that could not be read.
    fn bump_past_block(&mut self) {
        let mut depth = 0i32;
        while self.at < self.tokens.len() {
            match &self.tokens[self.at].kind {
                TokenKind::LBrace => depth += 1,
                TokenKind::RBrace => {
                    if depth == 0 {
                        return;
                    }
                    depth -= 1;
                    self.at += 1;
                    if depth == 0 {
                        return;
                    }
                    continue;
                }
                _ => {}
            }
            self.at += 1;
        }
    }

    /// Step past a property that could not be read, without leaving the block.
    fn skip_to_next_property(&mut self) {
        while self.at < self.tokens.len() {
            match &self.tokens[self.at].kind {
                TokenKind::RBrace => return,
                TokenKind::Ident(word)
                    if matches!(
                        word.as_str(),
                        "pad" | "silk" | "description" | "courtyard" | "bounds" | "layer" | "net"
                    ) =>
                {
                    return
                }
                _ => self.at += 1,
            }
        }
    }
}
