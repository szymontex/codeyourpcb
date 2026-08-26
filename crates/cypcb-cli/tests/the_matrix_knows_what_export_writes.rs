//! The manufacturing rows of the feature matrix, against the files the command
//! actually writes.
//!
//! `cargo test -p cypcb-cli --test the_matrix_knows_what_export_writes`
//!
//! `docs/competition-feature-matrix.md` claims four output formats and denies
//! four others. The claims were read off one run of `export --dry-run` and
//! written down, which is how the rest of that column came to be eight months
//! stale.
//!
//! The denials are the half worth guarding. A row saying **no** is the kind
//! that quietly stops being true: somebody adds an SVG writer, and the
//! document that says this project has none is nobody's next edit.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

fn matrix() -> String {
    std::fs::read_to_string(repo_root().join("docs/competition-feature-matrix.md"))
        .expect("the matrix is there")
}

/// The CodeYourPCB cell of the row with this feature name.
fn our_cell(matrix: &str, feature: &str) -> String {
    let mut found: Option<String> = None;
    for line in matrix.lines() {
        if !line.starts_with('|') {
            continue;
        }
        let fields: Vec<&str> = line.split('|').collect();
        if fields.len() > 3 && fields[1].trim() == feature {
            assert!(found.is_none(), "two rows are named `{feature}`");
            found = Some(fields[2].trim().to_string());
        }
    }
    found.unwrap_or_else(|| panic!("no row of the matrix is named `{feature}`"))
}

/// Every file `export` says it would write, one path per entry.
fn files_export_would_write(example: &str) -> Vec<String> {
    let board = repo_root().join("examples").join(example);
    let dir = std::env::temp_dir().join("cypcb-matrix-export");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");
    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .arg("export")
        .arg("--dry-run")
        .arg("--output")
        .arg(&dir)
        .arg(&board)
        .output()
        .expect("the binary runs");
    // Stdout alone: the paths are the answer this command is asked for, and a
    // list that arrives on stderr cannot be piped into anything.
    let said = String::from_utf8_lossy(&output.stdout).to_string();
    let files: Vec<String> = said
        .lines()
        .map(str::trim)
        .filter(|line| line.contains('/') && !line.ends_with(':'))
        .filter(|line| Path::new(line).extension().is_some())
        .map(str::to_string)
        .collect();
    assert!(
        files.len() > 5,
        "`export --dry-run {example}` listed nothing this test can read:\n{said}"
    );
    files
}

/// A row claiming a format, and how that format shows up in the file list.
const WRITTEN: &[(&str, &str)] = &[
    ("Gerber X2", ".gbr"),
    ("Excellon drill", ".drl"),
    ("BOM (CSV/JSON)", "-BOM.csv"),
    ("Pick & Place (CPL)", "-CPL.csv"),
];

/// A row denying a format, and the file names that would prove it wrong.
///
/// IPC-2581 is XML and ODB++ ships as a compressed directory, so those two are
/// recognised by their containers rather than by a name of their own.
const NOT_WRITTEN: &[(&str, &[&str])] = &[
    ("IPC-2581", &[".xml"]),
    ("ODB++", &[".tgz", ".tar", "odb"]),
    ("PDF/SVG output", &[".pdf", ".svg"]),
    ("STEP/3D model export", &[".step", ".stp"]),
];

#[test]
fn every_format_the_matrix_claims_is_a_file_the_command_writes() {
    let matrix = matrix();
    let files = files_export_would_write("blink.cypcb");

    for (feature, mark) in WRITTEN {
        let cell = our_cell(&matrix, feature);
        assert!(
            cell.starts_with('✅'),
            "`{feature}` reads `{cell}` while `export` writes `{mark}`: {files:?}"
        );
        assert!(
            files.iter().any(|file| file.contains(mark)),
            "`{feature}` is claimed and no `{mark}` is written: {files:?}"
        );
    }
}

#[test]
fn every_format_the_matrix_denies_is_a_file_nothing_writes() {
    let matrix = matrix();
    let files = files_export_would_write("blink.cypcb");

    for (feature, marks) in NOT_WRITTEN {
        let cell = our_cell(&matrix, feature);
        assert!(
            cell.starts_with('❌'),
            "`{feature}` reads `{cell}`, so this test is guarding the wrong direction"
        );
        for mark in *marks {
            let written: Vec<&String> = files.iter().filter(|file| file.contains(mark)).collect();
            assert!(
                written.is_empty(),
                "the matrix says `{feature}` is not written and `export` writes {written:?}"
            );
        }
    }
}

/// The file list is read, not assumed.
///
/// Both tests above pass on any list long enough to clear the length check, so
/// this one takes a format away and requires the list to lose it: `-BOM.csv`
/// and `-CPL.csv` are what `--no-assembly` skips, and the gerbers are what it
/// keeps.
#[test]
fn the_list_is_the_commands_own() {
    let board = repo_root().join("examples/blink.cypcb");
    let dir = std::env::temp_dir().join("cypcb-matrix-export-bare");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");
    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .arg("export")
        .arg("--dry-run")
        .arg("--no-assembly")
        .arg("--output")
        .arg(&dir)
        .arg(&board)
        .output()
        .expect("the binary runs");
    let said = String::from_utf8_lossy(&output.stdout).to_string();

    assert!(
        !said.contains("-BOM.csv") && !said.contains("-CPL.csv"),
        "`--no-assembly` still lists the assembly files:\n{said}"
    );
    assert!(
        said.contains(".gbr"),
        "`--no-assembly` should keep the gerbers:\n{said}"
    );
}
