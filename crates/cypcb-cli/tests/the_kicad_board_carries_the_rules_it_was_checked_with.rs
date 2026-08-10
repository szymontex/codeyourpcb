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
//! The first fix put the numbers in the board file, as
//! `(setup (rules (min_clearance ...) ...))`. That is not a node pcbnew has,
//! and it was written and shipped without a KiCad to try it on. Real KiCad
//! 10.0.5, run on 2026-08-10:
//!
//! ```text
//! Failed to load board: Unexpected rules in 'jlc.kicad_pcb', line 6, offset 6.
//! ```
//!
//! So the flag whose whole purpose was to make the two tools agree produced a
//! board the other tool would not open at all - and every test here passed
//! throughout, because they read the file this project wrote with the reader
//! this project wrote.
//!
//! KiCad keeps design rules in the `.kicad_pro` beside the board, under
//! `board.design_settings.rules`. That is where they go now, and these tests
//! read the project file for them.
//!
//! Without a preset neither file states any rules, and that is deliberate:
//! rules nobody chose are worse than none, because KiCad believes them.

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

/// Write the board with the given flags and hand back both files: the board,
/// then the project beside it - empty when no preset was asked for.
fn exported_pair(name: &str, preset: Option<&str>) -> (String, String) {
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

    let board = std::fs::read_to_string(&out).expect("the board is there");
    let project = std::fs::read_to_string(out.with_extension("kicad_pro")).unwrap_or_default();
    (board, project)
}

/// Just the board file.
fn exported(name: &str, preset: Option<&str>) -> String {
    exported_pair(name, preset).0
}

/// One number out of the project file's rules block.
fn project_rule(text: &str, name: &str) -> String {
    text.lines()
        .find_map(|line| line.trim().strip_prefix(&format!("\"{name}\": ")))
        .map(|rest| rest.trim_end_matches(',').to_string())
        .unwrap_or_else(|| panic!("no {name} in:\n{text}"))
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
    let (board, project) = exported_pair("jlc", Some("jlcpcb"));

    assert_eq!(project_rule(&project, "min_clearance"), "0.127");
    assert_eq!(project_rule(&project, "min_track_width"), "0.127");
    assert_eq!(project_rule(&project, "min_through_hole_diameter"), "0.3");

    // And the board itself carries no rules node, whatever it carries: that
    // node is what pcbnew refuses.
    assert!(
        !board.contains("(rules"),
        "pcbnew will not open a board with this in it:\n{board}"
    );
}

#[test]
fn the_editors_default_trace_is_the_fabs_number_too() {
    // A project that passes DRC and then offers a 0.2mm trace on a 0.127mm fab
    // is the same disagreement moved one dialog along.
    let (_, project) = exported_pair("jlc-class", Some("jlcpcb"));
    let class = project
        .split("\"classes\"")
        .nth(1)
        .expect("the project states a net class");

    assert!(class.contains("\"track_width\": 0.127"), "{class}");
    assert!(class.contains("\"clearance\": 0.127"), "{class}");
}

#[test]
fn a_different_fab_gives_different_numbers() {
    // The point of the flag: this is not one hardcoded rule set wearing a
    // preset's name.
    let jlcpcb = exported_pair("jlc2", Some("jlcpcb")).1;
    let other = exported_pair("other", Some("oshpark")).1;

    assert_ne!(
        project_rule(&jlcpcb, "min_clearance"),
        project_rule(&other, "min_clearance"),
        "two fabs, one number"
    );
}

#[test]
fn without_a_preset_neither_file_states_any_rules() {
    // Rules nobody chose are worse than none: KiCad believes them.
    let (board, project) = exported_pair("silent", None);

    assert!(!board.contains("(setup"), "{board}");
    assert!(!board.contains("min_clearance"), "{board}");
    assert!(
        project.is_empty(),
        "a project file was written for a board nobody chose a fab for:\n{project}"
    );
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

#[test]
fn the_pour_clearance_is_the_fabs_number_too() {
    // The same cross-crate check the mask expansion gets: the export preset
    // carries the number so the Gerber writer can reach it, the design rules
    // carry it so the checker can, and they have to agree. This one had a
    // third state until now - the exporter never read its own field, so the
    // shipped plane used a generous default instead.
    // Against the constraints table rather than `DesignRules`: the checker's
    // own view does not carry a pour clearance, because no rule reads one.
    // The fabricator publishes it and the exporter uses it, so that table is
    // where the two have to agree.
    use cypcb_rules::presets::RulesPreset;

    for (export_name, rules_preset) in [
        ("jlcpcb", RulesPreset::JlcpcbStandard2Layer),
        ("pcbway", RulesPreset::PcbWayStandard),
    ] {
        let export = cypcb_export::presets::from_name(export_name).expect("the preset is there");
        assert_eq!(
            export.pour_clearance,
            rules_preset.constraints().min_copper_pour_clearance,
            "{export_name}: the exporter and the fab table disagree about the plane"
        );
    }
}
