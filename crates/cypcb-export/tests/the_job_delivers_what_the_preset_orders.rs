//! An export is only finished if every file the preset asked for is there.
//!
//! `run_export` writes thirteen files for the JLCPCB preset. Each of them has
//! its own test elsewhere - copper against the routed board, mask against the
//! copper, drill against the vias - and nothing checked the set. A job that
//! silently writes twelve of thirteen sends a board house an order with a
//! layer missing, and the first person to notice is the one holding the boards.

use std::collections::BTreeSet;
use std::path::PathBuf;

use cypcb_core::{Nm, Point, Rect};
use cypcb_export::presets::from_name;
use cypcb_export::{run_export, ExportJob};
use cypcb_world::components::{
    FootprintRef, Layer, NetConnections, NetId, PadShape, PinConnection, Position, RefDes,
    Rotation, Value,
};
use cypcb_world::footprint::{Footprint, FootprintLibrary, PadDef};
use cypcb_world::BoardWorld;

/// A two-pad part, built rather than parsed: this crate does not depend on the
/// parser, and the export job does not care where its board came from.
fn two_pad_footprint() -> Footprint {
    let pad = |number: &str, x: f64| PadDef {
        number: number.to_string(),
        shape: PadShape::Rect,
        position: Point::from_mm(x, 0.0),
        size: (Nm::from_mm(0.6), Nm::from_mm(0.5)),
        drill: None,
        layers: vec![Layer::TopCopper],
    };

    Footprint {
        name: "0402".to_string(),
        description: "two pads".to_string(),
        pads: vec![pad("1", -0.5), pad("2", 0.5)],
        bounds: Rect::from_center_size(Point::ORIGIN, (Nm::from_mm(1.0), Nm::from_mm(0.5))),
        courtyard: Rect::from_center_size(Point::ORIGIN, (Nm::from_mm(2.0), Nm::from_mm(1.2))),
        silk: Vec::new(),
    }
}

fn board() -> (BoardWorld, FootprintLibrary) {
    let mut world = BoardWorld::new();
    world.set_board(
        "delivery".to_string(),
        (Nm::from_mm(20.0), Nm::from_mm(20.0)),
        2,
    );

    let mut library = FootprintLibrary::new();
    library.register(two_pad_footprint());

    for (refdes, x) in [("R1", 8.0), ("R2", 12.0)] {
        let mut nets = NetConnections::new();
        nets.add(PinConnection::new("1".to_string(), NetId::new(1)));
        nets.add(PinConnection::new("2".to_string(), NetId::new(2)));
        world.spawn_component(
            RefDes::new(refdes),
            Value::new("10k"),
            Position::from_mm(x, 10.0),
            Rotation(0),
            FootprintRef::new("0402"),
            nets,
        );
    }

    world.set_footprints(library.clone());
    world.rebuild_spatial_index_from_library(&library);
    (world, library)
}

fn scratch_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cypcb-export-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// Every file the preset's flags ask for, by the name the preset gives it.
fn expected_files(preset: &cypcb_export::presets::ExportPreset, board: &str) -> BTreeSet<String> {
    let naming = &preset.file_naming;
    let layers = &preset.layers;
    let mut wanted = BTreeSet::new();

    let mut want = |on: bool, suffix: &str| {
        if on {
            wanted.insert(format!("{board}{suffix}"));
        }
    };

    want(layers.top_copper, naming.top_copper);
    want(layers.bottom_copper, naming.bottom_copper);
    want(layers.top_mask, naming.top_mask);
    want(layers.bottom_mask, naming.bottom_mask);
    want(layers.top_silk, naming.top_silk);
    want(layers.bottom_silk, naming.bottom_silk);
    want(layers.top_paste, naming.top_paste);
    want(layers.bottom_paste, naming.bottom_paste);
    want(layers.outline, naming.outline);

    wanted
}

