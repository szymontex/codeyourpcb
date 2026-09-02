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

use cypcb_autoroute::AutorouteConfig;
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
    // K011 used to say the router marks every pad as a circle of max(w, h) / 2,
    // and this case used to confirm it by grepping grid.rs for that expression.
    // Both were wrong from 2026-08-28: the expression is the `None` arm of a
    // choice whose shipped default is the rectangle, so the grep answered yes
    // about a fallback while the entry described behaviour nobody gets. Ask the
    // default instead - it is what `cypcb route` uses, and it is the only thing
    // that can make the entry false again.
    let knowledge = read(".gsd/KNOWLEDGE.md");

    let ships_the_rectangle = AutorouteConfig::default().pad_rect_extra_cells.is_some();
    let entry_says_fixed =
        knowledge.contains("### K011") && !knowledge.contains("Still violated in one place");

    assert_eq!(
        entry_says_fixed, ships_the_rectangle,
        "K011 says the pad shape is fixed ({entry_says_fixed}) and the shipped \
         default marks a rectangle ({ships_the_rectangle}) - one of the two has moved"
    );

    assert!(
        knowledge.contains("the_pad_shape_is_the_one_asked_for"),
        "K011 no longer names the case that holds its claim"
    );
    assert!(
        repo_root()
            .join("crates/cypcb-autoroute/tests/the_pad_shape_is_the_one_asked_for.rs")
            .exists(),
        "K011 names a case that no longer exists"
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
