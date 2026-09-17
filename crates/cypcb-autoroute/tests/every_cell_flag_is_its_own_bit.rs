//! Every cell flag is its own bit.
//!
//! `CELL_HALO` and `CELL_ZONE` were both `1 << 2` from the day the zone flag
//! was written until 2026-09-17. Both compiled, both shipped, and the search
//! read every keepout cell as the copper a trace merely brushes - yieldable to
//! a net with nowhere else to go. A duplicated shift in a list of hand-written
//! shifts is invisible to a reader and to the compiler alike.
//!
//! This reads the flags out of the source rather than importing them, so a
//! flag added tomorrow is held to the same thing: a name the walk cannot see
//! is a name nothing checks.
//!
//! `cargo test -p cypcb-autoroute --test every_cell_flag_is_its_own_bit`

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Flags declared in that block, today 7 - six bits and the zero.
const FLAGS_EXAMINED_FLOOR: usize = 6;

fn grid_source() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/grid.rs")
}

/// The value of `0` or `1 << n`, which is every form this block uses.
fn shift_value(expression: &str) -> Option<u8> {
    let expression = expression.trim();
    if let Some(bit) = expression.strip_prefix("1 << ") {
        return bit
            .trim()
            .parse::<u32>()
            .ok()
            .and_then(|n| 1u8.checked_shl(n));
    }
    expression.parse::<u8>().ok()
}

#[test]
fn every_cell_flag_is_its_own_bit() {
    let source = std::fs::read_to_string(grid_source()).expect("the grid is there");

    let mut by_value: BTreeMap<u8, Vec<String>> = BTreeMap::new();
    let mut examined = 0usize;

    for line in source.lines() {
        let Some(rest) = line.trim().strip_prefix("pub const CELL_") else {
            continue;
        };
        let Some((name, value)) = rest.split_once(": u8 = ") else {
            continue;
        };
        let Some(value) = value.strip_suffix(';').and_then(shift_value) else {
            panic!("CELL_{name} is declared in a form this check cannot read: {line}");
        };
        examined += 1;
        by_value
            .entry(value)
            .or_default()
            .push(format!("CELL_{name}"));
    }

    let shared: Vec<String> = by_value
        .iter()
        .filter(|(value, names)| **value != 0 && names.len() > 1)
        .map(|(value, names)| format!("{} all read as {value}", names.join(", ")))
        .collect();

    println!(
        "cell flags examined: {examined} (floor {FLAGS_EXAMINED_FLOOR}); \
         distinct values among them: {}; sharing a value: {}",
        by_value.len(),
        shared.len()
    );

    assert!(
        examined >= FLAGS_EXAMINED_FLOOR,
        "the walk found {examined} flags and the block held {FLAGS_EXAMINED_FLOOR} when this \
         was written - a search that stops finding them passes for the wrong reason"
    );
    assert!(
        shared.is_empty(),
        "two cell flags are the same bit:\n  {}\
         \n  A cell carries several of these at once, so a shared bit makes one \
         question answer another: a keepout read as a halo, and the search yielded it.",
        shared.join("\n  ")
    );
}
