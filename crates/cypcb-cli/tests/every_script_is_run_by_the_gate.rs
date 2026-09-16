//! Every script in `scripts/` is run by the gate, or says why not.
//!
//! `scripts/desktop-smoke.sh` carries the reason this file exists: it was
//! written on 2026-08-12 and nothing ran it until somebody noticed. A script
//! nobody runs is worse than no script - it reads as coverage, and the thing it
//! was written to catch goes uncaught while the file sits there looking like an
//! answer.
//!
//! `cargo test -p cypcb-cli --test every_script_is_run_by_the_gate`

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|crates| crates.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

/// Scripts the gate does not run, and the reason each one is not a gap.
const NOT_RUN_BY_THE_GATE: &[(&str, &str)] = &[
    ("quality-gate.sh", "is the gate"),
    (
        "scheduled-gate.sh",
        "runs the gate on a timer, so the gate running it would be a loop",
    ),
    (
        "setup-dev.sh",
        "installs a developer's toolchain, which a gate must not do to the machine it runs on",
    ),
];

#[test]
fn every_script_is_run_by_the_gate() {
    let root = repo_root();
    let gate =
        std::fs::read_to_string(root.join("scripts/quality-gate.sh")).expect("the gate is there");

    // A mention in a comment is not a run, and this file is about the
    // difference: `desktop-smoke.sh` was mentioned in prose for weeks.
    let commands: String = gate
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");

    let mut scripts: Vec<String> = Vec::new();
    for entry in std::fs::read_dir(root.join("scripts")).expect("the scripts are there") {
        let path = entry.expect("a script").path();
        if path.extension().is_some_and(|e| e == "sh") {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                scripts.push(name.to_string());
            }
        }
    }
    scripts.sort();

    let excused: BTreeSet<&str> = NOT_RUN_BY_THE_GATE.iter().map(|(name, _)| *name).collect();
    let mut unrun: Vec<String> = Vec::new();
    let mut run = 0usize;
    for name in &scripts {
        if commands.contains(&format!("./scripts/{name}")) {
            run += 1;
        } else if !excused.contains(name.as_str()) {
            unrun.push(name.clone());
        }
    }

    let vanished: Vec<&str> = NOT_RUN_BY_THE_GATE
        .iter()
        .map(|(name, _)| *name)
        .filter(|name| !scripts.iter().any(|s| s == name))
        .collect();

    eprintln!(
        "scripts in scripts/: {}; run by the gate: {run}; excused: {}; neither: {}",
        scripts.len(),
        NOT_RUN_BY_THE_GATE.len(),
        unrun.len()
    );

    assert!(
        vanished.is_empty(),
        "a script excused here is no longer in scripts/: {vanished:#?}\n\
         \n  The excuse outlived the file. Drop the entry, or the list slowly becomes a \
         permission for scripts that do not exist."
    );

    assert!(
        run > 0,
        "this check found the gate running no script at all, so it has stopped reading the \
         gate and its other answer means nothing"
    );

    assert!(
        unrun.is_empty(),
        "a script sits in scripts/ and the gate never runs it: {unrun:#?}\n\
         \n  A script nobody runs reads as coverage while the thing it was written to catch \
         goes uncaught. Wire it into the gate, or add it here with the reason it is not a gap."
    );
}
