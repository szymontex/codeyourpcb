//! `--no-assembly` skips the three files assembly needs, and nothing else.
//!
//! `cargo test -p cypcb-cli --test no_assembly_skips_three_files`
//!
//! A fabricator makes the bare board from the Gerbers and the drill files; an
//! assembler needs three more - the bill of materials, the pick-and-place, and
//! the JSON the viewer's own tooling reads. A board being sent for bare
//! fabrication carries no parts list, so `export --no-assembly` leaves those
//! three out.
//!
//! Which three was never measured. It was the last flag on the binary that no
//! test ran, found by counting every flag against the suite and the README.
//!
//! Measured on `examples/blink.cypcb`: a full export writes **14** files and
//! the lean one writes **11**, and the difference is exactly
//! `assembly/blink-BOM.csv`, `assembly/blink-CPL.csv` and
//! `assembly/blink.json`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

/// Export the example, with the flag or without it, and list what was written.
fn exported(who: &str, args: &[&str]) -> BTreeSet<String> {
    let dir = std::env::temp_dir().join(format!("cypcb-no-assembly-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");

    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .arg("export")
        .args(args)
        .arg("examples/blink.cypcb")
        .arg("-o")
        .arg(&dir)
        .current_dir(repo_root())
        .output()
        .expect("the binary runs");
    assert!(
        output.status.success(),
        "the export failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let mut found = BTreeSet::new();
    walk(&dir, &dir, &mut found);
    found
}

fn walk(root: &Path, at: &Path, into: &mut BTreeSet<String>) {
    for entry in std::fs::read_dir(at).expect("the output directory is readable") {
        let path = entry.expect("an entry").path();
        if path.is_dir() {
            walk(root, &path, into);
        } else {
            into.insert(
                path.strip_prefix(root)
                    .expect("a path under the output directory")
                    .to_string_lossy()
                    .to_string(),
            );
        }
    }
}

#[test]
fn the_flag_leaves_out_the_bom_the_placement_and_the_json() {
    let full = exported("full", &[]);
    let lean = exported("lean", &["--no-assembly"]);

    let gone: Vec<&String> = full.difference(&lean).collect();
    assert_eq!(
        gone,
        vec![
            &"assembly/blink-BOM.csv".to_string(),
            &"assembly/blink-CPL.csv".to_string(),
            &"assembly/blink.json".to_string(),
        ],
        "these three are what an assembler needs and a bare-board order does \
         not:\nfull: {full:?}\nlean: {lean:?}"
    );
}

#[test]
fn everything_a_fabricator_needs_is_still_there() {
    let full = exported("keeps-full", &[]);
    let lean = exported("keeps-lean", &["--no-assembly"]);

    assert!(
        lean.difference(&full).count() == 0,
        "the flag drops files, it does not add any:\n{lean:?}"
    );
    // The copper, the mask, the legend, the outline, the drill files and the
    // job file that describes them: a set a board house can make from.
    for kept in [
        "gerber/blink-F_Cu.gbr",
        "gerber/blink-B_Cu.gbr",
        "gerber/blink-Edge_Cuts.gbr",
        "drill/blink-PTH.drl",
        "blink-job.gbrjob",
    ] {
        assert!(
            lean.contains(kept),
            "a bare-board order still needs `{kept}`:\n{lean:?}"
        );
    }
    assert_eq!(
        full.len() - lean.len(),
        3,
        "three files, no more and no fewer:\nfull: {full:?}\nlean: {lean:?}"
    );
}
