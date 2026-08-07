#![cfg(all(feature = "wasm", not(feature = "native")))]
//! The engine the browser loads can read `.cypcb` itself.
//!
//! `cargo test -p cypcb-render --no-default-features --features wasm --test the_browser_build_reads_the_language`
//!
//! Until now it could not: `cypcb-parser` was compiled with
//! `default-features = false` for wasm, which left it with no parser, so the
//! viewer read the DSL a second time in TypeScript and handed the engine a
//! snapshot. That second reader does not instantiate modules or follow
//! imports, which is measured board by board in
//! `viewer/src/__tests__/parser-drift.test.ts`.
//!
//! This runs the browser build's own path on the host: same features, same
//! reader, no C.

use cypcb_render::PcbEngine;

const A_MODULE_AND_AN_INSTANCE: &str = r#"
version 1

board t {
    size 40mm x 30mm
    layers 2
}

module Divider {
    component RTOP resistor "0402" {
        at 0mm, 0mm
    }
    component RBOT resistor "0402" {
        at 0mm, 2mm
    }
    net mid {
        RTOP.2
        RBOT.1
    }
    pin OUT
}

use Divider as DIV1 at 10mm, 10mm {
    OUT = SENSE
}
"#;

#[test]
fn a_board_the_browser_loads_comes_back_with_its_parts() {
    let mut engine = PcbEngine::new();
    let errors = engine.load_source(
        "board t {\n    size 30mm x 20mm\n    layers 2\n}\n\
         component R1 resistor \"0402\" {\n    at 5mm, 10mm\n}\n\
         component C1 capacitor \"0402\" {\n    at 20mm, 10mm\n}\n\
         net VCC {\n    R1.1\n    C1.1\n}\n",
    );
    assert!(errors.is_empty(), "the board is good, got: {errors}");

    let snapshot = engine.build_snapshot();
    assert_eq!(snapshot.components.len(), 2, "two parts were placed");
    assert_eq!(snapshot.nets.len(), 1, "one net was named");
    assert!(
        snapshot.board.is_some(),
        "the board itself has to reach the snapshot"
    );
}

#[test]
fn the_module_the_second_reader_could_not_instantiate_arrives() {
    // This is the defect the one-parser work exists for: the browser drew the
    // parts written inside a module body under their local names, or nothing
    // at all. The engine instantiates.
    let mut engine = PcbEngine::new();
    let errors = engine.load_source(A_MODULE_AND_AN_INSTANCE);
    assert!(errors.is_empty(), "the board is good, got: {errors}");

    let snapshot = engine.build_snapshot();
    let refdes: Vec<&str> = snapshot
        .components
        .iter()
        .map(|component| component.refdes.as_str())
        .collect();

    assert_eq!(
        refdes.len(),
        2,
        "the instance places both of the module's parts, got {refdes:?}"
    );
    for name in &refdes {
        assert!(
            name.starts_with("DIV1"),
            "a part of an instance carries the instance's name, got {name}"
        );
    }
}

#[test]
fn a_broken_board_says_so_rather_than_loading_empty() {
    let mut engine = PcbEngine::new();
    let errors = engine.load_source("version 1\nfrobnicate 3\n");
    assert!(
        !errors.is_empty(),
        "a word the language does not have has to be reported"
    );
}
