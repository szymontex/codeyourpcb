//! The part of a board that bends is named, not written as copper.
//!
//! `cargo test -p cypcb-cli --test the_flex_region_is_named_not_dropped`
//!
//! `flex bend { ... }` is V8's rigid-flex vocabulary: the region a build
//! folds, which the stackup's coverlay and stiffener are about. KiCad has no
//! area for it - a zone there is copper or a rule area, and this is neither.
//!
//! It was written as a pour with no net, once per copper layer: on
//! `examples/rigid-flex.cypcb`, whose region covers `all`, that was **32
//! netless zones** in a 13128-byte file, every one of which the importer then
//! refused with `a zone is poured to no net, so nothing connects to it`. A
//! reader of that output would think the design had thirty-two broken pours
//! rather than one region the format cannot hold.
//!
//! Now the writer leaves it out and the command says so, the way it already
//! says so for the drill spans and the fabricator.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

fn cypcb(args: &[&str]) -> String {
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

fn scratch(who: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cypcb-flex-region-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");
    dir
}

#[test]
fn the_command_names_the_region_it_cannot_carry() {
    let dir = scratch("named");
    let kicad = dir.join("rigid-flex.kicad_pcb");

    let said = cypcb(&[
        "to-kicad",
        "examples/rigid-flex.cypcb",
        "-o",
        kicad.to_str().expect("a path that is text"),
    ]);
    assert!(
        said.contains("the flexible region(s) this design states (bend)"),
        "the design's region is called `bend` and the warning has to name \
         it:\n{said}"
    );

    // And it is not in the file under another name.
    let written = std::fs::read_to_string(&kicad).expect("the KiCad board was written");
    assert!(
        !written.contains("(zone"),
        "this design has no copper pour, so the file has no zone in it:\n{}",
        written
            .lines()
            .filter(|l| l.contains("zone"))
            .take(4)
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn nothing_comes_back_as_a_pour_that_connects_to_nothing() {
    let dir = scratch("import");
    let kicad = dir.join("rigid-flex.kicad_pcb");
    let back = dir.join("rigid-flex.cypcb");

    cypcb(&[
        "to-kicad",
        "examples/rigid-flex.cypcb",
        "-o",
        kicad.to_str().expect("a path that is text"),
    ]);
    let said = cypcb(&[
        "from-kicad",
        kicad.to_str().expect("a path that is text"),
        "-o",
        back.to_str().expect("a path that is text"),
    ]);
    assert!(
        !said.contains("poured to no net"),
        "a region that bends is not a pour with a missing net, and the import \
         should have nothing to refuse:\n{said}"
    );
}
