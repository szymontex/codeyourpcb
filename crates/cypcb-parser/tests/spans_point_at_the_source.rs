//! Every span the reader hands out has to name the bytes it came from.
//!
//! `cargo test -p cypcb-parser --test spans_point_at_the_source`
//!
//! `differential.rs` strips spans before comparing the two parsers, on purpose:
//! tree-sitter's node boundaries are its own, and pinning them would pin a
//! parser rather than a language. That left the spans themselves unchecked,
//! and the LSP is built on them - hover, goto and completion all ask "what is
//! at this offset".
//!
//! So this checks them against the thing that cannot drift: the source text. An
//! identifier's span has to spell the identifier, a string's span has to be the
//! quoted literal, and a definition's span has to start with the keyword that
//! opens it and end after the block it closes.

use std::path::{Path, PathBuf};

use cypcb_parser::ast::{Definition, SourceFile};
use cypcb_parser::{parse, Span};

fn examples_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .join("examples")
}

/// Boards written to demonstrate a parse error, where a span may point at the
/// fault rather than at a whole definition.
const MEANT_TO_FAIL: &[&str] = &["invalid.cypcb", "unknown_keyword.cypcb"];

fn examples() -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(examples_dir())
        .expect("the examples directory is there")
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "cypcb"))
        .filter(|path| {
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            !MEANT_TO_FAIL.contains(&name.as_str())
        })
        .collect();
    files.sort();
    files
}

fn slice(source: &str, span: Span) -> &str {
    &source[span.start.min(source.len())..span.end.min(source.len())]
}

/// The keyword each definition is written with.
fn keyword(definition: &Definition) -> &'static [&'static str] {
    match definition {
        Definition::Board(_) => &["board"],
        Definition::Component(_) => &["component"],
        Definition::Net(_) => &["net"],
        Definition::Footprint(_) => &["footprint"],
        // Three words, one definition: a copper pour, an area nothing may
        // enter, and the part of a rigid-flex board that bends.
        Definition::Zone(_) => &["zone", "keepout", "flex"],
        Definition::Trace(_) => &["trace"],
        Definition::Module(_) => &["module"],
        Definition::ModuleInstance(_) => &["use"],
        Definition::NetClass(_) => &["netclass"],
        Definition::Outline(_) => &["outline"],
        Definition::Interface(_) => &["interface"],
        Definition::Import(_) => &["import"],
        Definition::Assert(_) => &["assert"],
        Definition::DiffPair(_) => &["diffpair"],
        Definition::Text(_) => &["text"],
    }
}

/// Every identifier and string in a file, as (what it should spell, its span).
fn named_things(ast: &SourceFile) -> Vec<(String, Span, &'static str)> {
    let mut found = Vec::new();

    for definition in &ast.definitions {
        match definition {
            Definition::Board(board) => {
                found.push((board.name.value.clone(), board.name.span, "identifier"));
            }
            Definition::Component(component) => {
                found.push((
                    component.refdes.value.clone(),
                    component.refdes.span,
                    "identifier",
                ));
                found.push((
                    component.footprint.value.clone(),
                    component.footprint.span,
                    "string",
                ));
                for assignment in &component.net_assignments {
                    found.push((
                        assignment.net.value.clone(),
                        assignment.net.span,
                        "identifier",
                    ));
                }
            }
            Definition::Net(net) => {
                found.push((net.name.value.clone(), net.name.span, "identifier"));
                for pin in &net.connections {
                    found.push((
                        pin.component.value.clone(),
                        pin.component.span,
                        "identifier",
                    ));
                }
            }
            Definition::Footprint(footprint) => {
                found.push((
                    footprint.name.value.clone(),
                    footprint.name.span,
                    "identifier",
                ));
            }
            Definition::Trace(trace) => {
                found.push((trace.net.value.clone(), trace.net.span, "identifier"));
            }
            Definition::Module(module) => {
                found.push((module.name.value.clone(), module.name.span, "identifier"));
                for pin in &module.pins {
                    found.push((pin.name.value.clone(), pin.name.span, "identifier"));
                }
            }
            Definition::ModuleInstance(instance) => {
                found.push((
                    instance.module.value.clone(),
                    instance.module.span,
                    "identifier",
                ));
                found.push((
                    instance.name.value.clone(),
                    instance.name.span,
                    "identifier",
                ));
                for port in &instance.ports {
                    found.push((port.pin.value.clone(), port.pin.span, "identifier"));
                    found.push((port.net.value.clone(), port.net.span, "identifier"));
                }
            }
            Definition::NetClass(class) => {
                found.push((class.name.value.clone(), class.name.span, "identifier"));
                for member in &class.members {
                    found.push((member.value.clone(), member.span, "identifier"));
                }
            }
            Definition::Interface(interface) => {
                found.push((
                    interface.name.value.clone(),
                    interface.name.span,
                    "identifier",
                ));
                for pin in &interface.pins {
                    found.push((pin.name.value.clone(), pin.name.span, "identifier"));
                }
            }
            Definition::Import(import) => {
                found.push((import.path.value.clone(), import.path.span, "string"));
                for name in &import.names {
                    found.push((name.value.clone(), name.span, "identifier"));
                }
            }
            Definition::Zone(zone) => {
                if let Some(name) = &zone.name {
                    found.push((name.value.clone(), name.span, "identifier"));
                }
                if let Some(net) = &zone.net {
                    found.push((net.value.clone(), net.span, "identifier"));
                }
            }
            Definition::DiffPair(pair) => {
                found.push((pair.name.value.clone(), pair.name.span, "identifier"));
                found.push((
                    pair.positive.value.clone(),
                    pair.positive.span,
                    "identifier",
                ));
                found.push((
                    pair.negative.value.clone(),
                    pair.negative.span,
                    "identifier",
                ));
            }
            // A legend's words are a string, but not a name: nothing refers
            // to them, so there is nothing for this to cross-check.
            Definition::Outline(_) | Definition::Assert(_) | Definition::Text(_) => {}
        }
    }

    found
}

