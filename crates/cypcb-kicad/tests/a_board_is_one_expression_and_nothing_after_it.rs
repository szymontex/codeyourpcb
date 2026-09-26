//! A `.kicad_pcb` is one `(kicad_pcb ...)` and nothing after it.
//!
//! `cargo test -p cypcb-kicad --test a_board_is_one_expression_and_nothing_after_it`
//!
//! The S-expression reader stops at the board's closing parenthesis and
//! ignores the rest. The viewer's save once appended a design to a KiCad
//! board, and the file read back as the board without its copper and with no
//! error. Measured on `led_blink.kicad_pcb` with that tail: exit 0 and
//! `trace_segment_count` 0.

use cypcb_kicad::{parse_kicad_pcb_str, KicadPcbError};

fn led_blink() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/benchmark/led_blink.kicad_pcb");
    std::fs::read_to_string(path).expect("the benchmark board reads")
}

#[test]
fn the_board_alone_reads() {
    // The control: without it a reader that refuses everything passes the
    // test below.
    let board = led_blink();
    assert!(parse_kicad_pcb_str(&board).is_ok());
    assert!(parse_kicad_pcb_str(&format!("{board}\n\n  \t\n")).is_ok());
}

#[test]
fn a_design_after_the_board_is_refused_where_it_starts() {
    let board = led_blink();
    let board = board.trim_end();
    let lines = board.lines().count();
    let text = format!("{board}\n\nversion 1\n\ntrace VCC {{\n}}\n");
    match parse_kicad_pcb_str(&text) {
        Err(KicadPcbError::TrailingContent { line, column }) => {
            assert_eq!((line, column), (lines + 2, 1));
        }
        Err(other) => panic!("refused for another reason: {other}"),
        Ok(_) => panic!("the tail was read as nothing"),
    }
}

#[test]
fn text_on_the_closing_line_is_placed_by_its_column() {
    let text = "(kicad_pcb (version 20240108)) x";
    match parse_kicad_pcb_str(text) {
        Err(KicadPcbError::TrailingContent { line, column }) => assert_eq!((line, column), (1, 32)),
        other => panic!("expected the tail refused, got {:?}", other.err()),
    }
}

#[test]
fn a_parenthesis_inside_a_string_does_not_end_the_board() {
    // `(property "Description" "LED (red)")` closes nothing, and neither does
    // an escaped quote: `"say \")\""`.
    let board = led_blink();
    let at = board.find("(kicad_pcb").expect("the root") + "(kicad_pcb".len();
    let text = format!(
        "{} (title_block (comment 1 \"LED (red) \\\")\\\" )\")){}",
        &board[..at],
        &board[at..]
    );
    let result = parse_kicad_pcb_str(&text);
    assert!(
        !matches!(result, Err(KicadPcbError::TrailingContent { .. })),
        "a parenthesis in a string was read as the end of the board"
    );
}
