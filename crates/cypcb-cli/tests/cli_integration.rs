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
        .arg(&example)
        .output()
        .expect("Failed to execute cypcb check");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("OK"));
    assert!(stdout.contains("validated successfully"));
}

#[test]
fn test_check_invalid_file_fails() {
    let example = examples_dir().join("invalid.cypcb");
    let output = Command::new(cypcb_binary())
        .arg("check")
        .arg(&example)
        .output()
        .expect("Failed to execute cypcb check");

    assert!(!output.status.success(), "Check should fail for invalid file");
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
