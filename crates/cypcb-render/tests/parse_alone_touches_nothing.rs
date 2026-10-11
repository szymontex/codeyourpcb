//! The engine can read a design and nothing else.
//!
//! `cargo test -p cypcb-render --test parse_alone_touches_nothing`
//!
//! `PcbEngine::load_source` is parse, imports, board model and the whole DRC in
//! one call, and the WASM engine had no other, so the browser could not say
//! how long reading a design takes or whether it reads without paying for the
//! rest (owner ruling 2026-10-07). `parse_only` and `parse_kicad_only` are that
//! first step on its own. What is held here: they accept what `load_source`
//! and `load_kicad` accept, they report what the reader reports, and they leave
//! the engine's board exactly as it was.

use std::path::Path;

use cypcb_render::PcbEngine;

fn read(relative: &str) -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(relative),
    )
    .unwrap_or_else(|e| panic!("{relative} does not read: {e}"))
}

const BROKEN: &str = "version 1\nboard b {\n    size 10mm x\n";

#[test]
fn a_design_that_reads_gives_an_empty_answer() {
    let engine = PcbEngine::new();
    for path in [
        "examples/blink.cypcb",
        "tests/fixtures/benchmark/esp32_starter.cypcb",
    ] {
        let said = engine.parse_only(&read(path));
        assert_eq!(said, "", "{path} reads");
    }
    let said = engine.parse_kicad_only(&read("tests/fixtures/benchmark/led_blink.kicad_pcb"));
    assert_eq!(said, "", "the KiCad board reads");
}

#[test]
fn control_a_text_that_does_not_read_is_reported() {
    let engine = PcbEngine::new();
    assert!(
        !engine.parse_only(BROKEN).is_empty(),
        "the broken design is reported"
    );
    assert!(
        !engine.parse_kicad_only("(kicad_pcb (version").is_empty(),
        "the broken KiCad text is reported"
    );
}

#[test]
fn it_reports_what_load_source_reports_of_the_reader() {
    // The reader's messages come first in `load_source`'s answer, so the
    // parse-only answer is the start of it, word for word.
    let mut engine = PcbEngine::new();
    let said = engine.parse_only(BROKEN);
    let loaded = engine.load_source(BROKEN);
    assert!(!said.is_empty());
    assert!(
        loaded.starts_with(&said),
        "parse_only: {said:?}, load_source: {loaded:?}"
    );
}

#[test]
fn the_board_the_engine_holds_stays_as_it_was() {
    let mut engine = PcbEngine::new();
    let said = engine.load_source(&read("examples/blink.cypcb"));
    assert_eq!(said, "", "the board loads");
    let before = engine.get_snapshot();
    let violations = engine.violation_count();
    assert!(before.contains("\"components\""), "the snapshot is a board");

    // A different board, a broken one and a KiCad one, read and let go.
    let _ = engine.parse_only(&read("tests/fixtures/benchmark/esp32_starter.cypcb"));
    let _ = engine.parse_only(BROKEN);
    let _ = engine.parse_kicad_only(&read("tests/fixtures/benchmark/led_blink.kicad_pcb"));

    assert_eq!(engine.get_snapshot(), before, "the snapshot did not move");
    assert_eq!(
        engine.violation_count(),
        violations,
        "the violations did not move"
    );
}
