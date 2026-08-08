//! A semantic error has to show the line it is about.
//!
//! `cargo test -p cypcb-cli --test an_error_points_at_the_line_that_caused_it`
//!
//! Every `SyncError` carries the source text, a span and a help string, and
//! implements `miette::Diagnostic`. The CLI printed them with `Display`:
//!
//! ```text
//! Semantic error: unknown component: 'R9'
//! ```
//!
//! No file, no line, no column, on a board that may be five hundred lines
//! long - while parse errors, three lines earlier in the same command, went
//! through `miette::Report` and rendered the offending line with a caret under
//! it. The information existed and nothing was showing it.
//!
//! These tests run the binary, because the defect was in which renderer the
//! command chose and nothing below the command can see that.

use std::process::Command;

fn cypcb_binary() -> std::path::PathBuf {
    let mut path = std::env::current_exe().unwrap();
    path.pop(); // the test binary
    path.pop(); // deps/
    path.push("cypcb");
    path
}

/// Write a board to a temporary file and run a subcommand on it.
fn run(name: &str, source: &str, args: &[&str]) -> String {
    let dir = std::env::temp_dir().join("cypcb-error-rendering");
    std::fs::create_dir_all(&dir).expect("a place to put the board");
    let path = dir.join(format!("{name}.cypcb"));
    std::fs::write(&path, source).expect("the board is writable");

    let output = Command::new(cypcb_binary())
        .args(args)
        .arg(&path)
        .output()
        .expect("the CLI runs");

    String::from_utf8_lossy(&output.stderr).to_string()
}

/// `cypcb check` on a board, which is what most of these ask about.
fn check(name: &str, source: &str) -> String {
    run(name, source, &["check"])
}

const A_PART: &str = r#"board demo {
    size 20mm x 20mm
    layers 2
}

component R1 resistor "0805" {
    value "10k"
    at 5mm, 10mm
}
"#;

/// The pieces a diagnostic has that a `Display` line does not.
fn assert_reads_like_a_diagnostic(report: &str, offending_line: &str, help_fragment: &str) {
    assert!(
        report.contains(offending_line),
        "the report has to quote the line it is about; got:\n{report}"
    );
    assert!(
        report.contains('╰') || report.contains('^'),
        "the report has to point at the token, not just name it; got:\n{report}"
    );
    assert!(
        report.contains(help_fragment),
        "the help the error carries has to be printed; got:\n{report}"
    );
}

#[test]
fn a_net_naming_a_pin_that_does_not_exist_shows_the_line() {
    let report = check(
        "unknown-pin",
        &format!("{A_PART}\nnet SIG {{\n    R1.3\n}}\n"),
    );

    assert!(report.contains("has no pin '3'"), "got:\n{report}");
    assert_reads_like_a_diagnostic(&report, "R1.3", "pins the footprint declares");
}

#[test]
fn a_net_naming_a_component_that_does_not_exist_shows_the_line() {
    let report = check(
        "unknown-component",
        &format!("{A_PART}\nnet SIG {{\n    R9.1\n}}\n"),
    );

    assert!(report.contains("unknown component"), "got:\n{report}");
    assert_reads_like_a_diagnostic(&report, "R9.1", "define the component");
}

#[test]
fn an_unknown_footprint_shows_the_line() {
    let report = check(
        "unknown-footprint",
        "board demo {\n    size 20mm x 20mm\n    layers 2\n}\n\ncomponent U1 ic \"NOSUCH-99\" {\n    value \"x\"\n    at 5mm, 5mm\n}\n",
    );

    assert!(report.contains("unknown footprint"), "got:\n{report}");
    assert_reads_like_a_diagnostic(&report, "NOSUCH-99", "add this footprint");
}

#[test]
fn a_duplicate_designator_shows_both_definitions() {
    // The one error with two spans. A report that shows only the second is
    // half an answer: the question is which two lines collide.
    let report = check(
        "duplicate",
        &format!("{A_PART}\ncomponent R1 resistor \"0805\" {{\n    value \"1k\"\n    at 15mm, 10mm\n}}\n"),
    );

    assert!(
        report.contains("duplicate reference designator"),
        "got:\n{report}"
    );
    assert!(
        report.contains("first defined here"),
        "both definitions have to be shown; got:\n{report}"
    );
    assert!(
        report.contains("duplicate definition"),
        "both definitions have to be shown; got:\n{report}"
    );
}

/// The board whose size is a bare number, which the grammar reads as
/// millimetres without saying so.
const A_SIZE_WITHOUT_A_UNIT: &str = "board demo {\n    size 20 x 20\n    layers 2\n}\n";

