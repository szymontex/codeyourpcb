//! Every shipped example, saved and read back.
//!
//! `cargo test -p cypcb-world --test every_example_survives_being_saved`
//!
//! The single-shape round trips - a trace, a via, a neck, the board block, a
//! pad name - each pin one thing this writer got wrong. None of them runs the
//! corpus, and the corpus is what a person actually opens: 23 files, several
//! of them written to demonstrate a feature this writer has to carry.
//!
//! What this asks of each: parse it, write it back with `board_as_dsl`, parse
//! that, and compare a census of the two worlds. Twenty of the twenty-three
//! shipped examples make that trip with an identical census; the other three
//! are named below and none of them is a writer fault.

use cypcb_world::components::trace::{Trace, Via};
use cypcb_world::components::zone::Zone;
use cypcb_world::components::RefDes;
use cypcb_world::dsl::board_as_dsl;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{sync_ast_to_world, BoardWorld};

/// What a board holds, in numbers a comparison can print.
#[derive(Debug, PartialEq, Eq)]
struct Census {
    components: usize,
    nets: usize,
    traces: usize,
    segments: usize,
    vias: usize,
    /// Traces that narrow on the way into a pad.
    necks: usize,
    zones: usize,
    layers: u8,
    fab: Option<String>,
    stackup_layers: usize,
}

fn census(world: &mut BoardWorld) -> Census {
    let components = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<&RefDes>();
        query.iter(ecs).count()
    };
    let (traces, segments) = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<&Trace>();
        let all: Vec<usize> = query.iter(ecs).map(|trace| trace.segments.len()).collect();
        (all.len(), all.iter().sum())
    };
    let vias = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<&Via>();
        query.iter(ecs).count()
    };
    let zones = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<&Zone>();
        query.iter(ecs).count()
    };
    Census {
        components,
        nets: world.nets().count(),
        traces,
        segments,
        vias,
        necks: {
            let ecs = world.ecs_mut();
            let mut query = ecs.query::<&cypcb_world::components::trace::TraceNeck>();
            query.iter(ecs).count()
        },
        zones,
        layers: world
            .board_info()
            .map(|(_, stack)| stack.count)
            .unwrap_or(0),
        fab: world.fab().map(|fab| fab.to_string()),
        stackup_layers: world.stackup().map(|s| s.layers.len()).unwrap_or(0),
    }
}

fn load(source: &str) -> Option<BoardWorld> {
    let parsed = cypcb_parser::parse(source);
    if !parsed.errors.is_empty() {
        return None;
    }
    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    let result = sync_ast_to_world(&parsed.value, source, &mut world, &mut library);
    if !result.errors.is_empty() {
        return None;
    }
    Some(world)
}

fn examples() -> Vec<(String, String)> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("examples");
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("the examples directory") {
        let path = entry.expect("a directory entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("cypcb") {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .expect("a file name")
            .to_string();
        let text = std::fs::read_to_string(&path).expect("the example reads");
        out.push((name, text));
    }
    out.sort();
    assert!(out.len() >= 20, "the corpus is {} files", out.len());
    out
}

/// The examples this cannot ask the question of, and why.
///
/// Two are invalid on purpose - they are what the error messages are tested
/// against - and the third needs a host to resolve its `import` lines, which
/// `cypcb_parser::parse` on its own does not do. None of the three is a
/// writer fault, and each is named so that a file which stops loading for a
/// real reason cannot hide in the same silence.
const CANNOT_BE_ASKED: &[&str] = &[
    // Deliberately unparseable: the fixture behind the parse error messages.
    "invalid.cypcb",
    // Deliberately unparseable: one unknown keyword, on purpose.
    "unknown_keyword.cypcb",
    // `import` needs the host to supply the imported files. The CLI does that
    // and this loader does not, so the file is fine and the question is not
    // askable here.
    "v2-imports.cypcb",
];

#[test]
fn every_example_comes_back_the_board_it_was() {
    let mut asked = 0;
    let mut failures = String::new();
    for (name, source) in examples() {
        if CANNOT_BE_ASKED.contains(&name.as_str()) {
            continue;
        }
        let Some(mut before) = load(&source) else {
            failures.push_str(&format!("{name}: does not load, and is not on the list\n"));
            continue;
        };
        let written = board_as_dsl(&mut before);
        let Some(mut after) = load(&written) else {
            failures.push_str(&format!(
                "{name}: the file it was written into does not load\n"
            ));
            continue;
        };
        asked += 1;
        let (a, b) = (census(&mut before), census(&mut after));
        if a != b {
            failures.push_str(&format!("{name}:\n  before {a:?}\n  after  {b:?}\n"));
        }
    }
    assert!(failures.is_empty(), "{failures}");
    assert_eq!(
        asked,
        examples().len() - CANNOT_BE_ASKED.len(),
        "every example that is not on the list was asked"
    );
}

