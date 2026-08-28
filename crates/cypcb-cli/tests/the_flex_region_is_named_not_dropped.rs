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
fn where_a_layer_stops_is_named_too() {
    // The same file states three bounded layers - two coverlays over the
    // ribbon and a stiffener everywhere but - and KiCad's stackup has a row
    // per layer with no area on it. The layer survives with its thickness and
    // its material; where it stops does not, and a board read back from the
    // file has the stiffener running through its own ribbon.
    let dir = scratch("bounded");
    let kicad = dir.join("rigid-flex.kicad_pcb");

    let said = cypcb(&[
        "to-kicad",
        "examples/rigid-flex.cypcb",
        "-o",
        kicad.to_str().expect("a path that is text"),
    ]);
    assert!(
        said.contains("where 3 stackup layer(s) stop"),
        "three layers of that stack say where they stop:\n{said}"
    );
    assert!(
        said.contains("stiffener covers connector_end"),
        "and the warning names each one as the design wrote it - the stiffener \
         is bonded under one end, which is what the file says:\n{said}"
    );
    assert!(
        said.contains("coverlay covers bend"),
        "the coverlays too:\n{said}"
    );
}

#[test]
fn a_named_area_is_named_too_and_is_not_written_as_copper() {
    // `region connector_end { ... }` is a rectangle with a name on it, there
    // so the stiffener can say which end it is bonded under. KiCad has no area
    // of that kind at all, and writing one as a pour would put copper on the
    // board the design never asked for - the defect the flex region had before
    // the writer learned to leave it out.
    let dir = scratch("named-area");
    let kicad = dir.join("rigid-flex.kicad_pcb");

    let said = cypcb(&[
        "to-kicad",
        "examples/rigid-flex.cypcb",
        "-o",
        kicad.to_str().expect("a path that is text"),
    ]);
    assert!(
        said.contains("the named area(s) this design states (connector_end)"),
        "the design's area is called `connector_end` and the warning has to \
         name it:\n{said}"
    );

    let written = std::fs::read_to_string(&kicad).expect("the KiCad board was written");
    assert!(
        !written.contains("(zone"),
        "a named area is not copper, so the file still has no zone in it"
    );
    assert!(
        !written.contains("connector_end"),
        "and the name is not in the file under any other spelling"
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
