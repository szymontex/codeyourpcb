//! CLI integration tests.
//!
//! These tests verify that the CLI binary works correctly.

use std::process::Command;

/// Path to the CLI binary being tested.
fn cypcb_binary() -> std::path::PathBuf {
    // When running tests, the binary is in target/debug/
    let mut path = std::env::current_exe().unwrap();
    path.pop(); // Remove test binary name
    path.pop(); // Remove deps directory
    path.push("cypcb");
    path
}

/// Get the examples directory path.
fn examples_dir() -> std::path::PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    std::path::Path::new(manifest_dir)
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples")
}

#[test]
fn test_help() {
    let output = Command::new(cypcb_binary())
        .arg("--help")
        .output()
        .expect("Failed to execute cypcb --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("CodeYourPCB"));
    assert!(stdout.contains("parse"));
    assert!(stdout.contains("check"));
}

#[test]
fn test_version() {
    let output = Command::new(cypcb_binary())
        .arg("--version")
        .output()
        .expect("Failed to execute cypcb --version");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("cypcb"));
}

#[test]
fn test_parse_valid_file() {
    let example = examples_dir().join("blink.cypcb");
    let output = Command::new(cypcb_binary())
        .arg("parse")
        .arg(&example)
        .output()
        .expect("Failed to execute cypcb parse");

    assert!(output.status.success(), "Parse failed: {:?}", output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    // The default format is the board model, which is what the flag's help
    // text has always said. This test asserted the AST's own keys until
    // 2026-08-07, and passed while the documented format did not exist.
    assert!(stdout.contains("\"components\""));
    assert!(stdout.contains("\"nets\""));
    assert!(!stdout.contains("\"definitions\""));
}

#[test]
fn test_parse_ast_output() {
    let example = examples_dir().join("blink.cypcb");
    let output = Command::new(cypcb_binary())
        .arg("parse")
        .arg("--output")
        .arg("ast")
        .arg(&example)
        .output()
        .expect("Failed to execute cypcb parse --output ast");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"version\": 1"));
}

#[test]
fn test_check_valid_file() {
    let example = examples_dir().join("blink.cypcb");
    let output = Command::new(cypcb_binary())
        .arg("check")
        .arg("--no-drc")
        .arg(&example)
        .output()
        .expect("Failed to execute cypcb check");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("OK"));
    assert!(stdout.contains("parsed and validated"));
}

#[test]
fn test_check_runs_drc() {
    let example = examples_dir().join("drc-test.cypcb");
    let output = Command::new(cypcb_binary())
        .arg("check")
        .arg(&example)
        .output()
        .expect("Failed to execute cypcb check");

    assert!(
        !output.status.success(),
        "Check should fail on a board with DRC violations"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("DRC violation"),
        "stderr should report DRC violations, got: {}",
        stderr
    );
    assert!(
        stderr.contains("Summary:"),
        "stderr should contain a per-kind summary, got: {}",
        stderr
    );
}

