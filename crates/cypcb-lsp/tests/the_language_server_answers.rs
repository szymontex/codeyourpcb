//! The language server is a program an editor talks to, so this drives it the
//! way an editor does: spawn the binary, speak JSON-RPC over its stdin and
//! stdout, and read what comes back.
//!
//! `cargo test -p cypcb-lsp --test the_language_server_answers`
//!
//! Nothing had ever run this crate. `server` was an off-by-default feature, so
//! `cargo build`, `cargo test` and the quality gate all compiled the crate with
//! `backend.rs` cfg'd out - 2,984 lines of language server that no command in
//! the repository touched. Asking for it directly said why:
//!
//! ```text
//! error[E0195]: lifetime parameters or bounds on method `initialize` do not
//!               match the trait declaration
//! error: could not compile `cypcb-lsp` (lib) due to 10 previous errors
//! ```
//!
//! One `#[async_trait::async_trait]` against a `tower-lsp` that takes native
//! async methods. The server is on by default now, which is what keeps this
//! test - and the compiler - pointed at it.

#![cfg(feature = "server")]

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use serde_json::{json, Value};

/// A board with two parts on one net, small enough to read in a failure.
const BOARD: &str = r#"version 1

board probe {
    size 20mm x 20mm
    layers 2
}

component R1 resistor "0402" {
    value "10k"
    at 5mm, 5mm
}

component R2 resistor "0402" {
    value "1k"
    at 12mm, 5mm
}

net SIG {
    R1.2
    R2.1
}
"#;

/// The same board with `size` given a unit the language does not have.
const BROKEN: &str = r#"version 1

board probe {
    size 20furlongs x 20mm
    layers 2
}
"#;

/// Where a substring starts, as a zero-based LSP line and character.
///
/// Written this way so a hover position survives an edit to the board above -
/// a hard-coded line number would make this test fail for the wrong reason.
fn position_of(source: &str, needle: &str) -> (u32, u32) {
    let offset = source.find(needle).expect("the needle is in the source");
    let before = &source[..offset];
    let line = before.matches('\n').count() as u32;
    let column = before.rsplit('\n').next().map_or(0, str::len) as u32;
    (line, column)
}

/// One end of an LSP connection: the child process and its message stream.
struct Server {
    child: Child,
    stdin: ChildStdin,
    messages: Receiver<Value>,
    next_id: i64,
}

impl Server {
    fn start() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_cypcb-lsp"))
            .arg("--stdio")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("the language server binary runs");

        let stdin = child.stdin.take().expect("stdin is piped");
        let stdout = child.stdout.take().expect("stdout is piped");

        // A reader thread, so a server that answers nothing times out here
        // instead of hanging the suite.
        let (tx, messages) = channel();
        thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            while let Some(message) = read_message(&mut reader) {
                if tx.send(message).is_err() {
                    break;
                }
            }
        });

        Server {
            child,
            stdin,
            messages,
            next_id: 1,
        }
    }

    fn send(&mut self, message: Value) {
        let body = serde_json::to_string(&message).expect("the message serializes");
        write!(self.stdin, "Content-Length: {}\r\n\r\n{}", body.len(), body)
            .expect("the server is still listening");
        self.stdin.flush().expect("the write reaches the server");
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        self.wait_for(&format!("a response to {method}"), |message| {
            message.get("id").and_then(Value::as_i64) == Some(id)
        })
    }

    fn notify(&mut self, method: &str, params: Value) {
        self.send(json!({"jsonrpc": "2.0", "method": method, "params": params}));
    }

    /// Read messages until one matches, or give up after five seconds.
    fn wait_for(&self, what: &str, matches: impl Fn(&Value) -> bool) -> Value {
        let deadline = Duration::from_secs(5);
        let mut seen: Vec<String> = Vec::new();
        loop {
            match self.messages.recv_timeout(deadline) {
                Ok(message) => {
                    if matches(&message) {
                        return message;
                    }
                    seen.push(
                        message
                            .get("method")
                            .and_then(Value::as_str)
                            .unwrap_or("(a response)")
                            .to_string(),
                    );
                }
                Err(RecvTimeoutError::Timeout) => {
                    panic!("waited 5s for {what}; the server sent {seen:?}")
                }
                Err(RecvTimeoutError::Disconnected) => {
                    panic!("the server exited before sending {what}; it sent {seen:?}")
                }
            }
        }
    }

    fn initialize(&mut self) -> Value {
        let result = self.request(
            "initialize",
            json!({"capabilities": {}, "processId": Value::Null, "rootUri": Value::Null}),
        );
        self.notify("initialized", json!({}));
        result
    }

    fn open(&mut self, uri: &str, text: &str) {
        self.notify(
            "textDocument/didOpen",
            json!({"textDocument": {
                "uri": uri,
                "languageId": "cypcb",
                "version": 1,
                "text": text,
            }}),
        );
    }

    fn diagnostics_for(&self, uri: &str) -> Vec<Value> {
        let message = self.wait_for(&format!("diagnostics for {uri}"), |message| {
            message.get("method").and_then(Value::as_str) == Some("textDocument/publishDiagnostics")
                && message.pointer("/params/uri").and_then(Value::as_str) == Some(uri)
        });
        message
            .pointer("/params/diagnostics")
            .and_then(Value::as_array)
            .cloned()
            .expect("a diagnostics notification carries a list")
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Read one `Content-Length`-framed JSON-RPC message, or `None` at end of stream.
fn read_message(reader: &mut BufReader<impl Read>) -> Option<Value> {
    let mut length = None;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some(value) = line.strip_prefix("Content-Length: ") {
            length = value.parse::<usize>().ok();
        }
    }

    let mut body = vec![0u8; length?];
    reader.read_exact(&mut body).ok()?;
    serde_json::from_slice(&body).ok()
}

