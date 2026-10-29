//! A KiCad part whose reference the language cannot spell is written under a word.
//!
//! `cargo test -p cypcb-cli --test a_reference_the_language_cannot_spell_is_renamed_not_refused`
//!
//! The language names a component with a word: a letter or underscore, then
//! letters, digits and underscores. KiCad keeps whatever was typed, and
//! `from-kicad` wrote it as it was, so 33 of the 132 boards of KiCad's own test
//! suite came out as files that did not parse (`REF**` on a logo, `??` on a part
//! nobody annotated, `TA-101`, `+3.3V2`), and 4 more parsed and would not load
//! (a reference twice, copper on no net). Each part is written under a word of
//! its own now, the board's reference stands in a comment above it, and the
//! command says how many it renamed.

use std::process::Command;

fn board(parts: &[(&str, f64)], extra: &str) -> String {
    let mut text = String::from(
        r#"(kicad_pcb (version 20240108) (generator "hand-written-test")
  (general (thickness 1.6))
  (layers (0 "F.Cu" signal) (31 "B.Cu" signal) (44 "Edge.Cuts" user))
  (net 0 "")
  (net 1 "VCC")
  (net 2 "GND")
  (gr_rect (start 0 0) (end 80 30) (layer "Edge.Cuts") (width 0.05))
"#,
    );
    for (i, (reference, x)) in parts.iter().enumerate() {
        let net = if i % 2 == 0 {
            "(net 1 \"VCC\")"
        } else {
            "(net 2 \"GND\")"
        };
        text.push_str(&format!(
            "  (footprint \"Lib:Part{i}\" (at {x} 10)
    (property \"Reference\" \"{reference}\")
    (property \"Value\" \"v\")
    (pad \"1\" smd rect (at 0 0) (size 1 1) (layers \"F.Cu\" \"F.Paste\" \"F.Mask\") {net}))\n"
        ));
    }
    text.push_str(extra);
    text.push_str(")\n");
    text
}

/// Run `from-kicad` on this text: (success, what it said, the design it wrote).
fn imported(name: &str, text: &str) -> (bool, String, String) {
    let dir = cypcb_fixtures::scratch_dir("cypcb-renamed-references");
    let input = dir.join(format!("{name}.kicad_pcb"));
    let out = dir.join(format!("{name}.cypcb"));
    let _ = std::fs::remove_file(&out);
    std::fs::write(&input, text).expect("the fixture is written");
    let run = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .arg("from-kicad")
        .arg(&input)
        .arg("--output")
        .arg(&out)
        .output()
        .expect("the binary runs");
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    (
        run.status.success(),
        said,
        std::fs::read_to_string(&out).unwrap_or_default(),
    )
}

fn components(design: &str) -> Vec<(String, String)> {
    design
        .lines()
        .filter_map(|line| line.strip_prefix("component "))
        .map(|rest| {
            let mut words = rest.split_whitespace();
            (
                words.next().unwrap_or("").to_string(),
                words.next().unwrap_or("").to_string(),
            )
        })
        .collect()
}

const AWKWARD: &[&str] = &[
    "REF**",
    "G***",
    "TA-101",
    "??",
    "J?",
    "+3.3V2",
    "5V1",
    "BUCKTOOT(BATT)1",
    "J1",
    "J1",
    "R_1",
    "R-1",
    "R1",
];

fn awkward_board() -> String {
    let parts: Vec<(&str, f64)> = AWKWARD
        .iter()
        .enumerate()
        .map(|(i, r)| (*r, 4.0 + 5.5 * i as f64))
        .collect();
    board(&parts, "")
}

#[test]
fn every_reference_the_language_cannot_spell_is_written_as_a_word_and_the_design_loads() {
    let (ok, said, design) = imported("awkward", &awkward_board());
    assert!(ok, "the command re-reads what it wrote: {said}");
    let names: Vec<String> = components(&design).into_iter().map(|(n, _)| n).collect();
    assert_eq!(names.len(), AWKWARD.len(), "{design}");
    for name in &names {
        assert!(
            name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
                && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
            "{name} is not a word"
        );
    }
    let unique: std::collections::BTreeSet<_> = names.iter().collect();
    assert_eq!(
        unique.len(),
        names.len(),
        "each part has a name of its own: {names:?}"
    );
    for expected in [
        "REF__",
        "G___",
        "TA_101",
        "__",
        "J_",
        "_3_3V2",
        "_5V1",
        "BUCKTOOT_BATT_1",
        "J1",
        "J1_2",
        "R_1",
        "R1",
    ] {
        assert!(
            names.iter().any(|n| n == expected),
            "{expected} in {names:?}"
        );
    }
}

#[test]
fn a_name_that_was_taken_is_not_taken_again() {
    // `R-1` sorts before `R_1` and spells `R_1` once its dash is an underscore,
    // which the board's own `R_1` already is.
    let (ok, said, design) = imported("collision", &awkward_board());
    assert!(ok, "{said}");
    let names: Vec<String> = components(&design).into_iter().map(|(n, _)| n).collect();
    assert!(names.contains(&"R_1".to_string()));
    assert!(names.contains(&"R_1_2".to_string()), "{names:?}");
    assert!(
        design.contains("// was \"R-1\"\ncomponent R_1_2 "),
        "{design}"
    );
    // And the board's own `R_1` is not the one that was moved.
    assert!(!design.contains("// was \"R_1\""), "{design}");
}

#[test]
fn the_kind_a_part_has_is_the_kind_it_had() {
    let (ok, said, design) = imported("kinds", &awkward_board());
    assert!(ok, "{said}");
    let kinds: std::collections::BTreeMap<String, String> =
        components(&design).into_iter().collect();
    // Letters first decide the kind, and only what comes after them changed.
    assert_eq!(kinds["J_"], "connector", "J? is a connector");
    assert_eq!(kinds["J1"], "connector");
    assert_eq!(kinds["J1_2"], "connector");
    assert_eq!(kinds["R_1_2"], "resistor", "R-1 is a resistor");
    assert_eq!(kinds["R1"], "resistor");
    assert_eq!(kinds["REF__"], "generic");
    assert_eq!(kinds["TA_101"], "generic");
    assert_eq!(kinds["__"], "generic");
    assert_eq!(kinds["_3_3V2"], "generic");
}

#[test]
fn the_pins_of_a_net_follow_the_new_names() {
    let (ok, said, design) = imported("pins", &awkward_board());
    assert!(ok, "{said}");
    assert!(design.contains("REF__.1"), "{design}");
    assert!(design.contains("TA_101.1"), "{design}");
    assert!(design.contains("J1_2.1"), "{design}");
    for was in ["REF**.1", "TA-101.1", "??.1"] {
        assert!(!design.contains(was), "{was} in {design}");
    }
}

#[test]
fn the_board_s_own_reference_stands_in_a_comment_and_the_command_names_the_first_few() {
    let (ok, said, design) = imported("comments", &awkward_board());
    assert!(ok, "{said}");
    assert!(
        design.contains("// was \"REF**\"\ncomponent REF__ "),
        "{design}"
    );
    assert!(
        design.contains("// was \"TA-101\"\ncomponent TA_101 "),
        "{design}"
    );
    assert!(said.contains("component(s) renamed"), "{said}");
    assert!(said.contains("+3.3V2 -> _3_3V2"), "{said}");
    assert!(
        said.contains("more"),
        "only the first few are named: {said}"
    );
}

#[test]
fn control_a_board_with_spellable_unique_references_is_written_as_it_was() {
    let parts = [("R1", 5.0), ("C1", 10.0), ("U1", 15.0)];
    let (ok, said, design) = imported("plain", &board(&parts, ""));
    assert!(ok, "{said}");
    assert!(!design.contains("// was"), "{design}");
    assert!(!said.contains("component(s) renamed"), "{said}");
    let names: Vec<String> = components(&design).into_iter().map(|(n, _)| n).collect();
    assert_eq!(names, ["C1", "R1", "U1"]);
}

#[test]
fn copper_on_no_net_is_written_on_a_net_the_file_declares() {
    // KiCad's net 0 is a track nobody connected. On a board with no named net
    // the writer called that net `unknown` and never declared it, so the design
    // parsed and did not load.
    let text = r#"(kicad_pcb (version 20240108) (generator "hand-written-test")
  (general (thickness 1.6))
  (layers (0 "F.Cu" signal) (31 "B.Cu" signal) (44 "Edge.Cuts" user))
  (net 0 "")
  (gr_rect (start 0 0) (end 30 20) (layer "Edge.Cuts") (width 0.05))
  (footprint "Lib:Part" (at 10 10)
    (property "Reference" "R1")
    (property "Value" "v")
    (pad "1" smd rect (at 0 0) (size 1 1) (layers "F.Cu" "F.Paste" "F.Mask")))
  (segment (start 5 5) (end 15 5) (width 0.25) (layer "F.Cu") (net 0))
  (via (at 20 5) (size 0.8) (drill 0.4) (layers "F.Cu" "B.Cu") (net 0))
)
"#;
    let (ok, said, design) = imported("no-net", text);
    assert!(ok, "{said}");
    assert!(design.contains("net unknown {"), "{design}");
    assert!(design.contains("trace unknown {"), "{design}");
    assert!(said.contains("copper on no net"), "{said}");
}