#[test]
fn test_export_board_with_custom_footprint() {
    let example = examples_dir().join("custom-footprint.cypcb");
    let out_dir = std::env::temp_dir().join("cypcb-export-custom-footprint");
    let _ = std::fs::remove_dir_all(&out_dir);

    let output = Command::new(cypcb_binary())
        .arg("export")
        .arg(&example)
        .arg("--output")
        .arg(&out_dir)
        .output()
        .expect("Failed to execute cypcb export");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "export must resolve footprints defined in the source, got: {}",
        stderr
    );
    assert!(
        out_dir.join("gerber").exists(),
        "gerber output directory should exist"
    );

    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn test_check_sees_drills_of_inline_footprints() {
    // A footprint defined in the source, with a drill under every preset's
    // minimum. DRC rules used to build their own built-in-only library, so a
    // board like this passed the drill check by being invisible.
    let dir = std::env::temp_dir().join("cypcb-inline-footprint-drc");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let board = dir.join("tiny-drill.cypcb");
    std::fs::write(
        &board,
        r#"version 1

footprint TINY_DRILL {
    pad 1 circle at 0mm, 0mm size 1mm x 1mm drill 0.1mm
}

board tiny { size 10mm x 10mm }

component J1 connector "TINY_DRILL" {
    at 5mm, 5mm
}
"#,
    )
    .expect("write board");

    let output = Command::new(cypcb_binary())
        .arg("check")
        .arg(&board)
        .output()
        .expect("Failed to execute cypcb check");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "a 0.1mm drill must fail the check, got: {}{}",
        String::from_utf8_lossy(&output.stdout),
        stderr
    );
    assert!(
        stderr.contains("drill-size"),
        "expected a drill-size violation, got: {}",
        stderr
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_check_unknown_preset_fails() {
    let example = examples_dir().join("blink.cypcb");
    let output = Command::new(cypcb_binary())
        .arg("check")
        .arg("--preset")
        .arg("no-such-fab")
        .arg(&example)
        .output()
        .expect("Failed to execute cypcb check");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Unknown preset"));
}

#[test]
fn test_check_invalid_file_fails() {
    let example = examples_dir().join("invalid.cypcb");
    let output = Command::new(cypcb_binary())
        .arg("check")
        .arg(&example)
        .output()
        .expect("Failed to execute cypcb check");

    assert!(
        !output.status.success(),
        "Check should fail for invalid file"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Should contain error information
    assert!(stderr.contains("cypcb::parse"));
}

#[test]
fn test_check_nonexistent_file_fails() {
    let output = Command::new(cypcb_binary())
        .arg("check")
        .arg("nonexistent_file.cypcb")
        .output()
        .expect("Failed to execute cypcb check");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Failed to read"));
}

#[test]
fn test_parse_help() {
    let output = Command::new(cypcb_binary())
        .arg("parse")
        .arg("--help")
        .output()
        .expect("Failed to execute cypcb parse --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // `parse` reads the .cypcb language and turns a KiCad board away, naming
    // `parse-kicad` as it goes. It said "both formats" for one commit, on the
    // strength of a source grep that could not tell support from detection.
    assert!(stdout.contains("Parse a .cypcb design"));
    assert!(stdout.contains("--output"));
}

#[test]
fn test_check_help() {
    let output = Command::new(cypcb_binary())
        .arg("check")
        .arg("--help")
        .output()
        .expect("Failed to execute cypcb check --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Both formats, for the reason recorded beside `test_parse_help`.
    assert!(stdout.contains("Check a .cypcb or .kicad_pcb board"));
}

#[test]
fn check_says_which_violations_are_copper_on_copper() {
    // A count reads the same whether the board shorts or runs 0.01mm under
    // spec, and those are different decisions: one board cannot work, the
    // other is a yield risk a fab may still build. drc-test.cypcb has one
    // clearance violation at 0.00mm among eight kinds of fault.
    let example = examples_dir().join("drc-test.cypcb");
    let output = Command::new(cypcb_binary())
        .arg("check")
        .arg(&example)
        .output()
        .expect("Failed to execute cypcb check");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("copper touching copper at 0.00mm: 1"),
        "check should name the shorts apart from the rest, got:\n{stderr}"
    );

    // A board with violations still fails. The split is what they are, not
    // permission to ship them.
    assert!(
        !output.status.success(),
        "check must fail on a board with violations"
    );
}

#[test]
fn check_gives_a_pour_island_its_size_and_corners() {
    // A coordinate in the middle of a plane tells a person nothing: the copper
    // there looks like every other part of the plane. examples/pour-island.cypcb
    // is a ground pour whose only ground pad sits below a signal trace that
    // crosses it, so the half above the trace reaches nothing.
    let example = examples_dir().join("pour-island.cypcb");
    let output = Command::new(cypcb_binary())
        .arg("check")
        .arg(&example)
        .output()
        .expect("Failed to execute cypcb check");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("pour-island"),
        "the island should be reported, got:\n{stderr}"
    );
    assert!(
        stderr.contains("copper 30.000mm x 14.773mm"),
        "the report should carry the size of the stranded sheet, got:\n{stderr}"
    );
    assert!(
        stderr.contains("from (5.000mm, 20.227mm) to (35.000mm, 35.000mm)"),
        "and its corners, got:\n{stderr}"
    );
}

#[test]
fn export_refuses_a_board_with_copper_touching_copper() {
    // A gap under spec is the designer's call and a fab will build it. Copper
    // on copper is not a call - the board cannot work - so the files are not
    // written until someone says --force. drc-test.cypcb has one such short
    // among its faults.
    let example = examples_dir().join("drc-test.cypcb");
    let out = std::env::temp_dir().join("cypcb-export-refuses");
    let _ = std::fs::remove_dir_all(&out);

    let output = Command::new(cypcb_binary())
        .arg("export")
        .arg(&example)
        .arg("--output")
        .arg(&out)
        .output()
        .expect("Failed to execute cypcb export");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "export should fail, got:\n{stderr}"
    );
    assert!(
        stderr.contains("copper touching copper"),
        "and say why, got:\n{stderr}"
    );
    assert!(
        !out.exists(),
        "nothing should be written for a board that cannot work"
    );
}

#[test]
fn export_writes_the_files_when_forced() {
    let example = examples_dir().join("drc-test.cypcb");
    let out = std::env::temp_dir().join("cypcb-export-forced");
    let _ = std::fs::remove_dir_all(&out);

    let output = Command::new(cypcb_binary())
        .arg("export")
        .arg(&example)
        .arg("--output")
        .arg(&out)
        .arg("--force")
        .output()
        .expect("Failed to execute cypcb export");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "--force should export, got:\n{stderr}"
    );
    assert!(
        stderr.contains("Forcing"),
        "and say that it went ahead with a short on the board, got:\n{stderr}"
    );
    assert!(
        std::fs::read_dir(&out).is_ok_and(|dir| dir.count() > 0),
        "the files should be there"
    );

    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn export_resolves_imports_the_way_check_does() {
    // A design built from a block library checked clean and could not be made:
    // export was the one command that skipped import resolution, so every
    // `use Divider ...` came back as `unknown module: 'Divider'`. The command
    // that produces the deliverable was the one that could not read the file.
    let example = examples_dir().join("v2-imports.cypcb");
    let out = std::env::temp_dir().join("cypcb-export-imports");
    let _ = std::fs::remove_dir_all(&out);

    let output = Command::new(cypcb_binary())
        .arg("export")
        .arg(&example)
        .arg("--output")
        .arg(&out)
        .output()
        .expect("Failed to execute cypcb export");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "a design that checks clean has to export, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("unknown module"),
        "the imported modules should resolve, got:\n{stderr}"
    );

    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn export_says_a_library_is_a_library() {
    // The exporter used to answer `NoBoardSize` for a file with no board,
    // which reads like a missing setting rather than a file nobody meant to
    // manufacture.
    //
    // This used to point at examples/v2-interfaces.cypcb, which had no board
    // at the time. That example has one now - it could not otherwise show the
    // interface contracts being held, which is its subject - so this writes
    // its own library instead. The message is what is under test, not which
    // file happens to lack a board this month.
    let dir = std::env::temp_dir().join("cypcb-export-library-src");
    std::fs::create_dir_all(&dir).expect("a place to put the library");
    let example = dir.join("blocks-only.cypcb");
    std::fs::write(
        &example,
        "version 1\n\n         interface I2C {\n    pin SDA\n    pin SCL\n}\n",
    )
    .expect("the library is writable");
    let out = std::env::temp_dir().join("cypcb-export-library");
    let _ = std::fs::remove_dir_all(&out);

    let output = Command::new(cypcb_binary())
        .arg("export")
        .arg(&example)
        .arg("--output")
        .arg(&out)
        .output()
        .expect("Failed to execute cypcb export");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "a library cannot be exported");
    assert!(
        stderr.contains("declares no board"),
        "and the message should say why, got:\n{stderr}"
    );
}