#[test]
fn it_starts_and_says_what_it_can_do() {
    let mut server = Server::start();
    let result = server.initialize();

    assert_eq!(
        result
            .pointer("/result/serverInfo/name")
            .and_then(Value::as_str),
        Some("cypcb-lsp"),
        "the server has to name itself in its initialize result: {result}"
    );
    // Each of these is a request the editor will only send if the server
    // advertises it, and each has an implementation in this crate.
    for capability in ["hoverProvider", "completionProvider", "definitionProvider"] {
        assert!(
            result
                .pointer(&format!("/result/capabilities/{capability}"))
                .is_some(),
            "{capability} is implemented and has to be advertised: {result}"
        );
    }
}

#[test]
fn a_file_it_cannot_read_comes_back_as_a_diagnostic() {
    let uri = "file:///virtual/lsp-probe/broken.cypcb";
    let mut server = Server::start();
    server.initialize();
    server.open(uri, BROKEN);

    let diagnostics = server.diagnostics_for(uri);
    assert!(
        !diagnostics.is_empty(),
        "a board measured in furlongs has to produce a diagnostic"
    );

    let (line, _) = position_of(BROKEN, "20furlongs");
    let on_that_line = diagnostics
        .iter()
        .any(|d| d.pointer("/range/start/line").and_then(Value::as_u64) == Some(u64::from(line)));
    assert!(
        on_that_line,
        "the diagnostic has to point at line {line}, where the bad unit is: {diagnostics:?}"
    );
    assert!(
        diagnostics
            .iter()
            .all(|d| d.get("severity").is_some() && d.get("message").is_some()),
        "an editor draws severity and message, so both have to be there: {diagnostics:?}"
    );
}

#[test]
fn a_board_being_written_is_not_an_error() {
    // The other half: a server that answered "broken" to everything would pass
    // the test above. This board parses; its pins are unconnected and unrouted
    // because that is what a board looks like while somebody is writing it,
    // and every one of those came back at LSP severity 1 - an error.
    let uri = "file:///virtual/lsp-probe/good.cypcb";
    let mut server = Server::start();
    server.initialize();
    server.open(uri, BOARD);

    let diagnostics = server.diagnostics_for(uri);
    let errors: Vec<&Value> = diagnostics
        .iter()
        .filter(|d| d.get("severity").and_then(Value::as_u64) == Some(1))
        .collect();
    assert!(
        errors.is_empty(),
        "an unfinished board has nothing an editor should paint red: {errors:?}"
    );
    assert!(
        !diagnostics.is_empty(),
        "the unconnected pins still have to be reported, as warnings"
    );
}

#[test]
fn a_diagnostic_points_at_the_part_it_names() {
    // Every DRC violation arrived at line 0, character 0, because
    // `DrcViolation::source_span` is `None` in every constructor in the DRC
    // crate. Twenty parts meant twenty squiggles stacked on the first
    // character of the file, none of them where the part was written.
    let uri = "file:///virtual/lsp-probe/where.cypcb";
    let mut server = Server::start();
    server.initialize();
    server.open(uri, BOARD);

    let diagnostics = server.diagnostics_for(uri);
    let (r2_line, _) = position_of(BOARD, "R2 resistor");

    let about_r2: Vec<&Value> = diagnostics
        .iter()
        .filter(|d| {
            d.get("message")
                .and_then(Value::as_str)
                .is_some_and(|m| m.contains("R2."))
        })
        .collect();
    assert!(
        !about_r2.is_empty(),
        "R2 has an unconnected pin, so something has to say so: {diagnostics:?}"
    );
    for diagnostic in &about_r2 {
        assert_eq!(
            diagnostic
                .pointer("/range/start/line")
                .and_then(Value::as_u64),
            Some(u64::from(r2_line)),
            "a diagnostic about R2 belongs on the line R2 is declared: {diagnostic}"
        );
    }
}