#[test]
fn the_list_of_examples_this_cannot_ask_is_not_stale() {
    // A name on that list which no longer exists, or which loads after all, is
    // an exemption nobody is using - and the next person reads it as a known
    // limitation rather than as a leftover.
    let names: Vec<String> = examples().into_iter().map(|(name, _)| name).collect();
    for excused in CANNOT_BE_ASKED {
        assert!(
            names.iter().any(|name| name == excused),
            "{excused} is on the list and not in the directory"
        );
        let source = examples()
            .into_iter()
            .find(|(name, _)| name == excused)
            .map(|(_, source)| source)
            .expect("the file that was just found");
        assert!(
            load(&source).is_none(),
            "{excused} loads now, so it does not need excusing"
        );
    }
}

#[test]
fn what_the_corpus_actually_exercises() {
    // A guard that guards nothing reads exactly like a guard that works. Every
    // column of the census is compared on every example, and a column no
    // example populates cannot fail - so this counts each column across the
    // corpus and states which ones are live.
    //
    // One is not, and it is named rather than left to be discovered:
    //
    // - **vias**: zero. Every mention of a via in `examples/` is prose in a
    //   comment. Dropping vias from the writer would leave this file green.
    //
    // Three woke up on 2026-08-24. `rigid-flex.cypcb` names a fab and carries a
    // six-entry stack; `slotted-connector.cypcb` gained a trace that necks on
    // the way into a 1.6mm pad, and the neck had been in the language, both
    // readers, four DRC rules and the router for four days with no example
    // using one - so the differential test that holds the two readers to the
    // same answer had never asked either of them about a neck.
    // That is what the assertions below are for - they failed the moment the
    // example landed, which is the whole point of asserting a zero.
    //
    // One more thing the corpus does not reach, measured the same way: no
    // example has a net that carries copper and connects to no pin, so
    // deleting the writer's declaration loop for those leaves this file green
    // too. `which_trace_does_not_survive_being_written_down` builds that shape
    // itself and is where it is covered.
    //
    // The corpus is 124 components and 86 nets against **9 trace segments**,
    // which is the same shape as the KiCad fixtures: boards written to show a
    // language feature, not to carry copper.
    let mut totals = Census {
        components: 0,
        nets: 0,
        traces: 0,
        segments: 0,
        vias: 0,
        necks: 0,
        zones: 0,
        layers: 0,
        fab: None,
        stackup_layers: 0,
    };
    for (name, source) in examples() {
        if CANNOT_BE_ASKED.contains(&name.as_str()) {
            continue;
        }
        let Some(mut world) = load(&source) else {
            continue;
        };
        let one = census(&mut world);
        totals.components += one.components;
        totals.nets += one.nets;
        totals.traces += one.traces;
        totals.segments += one.segments;
        totals.vias += one.vias;
        totals.necks += one.necks;
        totals.zones += one.zones;
        totals.layers += u8::from(one.layers > 2);
        totals.fab = totals.fab.or(one.fab);
        totals.stackup_layers += one.stackup_layers;
    }

    // Live: a change to the writer that loses any of these fails the guard.
    assert!(totals.components > 0, "{totals:?}");
    assert!(totals.nets > 0, "{totals:?}");
    assert!(totals.traces > 0, "{totals:?}");
    assert!(totals.segments > 0, "{totals:?}");
    assert!(
        totals.necks > 0,
        "no example necks, so the writer could drop the neck and leave this green: {totals:?}"
    );
    assert!(totals.zones > 0, "{totals:?}");
    assert!(totals.fab.is_some(), "no example names a fab: {totals:?}");
    assert!(
        totals.stackup_layers > 0,
        "no example carries a stackup: {totals:?}"
    );
    assert!(
        totals.layers > 0,
        "no example has more than two layers: {totals:?}"
    );

    // Not live. Each of these is an assertion that the corpus does **not**
    // cover something, so that the day it does, this test fails and says which
    // column woke up. Raise them rather than deleting them.
    assert_eq!(
        totals.vias, 0,
        "an example places a via now, so the via column guards something: {totals:?}"
    );
}
