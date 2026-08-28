//! `--ipc2581` writes the handoff document a modern fabricator reads.
//!
//! `cargo test -p cypcb-cli --test the_handoff_document_says_what_the_board_is`
//!
//! Gerber says what to etch and nothing about what the board is: which layer
//! is which, what the outline is, what the stackup was meant to be. IPC-2581
//! carries all of it in one XML document, and row 10 of the KiCad parity audit
//! is that this project wrote none. This is the frame - the sections every
//! feature hangs off - and it is worth being right before anything hangs off
//! it, because the schema is ordered: a document whose sections are out of
//! order is rejected even when every fact in it is true.
//!
//! The reader below is a tag scanner rather than a schema validator. It reads
//! the file back as a tree, which is what a test can do without pulling a
//! validator in; what it cannot check is the schema itself, and that is stated
//! rather than implied.

use std::path::{Path, PathBuf};
use std::process::Command;

fn cypcb() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cypcb"))
}

fn example(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .join("examples")
        .join(name)
}

fn scratch(who: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cypcb-ipc2581-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// Export with the handoff document and read it back.
fn document(board: &str, out: &Path) -> String {
    let status = cypcb()
        .arg("export")
        .arg(example(board))
        .arg("-o")
        .arg(out)
        .arg("--ipc2581")
        .status()
        .expect("the binary runs");
    assert!(status.success(), "the export failed");
    let name = board.trim_end_matches(".cypcb");
    std::fs::read_to_string(out.join("handoff").join(format!("{name}.xml")))
        .expect("the document is readable")
}

/// One tag as the scanner sees it: its name, and the path it sits under.
struct Tag {
    name: String,
    path: Vec<String>,
    attributes: String,
}

/// Read the document back as a tree, and say so when it is not one.
fn tags(xml: &str) -> Vec<Tag> {
    let mut open: Vec<String> = Vec::new();
    let mut found = Vec::new();
    let mut rest = xml;
    while let Some(at) = rest.find('<') {
        rest = &rest[at + 1..];
        let end = rest.find('>').expect("every tag closes");
        let body = &rest[..end];
        rest = &rest[end + 1..];
        if body.starts_with('?') {
            continue;
        }
        if let Some(name) = body.strip_prefix('/') {
            let opened = open.pop().expect("a closing tag closes something");
            assert_eq!(opened, name.trim(), "tags close in the order they opened");
            continue;
        }
        let self_closing = body.ends_with('/');
        let body = body.trim_end_matches('/');
        let (name, attributes) = match body.find(char::is_whitespace) {
            Some(space) => (&body[..space], body[space..].trim()),
            None => (body, ""),
        };
        found.push(Tag {
            name: name.to_string(),
            path: open.clone(),
            attributes: attributes.to_string(),
        });
        if !self_closing {
            open.push(name.to_string());
        }
    }
    assert!(open.is_empty(), "every tag that opened is closed: {open:?}");
    found
}

/// The value of one attribute of one tag.
fn attribute(tag: &Tag, name: &str) -> String {
    let at = tag
        .attributes
        .find(&format!("{name}=\""))
        .unwrap_or_else(|| panic!("<{}> states {name}: {}", tag.name, tag.attributes));
    let rest = &tag.attributes[at + name.len() + 2..];
    rest[..rest.find('"').expect("the value closes")].to_string()
}

#[test]
fn the_document_is_a_tree_and_says_which_standard_it_is() {
    let xml = document("curved-track.cypcb", &scratch("shape"));
    let tags = tags(&xml);

    let root = &tags[0];
    assert_eq!(root.name, "IPC-2581", "the root is the standard's own name");
    assert_eq!(attribute(root, "revision"), "C", "and states its revision");

    let header = tags
        .iter()
        .find(|tag| tag.name == "CadHeader")
        .expect("the document states its units");
    assert_eq!(
        attribute(header, "units"),
        "MILLIMETER",
        "a number with no unit is a board at 25.4 times the size"
    );
}

#[test]
fn the_sections_are_in_the_order_the_schema_fixes() {
    // The schema is a sequence, not a set: a validator rejects a document
    // whose sections are out of order even when every fact in it is true.
    let xml = document("curved-track.cypcb", &scratch("order"));
    let tags = tags(&xml);

    let index = |name: &str| {
        tags.iter()
            .position(|tag| tag.name == name)
            .unwrap_or_else(|| panic!("the document has a {name}"))
    };
    assert!(index("Content") < index("LogisticHeader"));
    assert!(index("LogisticHeader") < index("HistoryRecord"));
    assert!(index("HistoryRecord") < index("Ecad"));
    assert!(index("CadHeader") < index("CadData"));
    assert!(
        index("Layer") < index("Step"),
        "every layer before the step"
    );

    // And the sections really are nested where the schema puts them rather
    // than merely written in that order.
    let under = |name: &str| {
        tags.iter()
            .find(|tag| tag.name == name)
            .map(|tag| tag.path.join("/"))
            .unwrap_or_default()
    };
    assert_eq!(under("CadHeader"), "IPC-2581/Ecad");
    assert_eq!(under("Layer"), "IPC-2581/Ecad/CadData");
    assert_eq!(
        under("PolyBegin"),
        "IPC-2581/Ecad/CadData/Step/Profile/Polygon"
    );
}

#[test]
fn every_layer_the_contents_name_is_a_layer_the_board_has() {
    // `Content` is the index of the file: a reader takes it as the list of
    // what is inside, and a name in it with nothing behind it is a layer a
    // fabricator will ask about.
    let xml = document("curved-track.cypcb", &scratch("layers"));
    let tags = tags(&xml);

    let named: Vec<String> = tags
        .iter()
        .filter(|tag| tag.name == "LayerRef")
        .map(|tag| attribute(tag, "name"))
        .collect();
    let present: Vec<String> = tags
        .iter()
        .filter(|tag| tag.name == "Layer")
        .map(|tag| attribute(tag, "name"))
        .collect();

    assert_eq!(named, present, "the index and the layers agree");
    assert_eq!(
        named,
        vec![
            "F_Cu".to_string(),
            "B_Cu".to_string(),
            "Edge_Cuts".to_string()
        ],
        "a two-layer board with an outline"
    );
}

#[test]
fn the_profile_is_the_boards_own_outline_and_it_closes() {
    // A profile is a closed contour. One that does not come back to where it
    // began is a board with a gap in its edge, which a fabricator either
    // refuses or guesses at.
    let xml = document("curved-track.cypcb", &scratch("profile"));
    let tags = tags(&xml);

    let begin = tags
        .iter()
        .find(|tag| tag.name == "PolyBegin")
        .expect("the profile starts");
    let steps: Vec<&Tag> = tags
        .iter()
        .filter(|tag| tag.name == "PolyStepSegment")
        .collect();

    assert_eq!(
        (attribute(begin, "x"), attribute(begin, "y")),
        ("0.000".to_string(), "0.000".to_string())
    );
    let last = steps.last().expect("the profile has segments");
    assert_eq!(
        (attribute(last, "x"), attribute(last, "y")),
        (attribute(begin, "x"), attribute(begin, "y")),
        "the contour closes on where it began"
    );
    // 24mm by 20mm, which is what the example states.
    assert!(
        steps
            .iter()
            .any(|step| attribute(step, "x") == "24.000" && attribute(step, "y") == "20.000"),
        "and it is the board's own size"
    );
}

#[test]
fn an_inner_layer_is_written_as_one() {
    // Four layers means two the outside world never sees, and a document that
    // called them TOP would have a fabricator build the board inside out.
    let xml = document("v2-constraints.cypcb", &scratch("inner"));
    let tags = tags(&xml);
    let layers: Vec<(String, String)> = tags
        .iter()
        .filter(|tag| tag.name == "Layer")
        .map(|tag| (attribute(tag, "name"), attribute(tag, "side")))
        .collect();

    if layers.len() > 3 {
        assert!(
            layers
                .iter()
                .any(|(name, side)| name == "In1_Cu" && side == "INTERNAL"),
            "the first inner layer says it is inside: {layers:?}"
        );
    }
    assert!(
        layers
            .iter()
            .any(|(name, side)| name == "F_Cu" && side == "TOP"),
        "and the top says it is the top: {layers:?}"
    );
    assert!(
        layers
            .iter()
            .any(|(name, side)| name == "B_Cu" && side == "BOTTOM"),
        "and the bottom the bottom: {layers:?}"
    );
}

#[test]
fn a_board_that_does_not_ask_gets_no_handoff() {
    let out = scratch("silent");
    let status = cypcb()
        .arg("export")
        .arg(example("curved-track.cypcb"))
        .arg("-o")
        .arg(&out)
        .status()
        .expect("the binary runs");
    assert!(status.success(), "the export failed");
    assert!(
        !out.join("handoff").exists(),
        "the file set a house receives is unchanged unless the document is asked for"
    );
}
