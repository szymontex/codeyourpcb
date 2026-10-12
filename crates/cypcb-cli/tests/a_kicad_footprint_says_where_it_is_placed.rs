//! A KiCad footprint that names the point it is placed by is placed by it.
//!
//! `cargo test -p cypcb-cli --test a_kicad_footprint_says_where_it_is_placed`
//!
//! The pick-and-place file places a surface-mount part by its origin and a
//! through-hole part by the middle of its pads. The Fabrication Toolkit lets a
//! footprint override that with a custom field, `FT Origin` and then `Origin`,
//! whose text is `Anchor` or `Center` (`plugins/process.py` at commit
//! 642b069c, read 2026-10-12). KiCad's own position file has no such choice:
//! it writes the footprint's origin (`pcbnew/exporters/place_file_exporter.cpp`
//! at 3d5bdc4a, read 2026-10-12). The reader skipped the field, so a footprint
//! that asked for its origin was placed by the middle of its pads.
//!
//! These tests take a board and a footprint file through this project's own
//! KiCad readers and the pick-and-place writer, as `export` does.

use cypcb_export::cpl::export_cpl;
use cypcb_kicad::pcb_parser::parse_kicad_pcb_str;
use cypcb_world::footprint::PlacementOrigin;

/// A field the way KiCad 8 writes a custom one on a footprint.
fn field(name: &str, value: &str) -> String {
    format!(
        "(property \"{name}\" \"{value}\" (at 0 0 0) (unlocked yes) (layer \"F.Fab\") (hide yes) \
         (effects (font (size 1.27 1.27) (thickness 0.15))))"
    )
}