#[test]
fn every_layer_the_preset_asks_for_is_written() {
    let (mut world, library) = board();

    let preset = from_name("jlcpcb").expect("the jlcpcb preset");
    let output_dir = scratch_dir("delivery");
    let job = ExportJob {
        source_path: PathBuf::from("delivery.cypcb"),
        output_dir: output_dir.clone(),
        preset: preset.clone(),
        board_name: "delivery".to_string(),
    };

    let exported = run_export(&job, &mut world, &library).expect("the export runs");

    let written: BTreeSet<String> = exported
        .files
        .iter()
        .filter_map(|file| {
            file.path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
        })
        .collect();

    let wanted = expected_files(&preset, "delivery");
    let missing: Vec<&String> = wanted.difference(&written).collect();
    assert!(
        missing.is_empty(),
        "the preset asked for layers the job did not write: {missing:?}\n  written: {written:?}"
    );

    // Drill and assembly are separate flags with their own directories.
    assert!(
        written.iter().any(|name| name.ends_with(".drl")),
        "the preset asks for a drill file: {written:?}"
    );
    if preset.assembly {
        assert!(
            written.iter().any(|name| name.contains("BOM")),
            "an assembly preset has to write a bill of materials: {written:?}"
        );
        assert!(
            written.iter().any(|name| name.contains("CPL")),
            "an assembly preset has to write a pick-and-place file: {written:?}"
        );
    }

    // And every file it claims to have written is on disk with content in it.
    for file in &exported.files {
        let size = std::fs::metadata(&file.path)
            .unwrap_or_else(|e| panic!("{} was reported but not written: {e}", file.path.display()))
            .len();
        assert!(
            size > 0,
            "{} was written empty, which a fabricator reads as a blank layer",
            file.path.display()
        );
    }

    let _ = std::fs::remove_dir_all(&output_dir);
}

#[test]
fn an_unrouted_board_says_so_and_a_routed_one_does_not() {
    // `warnings` was declared and always empty, which promises warnings that
    // never come. The one worth saying is the one that produces a board nobody
    // wanted: copper with no traces on it is an unrouted design sent to be
    // made, and the files are written either way, so nothing else stops it.
    use cypcb_world::components::trace::{Trace, TraceSegment, TraceSource};

    let (mut world, library) = board();
    let preset = from_name("jlcpcb").expect("the jlcpcb preset");
    let run = |world: &mut BoardWorld, name: &str| -> Vec<String> {
        let output_dir = scratch_dir(name);
        let job = ExportJob {
            source_path: PathBuf::from("delivery.cypcb"),
            output_dir: output_dir.clone(),
            preset: preset.clone(),
            board_name: "delivery".to_string(),
        };
        let result = run_export(&job, world, &library).expect("the export runs");
        let _ = std::fs::remove_dir_all(&output_dir);
        result.warnings
    };

    let unrouted = run(&mut world, "unrouted");
    assert!(
        unrouted.iter().any(|w| w.contains("no traces")),
        "an unrouted board has to say so: {unrouted:?}"
    );

    // One trace between the two pads, and the warning goes away.
    let net = NetId::new(1);
    world.spawn_entity((
        Trace {
            segments: vec![TraceSegment::new(
                Point::from_mm(8.5, 10.0),
                Point::from_mm(11.5, 10.0),
            )],
            width: Nm::from_mm(0.2),
            layer: Layer::TopCopper,
            net_id: net,
            locked: false,
            source: TraceSource::Autorouted,
        },
        net,
    ));
    world.rebuild_spatial_index_from_library(&library);

    let routed = run(&mut world, "routed");
    assert!(
        !routed.iter().any(|w| w.contains("no traces")),
        "a routed board must not be warned about: {routed:?}"
    );
}

