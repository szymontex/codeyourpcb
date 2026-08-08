//! A board written in this project's own language can be bolted down.
//!
//! `cargo test -p cypcb-cli --test a_board_can_be_screwed_to_something`
//!
//! The importer learned what a non-plated hole is one commit ago, so a board
//! that came from KiCad could carry mounting holes. A board written in
//! `.cypcb` could not: `docs/SYNTAX.md` offered `keepout mounting_hole`, which
//! is a region the router avoids, and nothing that gets drilled. So the
//! project's own language could describe a board that cannot be screwed to
//! anything.
//!
//! It needed no new syntax. A mounting hole is a footprint with one drilled
//! pad and no copper - that is what it is, and it is how KiCad carries them
//! too - so four built-in footprints are enough and the existing `component`
//! declaration places them.
//!
//! What this checks is the whole way through, because a hole is only right if
//! it is right in every file: drilled and unplated, absent from the copper,
//! solid to the router, and absent from the two files that tell people and
//! machines what to buy and where to put it.

use std::path::PathBuf;

use cypcb_export::presets::from_name;
use cypcb_export::{run_export, ExportJob};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{sync_ast_to_world, BoardWorld};

/// Two resistors and two M3 mounting holes, in the project's own language.
const SOURCE: &str = r#"version 1

board bracket {
    size 40mm x 30mm
    layers 2
}

component R1 resistor "0402" {
    value "10k"
    at 12mm, 15mm
}

component R2 resistor "0402" {
    value "10k"
    at 20mm, 15mm
}

component H1 generic "MOUNT-M3" {
    value "M3"
    at 5mm, 5mm
}

component H2 generic "MOUNT-M3" {
    value "M3"
    at 35mm, 25mm
}

net SIG {
    R1.2, R2.1
}
"#;

/// The holes, in millimetres from the board origin.
const HOLES_MM: [(f64, f64); 2] = [(5.0, 5.0), (35.0, 25.0)];

fn built() -> (BoardWorld, FootprintLibrary) {
    let parsed = cypcb_parser::parse(SOURCE);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);

    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    let result = sync_ast_to_world(&parsed.value, SOURCE, &mut world, &mut library);
    assert!(result.errors.is_empty(), "{:?}", result.errors);

    world.set_footprints(library.clone());
    world.rebuild_spatial_index_from_library(&library);
    (world, library)
}

fn near(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() < 0.05 && (a.1 - b.1).abs() < 0.05
}

/// Millimetres out of an Excellon body, which keeps its decimal point.
fn hits(drill: &str) -> Vec<(f64, f64)> {
    drill
        .lines()
        .filter(|line| line.starts_with('X'))
        .filter_map(|line| {
            let (x, y) = line[1..].split_once('Y')?;
            Some((x.parse().ok()?, y.parse().ok()?))
        })
        .collect()
}

/// Named per test: these run in parallel, and one output directory shared
/// between them means one test deleting the files another is reading.
fn exported(who: &str) -> (PathBuf, Vec<String>) {
    let (mut world, library) = built();
    let preset = from_name("jlcpcb").expect("a known preset");
    let output_dir = std::env::temp_dir().join(format!("cypcb-screwed-down-{who}"));
    let _ = std::fs::remove_dir_all(&output_dir);

    let job = ExportJob {
        source_path: PathBuf::from("bracket.cypcb"),
        output_dir: output_dir.clone(),
        preset,
        board_name: "bracket".to_string(),
    };
    let result = run_export(&job, &mut world, &library).expect("the export runs");
    let names = result
        .files
        .iter()
        .filter_map(|file| {
            file.path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
        })
        .collect();
    (output_dir, names)
}

fn read(dir: &std::path::Path, subdir: &str, ending: &str, names: &[String]) -> String {
    let name = names
        .iter()
        .find(|name| name.ends_with(ending))
        .unwrap_or_else(|| panic!("no file ending in {ending}: {names:?}"));
    std::fs::read_to_string(dir.join(subdir).join(name))
        .unwrap_or_else(|e| panic!("{name} is readable: {e}"))
}

