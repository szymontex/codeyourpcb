//! A footprint with no pads is a part like any other: it loads, it is checked
//! and it is exported.
//!
//! `cargo test -p cypcb-render --features native --test a_footprint_with_no_pads_is_placed`
//!
//! A logo, a mark or a fiducial drawn as ink has artwork and no copper, and
//! KiCad's own libraries carry such footprints. The browser's
//! `register_footprint` builds its courtyard from the extremes of the pads,
//! and with none the extremes are the ends of the number line: a margin added
//! to `i64::MAX` is a courtyard nobody drew. Placing it overflowed in a debug
//! build (`attempt to add with overflow`, `place_pad`), and in a release build
//! the wrapped box put an `edge-clearance` violation, 0.00mm actual, on a part
//! in the middle of the board.
//!
//! Every other reader was measured on 2026-10-12 and builds such a footprint
//! without trouble: the KiCad footprint and board readers, the language, and
//! the built-in library (none of its footprints is without pads).
//! They are held here too, so the next reader that builds one from extremes
//! is compared with them.

use cypcb_export::{run_export, ExportJob};
use cypcb_render::PcbEngine;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::BoardWorld;

/// A circle of ink, 1mm in radius: a mark.
const MARK: &str = r#"[{"type":"circle","cx":0,"cy":0,"radius":1000000,"width":150000}]"#;

/// One 1mm square pad at the origin, for the controls.
const PAD: &str = r#"[{"number":"1","x_nm":0,"y_nm":0,"width_nm":1000000,
                       "height_nm":1000000,"shape":"rect","layer_mask":1,"drill_nm":null}]"#;

fn board(footprint: &str) -> String {
    format!(
        "version 1\n\nboard b {{\n    size 40mm x 20mm\n    layers 2\n}}\n\n\
         component G1 resistor \"{footprint}\" {{\n    at 20mm, 10mm\n}}\n"
    )
}

/// The snapshot and its violations after the browser registers a footprint
/// and loads a board with one part of it.
fn loaded(pads: &str, silk: &str) -> serde_json::Value {
    let mut engine = PcbEngine::new();
    assert_eq!(engine.register_footprint("MARK", pads, silk), "");
    assert_eq!(engine.load_source(&board("MARK")), "");
    serde_json::from_str(&engine.get_snapshot()).expect("the snapshot is JSON")
}

fn violations(snapshot: &serde_json::Value) -> Vec<String> {
    snapshot["violations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["message"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn a_footprint_the_browser_registers_with_no_pads_is_placed() {
    let snapshot = loaded("[]", "[]");
    let part = &snapshot["components"][0];
    assert_eq!(part["refdes"], "G1");
    assert_eq!(part["footprint"], "MARK");
    assert_eq!(part["pads"].as_array().unwrap().len(), 0);
    // In the middle of a 40mm board there is nothing to be clear of.
    assert_eq!(violations(&snapshot), Vec::<String>::new());
}

#[test]
fn a_mark_with_artwork_and_no_pads_arrives_with_its_artwork() {
    let snapshot = loaded("[]", MARK);
    assert_eq!(violations(&snapshot), Vec::<String>::new());
    assert_eq!(
        snapshot["components"][0]["silk"].as_array().unwrap().len(),
        1
    );
}

#[test]
fn control_a_footprint_with_a_pad_is_placed_as_before() {
    let snapshot = loaded(PAD, "[]");
    assert_eq!(
        snapshot["components"][0]["pads"].as_array().unwrap().len(),
        1
    );
    // The one thing wrong with it: its pad is on no net.
    assert_eq!(violations(&snapshot), ["Unconnected pin: G1.1"]);
}

/// The same board through the world and every writer, the way `export` runs.
fn through_the_writers(source: &str, mut library: FootprintLibrary) {
    let parsed = cypcb_parser::parse(source);
    assert!(parsed.errors.is_empty(), "the board does not parse");
    let mut world = BoardWorld::new();
    let synced = cypcb_world::sync_ast_to_world(&parsed.value, source, &mut world, &mut library);
    assert!(synced.errors.is_empty(), "the board does not load");
    cypcb_drc::run_drc(&mut world, &cypcb_drc::DesignRules::default());
    // One directory per call: the tests run side by side in one process.
    static CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let call = CALLS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("pad-less-{}-{call}", std::process::id()));
    let job = ExportJob {
        source_path: "b.cypcb".into(),
        output_dir: dir.clone(),
        preset: cypcb_export::presets::jlcpcb_2layer(),
        board_name: "b".into(),
        stamp: cypcb_export::stamp::Stamp::UNIX_EPOCH,
    };
    run_export(&job, &mut world, &library).expect("the set exports");
    let _ = std::fs::remove_dir_all(&dir);
    let stamp = cypcb_export::stamp::Stamp::UNIX_EPOCH;
    cypcb_export::ipc2581::export_ipc2581(&mut world, &library, Default::default(), stamp);
    cypcb_export::ipc356::export_ipc356(&mut world, &library, "b");
}