#[test]
fn the_dry_run_lists_the_inner_layers_a_board_declares() {
    // The dry run is what a person reads before spending money on a board.
    // It listed the preset's file set, and both presets ship an empty
    // inner-layer list - so it promised a two-layer set for a four-layer
    // design, which is exactly the board that had its inner copper dropped.
    let example = examples_dir().join("four-layer.cypcb");
    let output = Command::new(cypcb_binary())
        .arg("export")
        .arg(&example)
        .arg("--dry-run")
        .output()
        .expect("Failed to execute cypcb export");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "the dry run should succeed:\n{stderr}"
    );
    assert!(
        stderr.contains("Board stack: 4 copper layers (2 inner)"),
        "the stack should be stated beside the preset name, got:\n{stderr}"
    );
    // The prose is on stderr and the paths on stdout, so a dry run can be
    // piped into a file that holds nothing but the file set.
    let listing = String::from_utf8_lossy(&output.stdout);
    assert!(
        listing.contains("In1_Cu.gbr") && listing.contains("In2_Cu.gbr"),
        "both inner layers should be listed, got:\n{listing}"
    );
}

#[test]
fn route_says_how_many_vias_are_blind_or_buried() {
    // A blind or buried via costs several times what a through hole costs to
    // make, and a four-layer board collects them without anyone asking - 14 of
    // 26 on the multi_ic benchmark. The number belongs beside the via count,
    // before the files are sent anywhere. A two-layer board can only have
    // through vias, so it says nothing.
    let example = examples_dir().join("blink.cypcb");
    let out = std::env::temp_dir().join("cypcb-route-vias.cypcb");
    let _ = std::fs::remove_file(&out);

    let output = Command::new(cypcb_binary())
        .arg("route")
        .arg(&example)
        .arg("--in-house")
        .arg("--output")
        .arg(&out)
        .output()
        .expect("Failed to execute cypcb route");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "routing should succeed:\n{stderr}");
    assert!(
        !stderr.contains("blind or buried"),
        "a two-layer board has no such vias to report:\n{stderr}"
    );

    let _ = std::fs::remove_file(&out);
}
