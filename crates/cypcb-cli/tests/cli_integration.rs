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
    // Output should be valid JSON
    assert!(stdout.contains("\"version\": 1"));
    assert!(stdout.contains("\"definitions\""));
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
    assert!(stdout.contains("Parse a .cypcb file"));
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
    assert!(stdout.contains("Check a .cypcb file"));
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
    assert!(!output.status.success(), "check must fail on a board with violations");
}
