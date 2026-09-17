//! A flag for a program that never ran is refused, or printed where it belongs.
//!
//! `cypcb route` runs the built-in router unless a jar is named, so a timeout
//! or a pass count for FreeRouting is a flag the tool accepted and did nothing
//! with - which reads as an instruction it followed. Three of them are refused
//! by name; `--timeout` cannot be, because it carries a default and nothing
//! tells a person who typed `--timeout 300` from one who typed nothing, so it
//! is printed under a heading that says what it belongs to.
//!
//! This holds the pair: a flag under that heading is either refused or on the
//! list below with its reason. A fifth flag added to the heading tomorrow is a
//! failure until somebody decides which it is.
//!
//! `cargo test -p cypcb-cli --test a_flag_for_a_program_that_never_ran_is_refused`

use std::path::{Path, PathBuf};

/// Flags printed under the FreeRouting heading, today 4.
const UNDER_THE_HEADING_FLOOR: usize = 4;

/// A flag under that heading which is not refused, and why it cannot be.
const NOT_REFUSED: &[(&str, &str)] = &[(
    "--timeout",
    "carries a default, so a person who typed it cannot be told from one who did not",
)];

fn route_source() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/commands/route.rs")
}

/// The flag a field spells: `max_passes` is `--max-passes`.
fn flag_of(field: &str) -> String {
    format!("--{}", field.replace('_', "-"))
}

#[test]
fn a_flag_for_a_program_that_never_ran_is_refused() {
    let source = std::fs::read_to_string(route_source()).expect("the route command is there");
    let lines: Vec<&str> = source.lines().collect();

    let mut under_the_heading = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if !line.contains("help_heading = FREEROUTING_OPTIONS") {
            continue;
        }
        let field = lines[index + 1..]
            .iter()
            .find_map(|next| next.trim().strip_prefix("pub "))
            .and_then(|rest| rest.split(':').next())
            .unwrap_or_else(|| panic!("no field follows the heading at line {}", index + 1));
        under_the_heading.push(flag_of(field));
    }

    let refusal = source
        .split_once("fn refuse_freerouting_only_flags")
        .expect("the refusal is there")
        .1;
    let refusal = refusal
        .split_once("\n    }\n")
        .expect("the refusal ends somewhere")
        .0;

    let mut unrefused = Vec::new();
    for flag in &under_the_heading {
        if refusal.contains(flag.as_str()) {
            continue;
        }
        match NOT_REFUSED.iter().find(|(named, _)| named == flag) {
            Some(_) => {}
            None => unrefused.push(flag.clone()),
        }
    }

    println!(
        "flags under the FreeRouting heading: {} (floor {UNDER_THE_HEADING_FLOOR}); \
         neither refused nor excused: {}",
        under_the_heading.len(),
        unrefused.len()
    );

    assert!(
        under_the_heading.len() >= UNDER_THE_HEADING_FLOOR,
        "the walk found {} flags under that heading and there were {UNDER_THE_HEADING_FLOOR} \
         when this was written - a scan that stops finding them passes for the wrong reason",
        under_the_heading.len()
    );
    for (named, _) in NOT_REFUSED {
        assert!(
            under_the_heading.iter().any(|flag| flag == named),
            "{named} is excused from a heading it no longer sits under - drop the excuse"
        );
    }
    assert!(
        unrefused.is_empty(),
        "a FreeRouting-only flag is accepted and ignored when the built-in router runs: {}\
         \n  Refuse it in `refuse_freerouting_only_flags`, or put it on this file's list \
         with the reason it cannot be refused.",
        unrefused.join(", ")
    );
}
