//! A footprint keeps the name it was given, through KiCad and back.
//!
//! `cargo test -p cypcb-cli --test a_footprint_keeps_its_name_through_kicad`
//!
//! The round-trip census, run over the parsed model rather than the text,
//! showed every part on every example changing footprint: `0402` came home as
//! `_0402` and `PIN-HDR-1x2` as `PIN_HDR_1x2`. A `footprint` definition took a
//! bare identifier, so the writer rewrote anything the grammar would not
//! accept - a leading digit gained an underscore, a hyphen became one - and a
//! design that had been through KiCad no longer named the parts its author
//! had named.
//!
//! The definition takes the same kind of name a net does now, bare where it
//! can be and quoted where it cannot, and the writer keeps what it was given.
//! The library prefix still goes: `cypcb:0402` and
//! `Package_QFP:LQFP-48_7x7mm_P0.5mm` are KiCad's way of saying which library
//! a part came from, and this language's own library is keyed by the bare
//! name.

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
    String::from_utf8_lossy(&output.stdout).to_string()
}

/// `examples/blink.cypcb` after a trip to KiCad and back.
fn round_tripped(who: &str) -> (PathBuf, String) {
    let dir = std::env::temp_dir().join(format!("cypcb-footprint-name-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");
    let kicad = dir.join("blink.kicad_pcb");
    let back = dir.join("blink.cypcb");

    cypcb(&[
        "to-kicad",
        "examples/blink.cypcb",
        "-o",
        kicad.to_str().expect("a path that is text"),
    ]);
    cypcb(&[
        "from-kicad",
        kicad.to_str().expect("a path that is text"),
        "-o",
        back.to_str().expect("a path that is text"),
    ]);
    let source = std::fs::read_to_string(&back).expect("the design came back");
    (back, source)
}

#[test]
fn the_parts_come_home_named_what_they_were_called() {
    let (_, source) = round_tripped("names");

    // Two shapes the old writer could not keep: a name that starts with a
    // digit, and one with a hyphen in it.
    for name in ["0402", "0805", "1206", "PIN-HDR-1x2", "SOIC-8"] {
        assert!(
            source.contains(&format!("footprint \"{name}\" {{")),
            "the design uses `{name}` and it has to come back under that \
             name:\n{source}"
        );
    }
    assert!(
        !source.contains("_0402") && !source.contains("PIN_HDR"),
        "no name is rewritten into an identifier any more:\n{source}"
    );

    // And the components name the definitions, or the file names pads nobody
    // can resolve.
    assert!(
        source.contains("component R1 resistor \"0402\" {"),
        "the part points at the definition above it:\n{source}"
    );
}

#[test]
fn the_design_that_comes_back_still_reads() {
    let (back, _) = round_tripped("parses");
    cypcb(&[
        "check",
        "--no-drc",
        back.to_str().expect("a path that is text"),
    ]);
}
