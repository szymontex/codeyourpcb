//! `docs/language-server.md` is checked against the server it describes.
//!
//! `cargo test -p cypcb-lsp --test the_manual_matches_the_server`
//!
//! `docs/architecture.md` had this crate down as providing "semantic tokens
//! (syntax highlighting)", which it has never implemented - no `semantic`
//! appears anywhere in its source - and said the `server` feature was
//! "disabled in dev due to proc-macro loading issues", which stopped being
//! true when the flag was removed. Both lines were written once and read many
//! times.
//!
//! So the manual is not prose here. Every capability it claims has to appear
//! in the server's `initialize` result, every capability it says is missing
//! has to be absent, and anything the server advertises that the manual does
//! not mention fails this test - which is the direction that catches the next
//! feature nobody documented.

#![cfg(feature = "server")]

use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

use serde_json::{json, Value};

fn manual() -> String {
    let path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "docs",
        "language-server.md",
    ]
    .iter()
    .collect();
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{} is missing: {e}", path.display()))
}

/// Everything in backticks that names an LSP capability.
///
/// The convention the manual follows: capabilities are written exactly as the
/// protocol spells them, in backticks. Anything else in backticks - a command,
/// a crate, a file - is left alone by the `Provider` suffix test.
fn capabilities_in(section: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for piece in section.split('`').skip(1).step_by(2) {
        let is_capability = piece.ends_with("Provider") || piece == "textDocumentSync";
        if is_capability && !found.iter().any(|seen| seen == piece) {
            found.push(piece.to_string());
        }
    }
    found
}

/// The text between one heading and the next.
fn section<'a>(manual: &'a str, heading: &str) -> &'a str {
    let start = manual
        .find(heading)
        .unwrap_or_else(|| panic!("the manual has lost its '{heading}' heading"));
    let rest = &manual[start + heading.len()..];
    match rest.find("\n## ") {
        Some(end) => &rest[..end],
        None => rest,
    }
}

/// Start the server, ask it what it can do, return the `result` object.
/// The server, killed when this goes out of scope.
///
/// Everything between the spawn and the kill can panic - reading a header,
/// parsing a length, waiting for a whole message - and a language server left
/// on a pipe nobody reads runs until the machine is rebooted. The same shape
/// left three `cypcb watch` processes alive in the build container, the oldest
/// sixteen days old, before the watch test was given a guard on 2026-08-25.
struct Served(std::process::Child);

impl Drop for Served {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn initialize_result() -> Value {
    let mut child = Served(
        Command::new(env!("CARGO_BIN_EXE_cypcb-lsp"))
            .arg("--stdio")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("the language server binary runs"),
    );

    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {"capabilities": {}, "processId": Value::Null, "rootUri": Value::Null},
    });
    let body = serde_json::to_string(&request).expect("the request serializes");
    {
        let stdin = child.0.stdin.as_mut().expect("stdin is piped");
        write!(stdin, "Content-Length: {}\r\n\r\n{}", body.len(), body)
            .expect("the server listens");
        stdin.flush().expect("the write lands");
    }

    let mut reader = BufReader::new(child.0.stdout.take().expect("stdout is piped"));
    let mut length = 0usize;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).expect("the server answers");
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some(value) = line.strip_prefix("Content-Length: ") {
            length = value.parse().expect("a numeric length");
        }
    }
    let mut buffer = vec![0u8; length];
    reader.read_exact(&mut buffer).expect("a whole message");

    let message: Value = serde_json::from_slice(&buffer).expect("valid JSON-RPC");
    message
        .get("result")
        .cloned()
        .unwrap_or_else(|| panic!("no result in {message}"))
}

#[test]
fn every_capability_the_manual_claims_is_advertised() {
    let manual = manual();
    let claimed = capabilities_in(section(&manual, "## What it answers"));
    assert!(
        claimed.len() >= 4,
        "the manual's table lost its capabilities: {claimed:?}"
    );

    let result = initialize_result();
    let advertised = result
        .pointer("/capabilities")
        .and_then(Value::as_object)
        .cloned()
        .expect("initialize carries capabilities");

    let missing: Vec<&String> = claimed
        .iter()
        .filter(|name| !advertised.contains_key(name.as_str()))
        .collect();
    assert!(
        missing.is_empty(),
        "the manual promises what the server does not offer: {missing:?}"
    );
}

#[test]
fn every_capability_the_manual_denies_is_absent() {
    let manual = manual();
    let denied = capabilities_in(section(&manual, "## What it does not answer"));
    assert!(
        denied.len() >= 4,
        "the manual's list of what is missing went missing: {denied:?}"
    );

    let result = initialize_result();
    let advertised = result
        .pointer("/capabilities")
        .and_then(Value::as_object)
        .cloned()
        .expect("initialize carries capabilities");

    let offered: Vec<&String> = denied
        .iter()
        .filter(|name| advertised.contains_key(name.as_str()))
        .collect();
    assert!(
        offered.is_empty(),
        "these are advertised and the manual says they are not: {offered:?}"
    );
}

#[test]
fn nothing_the_server_offers_is_left_out_of_the_manual() {
    // The direction that catches the next feature nobody wrote down.
    let manual = manual();
    let claimed = capabilities_in(section(&manual, "## What it answers"));

    let result = initialize_result();
    let advertised = result
        .pointer("/capabilities")
        .and_then(Value::as_object)
        .cloned()
        .expect("initialize carries capabilities");

    let undocumented: Vec<&String> = advertised
        .keys()
        .filter(|name| !claimed.contains(name))
        .collect();
    assert!(
        undocumented.is_empty(),
        "the server advertises these and docs/language-server.md does not: {undocumented:?}"
    );
}

#[test]
fn the_manual_names_the_binary_that_exists() {
    let manual = manual();
    assert!(
        manual.contains("cargo build -p cypcb-lsp --release"),
        "the build command is how a reader gets a server at all"
    );
    assert!(
        manual.contains("target/release/cypcb-lsp"),
        "and the path it produces has to be the one named"
    );

    let result = initialize_result();
    assert_eq!(
        result.pointer("/serverInfo/name").and_then(Value::as_str),
        Some("cypcb-lsp"),
        "the binary the manual names is the one that answers: {result}"
    );
}
