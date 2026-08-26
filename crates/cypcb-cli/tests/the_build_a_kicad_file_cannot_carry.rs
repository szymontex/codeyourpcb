//! What a KiCad file loses from a rigid-flex build, and what that costs.
//!
//! `cargo test -p cypcb-cli --test the_build_a_kicad_file_cannot_carry`
//!
//! `examples/blind-via.cypcb` states two things a `.kicad_pcb` has no field
//! for: the drill spans the build makes - `drill Top to Bottom`, `drill Top to
//! Inner1` - and the fabricator whose table the board is graded against. Both
//! are announced when the file is written, and both sentences make a claim
//! about what happens next. Nothing checked either claim.
//!
//! Measured here end to end: the board comes back refused by a table that
//! does not drill blind vias, and passes again the moment its fab is restored
//! by hand - which is what "allows every span" means, because the list it
//! would have been held to is gone with the rest of the build.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

/// A directory of this test's own: cargo runs tests side by side.
fn scratch(who: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cypcb-kicad-build-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");
    dir
}

fn run(args: &[&str]) -> (String, String, bool) {
    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .args(args)
        .current_dir(repo_root())
        .output()
        .expect("the binary runs");
    (
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
        output.status.success(),
    )
}

/// What `check -o json` counted, by kind, and the table it used.
fn checked(board: &Path) -> (String, BTreeMap<String, usize>) {
    let (said, _, _) = run(&["check", "-o", "json", board.to_str().expect("a path")]);
    let report: serde_json::Value =
        serde_json::from_str(said.trim()).expect("check prints JSON on stdout");
    let mut counts = BTreeMap::new();
    for (kind, count) in report["summary"].as_object().expect("a summary") {
        counts.insert(kind.clone(), count.as_u64().expect("a count") as usize);
    }
    (
        report["preset"].as_str().unwrap_or_default().to_string(),
        counts,
    )
}

#[test]
fn the_two_losses_are_announced_and_they_are_real() {
    let dir = scratch("blind-via");
    let board = dir.join("board.kicad_pcb");

    let (_, said, ok) = run(&[
        "to-kicad",
        "examples/blind-via.cypcb",
        "-o",
        board.to_str().expect("a path"),
    ]);
    assert!(ok, "writing the KiCad board failed:\n{said}");
    assert!(
        said.contains("Top to Bottom, Top to Inner1"),
        "the spans the build drills are named when they are dropped:\n{said}"
    );
    assert!(
        said.contains("pcbway"),
        "so is the fabricator whose table the board is graded against:\n{said}"
    );

    // Back again. Neither the build nor the house survives the trip.
    let back = dir.join("back.cypcb");
    let (_, said, ok) = run(&[
        "from-kicad",
        board.to_str().expect("a path"),
        "-o",
        back.to_str().expect("a path"),
    ]);
    assert!(ok, "reading the KiCad board back failed:\n{said}");
    let design = std::fs::read_to_string(&back).expect("the design was written");
    assert!(
        !design.contains("drill Top") && !design.contains("fab "),
        "the trip drops both, which is what the warnings say:\n{design}"
    );

    // What that costs: the default table does not drill blind vias, and the
    // board has two of them.
    let (preset, counts) = checked(&back);
    assert_eq!(preset, "jlcpcb_standard_4layer", "{counts:?}");
    assert_eq!(counts.get("via-span").copied(), Some(2), "{counts:?}");

    // And what it does not cost. Put the house back by hand and the board
    // passes: the span list is gone, so nothing holds the vias to it - which
    // is the claim the first warning makes.
    let restored = dir.join("restored.cypcb");
    std::fs::write(
        &restored,
        design.replacen("layers 4", "layers 4\n    fab pcbway", 1),
    )
    .expect("the fixture is writable");
    let (preset, counts) = checked(&restored);
    assert_eq!(preset, "pcbway_standard");
    assert!(
        counts.is_empty(),
        "a house that drills blind vias and a design with no span list of its \
         own leaves nothing to refuse: {counts:?}"
    );
}
