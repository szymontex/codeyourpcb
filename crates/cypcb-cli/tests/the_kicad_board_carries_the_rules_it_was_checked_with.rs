//! KiCad checks the exported board against the same fab this project did.
//!
//! `cargo test -p cypcb-cli --test the_kicad_board_carries_the_rules_it_was_checked_with`
//!
//! The written file had **no `(setup ...)` block at all** - measured, 0
//! occurrences - so a board opened in KiCad was checked against KiCad's own
//! defaults: numbers with nothing to do with the fabricator the design was
//! checked for. A designer running KiCad's DRC on the exported board got a
//! different answer than `cypcb check` on the source, and neither tool said
//! why.
//!
//! `to-kicad` was also the last command in the CLI that never asked which
//! fabricator a board is for. `check`, `route`, `score` and `export` all take
//! `--preset`; this one wrote a board for nobody in particular.
//!
//! Without a preset it still writes no rules, and that is deliberate: rules
//! nobody chose are worse than none, because KiCad believes them.

use std::path::PathBuf;
use std::process::Command;

const BOARD: &str = r#"version 1

board ruled {
    size 30mm x 20mm
    layers 2
}

component R1 resistor "0402" {
    value "10k"
    at 10mm, 10mm
}
"#;

/// Write the board with the given flags and hand back the file.
fn exported(name: &str, preset: Option<&str>) -> String {
    let dir = std::env::temp_dir().join("cypcb-kicad-rules");
    std::fs::create_dir_all(&dir).expect("a place to work");
    let source = dir.join(format!("{name}.cypcb"));
    std::fs::write(&source, BOARD).expect("the board is written");
    let out: PathBuf = dir.join(format!("{name}.kicad_pcb"));

    let mut command = Command::new(env!("CARGO_BIN_EXE_cypcb"));
    command
        .args(["to-kicad"])
        .arg(&source)
        .arg("--output")
        .arg(&out);
    if let Some(preset) = preset {
        command.arg("--preset").arg(preset);
    }
    let output = command.output().expect("the binary runs");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    std::fs::read_to_string(&out).expect("the board is there")
}

/// One number out of the `(rules ...)` block.
fn rule(text: &str, name: &str) -> String {
    text.lines()
        .find_map(|line| line.trim().strip_prefix(&format!("({name} ")))
        .and_then(|rest| rest.strip_suffix(')'))
        .map(str::to_string)
        .unwrap_or_else(|| panic!("no {name} in:\n{text}"))
}

#[test]
fn the_board_states_the_fabs_own_numbers() {
    // JLCPCB's published minimums, the same ones `cypcb check --preset jlcpcb`
    // measures against.
    let text = exported("jlc", Some("jlcpcb"));

    assert_eq!(rule(&text, "min_clearance"), "0.127");
    assert_eq!(rule(&text, "min_track_width"), "0.127");
    assert_eq!(rule(&text, "min_through_hole"), "0.3");
}

#[test]
fn a_different_fab_gives_different_numbers() {
    // The point of the flag: this is not one hardcoded rule set wearing a
    // preset's name.
    let jlcpcb = exported("jlc2", Some("jlcpcb"));
    let other = exported("other", Some("oshpark"));

    assert_ne!(
        rule(&jlcpcb, "min_clearance"),
        rule(&other, "min_clearance"),
        "two fabs, one number"
    );
}

#[test]
fn without_a_preset_the_file_states_no_rules() {
    // Rules nobody chose are worse than none: KiCad believes them.
    let text = exported("silent", None);

    assert!(!text.contains("(setup"), "{text}");
    assert!(!text.contains("min_clearance"), "{text}");
}

#[test]
fn an_unknown_fab_is_refused_by_name() {
    let dir = std::env::temp_dir().join("cypcb-kicad-rules");
    std::fs::create_dir_all(&dir).expect("a place to work");
    let source = dir.join("bad.cypcb");
    std::fs::write(&source, BOARD).expect("the board is written");

    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .args(["to-kicad"])
        .arg(&source)
        .arg("--preset")
        .arg("not-a-fab")
        .output()
        .expect("the binary runs");

    assert!(!output.status.success(), "a typo has to be refused");
    let complaint = String::from_utf8_lossy(&output.stderr);
    assert!(
        complaint.contains("not-a-fab") && complaint.contains("jlcpcb"),
        "it has to say what was asked for and what there is:\n{complaint}"
    );
}

#[test]
fn the_mask_opening_is_the_fabs_number_in_both_writers() {
    // Two tables in two crates: the export preset carries the mask expansion
    // so the Gerber writer can reach it, and the design rules carry it so the
    // checker can. They have to be the same number, or one of them is
    // describing a different fabricator - and the KiCad file states it a third
    // time as `pad_to_mask_clearance`, which was a literal 0 until now.
    use cypcb_drc::{Preset, PresetRules};

    for (export_name, rules_preset) in [
        ("jlcpcb", Preset::JlcpcbStandard2Layer),
        ("pcbway", Preset::PcbWayStandard),
    ] {
        let export = cypcb_export::presets::from_name(export_name).expect("the preset is there");
        assert_eq!(
            export.mask_expansion,
            rules_preset.rules().solder_mask_expansion,
            "{export_name}: the exporter and the checker disagree about the mask"
        );
    }

    let text = exported("mask", Some("jlcpcb"));
    let stated = rule(&text, "pad_to_mask_clearance");
    assert_eq!(
        stated,
        format!(
            "{}",
            Preset::JlcpcbStandard2Layer
                .rules()
                .solder_mask_expansion
                .to_mm()
        ),
        "and KiCad is told the same"
    );
}
