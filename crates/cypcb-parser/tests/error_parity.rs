#![cfg(feature = "rust-parser")]
//! Both readers have to reject the same files.
//!
//! `cargo test -p cypcb-parser --features rust-parser --test error_parity`
//!
//! `differential.rs` proves the two agree on what a good file means. This is
//! the other half: a reader that quietly accepts a typo is worse than one that
//! is missing a construct, because the design comes out short a part and
//! nothing says so. It was measured before it was pinned - `frobnicate 3` at
//! the top level parsed clean under the Rust reader while tree-sitter reported
//! it.
//!
//! What is compared is whether a file is rejected and roughly where, not the
//! wording or the count. The two recover differently: tree-sitter reports every
//! token it could not place, this reader reports the first thing it wanted and
//! then walks to the next definition, so `unknown_keyword.cypcb` is two errors
//! against one. Pinning the counts equal would be pinning a recovery strategy,
//! not a language.

use cypcb_parser::{parse, reader};

/// A source, and whether the language accepts it.
struct Case {
    what: &'static str,
    source: &'static str,
}

const BAD: &[Case] = &[
    Case {
        what: "a property with its value missing",
        source: "version 1\nboard test {\n    size 50mm x 30mm\n    layers\n}\n",
    },
    Case {
        what: "a word the language does not have",
        source: "version 1\nfrobnicate 3\n",
    },
    Case {
        what: "a brace that closes nothing",
        source: "board t { layers 2 }\n}\n",
    },
    Case {
        what: "a component with no footprint",
        source: "board t { layers 2 }\ncomponent R1 resistor {\n    at 1mm, 1mm\n}\n",
    },
    Case {
        what: "a pad with no shape",
        source: "footprint F {\n    pad 1 at 0mm, 0mm size 1mm x 1mm\n}\n",
    },
];

const GOOD: &[Case] = &[
    Case {
        what: "a board on its own",
        source: "version 1\nboard t {\n    size 30mm x 20mm\n    layers 2\n}\n",
    },
    Case {
        what: "a part, a net and a trace",
        source: "board t { size 30mm x 20mm layers 2 }\n\
                 component R1 resistor \"0402\" { at 5mm, 10mm }\n\
                 component C1 capacitor \"0402\" { at 20mm, 10mm }\n\
                 net VCC { R1.1 C1.1 }\n\
                 trace VCC { from R1.1 to C1.1 layer Top width 0.3mm }\n",
    },
    Case {
        what: "a module and an instance of it",
        source: "board t { size 30mm x 20mm layers 2 }\n\
                 module Div {\n  component R1 resistor \"0402\" { at 1mm, 1mm }\n  pin OUT\n}\n\
                 use Div as D1 at 5mm, 5mm { OUT = SENSE }\n",
    },
];

#[test]
fn a_file_the_old_parser_rejects_is_rejected_by_the_new_one() {
    let mut accepted = Vec::new();
    for case in BAD {
        let expected = parse(case.source);
        assert!(
            !expected.errors.is_empty(),
            "{}: the fixture is supposed to be broken and tree-sitter accepted it",
            case.what
        );

        if reader::read(case.source).errors.is_empty() {
            accepted.push(case.what);
        }
    }

    assert!(
        accepted.is_empty(),
        "the Rust reader accepted files the language rejects: {accepted:?}"
    );
}

#[test]
fn a_file_the_old_parser_accepts_is_not_rejected_by_the_new_one() {
    // The expensive failure in the other direction: a reader that complains
    // about a good board makes the editor unusable.
    let mut rejected = Vec::new();
    for case in GOOD {
        let expected = parse(case.source);
        assert!(
            expected.errors.is_empty(),
            "{}: the fixture is supposed to be good and tree-sitter rejected it: {:?}",
            case.what,
            expected.errors
        );

        let actual = reader::read(case.source);
        if !actual.errors.is_empty() {
            rejected.push(format!("{}: {:?}", case.what, actual.errors));
        }
    }

    assert!(
        rejected.is_empty(),
        "the Rust reader rejected good boards:\n{}",
        rejected.join("\n")
    );
}

#[test]
fn the_examples_written_to_fail_still_fail_both_ways() {
    // These two are the repo's own demonstration of a broken file, and they
    // are the only boards the differential test skips.
    for name in ["invalid.cypcb", "unknown_keyword.cypcb"] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("the crate sits two levels below the repo root")
            .join("examples")
            .join(name);
        let source = std::fs::read_to_string(&path).expect("the example is readable");

        assert!(
            !parse(&source).errors.is_empty(),
            "{name} is supposed to demonstrate a parse error"
        );
        assert!(
            !reader::read(&source).errors.is_empty(),
            "{name} is a parse error the Rust reader does not report"
        );
    }
}

#[test]
fn an_error_points_at_the_fault_rather_than_at_the_start_of_the_file() {
    // A position, not just a count: an error reported at offset zero is an
    // error nobody can find. The two readers land a token apart on this file -
    // tree-sitter marks the missing value on line 4, this reader marks the `}`
    // on line 5 that it found where a number should have been - so what is
    // pinned is the neighbourhood, which is what a person needs to fix it.
    let source = "version 1\nboard test {\n    size 50mm x 30mm\n    layers\n}\n";
    let result = reader::read(source);
    let first = result.errors.first().expect("an error");

    let offset = match first {
        cypcb_parser::ParseError::Missing { span, .. } => span.offset(),
        other => panic!("unexpected error kind: {other:?}"),
    };
    let line = source[..offset].lines().count();
    assert!(
        (4..=5).contains(&line),
        "the fault is the empty `layers` on line 4, reported on line {line} at offset {offset}"
    );

    let expected = parse(source);
    let their_offset = match expected.errors.first().expect("tree-sitter finds it too") {
        cypcb_parser::ParseError::InvalidNumber { span, .. } => span.offset(),
        other => panic!("unexpected error kind from tree-sitter: {other:?}"),
    };
    assert!(
        offset.abs_diff(their_offset) < 20,
        "the two readers point {} bytes apart, which is not the same fault",
        offset.abs_diff(their_offset)
    );
}
