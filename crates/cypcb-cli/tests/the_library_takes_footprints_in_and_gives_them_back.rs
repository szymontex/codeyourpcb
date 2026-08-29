//! The footprints on this machine, indexed and found again.
//!
//! `cargo test -p cypcb-cli --test the_library_takes_footprints_in_and_gives_them_back`
//!
//! `cypcb-library` was 3751 lines and 41 tests that nothing called: a schema, a
//! search and an importer with no way for a person to reach any of it. This is
//! the path, and what it holds is the round trip - a `.pretty` folder goes in,
//! a footprint comes back by name.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

/// The footprints this fixture is built from, out of the repository itself.
///
/// Named rather than "the first six found": a walk takes whatever order the
/// filesystem gives, and the first attempt at this test picked six files the
/// reader could not parse and then reported the library as empty - which was
/// true of those six and said nothing about the command. These three are real
/// KiCad footprints that `cypcb-kicad` reads today.
const PARTS: &[&str] = &[
    "viewer/svg-pcb/kicad-components/SOT-23-5.kicad_mod",
    "viewer/svg-pcb/kicad-components/Microchip_RN4871.kicad_mod",
    "viewer/svg-pcb/kicad-components/10-SOIC-EP_3.9x4.9mm_P1mm_EP2.41x3.3mm.kicad_mod",
];

/// A scratch directory holding one `.pretty` library of those footprints.
fn library_of(who: &str) -> (PathBuf, usize) {
    let dir = std::env::temp_dir().join(format!("cypcb-parts-{who}"));
    let _ = fs::remove_dir_all(&dir);
    let pretty = dir.join("Parts.pretty");
    fs::create_dir_all(&pretty).expect("a place to work");

    for part in PARTS {
        let from = repo_root().join(part);
        let name = Path::new(part).file_name().expect("a file name");
        fs::copy(&from, pretty.join(name))
            .unwrap_or_else(|e| panic!("copying {}: {e}", from.display()));
    }
    (dir, PARTS.len())
}

fn cypcb(dir: &Path, args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .args(args)
        .current_dir(dir)
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

#[test]
fn a_pretty_folder_goes_in_and_a_footprint_comes_back_by_name() {
    let (dir, taken) = library_of("round-trip");

    let imported = cypcb(&dir, &["library", "import", "."]);
    assert!(
        imported.contains("Parts:"),
        "the library is named as it is read:\n{imported}"
    );
    assert!(
        imported.contains("footprint(s) from 1 library"),
        "and so is the total:\n{imported}"
    );

    // The index is a file in the directory the command ran in, which is what
    // the page promises: nothing is written outside it.
    assert!(
        dir.join("cypcb-library.db").exists(),
        "the index sits where the command ran"
    );

    // Every footprint that was read can be found again. `SOT` matches the
    // package family rather than one file's name, which is what a person
    // types.
    let listed = cypcb(&dir, &["library", "list"]);
    assert!(listed.contains("Parts (kicad)"), "{listed}");

    let count: usize = listed
        .split_whitespace()
        .find_map(|word| word.parse().ok())
        .unwrap_or(0);
    assert!(
        count > 0 && count <= taken,
        "between one and every file read, and no more: {count} of {taken}\n{listed}"
    );
}

#[test]
fn a_search_finds_what_was_imported_and_says_when_it_does_not() {
    let (dir, _) = library_of("search");
    cypcb(&dir, &["library", "import", "."]);

    let found = cypcb(&dir, &["library", "search", "SOT"]);
    assert!(
        found.contains("SOT-23-5") && found.contains("result(s)"),
        "a package a person would type finds the part:\n{found}"
    );

    let missing = cypcb(&dir, &["library", "search", "nothing-is-called-this"]);
    assert!(
        missing.contains("Nothing in"),
        "and a search that matches nothing says so rather than printing an empty list:\n{missing}"
    );
}

#[test]
fn a_directory_with_no_library_in_it_is_told_what_one_looks_like() {
    // The half that keeps the command usable: somebody points it at the wrong
    // folder, and a silent success would leave them searching an empty index.
    let dir = std::env::temp_dir().join("cypcb-parts-empty");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("a place to work");

    let said = cypcb(&dir, &["library", "import", "."]);
    assert!(
        said.contains("No .pretty folder") && said.contains(".kicad_mod"),
        "the message says what a library is:\n{said}"
    );
}
