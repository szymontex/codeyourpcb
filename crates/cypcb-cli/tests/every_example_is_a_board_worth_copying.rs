//! An example that ships a defect teaches the defect.
//!
//! `cargo test -p cypcb-cli --test every_example_is_a_board_worth_copying`
//!
//! Nothing had ever checked the examples as a set. Checking all twenty found
//! four shipping real faults, and each was the kind somebody would copy:
//!
//! - `syntax.cypcb`, the syntax reference, routed `VCC` out of R1's **left**
//!   pad towards a part on its right, so the trace ran straight across R1's
//!   own right pad at 0.00mm.
//! - `custom-footprint.cypcb` taught how to declare a footprint, and the
//!   footprint it taught put a 0.8mm drill in a 1mm pad - 0.1mm of copper
//!   where a fabricator will make 0.15mm.
//! - `alignment-test.cypcb` placed a 1206 at 2mm, so its left pad reached the
//!   board edge.
//! - `v2-modules.cypcb` had a module whose own capacitors sat 0.20mm from the
//!   regulator's courtyard against 0.25mm required.
//!
//! Two examples are deliberately bad and say so in their own names. They are
//! listed here rather than skipped by a rule, so a third one cannot join them
//! quietly.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use cypcb_drc::{run_drc, Preset};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{sync_ast_to_world, BoardWorld};

fn examples_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .join("examples")
}

/// Examples that exist to be wrong, with what they carry.
///
/// `drc-test` is a board of deliberate faults for the checker to find, and
/// `pour-island` is a pour with an island in it for the same reason. An entry
/// here is a claim that the example's whole point is the violation.
const DELIBERATELY_BAD: &[&str] = &["drc-test.cypcb", "pour-island.cypcb"];

/// Violations every unrouted board has, which say nothing about its quality.
///
/// A pin on a net no copper reaches is what an unrouted board *is*, and a pin
/// on no net at all is ordinary in a fragment that demonstrates one keyword.
fn is_about_being_unrouted(kind: &str) -> bool {
    kind == "unrouted-pin" || kind == "unconnected-pin" || kind == "assertion"
}

#[test]
fn no_example_ships_a_fault_somebody_would_copy() {
    let mut files: Vec<PathBuf> = std::fs::read_dir(examples_dir())
        .expect("the examples directory is there")
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "cypcb"))
        .collect();
    files.sort();
    assert!(files.len() > 10, "the examples directory went missing");

    let mut wrong: Vec<String> = Vec::new();
    let mut checked = 0;

    for file in &files {
        let name = file
            .file_name()
            .expect("a file has a name")
            .to_string_lossy()
            .to_string();

        let source = std::fs::read_to_string(file).expect("a readable example");
        let parsed = cypcb_parser::parse(&source);
        if !parsed.errors.is_empty() {
            // `invalid.cypcb` and `unknown_keyword.cypcb` exist to fail
            // parsing. That is a different test's business.
            continue;
        }

        let mut world = BoardWorld::new();
        let mut library = FootprintLibrary::new();
        let result = sync_ast_to_world(&parsed.value, &source, &mut world, &mut library);
        if !result.errors.is_empty() || world.board_entity().is_none() {
            continue;
        }
        world.rebuild_spatial_index_from_library(&library);
        checked += 1;

        let preset = Preset::from_name("jlcpcb").expect("a known preset");
        let drc = run_drc(&mut world, &preset.rules());

        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        for violation in &drc.violations {
            let kind = format!("{:?}", violation.kind);
            let kind = kind_slug(&kind);
            if is_about_being_unrouted(&kind) {
                continue;
            }
            *counts.entry(kind).or_default() += 1;
        }

        let deliberate = DELIBERATELY_BAD.contains(&name.as_str());
        if deliberate && counts.is_empty() {
            wrong.push(format!(
                "{name} is listed as deliberately bad and is clean. Take it off \
                 the list."
            ));
        }
        if !deliberate && !counts.is_empty() {
            wrong.push(format!("{name}: {counts:?}"));
        }
    }

    assert!(
        checked > 10,
        "only {checked} examples reached the checker, so this proves little"
    );
    assert!(
        wrong.is_empty(),
        "an example that ships a defect teaches the defect:\n{}",
        wrong.join("\n")
    );
}

/// `EdgeClearance` -> `edge-clearance`, to match what the CLI prints.
fn kind_slug(kind: &str) -> String {
    let mut out = String::new();
    for (i, ch) in kind.chars().enumerate() {
        if ch.is_uppercase() {
            if i > 0 {
                out.push('-');
            }
            out.extend(ch.to_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}