#[test]
fn an_identifier_span_spells_the_identifier() {
    let mut wrong = Vec::new();

    for file in examples() {
        let name = file
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let source = std::fs::read_to_string(&file).expect("the example is readable");
        let ast = parse(&source);
        assert!(
            ast.errors.is_empty(),
            "{name} should parse: {:?}",
            ast.errors
        );

        for (value, span, kind) in named_things(&ast.value) {
            let text = slice(&source, span);
            let matches = match kind {
                "string" => text == format!("\"{value}\""),
                _ => text == value,
            };
            if !matches {
                wrong.push(format!(
                    "{name}: {kind} `{value}` has a span holding `{text}`"
                ));
            }
        }
    }

    assert!(
        wrong.is_empty(),
        "spans that do not name what they point at:\n{}",
        wrong.join("\n")
    );
}

#[test]
fn a_definition_span_starts_at_its_keyword() {
    // What goto and hover need: the range of the thing under the cursor. A
    // definition whose span starts a token early or late sends the editor to
    // the wrong place.
    let mut wrong = Vec::new();

    for file in examples() {
        let name = file
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let source = std::fs::read_to_string(&file).expect("the example is readable");
        let ast = parse(&source);

        for definition in &ast.value.definitions {
            let span = definition.span();
            let text = slice(&source, span);
            let expected = keyword(definition);
            if !expected.iter().any(|word| text.starts_with(word)) {
                wrong.push(format!(
                    "{name}: a definition's span starts with `{}`, expected one of {expected:?}",
                    text.chars().take(20).collect::<String>()
                ));
            }
            if span.end <= span.start {
                wrong.push(format!("{name}: a definition has an empty span {span:?}"));
            }
        }
    }

    assert!(
        wrong.is_empty(),
        "definition spans that do not cover their definition:\n{}",
        wrong.join("\n")
    );
}

#[test]
fn a_definition_span_ends_after_everything_inside_it() {
    // The other half: a span that stops early hides the last property from
    // anything asking what is at an offset.
    let mut wrong = Vec::new();

    for file in examples() {
        let name = file
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let source = std::fs::read_to_string(&file).expect("the example is readable");
        let ast = parse(&source);

        for definition in &ast.value.definitions {
            let outer = definition.span();
            for (value, inner, _) in named_things(&SourceFile {
                version: None,
                definitions: vec![definition.clone()],
                span: outer,
            }) {
                if inner.start < outer.start || inner.end > outer.end {
                    wrong.push(format!(
                        "{name}: `{value}` at {inner:?} sits outside its definition at {outer:?}"
                    ));
                }
            }
        }
    }

    assert!(
        wrong.is_empty(),
        "things whose span escapes the definition holding them:\n{}",
        wrong.join("\n")
    );
}

/// How far apart the two parsers put the same definition.
///
/// `cargo test -p cypcb-parser --features tree-sitter-parser --test spans_point_at_the_source -- --ignored --nocapture`
///
/// A diagnostic, not a gate. The differential test strips spans because these
/// two do not have to agree; this prints how much they do, so the decision to
/// strip them is a measured one rather than a convenient one.
#[cfg(feature = "tree-sitter-parser")]
#[test]
#[ignore = "diagnostic: compares span boundaries between the two parsers"]
fn how_far_apart_the_two_parsers_put_a_definition() {
    let mut same = 0usize;
    let mut different = 0usize;
    let mut worst = 0usize;

    for file in examples() {
        let name = file
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let source = std::fs::read_to_string(&file).expect("the example is readable");
        let ours = parse(&source);
        let theirs = cypcb_parser::tree_sitter_parse(&source);

        for (a, b) in ours.value.definitions.iter().zip(&theirs.value.definitions) {
            let (x, y) = (a.span(), b.span());
            if x.start == y.start && x.end == y.end {
                same += 1;
            } else {
                different += 1;
                let delta = x.start.abs_diff(y.start).max(x.end.abs_diff(y.end));
                worst = worst.max(delta);
                eprintln!("  {name}: {x:?} against {y:?}");
            }
        }
    }

    eprintln!("definitions with identical spans: {same}, different: {different}, widest gap: {worst} bytes");
}
