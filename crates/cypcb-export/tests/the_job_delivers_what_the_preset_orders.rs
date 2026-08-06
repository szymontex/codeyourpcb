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
