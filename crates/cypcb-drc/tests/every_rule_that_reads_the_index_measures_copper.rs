//! A rule that reads the spatial index measures copper, not part bodies.
//!
//! `cargo test -p cypcb-drc --test every_rule_that_reads_the_index_measures_copper`
//!
//! A component sits in `world.spatial()` as its **courtyard** - the assembly
//! keepout that covers the part body. Copper is what a fabrication rule is
//! about, and three rules in a row were found measuring the box instead:
//! `clearance` (fixed long ago, and the reason `component_pads` exists),
//! `edge-clearance` and `mounting-hole-clearance` (both 2026-08-31). Each time
//! the symptom was the same - a board refused for copper that is not there,
//! because a part's plastic reached somewhere its pads do not.
//!
//! This is the gate rather than a fourth patch. A rule that walks the index
//! has to collect pads through `component_pads`; a rule that does not walk it
//! is not this file's business.
//!
//! What it cannot check is that the collected pads are then *used*. That is
//! what the fixtures beside each rule are for - a body hanging over the edge,
//! a body over a mounting hole - and this census is what makes a fifth rule
//! arrive with one.

use std::path::{Path, PathBuf};

fn rules_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/rules")
}

/// Rules that read the index for something that is copper already - a trace, a
/// via, a pour - and never for a component. Empty today; an entry here needs a
/// sentence saying why the rule never meets a courtyard.
const NOT_ABOUT_COMPONENTS: [&str; 0] = [];

#[test]
fn every_rule_that_walks_the_index_collects_pads() {
    let mut readers = Vec::new();
    let mut offenders = Vec::new();

    for entry in std::fs::read_dir(rules_dir()).expect("the rules live in one directory") {
        let path = entry.expect("a directory entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .expect("a file name")
            .to_string();
        if name == "mod" {
            continue; // the registry, not a rule
        }
        let source = std::fs::read_to_string(&path).expect("a rule is readable");

        // `world.spatial()` is how a rule asks what else is on the board.
        if !source.contains(".spatial()") {
            continue;
        }
        readers.push(name.clone());

        if NOT_ABOUT_COMPONENTS.contains(&name.as_str()) {
            continue;
        }
        if !source.contains("component_pads") {
            offenders.push(name);
        }
    }

    assert!(
        offenders.is_empty(),
        "these rules walk the spatial index without collecting pads, so they \
         measure a part's courtyard as if it were copper: {offenders:?}"
    );

    // The census has to be looking at something. Four rules read the index on
    // 2026-08-31: clearance, edge-clearance, mounting-hole-clearance,
    // slot-clearance.
    assert!(
        readers.len() >= 4,
        "only {} rule(s) look like index readers, which means this census is \
         no longer finding them: {readers:?}",
        readers.len()
    );
}
