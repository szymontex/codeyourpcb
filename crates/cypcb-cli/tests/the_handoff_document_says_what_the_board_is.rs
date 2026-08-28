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

/// Export with the handoff document and hand back what the run said.
fn document_and_run(board: &str, out: &Path) -> (String, String) {
    let result = cypcb()
        .arg("export")
        .arg(example(board))
        .arg("-o")
        .arg(out)
        .arg("--ipc2581")
        .output()
        .expect("the binary runs");
    assert!(result.status.success(), "the export failed");
    let name = board.trim_end_matches(".cypcb");
    let xml = std::fs::read_to_string(out.join("handoff").join(format!("{name}.xml")))
        .expect("the document is readable");
    (xml, String::from_utf8_lossy(&result.stderr).to_string())
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

#[test]
fn every_pad_points_at_a_shape_the_document_defines() {
    // A pad in this format is a reference: the shape lives once in the
    // dictionary at the top and every placement names it. A reference with
    // nothing behind it is a pad a fabricator cannot make.
    let xml = document("usb-diff-pair.cypcb", &scratch("pads"));
    let tags = tags(&xml);

    let defined: Vec<String> = tags
        .iter()
        .filter(|tag| tag.name == "EntryStandard")
        .map(|tag| attribute(tag, "id"))
        .collect();
    let used: Vec<String> = tags
        .iter()
        .filter(|tag| tag.name == "StandardPrimitiveRef")
        .map(|tag| attribute(tag, "id"))
        .collect();

    assert!(!used.is_empty(), "the board has pads:\n{xml}");
    for id in &used {
        assert!(
            defined.contains(id),
            "`{id}` is placed and never defined: {defined:?}"
        );
    }
    // Sixteen placements, because every pad of this board goes through it and
    // therefore appears on both outer layers.
    assert_eq!(
        tags.iter().filter(|tag| tag.name == "Pad").count(),
        16,
        "eight through-hole pads, seen from both sides"
    );
}

#[test]
fn the_dictionary_comes_before_the_pads_that_use_it() {
    // The schema puts the dictionary inside `Content`, which is the first
    // section: a reader building shapes as it goes has them all before the
    // first placement.
    let xml = document("usb-diff-pair.cypcb", &scratch("dictionary"));
    let tags = tags(&xml);

    let dictionary = tags
        .iter()
        .find(|tag| tag.name == "DictionaryStandard")
        .expect("the board has shapes");
    assert_eq!(dictionary.path.join("/"), "IPC-2581/Content");
    assert_eq!(
        attribute(dictionary, "units"),
        "MILLIMETER",
        "the shapes are in the same units as everything else"
    );

    let first_entry = tags
        .iter()
        .position(|tag| tag.name == "EntryStandard")
        .expect("an entry");
    let first_pad = tags
        .iter()
        .position(|tag| tag.name == "Pad")
        .expect("a pad");
    assert!(first_entry < first_pad, "defined before it is used");
}

#[test]
fn a_pad_is_placed_where_the_part_puts_it() {
    // A pad's own position is inside its footprint; where it lands is that
    // turned by the part's rotation and moved to where the part sits. Writing
    // the footprint's own numbers would put every pad of a turned part in the
    // wrong place.
    let xml = document("usb-diff-pair.cypcb", &scratch("placement"));
    let tags = tags(&xml);

    let located: Vec<(String, String)> = tags
        .iter()
        .filter(|tag| tag.name == "Location" && tag.path.ends_with(&["Pad".to_string()]))
        .map(|tag| (attribute(tag, "x"), attribute(tag, "y")))
        .collect();
    assert!(
        located.contains(&("5.000".to_string(), "6.730".to_string())),
        "the first pad lands where the SVG and the Gerber put it: {located:?}"
    );

    // Rotation is a non-negative number in this format: a pad turned by -90
    // degrees is written as 270.
    for tag in tags.iter().filter(|tag| tag.name == "Xform") {
        let turn = attribute(tag, "rotation");
        assert!(
            !turn.starts_with('-'),
            "a turn is stated the way the format states it: {turn}"
        );
    }
}

#[test]
fn a_board_with_no_parts_says_nothing_about_shapes() {
    // The schema wants at least one `Set` inside a `LayerFeature`, so a layer
    // with nothing on it gets no section rather than an empty one - and a
    // board with no parts gets no dictionary either.
    let xml = document("curved-track.cypcb", &scratch("bare"));
    let tags = tags(&xml);

    assert!(
        !tags.iter().any(|tag| tag.name == "Pad"),
        "no pads for a board that has no parts:\n{xml}"
    );
    assert!(
        !tags.iter().any(|tag| tag.name == "DictionaryStandard"),
        "and no dictionary of shapes it does not use"
    );
    // This board does carry copper - two straight runs and a curve - so it has
    // a feature section for the layer they are on and none for the other. The
    // rule the schema fixes is that a section always holds at least one `Set`.
    let sections: Vec<&Tag> = tags
        .iter()
        .filter(|tag| tag.name == "LayerFeature")
        .collect();
    assert_eq!(sections.len(), 1, "one layer carries this board's copper");
    assert_eq!(attribute(sections[0], "layerRef"), "F_Cu");
}

#[test]
fn the_copper_between_the_pads_is_in_the_document_too() {
    // Gerber says which copper is there and nothing about what it connects.
    // This format's whole point beside Gerber is that a run of copper says
    // which net it belongs to, so a track is written inside a `Set` that
    // names one.
    let xml = document("curved-track.cypcb", &scratch("tracks"));
    let tags = tags(&xml);

    let lines: Vec<&Tag> = tags.iter().filter(|tag| tag.name == "Line").collect();
    assert_eq!(lines.len(), 2, "the two straight runs:\n{xml}");
    assert_eq!(
        attribute(lines[0], "startX"),
        "8.000",
        "drawn where the board draws them"
    );

    let sets: Vec<String> = tags
        .iter()
        .filter(|tag| tag.name == "Set" && tag.attributes.contains("net="))
        .map(|tag| attribute(tag, "net"))
        .collect();
    // Three runs of copper - two straight, one curved - and each one says
    // which net it is. Counting them matters: a filter over sets that name a
    // net is vacuously satisfied when no set names one, which is exactly what
    // a writer that dropped the attribute would produce.
    assert_eq!(
        sets,
        vec!["SIG".to_string(), "SIG".to_string(), "SIG".to_string()],
        "every run says which net it is"
    );
}

#[test]
fn a_width_is_named_once_and_pointed_at() {
    // The same shape of rule the pads follow: the width lives in a dictionary
    // at the top and each segment names it, so two tracks at 0.2mm are one
    // entry rather than two.
    let xml = document("curved-track.cypcb", &scratch("widths"));
    let tags = tags(&xml);

    let defined: Vec<String> = tags
        .iter()
        .filter(|tag| tag.name == "EntryLineDesc")
        .map(|tag| attribute(tag, "id"))
        .collect();
    // Named after the width itself, so two tracks at the same width share one
    // entry and a board with two widths cannot collapse them into one.
    assert_eq!(
        defined,
        vec!["line_0_250".to_string()],
        "one width on this board, named after itself"
    );

    let used: Vec<String> = tags
        .iter()
        .filter(|tag| tag.name == "LineDescRef")
        .map(|tag| attribute(tag, "id"))
        .collect();
    assert!(!used.is_empty(), "the copper names its width");
    for id in &used {
        assert!(defined.contains(id), "`{id}` is used and never defined");
    }

    let dictionary = tags
        .iter()
        .find(|tag| tag.name == "DictionaryLineDesc")
        .expect("the widths are declared");
    assert_eq!(dictionary.path.join("/"), "IPC-2581/Content");
    assert_eq!(attribute(dictionary, "units"), "MILLIMETER");
}

#[test]
fn a_curve_reaches_the_handoff_as_a_curve() {
    // The checker reads chords and the fabricator should not have to. This
    // format states an arc by its ends, its centre and which way it turns,
    // which is exactly what the model holds.
    let xml = document("curved-track.cypcb", &scratch("arc"));
    let tags = tags(&xml);

    let arcs: Vec<&Tag> = tags.iter().filter(|tag| tag.name == "Arc").collect();
    assert_eq!(arcs.len(), 1, "one curve, one arc:\n{xml}");
    let arc = arcs[0];
    assert_eq!(attribute(arc, "startX"), "12.000");
    assert_eq!(attribute(arc, "startY"), "6.000");
    assert_eq!(attribute(arc, "endX"), "8.000");
    assert_eq!(attribute(arc, "endY"), "10.000");
    assert_eq!(attribute(arc, "centerX"), "12.000");
    assert_eq!(attribute(arc, "centerY"), "10.000");
    assert_eq!(
        attribute(arc, "clockwise"),
        "true",
        "and it turns the way the board says"
    );
    // The chords the curve was flattened into are not in the file: twelve
    // little lines would be a curve nobody can edit as one.
    assert_eq!(
        tags.iter().filter(|tag| tag.name == "Line").count(),
        2,
        "only the two straight runs are lines"
    );
}

#[test]
fn a_via_is_copper_and_a_hole_and_the_document_says_both() {
    // A drill file carries the hole and says nothing about the copper around
    // it; a Gerber carries the ring and says nothing about the hole. The whole
    // reason this format exists is that one file says both.
    let xml = document("stitched-plane.cypcb", &scratch("vias"));
    let tags = tags(&xml);

    let via_sets: Vec<&Tag> = tags
        .iter()
        .filter(|tag| tag.name == "Set" && tag.attributes.contains("padUsage=\"VIA\""))
        .collect();
    assert_eq!(via_sets.len(), 2, "one set of vias per outer layer");

    let holes: Vec<&Tag> = tags.iter().filter(|tag| tag.name == "Hole").collect();
    assert_eq!(holes.len(), 32, "sixteen vias, seen from both layers");
    for hole in &holes {
        assert_eq!(
            attribute(hole, "platingStatus"),
            "VIA",
            "a via hole is not a mounting hole"
        );
        assert_eq!(attribute(hole, "diameter"), "0.300", "drilled at 0.3mm");
    }

    // The ring and the hole are at the same place: a hole beside its own
    // copper is a board nobody can build.
    let first_hole = holes[0];
    let placed_at = tags
        .iter()
        .filter(|tag| tag.name == "Location" && tag.path.ends_with(&["Pad".to_string()]))
        .map(|tag| (attribute(tag, "x"), attribute(tag, "y")))
        .collect::<Vec<_>>();
    assert!(
        placed_at.contains(&(attribute(first_hole, "x"), attribute(first_hole, "y"))),
        "the hole sits inside its own ring"
    );

    // And every shape the vias name is a shape the document defines.
    let defined: Vec<String> = tags
        .iter()
        .filter(|tag| tag.name == "EntryStandard")
        .map(|tag| attribute(tag, "id"))
        .collect();
    for used in tags
        .iter()
        .filter(|tag| tag.name == "StandardPrimitiveRef")
        .map(|tag| attribute(tag, "id"))
    {
        assert!(
            defined.contains(&used),
            "`{used}` is placed and never defined"
        );
    }
}

#[test]
fn a_pour_reaches_the_document_as_the_copper_it_became() {
    // A pour is not the rectangle the design asked for: it is copper cut
    // around every pad, track and clearance on the layer. The file should say
    // what the filler laid down, which is what the checker measured.
    let xml = document("stitched-plane.cypcb", &scratch("pour"));
    let tags = tags(&xml);

    let contours: Vec<&Tag> = tags.iter().filter(|tag| tag.name == "Contour").collect();
    assert!(!contours.is_empty(), "the pour is in the document:\n{xml}");

    // Every piece closes: an open contour is copper with a gap in its edge.
    let begins: Vec<(String, String)> = tags
        .iter()
        .filter(|tag| tag.name == "PolyBegin" && tag.path.contains(&"Contour".to_string()))
        .map(|tag| (attribute(tag, "x"), attribute(tag, "y")))
        .collect();
    assert_eq!(begins.len(), contours.len(), "one polygon per contour");

    let pour_sets: Vec<String> = tags
        .iter()
        .filter(|tag| {
            tag.name == "Set"
                && tag.attributes.contains("net=")
                && !tag.attributes.contains("padUsage")
        })
        .map(|tag| attribute(tag, "net"))
        .collect();
    assert!(
        pour_sets.iter().any(|net| net == "GND"),
        "and the pour says which net it is: {pour_sets:?}"
    );
}

#[test]
fn a_board_with_neither_says_neither() {
    let xml = document("curved-track.cypcb", &scratch("neither"));
    let tags = tags(&xml);
    assert!(
        !tags.iter().any(|tag| tag.name == "Hole"),
        "no holes in a board that drills none"
    );
    assert!(
        !tags.iter().any(|tag| tag.name == "Contour"),
        "and no pour in a board that pours none"
    );
}

#[test]
fn the_stack_the_board_states_is_in_the_document() {
    // Every fact in a stackup travels beside a Gerber today as a note in an
    // email. This is the section that carries it: what each layer is made of,
    // how thick it is, and what the board comes to.
    let (xml, _) = document_and_run("blind-via.cypcb", &scratch("stack"));
    let tags = tags(&xml);

    let stack = tags
        .iter()
        .find(|tag| tag.name == "Stackup")
        .expect("the design states a stack");
    assert_eq!(stack.path.join("/"), "IPC-2581/Ecad/CadData");
    assert_eq!(
        attribute(stack, "whereMeasured"),
        "LAMINATE",
        "measured where a fabricator measures"
    );

    let entries: Vec<(String, String, String)> = tags
        .iter()
        .filter(|tag| tag.name == "StackupLayer")
        .map(|tag| {
            (
                attribute(tag, "layerOrGroupRef"),
                attribute(tag, "materialType"),
                attribute(tag, "thickness"),
            )
        })
        .collect();
    assert!(
        entries.len() >= 5,
        "a four-layer board is many entries: {entries:?}"
    );
    assert!(
        entries
            .iter()
            .any(|(name, material, _)| name == "F_Cu" && material == "COPPER"),
        "copper says it is copper: {entries:?}"
    );
    assert!(
        entries.iter().any(|(_, material, _)| material == "CORE"),
        "and the core says it is a core: {entries:?}"
    );

    // The overall thickness is the sum of the entries, not a figure of its own.
    let overall: f64 = attribute(stack, "overallThickness")
        .parse()
        .expect("a number");
    let summed: f64 = entries
        .iter()
        .map(|(_, _, thickness)| thickness.parse::<f64>().expect("a number"))
        .sum();
    // Each printed thickness is rounded to a micron, so the printed total can
    // sit a micron per entry away from the sum of the printed parts. The
    // document's own number is the sum of the unrounded ones.
    assert!(
        (overall - summed).abs() <= 0.001 * entries.len() as f64,
        "the board comes to what its layers come to: {overall} against {summed}"
    );
}

#[test]
fn the_run_says_the_tolerance_is_not_the_boards_own() {
    // A tolerance is a number a fabricator holds the board to, and this
    // language has nowhere to state one. Writing zero silently would be a
    // reader taking zero for a promise.
    let (xml, said) = document_and_run("blind-via.cypcb", &scratch("tolerance"));
    assert!(
        xml.contains("tolPlus=\"0\" tolMinus=\"0\""),
        "the document states zero:\n{xml}"
    );
    assert!(
        said.contains("states no thickness tolerance"),
        "and the run says why: {said}"
    );
}

#[test]
fn a_board_with_no_stack_gets_no_stack_section() {
    let (xml, said) = document_and_run("curved-track.cypcb", &scratch("nostack"));
    assert!(
        !xml.contains("<Stackup "),
        "a board that states no stack has none written for it:\n{xml}"
    );
    assert!(
        !said.contains("states no thickness tolerance"),
        "and nothing is said about a tolerance it never needed: {said}"
    );
}