#[test]
fn the_holes_are_drilled_and_not_plated() {
    let (dir, names) = exported("drill");

    let unplated = read(&dir, "drill", "-NPTH.drl", &names);
    for hole in HOLES_MM {
        assert!(
            hits(&unplated).iter().any(|hit| near(*hit, hole)),
            "the hole at {hole:?} is not in the unplated drill file:\n{unplated}"
        );
    }
    assert_eq!(
        hits(&unplated).len(),
        2,
        "two mounting holes and nothing else:\n{unplated}"
    );

    // The resistors are surface mount, so this board has no plated holes at
    // all - and the plated file must not have quietly received the two.
    let plated = read(&dir, "drill", "-PTH.drl", &names);
    for hole in HOLES_MM {
        assert!(
            !hits(&plated).iter().any(|hit| near(*hit, hole)),
            "the mounting hole at {hole:?} is in the plated file, so it comes \
             back narrower than the screw and shorted to the copper:\n{plated}"
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn nothing_is_soldered_to_a_mounting_hole() {
    let (dir, names) = exported("copper");

    for (ending, what) in [("F_Cu.gbr", "top copper"), ("B_Cu.gbr", "bottom copper")] {
        let gerber = read(&dir, "gerber", ending, &names);
        // 2.6 format: six implied decimals, no point.
        let flashes: Vec<(f64, f64)> = gerber
            .lines()
            .filter(|line| line.contains("D03"))
            .filter_map(|line| {
                let (x, rest) = line[1..].split_once('Y')?;
                let y: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                Some((
                    x.parse::<f64>().ok()? / 1_000_000.0,
                    y.parse::<f64>().ok()? / 1_000_000.0,
                ))
            })
            .collect();

        for hole in HOLES_MM {
            assert!(
                !flashes.iter().any(|flash| near(*flash, hole)),
                "{what} flashes a pad at the mounting hole {hole:?}: {flashes:?}"
            );
        }
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn nobody_is_asked_to_buy_a_hole_or_to_place_one() {
    let (dir, names) = exported("assembly");

    let bom = read(&dir, "assembly", ".csv", &names);
    assert!(
        bom.contains("0402"),
        "the bill of materials lost the parts that are real:\n{bom}"
    );
    assert!(
        !bom.contains("MOUNT-M3"),
        "the bill of materials asks somebody to buy a hole:\n{bom}"
    );
    for refdes in ["H1", "H2"] {
        assert!(
            !bom.contains(refdes),
            "{refdes} is a hole and it is on the purchase list:\n{bom}"
        );
    }

    let placement = names
        .iter()
        .find(|name| name.contains("cpl") || name.contains("pos") || name.contains("CPL"))
        .map(|name| {
            std::fs::read_to_string(dir.join("assembly").join(name)).expect("readable placement")
        })
        .expect("a placement file is written");
    assert!(
        placement.contains("R1"),
        "the placement file lost the parts a machine does place:\n{placement}"
    );
    for refdes in ["H1", "H2"] {
        assert!(
            !placement.contains(refdes),
            "the placement file tells the machine to place a hole ({refdes}):\n{placement}"
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_example_board_still_has_its_holes() {
    // `examples/panel-mount.cypcb` exists to show the feature working on a
    // board somebody would actually build. An example that quietly loses the
    // thing it demonstrates is worse than no example, because it reads as
    // proof that the feature is covered.
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .join("examples/panel-mount.cypcb");
    let source = std::fs::read_to_string(&path).expect("the example is there");

    let parsed = cypcb_parser::parse(&source);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);

    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    let result = sync_ast_to_world(&parsed.value, &source, &mut world, &mut library);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    world.set_footprints(library.clone());
    world.rebuild_spatial_index_from_library(&library);

    let preset = from_name("jlcpcb").expect("a known preset");
    let output_dir = std::env::temp_dir().join("cypcb-example-panel-mount");
    let _ = std::fs::remove_dir_all(&output_dir);
    let job = ExportJob {
        source_path: path.clone(),
        output_dir: output_dir.clone(),
        preset,
        board_name: "panel-mount".to_string(),
    };
    let exported = run_export(&job, &mut world, &library).expect("the export runs");

    let npth = exported
        .files
        .iter()
        .find(|file| {
            file.path
                .file_name()
                .is_some_and(|name| name.to_string_lossy().ends_with("-NPTH.drl"))
        })
        .expect("the example writes an unplated drill file");
    let drill = std::fs::read_to_string(&npth.path).expect("the unplated file is readable");

    // Four corners, 4mm in from each edge of a 40 x 30mm board.
    for corner in [(4.0, 4.0), (36.0, 4.0), (4.0, 26.0), (36.0, 26.0)] {
        assert!(
            hits(&drill).iter().any(|hit| near(*hit, corner)),
            "the example lost its mounting hole at {corner:?}:\n{drill}"
        );
    }
    assert_eq!(
        hits(&drill).len(),
        4,
        "four mounting holes and nothing else:\n{drill}"
    );
    assert!(
        drill.contains("C3.200000"),
        "the holes are drilled to the M3 clearance size:\n{drill}"
    );

    let _ = std::fs::remove_dir_all(&output_dir);
}
