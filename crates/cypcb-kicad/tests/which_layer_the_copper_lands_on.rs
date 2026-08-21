//! Does the importer put copper on the layer the file names?
//!
//! `cargo test -p cypcb-kicad --test which_layer_the_copper_lands_on`
//!
//! The tracker carries an open finding from the owner's own board: the viewer
//! draws copper on F.Cu that KiCad's own plot of the same file does not - a
//! fan of long diagonals radiating from the left edge, present on the canvas
//! and absent from `kicad-cli pcb export svg --layers F.Cu`. Either the
//! importer assigns geometry to the top layer that does not belong there, or
//! the renderer draws something on that layer it should not.
//!
//! The recorded next action was to count `(segment ...)` per layer in the file
//! and compare it against what the importer produces, because the difference
//! names which of the two it is. That is what this does, on every fixture the
//! project ships rather than on a board nobody else can open.
//!
//! It is a diagnostic that happens to be an assertion: if the counts agree on
//! all six boards, the importer is not the half at fault and the search moves
//! to the renderer. If they disagree, the fixture that disagrees is the one to
//! open.

use std::collections::BTreeMap;
use std::path::Path;

use cypcb_kicad::{parse_kicad_pcb, write_board, BENCHMARKS};
use cypcb_world::components::Layer;

fn fixture_path(filename: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/fixtures/benchmark")
        .join(filename)
}

/// Count `(segment ...)` nodes per layer, straight out of the text.
///
/// Deliberately not through this project's own reader: the whole point is to
/// compare what the importer produces against what the file says, and using
/// the importer to establish what the file says would compare it with itself.
/// That closed loop is what let a whole class of KiCad faults ship unnoticed
/// until real KiCad was installed.
fn segments_per_layer_in_file(source: &str) -> BTreeMap<String, usize> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut rest = source;

    while let Some(at) = rest.find("(segment") {
        rest = &rest[at + "(segment".len()..];

        // The node ends at its matching paren. A segment holds no nested
        // strings with parens in them, so depth counting over the raw text is
        // enough here and needs no tokeniser.
        let mut depth = 1i32;
        let mut end = rest.len();
        for (i, ch) in rest.char_indices() {
            match ch {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = i;
                        break;
                    }
                }
                _ => {}
            }
        }
        let node = &rest[..end];

        // `(layer "F.Cu")`, and older files write it unquoted.
        if let Some(layer_at) = node.find("(layer") {
            let tail = &node[layer_at + "(layer".len()..];
            let name: String = tail
                .trim_start()
                .trim_start_matches('"')
                .chars()
                .take_while(|c| !c.is_whitespace() && *c != '"' && *c != ')')
                .collect();
            if !name.is_empty() {
                *counts.entry(name).or_insert(0) += 1;
            }
        }
        rest = &rest[end.min(rest.len())..];
    }
    counts
}

/// What KiCad calls the layer this project means.
///
/// KiCad numbers inner layers from 1 and this project from 0, which is a
/// difference worth writing out rather than leaving in an off-by-one nobody
/// reads: `Layer::Inner(0)` is `In1.Cu`.
fn kicad_name(layer: Layer) -> String {
    match layer {
        Layer::TopCopper => "F.Cu".to_string(),
        Layer::BottomCopper => "B.Cu".to_string(),
        Layer::Inner(n) => format!("In{}.Cu", n + 1),
        other => format!("{other:?}"),
    }
}

