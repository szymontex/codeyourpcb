//! A preset's table is the preset.
//!
//! `cargo test -p cypcb-drc --test a_preset_table_is_the_preset`
//!
//! Every constructor in `src/presets` carries the table a person quotes when
//! they pick a fab:
//!
//! ```text
//! /// | Min edge clearance | 0.381mm (15 mil) | More conservative |
//! ```
//!
//! and the figures come from `RulesPreset::<variant>.constraints()` in another
//! crate. Nothing compared the two, and `scripts/claims-in-comments.sh` lists
//! these among the figures no test names - the last class in that list where
//! the claim and the code are both in this repository.
//!
//! This one asks the code rather than reading it: the table is parsed out of
//! the source, the constructor is called, and the two are held together. So a
//! preset whose constraints move in `cypcb-rules` fails here even though
//! nothing in `cypcb-drc` was edited, which is the direction the drift comes
//! from.

use cypcb_core::Nm;
use cypcb_drc::DesignRules;
use std::path::{Path, PathBuf};

/// Every preset the tables describe. A constructor missing from this list is a
/// constructor nothing checks, so the last case holds the list to the source.
type NamedPreset = (&'static str, fn() -> DesignRules);

const PRESETS: [NamedPreset; 8] = [
    ("jlcpcb_2layer", DesignRules::jlcpcb_2layer),
    ("jlcpcb_4layer", DesignRules::jlcpcb_4layer),
    (
        "jlcpcb_advanced_2layer",
        DesignRules::jlcpcb_advanced_2layer,
    ),
    (
        "jlcpcb_advanced_4layer",
        DesignRules::jlcpcb_advanced_4layer,
    ),
    ("oshpark_2layer", DesignRules::oshpark_2layer),
    ("oshpark_4layer", DesignRules::oshpark_4layer),
    ("pcbway_standard", DesignRules::pcbway_standard),
    ("prototype", DesignRules::prototype),
];

fn preset_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/presets")
}

/// Every preset module, as (file stem, source).
fn sources() -> Vec<(String, String)> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(preset_dir())
        .expect("the preset modules are beside this crate's source")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|kind| kind == "rs"))
        .collect();
    files.sort();
    files
        .iter()
        .map(|path| {
            let stem = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let text = std::fs::read_to_string(path).expect("a module this crate compiles");
            (stem, text)
        })
        .collect()
}

fn field(rules: &DesignRules, parameter: &str) -> Option<Nm> {
    match parameter {
        "Min clearance" => Some(rules.min_clearance),
        "Min trace width" => Some(rules.min_trace_width),
        "Min drill" => Some(rules.min_drill_size),
        "Min via drill" => Some(rules.min_via_drill),
        "Min annular ring" => Some(rules.min_annular_ring),
        "Min silk width" => Some(rules.min_silk_width),
        "Min edge clearance" => Some(rules.min_edge_clearance),
        _ => None,
    }
}

/// The `0.381` out of `| Min edge clearance | 0.381mm (15 mil) | ... |`.
fn millimetres(cell: &str) -> Option<f64> {
    let at = cell.find("mm")?;
    let digits: String = cell[..at]
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    digits.parse().ok()
}

/// The rows of the table above `pub fn <name>() -> Self`, in every module.
fn table_of(name: &str) -> Vec<(String, f64)> {
    let opening = format!("pub fn {name}() -> Self");
    for (_, source) in sources() {
        let Some(at) = source.find(&opening) else {
            continue;
        };
        let mut rows = Vec::new();
        for line in source[..at].lines().rev() {
            let trimmed = line.trim_start();
            if !trimmed.starts_with("///") {
                if rows.is_empty() {
                    continue;
                }
                break;
            }
            let cells: Vec<&str> = trimmed.split('|').collect();
            if cells.len() < 3 {
                continue;
            }
            let parameter = cells[1].trim();
            if let Some(value) = millimetres(cells[2]) {
                rows.push((parameter.to_string(), value));
            }
        }
        return rows;
    }
    Vec::new()
}

#[test]
fn every_figure_a_preset_table_states_is_the_figure_the_preset_returns() {
    let mut checked = 0;
    let mut wrong: Vec<String> = Vec::new();

    for (name, build) in PRESETS {
        let rules = build();
        let rows = table_of(name);
        assert!(
            rows.len() >= 7,
            "{name}: {} rows read from its table, so the reader is broken",
            rows.len()
        );

        for (parameter, stated) in rows {
            let Some(actual) = field(&rules, &parameter) else {
                panic!("{name}: the table names {parameter}, which no field answers");
            };
            let wanted = Nm::from_mm(stated);
            checked += 1;
            // Collected rather than asserted one at a time: the first run found
            // more than one row wrong, and a case that stops at the first says
            // nothing about the rest.
            if (actual.0 - wanted.0).abs() > 100 {
                wrong.push(format!(
                    "{name}: the table says {parameter} is {stated}mm, the preset \
                     returns {}mm",
                    actual.0 as f64 / 1_000_000.0
                ));
            }
        }
    }

    assert!(wrong.is_empty(), "{}", wrong.join("\n"));

    println!("{checked} table rows held against the presets they describe");
    assert!(
        checked >= 56,
        "only {checked} rows were checked, so the reader is broken"
    );
}

#[test]
fn no_preset_is_missing_from_the_list_above() {
    let mut in_source = Vec::new();
    for (_, source) in sources() {
        for line in source.lines() {
            if let Some(rest) = line.trim_start().strip_prefix("pub fn ") {
                if let Some(name) = rest.strip_suffix("() -> Self {") {
                    in_source.push(name.to_string());
                }
            }
        }
    }
    in_source.sort();

    let mut listed: Vec<String> = PRESETS.iter().map(|(name, _)| name.to_string()).collect();
    listed.sort();

    assert_eq!(
        in_source, listed,
        "the preset modules and the list in this case have drifted apart"
    );
}

#[test]
fn a_preset_named_after_a_fab_lives_in_that_fab_s_module() {
    // `jlcpcb_advanced_2layer` and `jlcpcb_advanced_4layer` were defined in
    // `oshpark.rs`, whose own header says "OSHPark manufacturer design rules".
    // Anybody looking for the advanced JLCPCB rules grepped `jlcpcb.rs` and
    // found two of the four.
    let modules: Vec<String> = sources().into_iter().map(|(stem, _)| stem).collect();
    let mut checked = 0;

    for (name, _) in PRESETS {
        let Some(fab) = name.split('_').next() else {
            continue;
        };
        if !modules.iter().any(|stem| stem == fab) {
            // A preset named after no module - `prototype` - is nobody's fab.
            continue;
        }
        let opening = format!("pub fn {name}() -> Self");
        let holder = sources()
            .into_iter()
            .find(|(_, source)| source.contains(&opening))
            .map(|(stem, _)| stem)
            .unwrap_or_else(|| panic!("{name} is in the list and in no module"));

        checked += 1;
        assert_eq!(
            holder, fab,
            "{name} is defined in {holder}.rs, and {fab}.rs is the module named after its fab"
        );
    }

    // Every preset but `prototype` is named after a fab with a module, so a
    // run that checked fewer than seven skipped its way to a pass.
    assert!(
        checked >= 7,
        "only {checked} presets were placed, so the reader is broken"
    );
}
