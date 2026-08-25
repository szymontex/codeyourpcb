//! A pour keeps the name its design gave it.
//!
//! `cargo test -p cypcb-cli --test a_pour_keeps_its_name_through_kicad`
//!
//! The round-trip census ended with one unannounced loss:
//! `examples/pour-island.cypcb` calls its pour `gnd_pour`, and the trip
//! renamed it to `GND`. The writer had been putting `(name "gnd_pour")` into
//! the `(zone ...)` all along - KiCad does keep a name for a zone - and the
//! reader never looked at it, falling back to the net the pour fills.
//!
//! A pour nobody has named still arrives under its net's name, which is what
//! every board pcbnew writes looks like and what this importer has always
//! done.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

fn cypcb(args: &[&str]) {
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
}

fn scratch(who: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cypcb-pour-name-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");
    dir
}

#[test]
fn the_name_goes_into_the_zone_and_comes_back_out() {
    let dir = scratch("trip");
    let kicad = dir.join("pour.kicad_pcb");
    let back = dir.join("pour.cypcb");

    cypcb(&[
        "to-kicad",
        "examples/pour-island.cypcb",
        "-o",
        kicad.to_str().expect("a path that is text"),
    ]);
    let written = std::fs::read_to_string(&kicad).expect("the KiCad board was written");
    assert!(
        written.contains("(name \"gnd_pour\")"),
        "KiCad keeps a name for a zone and this is where it goes:\n{}",
        written
            .lines()
            .filter(|l| l.contains("zone") || l.contains("name"))
            .take(8)
            .collect::<Vec<_>>()
            .join("\n")
    );

    cypcb(&[
        "from-kicad",
        kicad.to_str().expect("a path that is text"),
        "-o",
        back.to_str().expect("a path that is text"),
    ]);
    let source = std::fs::read_to_string(&back).expect("the design came back");
    assert!(
        source.contains("zone gnd_pour {"),
        "the design calls its pour `gnd_pour`, so the trip has to leave it \
         called that:\n{source}"
    );
}

#[test]
fn a_pour_nobody_named_arrives_under_its_net() {
    // The half that keeps the other from being a rename: the USB fixture's
    // pours carry no `(name ...)`, the way pcbnew writes them, and they have
    // always come in named after the net they fill.
    let dir = scratch("unnamed");
    let back = dir.join("usb.cypcb");

    cypcb(&[
        "from-kicad",
        "tests/fixtures/usb_c_named_pads.kicad_pcb",
        "-o",
        back.to_str().expect("a path that is text"),
    ]);
    let source = std::fs::read_to_string(&back).expect("the design came back");
    assert!(
        source.contains("zone GND {") && source.contains("zone \"VBUS+\" {"),
        "a pour with no name of its own is named after its net:\n{source}"
    );
}