#[test]
fn the_legend_prints_the_names_of_the_parts_it_labels() {
    // A fabricated board with no `R1` beside R1 cannot be assembled by eye:
    // the person holding the reel has to read the design file instead. Gerber
    // has no text a fabricator is obliged to honour, so the letters are
    // strokes like everything else on the layer.
    use cypcb_export::gerber::{export_silkscreen, Side as GerberSide, SilkConfig};

    let (mut world, library) = board();
    let format = cypcb_export::coords::CoordinateFormat::FORMAT_MM_2_6;

    let legend = export_silkscreen(
        &mut world,
        &library,
        GerberSide::Top,
        &format,
        &SilkConfig::default(),
    )
    .expect("top silkscreen");

    let draws = legend.lines().filter(|line| line.contains("D01")).count();

    // Two parts named R1 and R2: four glyphs, and a glyph is several strokes.
    // The number matters less than the fact that a legend carrying only
    // courtyards and crosshairs cannot reach it.
    assert!(
        draws > 20,
        "a legend with two named parts has more ink than this: {draws} draws\n{legend}"
    );

    // And a part whose name the font cannot spell still gets a mark, rather
    // than vanishing from the legend.
    let strokes_per_part = draws / 2;
    assert!(
        strokes_per_part > 5,
        "each part's label is more than a crosshair: {strokes_per_part} draws each"
    );
}

#[test]
fn a_declared_pour_that_cannot_be_made_is_named() {
    // A `zone` carries a net and the ratsnest treats a pad inside it as
    // connected, so a design with a ground plane reads as finished. The
    // exporter draws nothing there - measured on a board with one pour: four
    // pad flashes and no copper. Someone who draws a plane and sends these
    // files gets a board without one, so the export has to say so.
    use cypcb_core::Rect;
    use cypcb_world::components::zone::{Zone, ZoneKind};

    let (mut world, library) = board();
    world.spawn_entity(Zone {
        bounds: Rect::from_center_size(
            Point::from_mm(10.0, 10.0),
            (Nm::from_mm(16.0), Nm::from_mm(16.0)),
        ),
        kind: ZoneKind::CopperPour,
        layer_mask: Layer::TopCopper.to_copper_mask(),
        name: Some("GND_POUR".to_string()),
        net: Some(NetId::new(1)),
    });

    let preset = from_name("jlcpcb").expect("the jlcpcb preset");
    let output_dir = scratch_dir("pour");
    let job = ExportJob {
        source_path: PathBuf::from("pour.cypcb"),
        output_dir: output_dir.clone(),
        preset,
        board_name: "pour".to_string(),
    };
    let result = run_export(&job, &mut world, &library).expect("the export runs");
    let _ = std::fs::remove_dir_all(&output_dir);

    assert!(
        result.warnings.iter().any(|w| w.contains("thermal relief")),
        "what the pour cannot do yet has to be named: {:?}",
        result.warnings
    );

    // And the pour itself reaches the copper layer.
    let format = cypcb_export::coords::CoordinateFormat::FORMAT_MM_2_6;
    let copper =
        cypcb_export::gerber::export_copper_layer(&mut world, &library, Layer::TopCopper, &format)
            .expect("top copper");
    assert!(
        copper.contains("G36*") && copper.contains("G37*"),
        "a declared pour has to be filled:\n{copper}"
    );
}

