//! `to-kicad` asks the board which fabricator it is for.
//!
//! `cargo test -p cypcb-cli --test the_kicad_board_carries_the_fabs_numbers`
//!
//! `check`, `route`, `score` and `watch` all read `board b { fab pcbway }`
//! when no `--preset` is given. `to-kicad` did not: without the flag it wrote
//! a board stating no rules at all, so a design checked here against PCBWay
//! opened in KiCad graded against KiCad's own defaults. It was the last
//! command in the binary that ignored what the board says about itself.
//!
//! What has not changed is the silence when nobody chose: a board naming no
//! fab, exported with no flag, still states no rules, because rules nobody
//! chose are worse than none - KiCad believes them.
//!
//! One of those numbers has a home in the board file. KiCad keeps clearance,
//! track width and the via figures in the project file, and this writer has no
//! example of the format that assigns them, so what travels is the fab's
//! **mask expansion** - and that is enough to tell the houses apart: PCBWay
//! publishes 0.0508mm where JLCPCB publishes 0.05mm.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

/// Write one example out to KiCad and return the file.
fn to_kicad(who: &str, example: &str, preset: Option<&str>) -> String {
    let dir = std::env::temp_dir().join(format!("cypcb-to-kicad-fab-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");
    let out = dir.join("board.kicad_pcb");

    let mut args: Vec<String> = vec![
        "to-kicad".to_string(),
        example.to_string(),
        "-o".to_string(),
        out.to_str().expect("a path that is text").to_string(),
    ];
    if let Some(preset) = preset {
        args.push("--preset".to_string());
        args.push(preset.to_string());
    }

    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .args(&args)
        .current_dir(repo_root())
        .output()
        .expect("the binary runs");
    assert!(
        output.status.success(),
        "`cypcb {}` failed:\n{}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::read_to_string(&out).expect("the KiCad board was written")
}

#[test]
fn a_board_that_names_its_fab_carries_that_fabs_numbers() {
    // `examples/blind-via.cypcb` says `fab pcbway`, and no flag is given.
    let written = to_kicad("design", "examples/blind-via.cypcb", None);
    assert!(
        written.contains("(pad_to_mask_clearance 0.0508)"),
        "the design names PCBWay, whose mask expansion is 0.0508mm:\n{}",
        written.lines().take(12).collect::<Vec<_>>().join("\n")
    );
}

#[test]
fn a_board_nobody_chose_a_fab_for_is_written_with_no_rules_at_all() {
    // The half this change had to keep: `examples/blink.cypcb` names no fab
    // and no flag was given, so nobody chose. Falling back to JLCPCB here
    // would put numbers nobody asked for into a file KiCad believes.
    let written = to_kicad("default", "examples/blink.cypcb", None);
    assert!(
        !written.contains("pad_to_mask_clearance"),
        "rules nobody chose are worse than none:\n{}",
        written.lines().take(12).collect::<Vec<_>>().join("\n")
    );
}

#[test]
fn the_flag_still_wins_over_the_design() {
    // A question about a specific house is not overruled by the file, which is
    // the rule every other command follows.
    let written = to_kicad("flag", "examples/blink.cypcb", Some("pcbway"));
    assert!(
        written.contains("(pad_to_mask_clearance 0.0508)"),
        "`--preset pcbway` was asked for and has to be answered:\n{}",
        written.lines().take(12).collect::<Vec<_>>().join("\n")
    );
}