#[test]
fn a_kicad_footprint_with_no_pads_is_placed_and_exported() {
    for text in [
        // Without a courtyard: the reader derives one about the origin.
        r#"(footprint "Logo" (layer "F.Cu") (descr "mark")
             (fp_circle (center 0 0) (end 1 0) (layer "F.SilkS") (width 0.15)))"#,
        // With the courtyard it draws.
        r#"(footprint "Logo" (layer "F.Cu")
             (fp_line (start -2 -1) (end 2 -1) (layer "F.CrtYd") (width 0.05))
             (fp_line (start 2 -1) (end 2 1) (layer "F.CrtYd") (width 0.05))
             (fp_line (start 2 1) (end -2 1) (layer "F.CrtYd") (width 0.05))
             (fp_line (start -2 1) (end -2 -1) (layer "F.CrtYd") (width 0.05)))"#,
    ] {
        let footprint = cypcb_kicad::import_footprint_from_str(text).expect("the footprint reads");
        assert!(footprint.pads.is_empty());
        let mut library = FootprintLibrary::new();
        library.register(footprint);
        through_the_writers(&board("Logo"), library);
    }
}

#[test]
fn a_footprint_the_language_defines_with_no_pads_is_placed_and_exported() {
    for body in [
        "    courtyard 4mm x 2mm\n    silk circle at 0mm, 0mm radius 1mm\n",
        "",
    ] {
        let source = format!(
            "version 1\n\nfootprint MARK {{\n    description \"logo\"\n{body}}}\n\n\
             board b {{\n    size 40mm x 20mm\n    layers 2\n}}\n\n\
             component G1 resistor \"MARK\" {{\n    at 20mm, 10mm\n}}\n"
        );
        let mut engine = PcbEngine::new();
        assert_eq!(engine.load_source(&source), "");
        through_the_writers(&source, FootprintLibrary::new());
    }
}

#[test]
fn a_kicad_board_with_a_footprint_with_no_pads_opens() {
    let pcb = r#"(kicad_pcb (version 20240108) (generator "pcbnew") (generator_version "8.0.0")
  (general (thickness 1.6)) (paper "A4")
  (layers (0 "F.Cu" signal) (31 "B.Cu" signal) (44 "Edge.Cuts" user))
  (net 0 "")
  (gr_line (start 100.0 100.0) (end 140.0 100.0) (layer "Edge.Cuts") (width 0.05))
  (gr_line (start 140.0 100.0) (end 140.0 120.0) (layer "Edge.Cuts") (width 0.05))
  (gr_line (start 140.0 120.0) (end 100.0 120.0) (layer "Edge.Cuts") (width 0.05))
  (gr_line (start 100.0 120.0) (end 100.0 100.0) (layer "Edge.Cuts") (width 0.05))
  (footprint "Logo:Mark" (layer "F.Cu") (at 120 110)
    (property "Reference" "G1" (at 0 0) (layer "F.SilkS"))
    (fp_circle (center 0 0) (end 1 0) (layer "F.SilkS") (width 0.15)))
)"#;
    let mut engine = PcbEngine::new();
    assert_eq!(engine.load_kicad(pcb), "");
    let snapshot: serde_json::Value = serde_json::from_str(&engine.get_snapshot()).unwrap();
    assert_eq!(snapshot["components"][0]["refdes"], "G1");
    assert_eq!(violations(&snapshot), Vec::<String>::new());
}

#[test]
fn the_built_in_library_has_no_footprint_without_pads() {
    // So a footprint without pads reaches the engine from a reader or from a
    // host, never from the library; if one is added, this is the place that
    // says the readers above are the ones that carry it.
    let library = FootprintLibrary::new();
    let bare: Vec<&str> = library
        .iter()
        .filter(|(_, footprint)| footprint.pads.is_empty())
        .map(|(name, _)| name)
        .collect();
    assert_eq!(bare, Vec::<&str>::new());
}