/// A three-pin through-hole footprint, pin 1 on its origin and the pins 2.54mm
/// apart down the page, with whatever extra children `extra` adds.
fn header(refdes: &str, x: f64, extra: &str) -> String {
    format!(
        "(footprint \"Conn:Header3\" (layer \"F.Cu\") (at {x} 100)
           (property \"Reference\" \"{refdes}\" (at 0 -2 0) (layer \"F.SilkS\"))
           (property \"Value\" \"hdr\" (at 0 8 0) (layer \"F.Fab\"))
           {extra}
           (pad \"1\" thru_hole rect (at 0 0) (size 1.7 1.7) (drill 1.0) (layers \"*.Cu\" \"*.Mask\"))
           (pad \"2\" thru_hole circle (at 0 2.54) (size 1.7 1.7) (drill 1.0) (layers \"*.Cu\" \"*.Mask\"))
           (pad \"3\" thru_hole circle (at 0 5.08) (size 1.7 1.7) (drill 1.0) (layers \"*.Cu\" \"*.Mask\")))"
    )
}

fn board(parts: &[String]) -> String {
    format!(
        "(kicad_pcb (version 20240108) (generator \"pcbnew\") (generator_version \"8.0.0\")
           (general (thickness 1.6)) (paper \"A4\")
           (layers (0 \"F.Cu\" signal) (31 \"B.Cu\" signal) (44 \"Edge.Cuts\" user))
           (net 0 \"\")
           (gr_line (start 90 90) (end 150 90) (layer \"Edge.Cuts\") (width 0.05))
           (gr_line (start 150 90) (end 150 120) (layer \"Edge.Cuts\") (width 0.05))
           (gr_line (start 150 120) (end 90 120) (layer \"Edge.Cuts\") (width 0.05))
           (gr_line (start 90 120) (end 90 90) (layer \"Edge.Cuts\") (width 0.05))
           {}
         )",
        parts.join("\n")
    )
}

/// Mid Y of each reference in the position file of a board.
fn mid_y(source: &str) -> std::collections::HashMap<String, f64> {
    let parsed = parse_kicad_pcb_str(source).expect("the board reads");
    let mut world = parsed.world;
    let library = parsed.library;
    let csv = export_cpl(&mut world, &library, None).expect("the position file is written");
    csv.lines()
        .skip(1)
        .map(|row| {
            let cells: Vec<&str> = row.split(',').collect();
            let y = cells[2].trim_end_matches("mm").parse().unwrap();
            (cells[0].to_string(), y)
        })
        .collect()
}

#[test]
fn a_header_with_no_field_is_placed_by_the_middle_of_its_pads() {
    // The control: nothing stated, nothing changed.
    let y = mid_y(&board(&[header("J1", 100.0, "")]));
    let anchored = mid_y(&board(&[header(
        "J2",
        100.0,
        &field("FT Origin", "Anchor"),
    )]));
    assert!(
        ((y["J1"] - anchored["J2"]).abs() - 2.54).abs() < 0.001,
        "the middle of pins 2.54 apart is 2.54mm from pin 1: {} and {}",
        y["J1"],
        anchored["J2"]
    );
}

#[test]
fn ft_origin_anchor_places_it_by_pin_1_and_center_by_the_middle() {
    let y = mid_y(&board(&[
        header("J1", 100.0, ""),
        header("J2", 110.0, &field("FT Origin", "Anchor")),
        header("J3", 120.0, &field("FT Origin", "Center")),
        header("J4", 130.0, &field("Origin", "Anchor")),
    ]));
    assert!(
        (y["J1"] - y["J3"]).abs() < 0.001,
        "Center is what a header does anyway"
    );
    assert!(
        ((y["J1"] - y["J2"]).abs() - 2.54).abs() < 0.001,
        "Anchor is pin 1"
    );
    assert!(
        (y["J2"] - y["J4"]).abs() < 0.001,
        "`Origin` asks for the same"
    );
}

#[test]
fn the_field_is_read_the_way_the_plugin_reads_it() {
    // Value: stripped and capitalised. Key: exact.
    let y = mid_y(&board(&[
        header("J1", 100.0, &field("FT Origin", "  anchor ")),
        header("J2", 110.0, &field("ft origin", "Anchor")),
        header("J3", 120.0, &field("FT Origin", "Anchor")),
    ]));
    assert!((y["J1"] - y["J3"]).abs() < 0.001, "' anchor ' is Anchor");
    assert!(
        ((y["J2"] - y["J3"]).abs() - 2.54).abs() < 0.001,
        "a key in another case is another field"
    );
}

#[test]
fn ft_origin_wins_over_origin_and_a_bad_one_does_not_fall_through() {
    let both = format!(
        "{} {}",
        field("FT Origin", "Anchor"),
        field("Origin", "Center")
    );
    let bad_first = format!(
        "{} {}",
        field("FT Origin", "Middle"),
        field("Origin", "Anchor")
    );
    let y = mid_y(&board(&[
        header("J1", 100.0, ""),
        header("J2", 110.0, &both),
        header("J3", 120.0, &bad_first),
    ]));
    assert!(
        ((y["J1"] - y["J2"]).abs() - 2.54).abs() < 0.001,
        "FT Origin Anchor wins"
    );
    // The plugin takes the first key the footprint has and ignores its value
    // when it is neither word, without trying the second.
    assert!(
        (y["J1"] - y["J3"]).abs() < 0.001,
        "a bad FT Origin leaves the default"
    );
}

#[test]
fn a_value_that_is_neither_word_is_said_not_lost() {
    let parsed = parse_kicad_pcb_str(&board(&[
        header("J1", 100.0, &field("FT Origin", "Middle")),
        header("J2", 110.0, &field("FT Origin", "Middle")),
    ]))
    .expect("the board reads");
    let notes = &parsed.metadata.placement_notes;
    assert_eq!(notes.len(), 1, "once for the one footprint: {notes:?}");
    assert!(
        notes[0].contains("Header3")
            && notes[0].contains("\"Middle\"")
            && notes[0].contains("FT Origin"),
        "{}",
        notes[0]
    );

    // Control: a board that asks for nothing, or for a word, says nothing.
    for extra in ["", &field("FT Origin", "Center")] {
        let quiet = parse_kicad_pcb_str(&board(&[header("J1", 100.0, extra)])).unwrap();
        assert_eq!(quiet.metadata.placement_notes, Vec::<String>::new());
    }
}

#[test]
fn two_parts_of_one_footprint_that_ask_for_different_points_are_two_footprints() {
    let parsed = parse_kicad_pcb_str(&board(&[
        header("J1", 100.0, ""),
        header("J2", 110.0, &field("FT Origin", "Anchor")),
        header("J3", 120.0, &field("FT Origin", "Anchor")),
    ]))
    .expect("the board reads");
    // The library also holds the built-ins; these two are the board's own.
    let kinds: Vec<Option<PlacementOrigin>> = parsed
        .library
        .iter()
        .filter(|(name, _)| name.contains("Header3"))
        .map(|(_, f)| f.placement_origin)
        .collect();
    assert_eq!(kinds.len(), 2, "one without and one with: {kinds:?}");
    assert!(kinds.contains(&None) && kinds.contains(&Some(PlacementOrigin::Anchor)));
}

#[test]
fn a_footprint_file_states_its_point_and_its_bad_value_is_said() {
    let file = |extra: &str| {
        format!(
            "(footprint \"Header3\" (version 20240108) (generator \"pcbnew\") (layer \"F.Cu\")
               {extra}
               (pad \"1\" thru_hole rect (at 0 0) (size 1.7 1.7) (drill 1.0) (layers \"*.Cu\" \"*.Mask\")))"
        )
    };
    let (footprint, notes) =
        cypcb_kicad::import_footprint_noted_from_str(&file(&field("FT Origin", "Anchor"))).unwrap();
    assert_eq!(footprint.placement_origin, Some(PlacementOrigin::Anchor));
    assert_eq!(notes, Vec::<String>::new());

    let (footprint, notes) =
        cypcb_kicad::import_footprint_noted_from_str(&file(&field("Origin", "Center"))).unwrap();
    assert_eq!(
        footprint.placement_origin,
        Some(PlacementOrigin::PadsCentre)
    );
    assert_eq!(notes.len(), 0);

    let (footprint, notes) =
        cypcb_kicad::import_footprint_noted_from_str(&file(&field("Origin", "top-left"))).unwrap();
    assert_eq!(footprint.placement_origin, None);
    assert_eq!(notes.len(), 1);
    assert!(
        notes[0].contains("Header3") && notes[0].contains("top-left"),
        "{}",
        notes[0]
    );

    // The control: no field.
    let (footprint, notes) = cypcb_kicad::import_footprint_noted_from_str(&file("")).unwrap();
    assert_eq!((footprint.placement_origin, notes.len()), (None, 0));
}

/// A scratch directory of this test's own.
fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("placed-by-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("the scratch directory is made");
    dir
}

fn run(args: &[&std::path::Path], first: &str) -> (bool, String) {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .arg(first)
        .args(args)
        .output()
        .expect("the binary runs");
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    )
}

#[test]
fn from_kicad_says_the_language_cannot_state_it() {
    let dir = scratch("said");
    let stated = dir.join("board.kicad_pcb");
    std::fs::write(
        &stated,
        board(&[header("J1", 100.0, &field("FT Origin", "Anchor"))]),
    )
    .expect("the board is writable");
    let written = dir.join("board.cypcb");
    let (ok, said) = run(
        &[&stated, std::path::Path::new("-o"), &written],
        "from-kicad",
    );
    assert!(ok, "{said}");
    assert!(
        said.contains("footprint(s) that state the point they are placed by"),
        "{said}"
    );
    assert!(written.exists(), "the board still arrives");

    // The control: a board that states nothing says nothing of the kind.
    let plain = dir.join("plain.kicad_pcb");
    std::fs::write(&plain, board(&[header("J1", 100.0, "")])).unwrap();
    let out = dir.join("plain.cypcb");
    let (ok, said) = run(&[&plain, std::path::Path::new("-o"), &out], "from-kicad");
    assert!(ok, "{said}");
    assert!(!said.contains("placed by"), "{said}");
}

#[test]
fn export_says_a_value_it_could_not_take() {
    let dir = scratch("export");
    let bad = dir.join("bad.kicad_pcb");
    std::fs::write(
        &bad,
        board(&[header("J1", 100.0, &field("FT Origin", "Middle"))]),
    )
    .unwrap();
    let out = dir.join("out");
    let (_, said) = run(&[&bad, std::path::Path::new("-o"), &out], "export");
    assert!(
        said.contains("warning:") && said.contains("\"Middle\"") && said.contains("Header3"),
        "{said}"
    );
    // It said so and still wrote the file: placed by the default for its kind,
    // which for a through-hole header is the middle of its pads.
    let default_y = mid_y_in(&out);

    let good = dir.join("good.kicad_pcb");
    std::fs::write(
        &good,
        board(&[header("J1", 100.0, &field("FT Origin", "Center"))]),
    )
    .unwrap();
    let out2 = dir.join("out2");
    let (_, said) = run(&[&good, std::path::Path::new("-o"), &out2], "export");
    assert!(!said.contains("FT Origin"), "{said}");
    assert!(
        (mid_y_in(&out2) - default_y).abs() < 0.001,
        "the same place"
    );

    let anchored = dir.join("anchored.kicad_pcb");
    std::fs::write(
        &anchored,
        board(&[header("J1", 100.0, &field("FT Origin", "Anchor"))]),
    )
    .unwrap();
    let out3 = dir.join("out3");
    run(&[&anchored, std::path::Path::new("-o"), &out3], "export");
    assert!(
        ((mid_y_in(&out3) - default_y).abs() - 2.54).abs() < 0.001,
        "Anchor is 2.54mm from the middle of three pins that far apart"
    );
}

/// Mid Y of J1 in the position file an export wrote under `out`.
fn mid_y_in(out: &std::path::Path) -> f64 {
    let assembly = out.join("assembly");
    let file = cypcb_fixtures::tree::written_entries(&assembly)
        .unwrap_or_else(|e| panic!("{}: {e}", assembly.display()))
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| p.to_string_lossy().ends_with("CPL.csv"))
        .expect("the export wrote a position file");
    let csv = std::fs::read_to_string(file).unwrap();
    let row = csv
        .lines()
        .find(|l| l.starts_with("J1,"))
        .expect("J1 is in it");
    row.split(',')
        .nth(2)
        .unwrap()
        .trim_end_matches("mm")
        .parse()
        .unwrap()
}