#[test]
fn every_segment_the_file_names_lands_on_the_layer_it_names() {
    let mut disagreements = Vec::new();

    for benchmark in BENCHMARKS {
        let path = fixture_path(benchmark.filename);
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", benchmark.filename));
        let in_file = segments_per_layer_in_file(&source);

        let parsed = parse_kicad_pcb(&path)
            .unwrap_or_else(|e| panic!("cannot parse {}: {e:?}", benchmark.filename));

        let mut imported: BTreeMap<String, usize> = BTreeMap::new();
        if let Some(routes) = &parsed.reference_routes {
            for segment in &routes.routes {
                *imported.entry(kicad_name(segment.layer)).or_insert(0) += 1;
            }
        }

        println!(
            "{:<26} file {:?}  imported {:?}",
            benchmark.filename, in_file, imported
        );

        // Every layer either side mentions, so a layer that gained copper is
        // as visible as one that lost it.
        let mut layers: Vec<&String> = in_file.keys().chain(imported.keys()).collect();
        layers.sort();
        layers.dedup();
        for layer in layers {
            let said = in_file.get(layer).copied().unwrap_or(0);
            let got = imported.get(layer).copied().unwrap_or(0);
            if said != got {
                disagreements.push(format!(
                    "{} {}: the file names {said} segments, the importer produced {got}",
                    benchmark.filename, layer
                ));
            }
        }
    }

    assert!(
        disagreements.is_empty(),
        "the importer moved copper between layers:\n  {}",
        disagreements.join("\n  ")
    );
}

#[test]
fn the_fixtures_carry_almost_no_copper_and_that_is_worth_saying() {
    // The measurement that makes the test above weaker than it looks. Every
    // KiCad file this project ships holds five `(segment ...)` nodes between
    // them, five of the nine having none at all: the benchmark boards are
    // placed and unrouted, which is what makes them benchmarks.
    //
    // So the comparison above runs on a sample of one segment, and a passing
    // result there is not evidence that the importer assigns layers correctly.
    // This asserts the sample size rather than letting a future reader mistake
    // a green tick for coverage. If somebody routes a fixture, this fails and
    // the number in it gets raised on purpose.
    let mut total = 0usize;
    for benchmark in BENCHMARKS {
        let source = std::fs::read_to_string(fixture_path(benchmark.filename)).unwrap();
        total += segments_per_layer_in_file(&source).values().sum::<usize>();
    }
    assert_eq!(
        total, 1,
        "the six benchmark boards hold {total} copper segments between them;          if that changed on purpose, raise this number and say why"
    );
}

#[test]
fn a_board_written_here_comes_back_on_the_layers_it_left_on() {
    // The sample the fixtures do not provide, built rather than found. Two
    // traces, one per copper layer, through the writer and back through the
    // reader - and checked against the **text** in between, not only against
    // each other, so a writer and a reader that agree on the wrong layer are
    // still caught.
    use cypcb_core::{Nm, Point};
    use cypcb_world::components::trace::{Trace, TraceSegment, TraceSource};
    use cypcb_world::BoardWorld;

    let mut world = BoardWorld::new();
    world.set_board(
        "layers".to_string(),
        (Nm::from_mm(20.0), Nm::from_mm(20.0)),
        2,
    );
    let net = world.intern_net("SIG");

    for (layer, y) in [(Layer::TopCopper, 4.0), (Layer::BottomCopper, 8.0)] {
        world.ecs_mut().spawn((
            Trace {
                layer,
                width: Nm::from_mm(0.25),
                segments: vec![TraceSegment {
                    start: Point::from_mm(4.0, y),
                    end: Point::from_mm(14.0, y),
                    width: None,
                }],
                net_id: net,
                locked: false,
                source: TraceSource::Manual,
            },
            net,
        ));
    }

    let text = write_board(&mut world, "cypcb-test");
    let in_text = segments_per_layer_in_file(&text);
    assert_eq!(
        in_text.get("F.Cu").copied().unwrap_or(0),
        1,
        "one top trace was written; the file says {in_text:?}"
    );
    assert_eq!(
        in_text.get("B.Cu").copied().unwrap_or(0),
        1,
        "one bottom trace was written; the file says {in_text:?}"
    );

    let path = std::env::temp_dir().join("cypcb-which-layer.kicad_pcb");
    std::fs::write(&path, &text).expect("write the board out");
    let parsed = parse_kicad_pcb(&path).expect("read it back");
    let _ = std::fs::remove_file(&path);

    let mut imported: BTreeMap<String, usize> = BTreeMap::new();
    for segment in &parsed
        .reference_routes
        .expect("the board carries copper")
        .routes
    {
        *imported.entry(kicad_name(segment.layer)).or_insert(0) += 1;
    }
    assert_eq!(
        imported, in_text,
        "the importer put the copper somewhere other than where the file says"
    );
}
