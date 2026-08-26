//! The browser's engine and `cypcb check` count the same board the same way.
//!
//! `cargo test -p cypcb-cli --test the_two_paths_count_the_same`
//!
//! They have disagreed before: `score` and `check` once differed by a factor
//! of six, and the tracker carried a note that the command line said 4
//! clearance rows where the browser said 6. Both are one rule registry now and
//! the counts match on every example, which is a thing worth holding rather
//! than a thing worth re-measuring by hand next time somebody wonders.
//!
//! Row counts, not grouped rows: what a person reads is grouped by contact in
//! both places, and every published number in this project - the ratchets, the
//! noise bands, the tables in `docs/routing.md` - is a count of rows.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use cypcb_render::PcbEngine;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

/// Every example that both paths can read.
///
/// A file that does not parse is a different test's business, and one that
/// imports another is skipped because the engine's `load_source` resolves no
/// imports - the browser hands it the files it has open, which a test with no
/// browser cannot do.
fn readable_examples() -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(repo_root().join("examples"))
        .expect("the examples are there")
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "cypcb"))
        .collect();
    files.sort();
    files
        .into_iter()
        .filter(|path| {
            let source = std::fs::read_to_string(path).unwrap_or_default();
            !source
                .lines()
                .any(|line| line.trim_start().starts_with("import "))
        })
        .collect()
}

/// What the command line found, by kind.
fn what_the_command_found(board: &Path) -> Option<BTreeMap<String, usize>> {
    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .arg("check")
        .arg("-o")
        .arg("json")
        .arg(board)
        .output()
        .expect("the binary runs");
    let said = String::from_utf8_lossy(&output.stdout).to_string();
    let report: serde_json::Value = serde_json::from_str(said.trim()).ok()?;
    let mut counts = BTreeMap::new();
    for (kind, count) in report["summary"].as_object()? {
        counts.insert(kind.clone(), count.as_u64()? as usize);
    }
    Some(counts)
}

/// What the engine found in a KiCad board, by kind.
fn what_the_engine_found_in_kicad(source: &str) -> Option<BTreeMap<String, usize>> {
    let mut engine = PcbEngine::new();
    let errors = engine.load_kicad(source);
    if !errors.is_empty() {
        return None;
    }
    counts_of(&engine.get_violations_json())
}

/// Rows of the engine's report, by kind.
fn counts_of(json: &str) -> Option<BTreeMap<String, usize>> {
    let rows: serde_json::Value = serde_json::from_str(json).ok()?;
    let mut counts = BTreeMap::new();
    for row in rows.as_array()? {
        let kind = row["kind"].as_str()?.to_string();
        *counts.entry(kind).or_insert(0) += 1;
    }
    Some(counts)
}

/// What the engine the browser runs found, by kind.
fn what_the_engine_found(source: &str) -> Option<BTreeMap<String, usize>> {
    let mut engine = PcbEngine::new();
    if !engine.load_source(source).is_empty() {
        return None;
    }
    counts_of(&engine.get_violations_json())
}

#[test]
fn every_example_is_counted_the_same_by_both() {
    let mut checked = 0;
    let mut disagreements: Vec<String> = Vec::new();

    for board in readable_examples() {
        let source = std::fs::read_to_string(&board).expect("a readable example");
        let (Some(engine), Some(command)) = (
            what_the_engine_found(&source),
            what_the_command_found(&board),
        ) else {
            continue;
        };
        checked += 1;
        if engine != command {
            disagreements.push(format!(
                "{}: the command says {command:?} and the engine says {engine:?}",
                board.file_name().expect("a name").to_string_lossy()
            ));
        }
    }

    assert!(
        checked > 15,
        "only {checked} examples reached both paths, so this proves little"
    );
    assert!(
        disagreements.is_empty(),
        "the two paths count the same board differently:\n{}",
        disagreements.join("\n")
    );
}

/// The census above is taken on boards nobody has routed.
///
/// Every example is a placement with no copper between the parts, so what the
/// two paths were compared on is mostly pins nobody joined. Clearance rows -
/// the kind that made `score` and `check` differ by a factor of six once - only
/// appear in numbers once there is copper to measure, and a router puts it
/// there in a way no fixture in this repository does.
#[test]
fn a_routed_board_is_counted_the_same_by_both() {
    let dir = std::env::temp_dir().join("cypcb-two-paths-routed");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");
    let routed = dir.join("routed.kicad_pcb");

    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .arg("route")
        .arg(repo_root().join("tests/fixtures/benchmark/led_blink.kicad_pcb"))
        .arg("--in-house")
        .arg("--fast")
        .arg("-o")
        .arg(&routed)
        .output()
        .expect("the binary runs");
    assert!(
        output.status.success(),
        "routing the fixture failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let source = std::fs::read_to_string(&routed).expect("the routed board was written");
    let engine = what_the_engine_found_in_kicad(&source).expect("the engine reads a KiCad board");
    let command = what_the_command_found(&routed).expect("the command reads it too");

    assert!(
        command.get("clearance").copied().unwrap_or(0) > 0,
        "a routed board has copper close enough to measure: {command:?}"
    );
    assert_eq!(
        command, engine,
        "the two paths count a routed board differently"
    );
}
