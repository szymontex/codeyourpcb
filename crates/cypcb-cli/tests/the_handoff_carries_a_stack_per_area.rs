//! A rigid-flex board is not one stack, and the handoff document says so.
//!
//! `cargo test -p cypcb-cli --test the_handoff_carries_a_stack_per_area`
//!
//! `stiffener 0.2mm covers connector_end` states a layer over one end of the
//! board. Every one of those layers used to be written into the single
//! `StackupGroup` the document carried, so the file ordered a stiffener across
//! the whole panel - a board built to it would not fold.
//!
//! IPC-2581 Revision C carries several stackup groups and ties each to a zone
//! of the board. The groups are written from the design's own areas; the tie
//! is not, because this project has not read the element that carries it, and
//! an invented boundary reference reads to a fabricator's tool as a link to
//! something that is not there. So the export says what it left out.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

/// Export a board and hand back the handoff document and what was said while
/// writing it.
fn handoff(source: &str, who: &str) -> (String, String) {
    let dir = cypcb_fixtures::scratch_dir(&format!("cypcb-stack-per-area-{who}"));

    let board = dir.join("board.cypcb");
    std::fs::write(&board, source).expect("the board is written");
    let out = dir.join("out");

    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .args([
            "export",
            board.to_str().expect("a path that is text"),
            "-o",
            out.to_str().expect("a path that is text"),
            "--ipc2581",
            "--force",
        ])
        .current_dir(repo_root())
        .output()
        .expect("the binary runs");
    assert!(
        output.status.success(),
        "export failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let document = std::fs::read_to_string(out.join("handoff").join("board.xml"))
        .expect("the handoff document was written");
    (
        document,
        String::from_utf8_lossy(&output.stderr).to_string(),
    )
}

/// The ribbon, one rigid end, and three layers that stop at one or the other.
const RIGID_FLEX: &str = r#"version 1

board wearable {
    size 60mm x 16mm
    layers 2
    stackup {
        coverlay 0.025mm material "Kapton" covers bend
        copper 0.0175mm
        core 0.05mm material "Kapton" dk 3.4
        copper 0.0175mm
        stiffener 0.2mm material "FR4" covers connector_end
    }
}

flex bend {
    bounds 22mm, 0mm to 38mm, 16mm
    layer all
}

region connector_end {
    bounds 0mm, 0mm to 22mm, 16mm
    layer all
}
"#;

/// Export the same board the ordinary way - Gerbers, drill, job file - and
/// hand back where it went and what was said while writing it.
fn gerbers(source: &str, who: &str) -> (cypcb_fixtures::ScratchDir, String) {
    let dir = cypcb_fixtures::scratch_dir(&format!("cypcb-stack-per-area-{who}"));

    let board = dir.join("board.cypcb");
    std::fs::write(&board, source).expect("the board is written");
    let out = dir.join("out");

    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .args([
            "export",
            board.to_str().expect("a path that is text"),
            "-o",
            out.to_str().expect("a path that is text"),
            "--force",
        ])
        .current_dir(repo_root())
        .output()
        .expect("the binary runs");
    assert!(
        output.status.success(),
        "export failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let said = String::from_utf8_lossy(&output.stderr).to_string();
    (dir, said)
}

fn group(document: &str, name: &str) -> String {
    let start = document
        .find(&format!("<StackupGroup name=\"{name}\""))
        .unwrap_or_else(|| panic!("no group called {name} in:\n{document}"));
    let end = document[start..]
        .find("</StackupGroup>")
        .expect("the group closes");
    document[start..start + end].to_string()
}

#[test]
fn every_area_a_layer_stops_at_gets_its_own_group() {
    let (document, _) = handoff(RIGID_FLEX, "areas");

    // The whole panel: everything, 0.310mm of it.
    let whole = group(&document, "wearable");
    assert!(whole.contains("thickness=\"0.310\""), "{whole}");
    assert_eq!(whole.matches("<StackupLayer").count(), 5);

    // The ribbon: no stiffener, so 0.110mm - the foils, the core and the film
    // over them, which is the whole of what bends.
    let bend = group(&document, "wearable_bend");
    assert!(bend.contains("thickness=\"0.110\""), "{bend}");
    assert!(
        !bend.contains("STIFFENER"),
        "a stiffener through the ribbon is a ribbon that does not bend:\n{bend}"
    );

    // The end J1 sits on: the stiffener is there and the coverlay is not,
    // because a coverlay is the film over the bend and this end takes mask.
    let end = group(&document, "wearable_connector_end");
    assert!(end.contains("thickness=\"0.285\""), "{end}");
    assert!(end.contains("STIFFENER"), "{end}");
    assert!(
        !end.contains("COVERLAY"),
        "the coverlay says `covers bend`, so it is not over this end:\n{end}"
    );
}

#[test]
fn a_layer_keeps_one_name_in_every_group_it_is_in() {
    let (document, _) = handoff(RIGID_FLEX, "names");

    // The two copper foils are unnamed in the design, so the writer names them
    // after their place in the board's stack. Numbering each group from zero
    // would give the same foil two names and a reader two foils.
    for name in ["wearable", "wearable_bend", "wearable_connector_end"] {
        let text = group(&document, name);
        assert!(
            text.contains("layerOrGroupRef=\"copper_1\"")
                && text.contains("layerOrGroupRef=\"copper_3\""),
            "{name} names the foils by their place in the whole stack:\n{text}"
        );
    }
}

#[test]
fn the_link_the_document_does_not_carry_is_named() {
    let (_, said) = handoff(RIGID_FLEX, "warning");
    assert!(
        said.contains("stackup group per area (bend, connector_end)"),
        "the areas are named:\n{said}"
    );
    assert!(
        said.contains("not the boundary each group belongs to"),
        "and so is the thing that is missing:\n{said}"
    );
}

#[test]
fn a_board_whose_layers_stop_nowhere_gets_one_group_and_no_warning() {
    let plain = RIGID_FLEX
        .replace(" covers bend", "")
        .replace(" covers connector_end", "");
    let (document, said) = handoff(&plain, "plain");

    assert_eq!(
        document.matches("<StackupGroup").count(),
        1,
        "one stack, one group"
    );
    assert!(
        !said.contains("stackup group per area"),
        "and nothing to say about areas:\n{said}"
    );
}

#[test]
fn the_job_file_says_it_is_flattening_the_stack() {
    // `MaterialStackup` is one array of one build. The specification wants it
    // complete - all layers of the PCB and only those materials - and has no
    // notion of an area, so a rigid-flex design reads there as a board pressed
    // the same way end to end, and a fabricator quoting from it quotes the
    // wrong board.
    let (dir, said) = gerbers(RIGID_FLEX, "job-file");
    assert!(
        said.contains("the job file states one stack and this design states a different one per area (bend, connector_end)"),
        "the job file names the areas it cannot hold:\n{said}"
    );
    assert!(
        said.contains("--ipc2581"),
        "and points at the document that can:\n{said}"
    );

    // The warning is about a file, and until now only the warning was read. The
    // file says the same thing, and it says it positively: one `MaterialStackup`
    // array, nine layers in it, and not one of the design's two area names
    // anywhere in the document. The count comes first - "the job file does not
    // mention an area" is true of an empty file, of a truncated write, and of a
    // file that was never created.
    let written = job_file(&dir);
    assert_eq!(
        written.matches("\"MaterialStackup\"").count(),
        1,
        "the job file holds exactly one stack, which is the whole difficulty:\n{written}"
    );
    let layers = written.matches("\"Type\"").count();
    assert!(
        layers >= 5,
        "and that one stack really describes a build - {layers} layers in it"
    );
    for area in ["bend", "connector_end"] {
        assert!(
            !written.contains(area),
            "the design states a build for `{area}` and the job file has no way to \
             carry it, so the name is not in the file either"
        );
    }

    // A board whose layers stop nowhere is described by the job file exactly,
    // and says nothing about areas.
    let plain = RIGID_FLEX
        .replace(" covers bend", "")
        .replace(" covers connector_end", "");
    let (_, said) = gerbers(&plain, "job-file-plain");
    assert!(
        !said.contains("states a different one per area"),
        "one stack, nothing to say:\n{said}"
    );
}

/// The job file a run of `gerbers` left behind, read from the directory that
/// run wrote into.
fn job_file(dir: &std::path::Path) -> String {
    let out = dir.join("out");
    let path = std::fs::read_dir(&out)
        .expect("the export wrote its directory")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.extension().and_then(|e| e.to_str()) == Some("gbrjob"))
        .expect("the export wrote a job file");
    std::fs::read_to_string(path).expect("the job file is readable")
}
