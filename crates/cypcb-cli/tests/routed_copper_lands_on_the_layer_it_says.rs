//! Which layer routed copper lands on, on a board with hundreds of segments.
//!
//! `cargo test -p cypcb-cli --test routed_copper_lands_on_the_layer_it_says`
//!
//! The open F.Cu finding is the owner's board: the viewer draws copper on the
//! top layer that `kicad-cli pcb export svg --layers F.Cu` has no trace of.
//! The per-layer comparison written for it in `cypcb-kicad` could only run on
//! the fixtures, and the fixtures turned out to hold one copper segment
//! between them - they are placed and unrouted, which is what makes them
//! benchmarks. A green tick on a sample of one is not evidence.
//!
//! This is the sample the fixtures do not provide, built the way the tracker's
//! second option describes: route a benchmark board in-house, write it back as
//! KiCad, and count what came out per layer. Hundreds of segments instead of
//! one, and it lives here rather than in `cypcb-kicad` because routing is what
//! makes the copper and only this crate has the router.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

fn cypcb() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cypcb"))
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

/// A copy of a fixture in a scratch directory of its own.
fn scratch_copy(fixture: &str, who: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cypcb-layer-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");
    let source = repo_root().join("tests/fixtures/benchmark").join(fixture);
    let target = dir.join(fixture);
    std::fs::copy(&source, &target).expect("the fixture is copyable");
    target
}

/// Count `(segment ...)` nodes per layer, out of the file's own text.
///
/// Not through this project's reader, deliberately: comparing the writer
/// against the reader by using the reader to say what the writer produced
/// compares the pair with itself.
fn segments_per_layer(source: &str) -> BTreeMap<String, usize> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut rest = source;
    while let Some(at) = rest.find("(segment") {
        rest = &rest[at + "(segment".len()..];
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
        if let Some(layer_at) = node.find("(layer") {
            let name: String = node[layer_at + "(layer".len()..]
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

/// `trace_segment_count` out of `parse-kicad`'s JSON, without a JSON parser.
fn segment_count_from_parse(path: &Path) -> usize {
    let out = cypcb()
        .arg("parse-kicad")
        .arg(path)
        .output()
        .expect("the binary runs");
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let key = "\"trace_segment_count\":";
    let at = text
        .find(key)
        .unwrap_or_else(|| panic!("no trace_segment_count in:\n{text}"));
    text[at + key.len()..]
        .trim_start()
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse()
        .expect("a number")
}

#[test]
fn a_routed_board_writes_its_copper_onto_both_layers_and_reads_back_the_same() {
    let board = scratch_copy("plane_board.kicad_pcb", "plane");
    let routed = board.with_extension("routed.kicad_pcb");

    let out = cypcb()
        .arg("route")
        .arg("--in-house")
        .arg(&board)
        .arg("-o")
        .arg(&routed)
        .output()
        .expect("the binary runs");
    assert!(
        out.status.success(),
        "routing failed:\n{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let text = std::fs::read_to_string(&routed).expect("the routed board was written");
    let per_layer = segments_per_layer(&text);
    let total: usize = per_layer.values().sum();

    // The sample the fixtures could not provide. If this ever collapses to a
    // handful the comparison below stops meaning anything, so it is asserted
    // rather than assumed.
    assert!(
        total > 100,
        "only {total} segments were routed; this test needs a real board:\n{per_layer:?}"
    );

    // A two-layer board routed by a router that places vias has to use both
    // sides. One-sided output would be a router fault this would otherwise
    // hide behind a passing total.
    assert!(
        per_layer.get("F.Cu").copied().unwrap_or(0) > 0,
        "no top copper was written: {per_layer:?}"
    );
    assert!(
        per_layer.get("B.Cu").copied().unwrap_or(0) > 0,
        "no bottom copper was written: {per_layer:?}"
    );

    // Nothing landed on a layer the board does not have. This is the assertion
    // the F.Cu finding is about: copper appearing where the file does not put
    // it would show here as a third key.
    let unexpected: Vec<&String> = per_layer
        .keys()
        .filter(|name| name.as_str() != "F.Cu" && name.as_str() != "B.Cu")
        .collect();
    assert!(
        unexpected.is_empty(),
        "copper was written onto a layer this two-layer board does not have: \
         {unexpected:?} in {per_layer:?}"
    );

    // And the reader agrees with the text about how much there is.
    assert_eq!(
        segment_count_from_parse(&routed),
        total,
        "the importer counts a different number of segments than the file \
         holds: {per_layer:?}"
    );

    println!("plane_board routed: {per_layer:?}, {total} segments total");
}
