//! What the Gerber set cannot carry is said at the point of export.
//!
//! `cargo test -p cypcb-cli --test the_export_names_what_the_files_cannot_carry`
//!
//! `to-kicad` names the drill spans, the fabricator and the net constraints it
//! drops. `export` said nothing, and it drops one thing: a **stiffener**.
//!
//! That is the right call about the file. The Gerber job file's material
//! stackup is specified as the layers of the bare board and only those, and a
//! stiffener is bonded on after the stack is pressed - `material_type` returns
//! nothing for it on purpose, next to solder paste, which is deposited at
//! assembly. It is the wrong thing to do in silence: a design that states one
//! is asking for a board nobody can make from this set of files alone.
//!
//! `examples/rigid-flex.cypcb` states `stiffener 0.2mm material "FR4"` under
//! the rigid end that carries the connector.
//!
//! Opening the files rather than the message adds one thing the warning does
//! not say: **the stiffener does reach exactly one file, as thickness.** The
//! job file's `BoardThickness` is 0.2 mm larger for the design that states it,
//! and nothing else in the set changes. So the warning is right that the files
//! cannot carry a stiffener, and the fabricator quoting from them is reading a
//! board of the correct total thickness with no idea why.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

/// Everything a run of `export` left behind, keyed by the path under its output
/// directory, with the timestamp lines dropped: every file this project writes
/// carries the moment it was written, so two runs never compare equal without
/// that.
fn files_written(who: &str) -> std::collections::BTreeMap<String, String> {
    let dir = std::env::temp_dir().join(format!("cypcb-export-says-{who}"));
    let mut found = std::collections::BTreeMap::new();
    let mut stack = vec![dir.clone()];
    while let Some(here) = stack.pop() {
        for entry in std::fs::read_dir(&here).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue; // a file this comparison cannot read is not compared
            };
            let relative = path
                .strip_prefix(&dir)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();
            let stripped: String = text
                .lines()
                .filter(|line| !line.contains("CreationDate") && !line.contains("export_date"))
                .collect::<Vec<_>>()
                .join("\n");
            found.insert(relative, stripped);
        }
    }
    found
}

fn export(who: &str, example: &str) -> String {
    let dir = std::env::temp_dir().join(format!("cypcb-export-says-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");

    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .args([
            "export",
            example,
            "-o",
            dir.to_str().expect("a path that is text"),
        ])
        .current_dir(repo_root())
        .output()
        .expect("the binary runs");
    assert!(
        output.status.success(),
        "`cypcb export {example}` failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string() + &String::from_utf8_lossy(&output.stderr)
}

#[test]
fn the_stiffener_reaches_exactly_one_file_and_it_is_not_the_copper() {
    // The warning above says the files cannot carry a stiffener. Opening them
    // says something more precise, and it corrects the sentence: **one file
    // changes.** The job file states a board thickness, and a stiffener bonded
    // to the outside of the stack is 0.2 mm of it. Nothing else moves - not the
    // copper, not the drill, not the assembly.
    //
    // This is the difference form rather than an absence: exporting the same
    // design twice, once stating the stiffener and once with the line removed,
    // and naming exactly which of the fourteen files is not the same. An
    // absence would have been satisfied by an export that wrote nothing.
    let source = std::fs::read_to_string(repo_root().join("examples/rigid-flex.cypcb"))
        .expect("the example is readable");
    let plain_source: String = source
        .lines()
        .filter(|line| !line.trim().starts_with("stiffener 0.2mm"))
        .collect::<Vec<_>>()
        .join("\n");
    assert_ne!(
        plain_source, source,
        "the example carries the line this test removes"
    );

    let stated_dir = std::env::temp_dir().join("cypcb-stiffener-stated");
    let plain_dir = std::env::temp_dir().join("cypcb-stiffener-plain");
    for (dir, text) in [(&stated_dir, &source), (&plain_dir, &plain_source)] {
        let _ = std::fs::remove_dir_all(dir);
        std::fs::create_dir_all(dir).expect("a place to work");
        std::fs::write(dir.join("board.cypcb"), text).expect("the board is writable");
    }
    export(
        "stiffener-stated",
        stated_dir.join("board.cypcb").to_str().unwrap(),
    );
    export(
        "stiffener-plain",
        plain_dir.join("board.cypcb").to_str().unwrap(),
    );

    let stated = files_written("stiffener-stated");
    let plain = files_written("stiffener-plain");
    assert!(
        stated.len() > 10,
        "the export wrote a set of files, or nothing below means anything: {} files",
        stated.len()
    );
    assert_eq!(
        stated.keys().collect::<Vec<_>>(),
        plain.keys().collect::<Vec<_>>(),
        "a stiffener adds no file and removes none"
    );

    let differing: Vec<&String> = stated
        .iter()
        .filter(|(name, text)| plain.get(*name) != Some(text))
        .map(|(name, _)| name)
        .collect();
    assert_eq!(
        differing.len(),
        1,
        "exactly one file carries the stiffener at all: {differing:?}"
    );
    assert!(
        differing[0].ends_with("-job.gbrjob"),
        "and it is the job file, which states a board thickness: {differing:?}"
    );

    // How it carries it: as thickness, not as a stiffener. The design says
    // 0.2mm and the job file's board is 0.2mm thicker - which is the whole of
    // what survives, and the reason the warning is right that the files cannot
    // carry the thing itself.
    let thickness = |set: &std::collections::BTreeMap<String, String>| -> f64 {
        set.values()
            .flat_map(|text| text.lines())
            .find(|line| line.contains("\"BoardThickness\""))
            .and_then(|line| {
                line.split(':')
                    .nth(1)
                    .map(|value| value.trim().trim_end_matches(',').to_string())
            })
            .and_then(|value| value.parse::<f64>().ok())
            .expect("the job file states a board thickness")
    };
    let grown = thickness(&stated) - thickness(&plain);
    assert!(
        (grown - 0.2).abs() < 1e-6,
        "the stiffener reaches the job file as its own thickness and nothing else: \
         the board grew by {grown} mm where the design states 0.2"
    );
}

#[test]
fn a_stated_stiffener_is_named_with_its_thickness_and_material() {
    let said = export("flex", "examples/rigid-flex.cypcb");

    assert!(
        said.contains("the stiffener this design states (0.200mm of FR4)"),
        "the design states a stiffener and the export has to name it:\n{said}"
    );
    assert!(
        said.contains("bonded on after it is built"),
        "and say why it is not in the files, which is the half that stops \
         somebody trying to find it in them:\n{said}"
    );
}

#[test]
fn a_board_with_no_stiffener_is_told_nothing_about_one() {
    // The half that keeps the other from being noise. `examples/four-layer.cypcb`
    // states a full stackup and no stiffener.
    let said = export("rigid", "examples/four-layer.cypcb");
    assert!(
        !said.contains("stiffener"),
        "nothing was bonded to this board, so nothing is owed about one:\n{said}"
    );
}