#[test]
fn a_pour_keeps_clear_of_other_nets_and_reaches_its_own() {
    // The pour is a net. Copper on another net has to stay outside it or the
    // plane shorts the board it was meant to ground; copper on its own net has
    // to be inside it, or the plane grounds nothing.
    use cypcb_core::Rect;
    use cypcb_world::components::zone::{Zone, ZoneKind};

    let (mut world, library) = board();
    // R1's pads are net 1 and net 2; the pour is net 1.
    world.spawn_entity(Zone {
        bounds: Rect::from_center_size(
            Point::from_mm(10.0, 10.0),
            (Nm::from_mm(16.0), Nm::from_mm(16.0)),
        ),
        kind: ZoneKind::CopperPour,
        layer_mask: Layer::TopCopper.to_copper_mask(),
        name: Some("GND_POUR".to_string()),
        net: Some(NetId::new(1)),
    });

    let format = cypcb_export::coords::CoordinateFormat::FORMAT_MM_2_6;
    let copper = cypcb_export::gerber::export_copper_layer_with(
        &mut world,
        &library,
        Layer::TopCopper,
        &format,
        &cypcb_export::pour::PourOptions::default(),
    )
    .expect("top copper");

    let regions = copper.matches("G36*").count();
    assert!(regions > 0, "the pour produced no copper:\n{copper}");

    // Read the regions back as rectangles. Checking a coordinate alone is not
    // enough - a band running under an obstacle legitimately spans the same x -
    // so the question is whether any region overlaps the pad's keepout.
    let mut rects: Vec<(f64, f64, f64, f64)> = Vec::new();
    let mut corners: Vec<(f64, f64)> = Vec::new();
    let mut inside = false;
    for line in copper.lines() {
        if line.starts_with("G36") {
            inside = true;
            corners.clear();
        } else if line.starts_with("G37") {
            inside = false;
            if !corners.is_empty() {
                let xs: Vec<f64> = corners.iter().map(|c| c.0).collect();
                let ys: Vec<f64> = corners.iter().map(|c| c.1).collect();
                rects.push((
                    xs.iter().cloned().fold(f64::MAX, f64::min),
                    ys.iter().cloned().fold(f64::MAX, f64::min),
                    xs.iter().cloned().fold(f64::MIN, f64::max),
                    ys.iter().cloned().fold(f64::MIN, f64::max),
                ));
            }
        } else if inside && line.starts_with('X') {
            let rest = &line[1..];
            if let Some((x, tail)) = rest.split_once('Y') {
                let y: String = tail
                    .chars()
                    .take_while(|c| c.is_ascii_digit() || *c == '.')
                    .collect();
                if let (Ok(x), Ok(y)) = (x.parse::<f64>(), y.parse::<f64>()) {
                    corners.push((x, y));
                }
            }
        }
    }
    assert!(!rects.is_empty(), "no regions parsed back:\n{copper}");

    // R1 sits at 8mm, 10mm; its pad 2 is on net 2, a foreign net, at +0.5mm,
    // and measures 0.6 x 0.5mm. With 0.3mm of clearance the pour must stay out
    // of 7.9..9.1 by 9.45..10.55.
    let keepout = (7.9_f64, 9.45_f64, 9.1_f64, 10.55_f64);
    for r in &rects {
        let overlap = r.0 < keepout.2 - 1e-9
            && keepout.0 < r.2 - 1e-9
            && r.1 < keepout.3 - 1e-9
            && keepout.1 < r.3 - 1e-9;
        assert!(
            !overlap,
            "a region {r:?} reaches into the keepout {keepout:?} around a foreign pad"
        );
    }

    // And the pour's own net is reached, by spokes rather than solid copper.
    // R1 pad 1 sits at 7.5mm, 10mm; a spoke crosses its centre, so some region
    // contains that point - and the ring around it is open, so no region covers
    // the whole thermal gap.
    let own_pad = (7.5_f64, 10.0_f64);
    let bridged = rects
        .iter()
        .any(|r| r.0 <= own_pad.0 && own_pad.0 <= r.2 && r.1 <= own_pad.1 && own_pad.1 <= r.3);
    assert!(
        bridged,
        "no spoke reaches the pad on the pour's own net, so it grounds nothing: {rects:?}"
    );

    // A corner of the gap ring, diagonally out from the pad and clear of both
    // spokes: solid flooding would cover it, a thermal relief does not.
    let corner = (own_pad.0 + 0.45, own_pad.1 + 0.4);
    let flooded = rects
        .iter()
        .any(|r| r.0 <= corner.0 && corner.0 <= r.2 && r.1 <= corner.1 && corner.1 <= r.3);
    assert!(
        !flooded,
        "the pad is flooded solid instead of relieved: {corner:?} is covered by {rects:?}"
    );
}
