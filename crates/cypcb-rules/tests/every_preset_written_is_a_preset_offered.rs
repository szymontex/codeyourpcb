//! A preset nobody listed is a table nobody checks.
//!
//! `cargo test -p cypcb-rules --test every_preset_written_is_a_preset_offered`
//!
//! `RulesPreset::ALL` is a hand-written array and three things read it: the
//! command line's "available presets" message when a name is refused, the
//! browser engine's list, and every audit in this crate that walks the tables:
//! the surface figures, the IPC provenance, the copper weight. A variant
//! left out of it is a fab table no test ever reads and a name no user is ever
//! offered, while `--preset` still accepts it.
//!
//! The compiler already holds one half: `name()` is an exhaustive match, so a
//! new variant cannot be added without an arm there. This holds the other
//! half by reading those arms and asking that each one is in the array. Same
//! shape as `cypcb-drc`'s `every_rule_written_is_a_rule_run`, and the same
//! reason: Rust cannot be asked what variants an enum has.

use std::fs;
use std::path::Path;

use cypcb_rules::presets::RulesPreset;

fn presets_source() -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/presets/mod.rs"))
        .expect("the presets module is readable")
}

/// The identifier after each `marker`, within `section`.
fn names_after(section: &str, marker: &str) -> Vec<String> {
    section
        .split(marker)
        .skip(1)
        .map(|piece| {
            piece
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect::<String>()
        })
        .filter(|name| !name.is_empty())
        .collect()
}

/// Everything between `open` and the first `close` after it.
fn section<'a>(source: &'a str, open: &str, close: &str) -> &'a str {
    let start = source
        .find(open)
        .unwrap_or_else(|| panic!("the source no longer contains `{open}`"))
        + open.len();
    let rest = &source[start..];
    let end = rest
        .find(close)
        .unwrap_or_else(|| panic!("no `{close}` after `{open}`"));
    &rest[..end]
}

/// Every variant, taken from the exhaustive match the compiler enforces.
fn variants_written(source: &str) -> Vec<String> {
    let body = section(source, "pub fn name(self) -> &'static str {", "\n    }");
    names_after(body, "Self::")
}

/// Every variant the hand-written array offers.
fn variants_offered(source: &str) -> Vec<String> {
    let body = section(source, "pub const ALL: [RulesPreset; ", "];");
    names_after(body, "RulesPreset::")
}

#[test]
fn every_preset_written_is_a_preset_offered() {
    let source = presets_source();
    let written = variants_written(&source);
    let offered = variants_offered(&source);

    // A reader that found nothing would make the comparison below pass while
    // proving nothing. Eleven presets ship today; a floor, not a census, so the
    // twelfth does not fail here on the day it is written.
    assert!(
        written.len() >= 11,
        "only {} variants were read out of `name()`, so the reader is broken rather than the crate: {written:?}",
        written.len()
    );

    let missing: Vec<&String> = written
        .iter()
        .filter(|name| !offered.contains(name))
        .collect();
    assert!(
        missing.is_empty(),
        "written and never offered, so no audit walks their tables and no user is told they exist: {missing:?}"
    );
}

#[test]
fn the_array_lists_each_preset_once() {
    // The other way a hand-written list goes wrong. A preset listed twice is
    // checked twice by every audit that walks `all()`, which is harmless until
    // one of them counts.
    let source = presets_source();
    let mut offered = variants_offered(&source);
    let listed = offered.len();
    offered.sort();
    offered.dedup();

    assert_eq!(
        listed,
        offered.len(),
        "the array has {listed} entries and {} distinct presets in it",
        offered.len()
    );
    assert_eq!(
        listed,
        RulesPreset::all().len(),
        "the array the source declares and the slice `all()` returns are different lengths"
    );
}
