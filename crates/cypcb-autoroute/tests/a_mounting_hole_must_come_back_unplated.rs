//! A hole the fabricator must not plate has to survive the whole way out.
//!
//! `cargo test -p cypcb-kicad --test a_mounting_hole_must_come_back_unplated`
//!
//! KiCad writes a mounting hole as `np_thru_hole`, and the importer folded it
//! into the same branch as an ordinary pin:
//!
//! ```ignore
//! let is_through_hole = pad_type_str == "thru_hole" || pad_type_str == "np_thru_hole";
//! ```
//!
//! Downstream, every drill hit was `DrillType::Plated` with the comment
//! "Component pads are always plated", and every preset named an NPTH file
//! that nothing ever wrote. So a board imported from KiCad with four M3
//! mounting holes shipped them in the plated file. Plating narrows a 3.2mm
//! hole by roughly a tenth of a millimetre and joins it to any copper it
//! passes, so the screw does not fit and the bracket is live.
//!
//! Four questions, because the hole has to be right in four places and being
//! right in one of them is what made this hard to see:
//!
//! 1. the drill file lists it as non-plated,
//! 2. the plated drill file does not list it,
//! 3. no copper layer flashes a pad there,
//! 4. the router treats it as solid - a hole is a hole whether or not it has
//!    copper, and the obstacle used to come from the copper alone.

use cypcb_export::coords::CoordinateFormat;
use cypcb_export::excellon::{export_excellon, DrillType};
use cypcb_export::gerber::copper::export_copper_layer;
use cypcb_kicad::parse_kicad_pcb;
use cypcb_world::components::Layer;

use std::io::Write;

/// Two pads that must be plated and one M3 mounting hole that must not.
///
/// The mounting hole carries `(layers "*.Cu" "*.Mask")`, which is what pcbnew
/// writes for a stock `MountingHole_3.2mm_M3` - the layer list in the file
/// says copper even though the pad type says the hole is bare, and trusting
/// the layers instead of the type is the other way to get this wrong.
const BOARD_WITH_A_MOUNTING_HOLE: &str = r#"(kicad_pcb (version 20240108) (generator "pcbnew")

  (general (thickness 1.6))
  (paper "A4")
  (layers
    (0 "F.Cu" signal)
    (31 "B.Cu" signal)
    (44 "Edge.Cuts" user)
  )

  (net 0 "")
  (net 1 "SIG")

  (gr_rect (start 100 100) (end 140 130) (layer "Edge.Cuts") (width 0.05))

  (footprint "Connector:Conn_01x02"
    (layer "F.Cu")
    (at 110 110)
    (property "Reference" "J1")
    (property "Value" "conn")
    (pad "1" thru_hole rect (at 0 0) (size 1.7 1.7) (drill 1.0) (layers "*.Cu" "*.Mask") (net 1 "SIG"))
    (pad "2" thru_hole oval (at 0 2.54) (size 1.7 1.7) (drill 1.0) (layers "*.Cu" "*.Mask") (net 1 "SIG"))
  )

  (footprint "MountingHole:MountingHole_3.2mm_M3"
    (layer "F.Cu")
    (at 130 120)
    (property "Reference" "H1")
    (property "Value" "MountingHole_3.2mm_M3")
    (pad "" np_thru_hole circle (at 0 0) (size 3.2 3.2) (drill 3.2) (layers "*.Cu" "*.Mask"))
  )
)
"#;

/// Named per test, because these run in parallel and one path shared between
/// them means one test truncating the file another is reading. It showed up as
/// `SexprParseError("Root is not a list")` on whichever test lost the race.
fn parsed(who: &str) -> cypcb_kicad::KicadPcbParseResult {
    let dir = std::env::temp_dir().join("cypcb-mounting-hole");
    std::fs::create_dir_all(&dir).expect("a place to put the board");
    let path = dir.join(format!("mounting_hole-{who}.kicad_pcb"));
    let mut file = std::fs::File::create(&path).expect("the board is writable");
    file.write_all(BOARD_WITH_A_MOUNTING_HOLE.as_bytes())
        .expect("the board is written");
    drop(file);

    parse_kicad_pcb(&path).unwrap_or_else(|e| panic!("the board must parse: {e:?}"))
}

/// The hole sits at 130mm, 120mm in file coordinates. The board's origin is at
/// 100, 100, so it lands 30mm, 20mm from the corner.
const HOLE_MM: (f64, f64) = (30.0, 20.0);

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

fn near(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() < 0.05 && (a.1 - b.1).abs() < 0.05
}

#[test]
fn the_importer_gives_a_mounting_hole_no_copper() {
    let result = parsed("copper-layers");
    let footprint = result
        .library
        .get("MountingHole:MountingHole_3.2mm_M3")
        .expect("the mounting hole is in the library");

    let pad = footprint
        .pads
        .first()
        .expect("the mounting hole has its one pad");
    assert!(
        pad.is_non_plated(),
        "the file said np_thru_hole and the pad came back with copper: {:?}",
        pad.layers
    );
    assert_eq!(
        pad.drill,
        Some(cypcb_core::Nm::from_mm(3.2)),
        "the hole keeps its size - it is the copper that goes, not the drill"
    );

    // And the connector next to it is untouched, which is the half of this
    // that a blanket rule would break.
    let connector = result
        .library
        .get("Connector:Conn_01x02")
        .expect("the connector is in the library");
    for pad in &connector.pads {
        assert!(
            !pad.is_non_plated(),
            "a plain thru_hole pad must stay plated: {}",
            pad.number
        );
    }
}