#[test]
fn every_command_that_builds_the_board_says_what_it_assumed() {
    // Only `check` printed warnings until 2026-08-08. The others built the
    // same board through the same `sync_ast_to_world` and dropped them - so a
    // board whose size was assumed exported at that size in silence, and
    // `export` is the command whose output goes to a fabricator.
    let commands: [&[&str]; 4] = [&["check"], &["score"], &["parse"], &["export", "-o"]];

    for args in commands {
        let mut args = args.to_vec();
        let out_dir = std::env::temp_dir().join("cypcb-warning-export");
        if args.last() == Some(&"-o") {
            args.push(out_dir.to_str().expect("a utf-8 temp path"));
        }
        let report = run("assumed-unit", A_SIZE_WITHOUT_A_UNIT, &args);

        assert!(
            report.contains("has no unit"),
            "`cypcb {}` has to say what it assumed; got:\n{report}",
            args[0]
        );
        assert!(
            report.contains("assumed here"),
            "`cypcb {}` has to point at the number; got:\n{report}",
            args[0]
        );
    }
}

#[test]
fn a_warning_does_not_reach_machine_readable_output() {
    // `parse` writes JSON to stdout and something reads it. A warning on
    // stdout would break that reader.
    let dir = std::env::temp_dir().join("cypcb-error-rendering");
    std::fs::create_dir_all(&dir).expect("a place to put the board");
    let path = dir.join("stdout-clean.cypcb");
    std::fs::write(&path, A_SIZE_WITHOUT_A_UNIT).expect("the board is writable");

    let output = Command::new(cypcb_binary())
        .arg("parse")
        .arg(&path)
        .output()
        .expect("the CLI runs");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("has no unit"),
        "the warning belongs on stderr; stdout was:\n{stdout}"
    );
    serde_json::from_str::<serde_json::Value>(&stdout)
        .expect("stdout is still the JSON document `parse` promises");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("has no unit"),
        "and on stderr it is: {stderr}"
    );
}

#[test]
fn a_drc_violation_names_the_line_of_the_part_it_is_about() {
    // A violation is found in board coordinates, and `check` printed only
    // those: `unconnected-pin at (14.050mm, 10.000mm)`. A millimetre is not
    // something a reader can search a text file for. The definition it belongs
    // to is, and `path:line:` is what an editor and a terminal both jump to.
    let source = "board demo {\n    size 30mm x 30mm\n    layers 2\n}\n\ncomponent R1 resistor \"0805\" {\n    value \"10k\"\n    at 10mm, 10mm\n}\n\ncomponent C1 capacitor \"0805\" {\n    value \"100nF\"\n    at 20mm, 20mm\n}\n";

    let report = check("violation-lines", source);

    // R1 is defined on line 6 and C1 on line 11. Both are unconnected, so both
    // report, and each has to name its own line.
    assert!(
        report.contains(":6: ") && report.contains("R1."),
        "R1's violations have to point at line 6; got:\n{report}"
    );
    assert!(
        report.contains(":11: ") && report.contains("C1."),
        "C1's violations have to point at line 11; got:\n{report}"
    );
    assert!(
        !report.contains(":1: "),
        "nothing here is defined on line 1; got:\n{report}"
    );
}

#[test]
fn a_preset_export_cannot_use_is_refused_with_the_reason() {
    // `--preset` means two things. `check` takes design rules - what a house
    // can etch - and knows eight names; `export` takes file conventions - what
    // a house wants the Gerbers called - and knows two. A reader who checks a
    // board against oshpark and then cannot export for it deserves the reason,
    // not just a no.
    let dir = std::env::temp_dir().join("cypcb-error-rendering");
    std::fs::create_dir_all(&dir).expect("a place to put the board");
    let path = dir.join("preset-gap.cypcb");
    std::fs::write(&path, A_PART).expect("the board is writable");

    let output = Command::new(cypcb_binary())
        .args(["export", "--preset", "oshpark", "-o"])
        .arg(dir.join("out"))
        .arg(&path)
        .output()
        .expect("the CLI runs");

    let report = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "an unusable preset has to fail");
    assert!(
        report.contains("not an export preset"),
        "the refusal has to name what kind of preset this is; got:\n{report}"
    );
    assert!(
        report.contains("jlcpcb") && report.contains("pcbway"),
        "and what it could have been; got:\n{report}"
    );
    assert!(
        report.contains("check --preset"),
        "and why the other command accepted it; got:\n{report}"
    );

    // And it refuses before doing the work. "Exporting..." is the first thing
    // the command prints once it starts.
    assert!(
        !report.contains("Exporting"),
        "a preset it cannot use should be caught before the build; got:\n{report}"
    );
}

#[test]
fn a_short_preset_name_says_which_rules_it_resolved_to() {
    // `oshpark` is a short form. It is only safe because the header names the
    // preset it became - a board checked against the wrong house, silently,
    // is the failure this guards.
    let report = run("short-preset", A_PART, &["check", "--preset", "oshpark"]);
    assert!(
        report.contains("oshpark_2layer"),
        "the output has to name the rules it used; got:\n{report}"
    );
}
