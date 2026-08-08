//! A design split across files has to load in the browser too.
//!
//! `cargo test -p cypcb-render --features native --test the_browser_can_follow_an_import`
//!
//! Every CLI command resolves `import "lib/blocks.cypcb"` against the disk,
//! and the language server learned to. A browser tab has no disk: the engine
//! could not open the file, so `examples/v2-imports.cypcb` loaded in the
//! viewer as `unknown module: 'Divider'` for every block it imports, while the
//! same file reports ten unrouted pins on the command line.
//!
//! The host fetches the files - it is the side with a network - and hands them
//! to the engine as a JSON object of path to text.

use cypcb_render::PcbEngine;

/// A library with no board of its own.
const BLOCKS: &str = r#"version 1

module Divider {
    pin IN
    pin OUT

    component RTOP resistor "0402" {
        value 10kohm
        at 0mm, 0mm
    }

    component RBOT resistor "0402" {
        value 10kohm
        at 0mm, 2mm
    }

    net IN {
        RTOP.1
    }

    net OUT {
        RTOP.2
        RBOT.1
    }
}
"#;

/// A design that is nothing without it.
const DESIGN: &str = r#"version 1

import Divider from "lib/blocks.cypcb"

board sensor {
    size 30mm x 20mm
    layers 2
}

use Divider as DIV_A at 10mm, 8mm {
    IN = VIN
    OUT = SENSE
}
"#;

fn files(pairs: &[(&str, &str)]) -> String {
    let map: std::collections::BTreeMap<&str, &str> = pairs.iter().copied().collect();
    serde_json::to_string(&map).expect("the fixture serializes")
}

fn refdes_of(engine: &mut PcbEngine) -> Vec<String> {
    let snapshot: serde_json::Value =
        serde_json::from_str(&engine.get_snapshot()).expect("the snapshot is JSON");
    snapshot["components"]
        .as_array()
        .expect("a snapshot carries components")
        .iter()
        .filter_map(|c| c["refdes"].as_str().map(str::to_string))
        .collect()
}

#[test]
fn the_host_supplies_the_file_and_the_blocks_arrive() {
    let mut engine = PcbEngine::new();
    let errors = engine.load_source_with_imports(DESIGN, &files(&[("lib/blocks.cypcb", BLOCKS)]));

    assert_eq!(errors, "", "the design and its library are both here");

    let parts = refdes_of(&mut engine);
    assert_eq!(
        parts,
        vec!["DIV_A_RTOP".to_string(), "DIV_A_RBOT".to_string()],
        "the instance brings its own copy of the imported module's parts"
    );
}

#[test]
fn a_file_the_host_did_not_supply_says_so() {
    // The failure has to name the path and what was on offer, because the host
    // is a program: a message saying only "unknown module" sends whoever is
    // debugging it to the design, which is not where the fault is.
    let mut engine = PcbEngine::new();
    let errors = engine.load_source_with_imports(DESIGN, &files(&[("lib/other.cypcb", BLOCKS)]));

    assert!(
        errors.contains("lib/blocks.cypcb"),
        "the missing path has to be named: {errors}"
    );
    assert!(
        errors.contains("lib/other.cypcb"),
        "and what the host did supply: {errors}"
    );
}

#[test]
fn a_design_with_no_imports_loads_as_it_always_did() {
    let mut engine = PcbEngine::new();
    let errors = engine.load_source_with_imports(
        "version 1\n\nboard b {\n    size 10mm x 10mm\n    layers 2\n}\n\ncomponent R1 resistor \"0402\" {\n    at 5mm, 5mm\n}\n",
        "{}",
    );

    assert_eq!(errors, "");
    assert_eq!(refdes_of(&mut engine), vec!["R1".to_string()]);
}

#[test]
fn a_library_may_be_built_from_libraries() {
    let inner = "version 1\n\nfootprint TINY {\n    courtyard 1mm x 1mm\n    pad 1 rect at 0mm, 0mm size 0.4mm x 0.4mm\n}\n";
    let middle = "version 1\n\nimport \"shared/tiny.cypcb\"\n\nmodule Dot {\n    pin P\n\n    component U1 generic \"TINY\" {\n        at 0mm, 0mm\n    }\n\n    net P {\n        U1.1\n    }\n}\n";
    let design = "version 1\n\nimport Dot from \"lib/dot.cypcb\"\n\nboard b {\n    size 10mm x 10mm\n    layers 2\n}\n\nuse Dot as D1 at 5mm, 5mm {\n    P = SIG\n}\n";

    let mut engine = PcbEngine::new();
    // `shared/tiny.cypcb` is imported by `lib/dot.cypcb`, so its path resolves
    // relative to that file: `lib/shared/tiny.cypcb`.
    let errors = engine.load_source_with_imports(
        design,
        &files(&[("lib/dot.cypcb", middle), ("lib/shared/tiny.cypcb", inner)]),
    );

    assert_eq!(errors, "", "a library may import a library");
    assert_eq!(refdes_of(&mut engine), vec!["D1_U1".to_string()]);
}

#[test]
fn the_files_have_to_be_a_json_object() {
    let mut engine = PcbEngine::new();
    let errors = engine.load_source_with_imports(DESIGN, "not json");
    assert!(
        errors.contains("JSON object"),
        "a host that sends rubbish is told what was expected: {errors}"
    );
}