#[test]
fn the_hole_is_in_the_unplated_drill_file_and_not_the_plated_one() {
    let mut result = parsed("drill-files");
    let format = CoordinateFormat::FORMAT_MM_2_6;

    let plated = export_excellon(
        &mut result.world,
        &result.library,
        &format,
        Some(DrillType::Plated),
    )
    .expect("a plated drill file");
    let unplated = export_excellon(
        &mut result.world,
        &result.library,
        &format,
        Some(DrillType::NonPlated),
    )
    .expect("an unplated drill file");

    assert!(
        hits(&unplated).iter().any(|hit| near(*hit, HOLE_MM)),
        "the mounting hole is missing from the unplated file:\n{unplated}"
    );
    assert!(
        !hits(&plated).iter().any(|hit| near(*hit, HOLE_MM)),
        "the mounting hole is in the plated file, so the fabricator plates it \
         and the M3 screw no longer fits:\n{plated}"
    );

    // The connector's two pins are still plated, and there are exactly two of
    // them: a fix that moved every hole to the unplated file would pass the
    // two assertions above.
    assert_eq!(
        hits(&plated).len(),
        2,
        "the connector's two pins are the plated holes on this board:\n{plated}"
    );
    assert_eq!(
        hits(&unplated).len(),
        1,
        "one mounting hole, and nothing else:\n{unplated}"
    );
}

#[test]
fn no_copper_layer_flashes_a_pad_at_the_mounting_hole() {
    let mut result = parsed("gerber-copper");
    let format = CoordinateFormat::FORMAT_MM_2_6;

    for layer in [Layer::TopCopper, Layer::BottomCopper] {
        let gerber = export_copper_layer(&mut result.world, &result.library, layer, &format)
            .unwrap_or_else(|e| panic!("{layer:?} copper: {e:?}"));

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

        assert!(
            !flashes.iter().any(|flash| near(*flash, HOLE_MM)),
            "{layer:?} flashes copper at the mounting hole, which is the one \
             thing a mounting hole does not have: {flashes:?}"
        );
        assert!(
            !flashes.is_empty(),
            "{layer:?} has no copper at all, so the check above proved nothing"
        );
    }
}

#[test]
fn the_router_cannot_route_through_a_mounting_hole() {
    use cypcb_autoroute::grid::RoutingGrid;
    use cypcb_core::Point;
    use cypcb_rules::presets::{PresetRuleSet, RulesPreset};

    let mut result = parsed("router-grid");
    let rules = PresetRuleSet::new(RulesPreset::from_name("jlcpcb").expect("a known preset"));
    let grid = RoutingGrid::from_board(
        &mut result.world,
        &result.library,
        &rules,
        cypcb_core::Nm::from_mm(0.254).raw(),
    )
    .expect("the board states a size");

    let free_at = |mm: (f64, f64), layer: usize| {
        let (gx, gy) = grid.nm_to_grid(Point::from_mm(mm.0, mm.1));
        grid.is_free(gx, gy, layer)
    };

    // The obstacle used to come from the pad's copper layers, and a mounting
    // hole has none - so the grid saw free space and the router would lay a
    // trace across a 3.2mm hole, on both layers.
    for layer in 0..2usize {
        assert!(
            !free_at(HOLE_MM, layer),
            "layer {layer} is free at the mounting hole, so the router will \
             route straight through it"
        );
    }

    // A point a few millimetres away is free, so the assertion above is about
    // the hole rather than about a grid that blocks everything.
    assert!(
        free_at((HOLE_MM.0 - 8.0, HOLE_MM.1), 0),
        "the grid blocks open board as well, so it proves nothing about the hole"
    );
}

#[test]
fn the_export_job_writes_the_unplated_file_every_preset_already_named() {
    use cypcb_export::presets::from_name;
    use cypcb_export::{run_export, ExportJob};
    use std::path::PathBuf;

    // Every preset carries a `drill_npth` name - `-NPTH.drl` for JLCPCB,
    // `_npth.xln` for PCBWay - and the job never wrote a file under it. The
    // hole went out in the plated file instead, so the promise in the preset
    // was the only place this was visible.
    let mut result = parsed("export-job");
    let preset = from_name("jlcpcb").expect("a known preset");
    let output_dir = std::env::temp_dir().join("cypcb-npth-job");
    let _ = std::fs::remove_dir_all(&output_dir);

    let job = ExportJob {
        source_path: PathBuf::from("mounting_hole.kicad_pcb"),
        output_dir: output_dir.clone(),
        preset: preset.clone(),
        board_name: "mounting".to_string(),
    };
    let exported = run_export(&job, &mut result.world, &result.library).expect("the export runs");

    let names: Vec<String> = exported
        .files
        .iter()
        .filter_map(|file| {
            file.path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
        })
        .collect();

    let npth = names
        .iter()
        .find(|name| name.ends_with(preset.file_naming.drill_npth))
        .unwrap_or_else(|| {
            panic!(
                "no file named for the preset's own `drill_npth` ({}): {names:?}",
                preset.file_naming.drill_npth
            )
        });

    let written = std::fs::read_to_string(output_dir.join("drill").join(npth))
        .expect("the unplated file is on disk");
    assert!(
        hits(&written).iter().any(|hit| near(*hit, HOLE_MM)),
        "the file named for unplated holes does not carry the one on the board:\n{written}"
    );

    let pth_name = names
        .iter()
        .find(|name| name.ends_with(preset.file_naming.drill_pth))
        .expect("the plated file is written too");
    let pth = std::fs::read_to_string(output_dir.join("drill").join(pth_name))
        .expect("the plated file is on disk");
    assert!(
        !hits(&pth).iter().any(|hit| near(*hit, HOLE_MM)),
        "the mounting hole is in the plated file as well:\n{pth}"
    );

    let _ = std::fs::remove_dir_all(&output_dir);
}
