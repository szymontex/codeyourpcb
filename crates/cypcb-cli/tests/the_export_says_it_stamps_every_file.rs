//! The command's help says every exported file is stamped, and it is.
//!
//! `cargo test -p cypcb-cli --test the_export_says_it_stamps_every_file`
//!
//! Two exports of one board never compare equal byte for byte, and finding
//! that out cost this project three flaky comparisons: a silkscreen gerber, a
//! drill file and a bill of materials, each written as a whole-file diff and
//! each passing only because two runs land in the same second.
//!
//! So the fact is in `cypcb export --help` now, and this holds the sentence to
//! the files: the help names three stamps and all three are really written.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

fn run(args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .args(args)
        .current_dir(repo_root())
        .output()
        .expect("the binary runs");
    assert!(
        output.status.success(),
        "`cypcb {}` failed:\n{}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string() + &String::from_utf8_lossy(&output.stderr)
}

/// A file under `dir` whose name ends this way, read.
fn written(dir: &Path, ends_with: &str) -> String {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(next) = stack.pop() {
        for entry in std::fs::read_dir(&next)
            .expect("the export directory is there")
            .flatten()
        {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.to_string_lossy().ends_with(ends_with) {
                return std::fs::read_to_string(&path).expect("a readable export");
            }
        }
    }
    panic!("no file ending in {ends_with} under {}", dir.display());
}

#[test]
fn the_help_names_the_stamps_the_files_carry() {
    let help = run(&["export", "--help"]);
    for named in ["TF.CreationDate", "CreationDate", "export_date"] {
        assert!(
            help.contains(named),
            "the help says which stamp goes where, and `{named}` is missing:\n{help}"
        );
    }

    let dir = std::env::temp_dir().join("cypcb-export-stamps");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");
    run(&[
        "export",
        "examples/blink.cypcb",
        "-o",
        dir.to_str().expect("a path"),
    ]);

    assert!(
        written(&dir, "F_Cu.gbr").contains("TF.CreationDate"),
        "a gerber carries the stamp the help names"
    );
    assert!(
        written(&dir, "PTH.drl").contains("CreationDate"),
        "so does a drill file"
    );
    assert!(
        written(&dir, ".gbrjob").contains("\"CreationDate\""),
        "so does the job file"
    );
    assert!(
        written(&dir, "blink.json").contains("export_date"),
        "and the assembly JSON, under its own name"
    );
}
