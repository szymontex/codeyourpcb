//! The knowledge base says what the code does.
//!
//! `cargo test -p cypcb-cli --test the_knowledge_base_says_what_the_code_does`
//!
//! `.gsd/KNOWLEDGE.md` is an append-only register of lessons, and two of its
//! entries are claims about code that moves. One of them had been false for
//! five weeks: K010 said the same-net exemption was still per component, and
//! `component_pads` had carried each pad's own net since long before it was
//! read again. A register nobody checks is a register that teaches what used
//! to be true.
//!
//! So the two claims that name code are held here. Neither case reads prose:
//! each asks the file the entry points at.

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the workspace root is two directories above this crate")
}

fn read(path: &str) -> String {
    std::fs::read_to_string(repo_root().join(path))
        .unwrap_or_else(|error| panic!("{path}: {error}"))
}

#[test]
fn k011_is_true_while_it_says_it_is() {
    // K011 says the router still marks a pad as a circle of max(w, h) / 2
    // while the checker uses rotated bounds. The day somebody fixes that, this
    // fails and the entry gets its update in the same commit.
    let knowledge = read(".gsd/KNOWLEDGE.md");
    let grid = read("crates/cypcb-autoroute/src/grid.rs");

    let claims_violated =
        knowledge.contains("### K011") && knowledge.contains("Still violated in one place");
    let still_a_circle = grid.contains("pad.size.0.raw().max(pad.size.1.raw()) / 2");

    assert_eq!(
        claims_violated, still_a_circle,
        "K011 says the router marks pads as circles ({claims_violated}) and \
         grid.rs does ({still_a_circle}) - one of the two has moved"
    );
}

#[test]
fn k010_names_the_cases_that_hold_it() {
    // K010's status says the exemption is per pad and names the two cases that
    // prove it. A renamed or deleted case leaves the entry pointing at
    // nothing, which is how the paragraph it replaced went stale.
    let knowledge = read(".gsd/KNOWLEDGE.md");
    let tests = read("crates/cypcb-drc/tests/clearance_measures_copper.rs");

    for case in [
        "a_net_a_part_carries_does_not_exempt_that_part_s_other_pads",
        "a_trace_meeting_the_pad_it_belongs_to_is_still_exempt",
    ] {
        assert!(
            knowledge.contains(case),
            "K010 no longer names {case}, so nothing says what holds its claim"
        );
        assert!(
            tests.contains(&format!("fn {case}(")),
            "K010 names {case} and clearance_measures_copper.rs does not have it"
        );
    }
}