#[test]
fn hovering_a_component_explains_it() {
    let uri = "file:///virtual/lsp-probe/hover.cypcb";
    let mut server = Server::start();
    server.initialize();
    server.open(uri, BOARD);

    // The refdes of the second part, which is where a user points to ask what
    // R2 is.
    let (line, character) = position_of(BOARD, "R2 resistor");
    let result = server.request(
        "textDocument/hover",
        json!({
            "textDocument": {"uri": uri},
            "position": {"line": line, "character": character},
        }),
    );

    let contents =
        serde_json::to_string(result.pointer("/result/contents").unwrap_or(&Value::Null))
            .expect("hover contents serialize");
    assert!(
        contents.contains("R2"),
        "hovering R2 has to say something about R2: {result}"
    );
    assert!(
        contents.contains("1k"),
        "and the value is the thing a reader is looking for: {result}"
    );
}

#[test]
fn completion_works_on_the_half_typed_line_it_is_asked_about() {
    // The moment completion is for: the user has typed `component R3 resistor "`
    // and stopped. The file does not parse - it cannot, the string is open and
    // the block is unclosed - and that is precisely when the list of footprints
    // is worth having.
    let uri = "file:///virtual/lsp-probe/typing.cypcb";
    let mut typing = String::from(BOARD);
    typing.push_str("\ncomponent R3 resistor \"");

    let mut server = Server::start();
    server.initialize();
    server.open(uri, &typing);

    let (line, character) = position_of(&typing, "component R3 resistor \"");
    let result = server.request(
        "textDocument/completion",
        json!({
            "textDocument": {"uri": uri},
            "position": {
                "line": line,
                "character": character + "component R3 resistor \"".len() as u32,
            },
        }),
    );

    let labels: Vec<&str> = result
        .pointer("/result")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("label").and_then(Value::as_str))
                .collect()
        })
        .unwrap_or_default();
    assert!(
        labels.contains(&"0402"),
        "a footprint list is what the cursor is asking for here: {result}"
    );
}

#[test]
fn completion_inside_a_block_nobody_has_closed_yet_offers_its_properties() {
    // The other half of the same defect: the block is open, so the parser has
    // no component to return, so the cursor read as "after the last
    // definition" and the editor offered `version` and `board` inside a
    // component body.
    let uri = "file:///virtual/lsp-probe/open-block.cypcb";
    let mut typing = String::from(BOARD);
    typing.push_str("\ncomponent R3 resistor \"0402\" {\n    ");

    let mut server = Server::start();
    server.initialize();
    server.open(uri, &typing);

    let line = typing.matches('\n').count() as u32;
    let result = server.request(
        "textDocument/completion",
        json!({
            "textDocument": {"uri": uri},
            "position": {"line": line, "character": 4},
        }),
    );

    let labels: Vec<&str> = result
        .pointer("/result")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("label").and_then(Value::as_str))
                .collect()
        })
        .unwrap_or_default();
    assert!(
        labels.contains(&"value") && labels.contains(&"at"),
        "inside a component body the properties are what to offer: {labels:?}"
    );
    assert!(
        !labels.contains(&"board"),
        "and `board` inside a component body pastes a board into it: {labels:?}"
    );
}

#[test]
fn going_to_a_definition_lands_on_the_part() {
    let uri = "file:///virtual/lsp-probe/goto.cypcb";
    let mut server = Server::start();
    server.initialize();
    server.open(uri, BOARD);

    // Ctrl-click on `R1.2` inside the net block. The answer has to be where R1
    // is declared, which is the whole point of the request.
    let (line, character) = position_of(BOARD, "R1.2");
    let result = server.request(
        "textDocument/definition",
        json!({
            "textDocument": {"uri": uri},
            "position": {"line": line, "character": character + 1},
        }),
    );

    let (declared_line, _) = position_of(BOARD, "component R1");
    assert_eq!(
        result.pointer("/result/uri").and_then(Value::as_str),
        Some(uri),
        "the definition is in the same file: {result}"
    );
    assert_eq!(
        result
            .pointer("/result/range/start/line")
            .and_then(Value::as_u64),
        Some(u64::from(declared_line)),
        "R1.2 has to lead to the line `component R1` is on: {result}"
    );
}

#[test]
fn completing_a_footprint_offers_the_library() {
    let uri = "file:///virtual/lsp-probe/complete.cypcb";
    let mut server = Server::start();
    server.initialize();
    server.open(uri, BOARD);

    // Inside the quoted footprint of R1, where a user asks what they may type.
    let (line, character) = position_of(BOARD, "0402\"");
    let result = server.request(
        "textDocument/completion",
        json!({
            "textDocument": {"uri": uri},
            "position": {"line": line, "character": character},
        }),
    );

    let items = result
        .pointer("/result")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    assert!(
        !items.is_empty(),
        "the built-in footprint library is not empty, so neither is this: {result}"
    );

    let labels: Vec<&str> = items
        .iter()
        .filter_map(|item| item.get("label").and_then(Value::as_str))
        .collect();
    assert!(
        labels.contains(&"0402"),
        "the library ships a 0402 and the list has to carry it: {labels:?}"
    );
    assert!(
        items
            .iter()
            .all(|item| item.get("kind").is_some() && item.get("detail").is_some()),
        "an editor sorts by kind and shows the detail, so both have to be there: {items:?}"
    );
}
