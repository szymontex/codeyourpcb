//! The help names every property the block takes.
//!
//! `cargo test -p cypcb-cli --test the_help_names_every_property_a_block_takes`
//!
//! Mistype inside a block and the parser answers with what the block does
//! take. The `trace` list said *from, to, path, layer, width, via, locked* -
//! seven of the eight - while the reader beside it had an arm for `neck` and
//! `trace SIG { neck 0.15mm for 1mm }` parsed. A designer who mistyped was
//! told the block does not take a neck, which is worse than saying nothing:
//! the list is the reason to trust the message.

use std::path::PathBuf;
use std::process::Command;

fn cypcb() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cypcb"))
}

fn scratch(who: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cypcb-block-help-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");
    dir
}

/// A board with two parts and a net, and whatever trace body is handed in.
fn board_with_trace(body: &str) -> String {
    format!(
        r#"version 1

board t {{
    size 20mm x 20mm
    layers 2
}}

component R1 resistor "0402" {{
    value "10k"
    at 5mm, 5mm
}}

component R2 resistor "0402" {{
    value "10k"
    at 15mm, 5mm
}}

net SIG {{
    R1.1
    R2.1
}}

trace SIG {{
    from R1.1
    to R2.1
    layer Top
    width 0.3mm
{body}
}}
"#
    )
}

/// Check that source and return everything the command said.
fn check(who: &str, source: &str) -> String {
    let board = scratch(who).join("board.cypcb");
    std::fs::write(&board, source).expect("the fixture is writable");
    let output = cypcb()
        .arg("check")
        .arg(&board)
        .output()
        .expect("the binary runs");
    String::from_utf8_lossy(&output.stdout).to_string() + &String::from_utf8_lossy(&output.stderr)
}

/// The rendered diagnostic as one line: miette wraps and draws a gutter.
fn flattened(text: &str) -> String {
    text.replace('\u{2502}', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn a_trace_that_necks_is_a_trace_that_takes_a_neck() {
    // Both halves in one place, which is the pairing that was broken: the
    // property parses, and the help names it.
    let parsed = check("neck-parses", &board_with_trace("    neck 0.15mm for 1mm"));
    assert!(
        !parsed.contains("no property `neck`"),
        "a neck on a trace is read, not refused:\n{parsed}"
    );

    let refused = check(
        "neck-listed",
        &board_with_trace("    neck 0.15mm for 1mm\n    nonsense 1mm"),
    );
    let said = flattened(&refused);
    assert!(
        said.contains("has no property `nonsense`"),
        "the mistyped word is the one refused:\n{said}"
    );
    assert!(
        said.contains("`trace` takes:") && said.contains("neck"),
        "and the list has to name the neck the reader accepts:\n{said}"
    );
}

#[test]
fn every_property_the_trace_help_names_is_one_a_trace_takes() {
    // The other direction. A list is a promise, and a name on it that the
    // reader refuses sends a person to write something that will not parse.
    let refused = check("list", &board_with_trace("    nonsense 1mm"));
    let said = flattened(&refused);
    let list = said
        .split("`trace` takes:")
        .nth(1)
        .expect("the help lists the properties")
        .trim()
        .to_string();

    // Each is exercised in the body a trace of this shape would carry.
    let bodies = [
        ("width", "    width 0.4mm"),
        ("layer", "    layer Bottom"),
        ("neck", "    neck 0.15mm for 1mm"),
        ("locked", "    locked"),
        ("via", "    via 10mm, 5mm"),
    ];
    for (name, body) in bodies {
        assert!(
            list.contains(name),
            "`{name}` is read by the trace block and the help does not name it: {list}"
        );
        let said = check(&format!("takes-{name}"), &board_with_trace(body));
        assert!(
            !said.contains(&format!("no property `{name}`")),
            "the help names `{name}` and the reader refuses it:\n{said}"
        );
    }
}
