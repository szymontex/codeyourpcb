//! A board saved as `.cypcb` is the board it was.
//!
//! `cargo test -p cypcb-cli --test a_saved_board_is_the_board_it_was`
//!
//! Every board in the benchmark and every example goes through the trip a save
//! makes: world, `board_as_dsl`, parse, world. The two worlds are compared
//! kind by kind as sorted text - parts, pads, courtyards, legend, pins, nets
//! with what they ask for, copper, vias, zones, outline, words, measurements,
//! the board block, assertions - not as counts, because a count survives a
//! courtyard that moved. Measured before this file: every count matched on
//! all 34 boards that load, while 19 courtyards on the six KiCad boards had
//! moved, three assertions and a part's `spec` block were gone.
//!
//! And every ECS component type any of those worlds holds has to be on the
//! list below, which says how the file carries it. A new kind of thing in the
//! model that nobody taught the writer is a new type, and it turns this red.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use cypcb_world::components::trace::{Trace, TraceSource, Via};
use cypcb_world::components::zone::Zone;
use cypcb_world::components::{
    BoardDimension, BoardOutline, BoardText, FootprintRef, Hatch, LcscPart, NetConnections, RefDes,
    Rotation, Side, StitchPitch, Value,
};
use cypcb_world::dsl::{board_as_dsl, board_as_dsl_reporting};
use cypcb_world::footprint::{base_name, FootprintLibrary};
use cypcb_world::{sync_ast_to_world, BoardWorld, NetId};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The benchmark and the examples: every board this repository ships.
fn boards() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for dir in ["tests/fixtures/benchmark", "examples"] {
        for entry in std::fs::read_dir(repo_root().join(dir)).expect("the directory") {
            let path = entry.expect("an entry").path();
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext == "cypcb" || ext == "kicad_pcb" {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

fn name_of(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .expect("a name")
        .to_string()
}

/// The boards this cannot ask, and why - the same three as
/// `every_example_survives_being_saved`.
const CANNOT_BE_ASKED: &[&str] = &[
    // Written to fail parsing.
    "invalid.cypcb",
    "unknown_keyword.cypcb",
    // Its `import` lines need the host that resolves them.
    "v2-imports.cypcb",
];

fn load(path: &Path) -> Option<BoardWorld> {
    if path.extension().is_some_and(|e| e == "kicad_pcb") {
        load_kicad(path).ok()
    } else {
        load_dsl(&std::fs::read_to_string(path).expect("the board reads")).ok()
    }
}

fn load_dsl(source: &str) -> Result<BoardWorld, String> {
    let parsed = cypcb_parser::parse(source);
    if !parsed.errors.is_empty() {
        return Err(format!("parse: {:?}", parsed.errors.first()));
    }
    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    let r = sync_ast_to_world(&parsed.value, source, &mut world, &mut library);
    if !r.errors.is_empty() {
        return Err(format!("sync: {:?}", r.errors.first()));
    }
    Ok(world)
}

fn load_kicad(path: &Path) -> Result<BoardWorld, String> {
    let parsed = cypcb_kicad::parse_kicad_pcb(path).map_err(|e| e.to_string())?;
    let mut world = parsed.world;
    if let Some(routes) = parsed.reference_routes {
        cypcb_router::apply_routes_as(
            &mut world,
            &routes,
            cypcb_world::components::trace::TraceSource::Manual,
        );
    }
    Ok(world)
}

type Content = BTreeMap<&'static str, Vec<String>>;

/// What each kind holds, as sorted text, with nets named rather than numbered.
fn content(world: &mut BoardWorld) -> Content {
    let mut c = Content::new();
    let names: std::collections::HashMap<u32, String> =
        world.nets().map(|(id, n)| (id.0, n.to_string())).collect();
    let net = |id: u32| names.get(&id).cloned().unwrap_or_else(|| format!("#{id}"));
    let builtin = FootprintLibrary::new();
    let library = world.footprints().clone();
    let parts: Vec<(String, String, String, String)> = {
        let ecs = world.ecs_mut();
        let mut q = ecs.query::<(
            &RefDes,
            &FootprintRef,
            &cypcb_world::components::Position,
            &Rotation,
            Option<&Side>,
            Option<&Value>,
            Option<&LcscPart>,
            Option<&NetConnections>,
            Option<&cypcb_world::components::PartSpec>,
        )>();
        q.iter(ecs)
            .map(|(r, fp, pos, rot, side, v, l, n, spec)| {
                let mut pins: Vec<String> = n
                    .map(|n| n.iter().map(|c| format!("{}={}", c.pin, c.net.0)).collect())
                    .unwrap_or_default();
                pins.sort();
                (
                    r.0.clone(),
                    fp.0.clone(),
                    format!(
                        "{}|{:?}|{}|{:?}|{:?}|{:?}|{:?}",
                        r.0,
                        pos.0,
                        rot.0,
                        side,
                        v.map(|v| &v.0),
                        l.map(|l| &l.0),
                        spec
                    ),
                    pins.join(","),
                )
            })
            .collect()
    };
    let mut part_rows = Vec::new();
    let mut pad_rows = Vec::new();
    let mut silk_rows = Vec::new();
    let mut pin_rows = Vec::new();
    let mut layer_rows = Vec::new();
    for (refdes, fp, row, pins) in &parts {
        part_rows.push(row.clone());
        for pin in pins.split(',').filter(|p| !p.is_empty()) {
            let (pin, id) = pin.split_once('=').unwrap();
            pin_rows.push(format!("{refdes}.{pin}={}", net(id.parse().unwrap())));
        }
        if let Some(f) = library
            .get(fp)
            .or_else(|| library.get(base_name(fp)))
            .or_else(|| builtin.get(base_name(fp)))
        {
            for pad in &f.pads {
                let mut bare = pad.clone();
                bare.layers.clear();
                pad_rows.push(format!("{refdes}|{bare:?}"));
                layer_rows.push(format!("{refdes}|{}|{:?}", pad.number, stated_layers(pad)));
            }
            for shape in &f.silk {
                silk_rows.push(format!("{refdes}|{shape:?}"));
            }
            pad_rows.push(format!("{refdes}|courtyard {:?}", f.courtyard));
        }
    }
    c.insert("parts", part_rows);
    c.insert("pads", pad_rows);
    c.insert("pad_layers", layer_rows);
    c.insert("silk_shapes", silk_rows);
    c.insert("pins_on_nets", pin_rows);
    let ids: Vec<_> = world.nets().map(|(id, n)| (id, n.to_string())).collect();
    c.insert(
        "nets",
        ids.iter()
            .map(|(id, n)| format!("{n}|{:?}", world.net_constraints(*id)))
            .collect(),
    );
    let traces: Vec<String> = {
        let ecs = world.ecs_mut();
        let mut q = ecs.query::<&Trace>();
        q.iter(ecs)
            .flat_map(|t| {
                t.segments
                    .iter()
                    .map(|s| format!("{}|{:?}|{:?}|{:?}", net(t.net_id.0), t.layer, t.width, s))
                    .collect::<Vec<_>>()
            })
            .collect()
    };
    c.insert("segments", traces);
    let vias: Vec<String> = {
        let ecs = world.ecs_mut();
        let mut q = ecs.query::<&Via>();
        q.iter(ecs)
            .map(|v| {
                format!(
                    "{}|{:?}|{:?}|{:?}|{:?}|{:?}|{}",
                    net(v.net_id.0),
                    v.position,
                    v.drill,
                    v.outer_diameter,
                    v.start_layer,
                    v.end_layer,
                    v.locked
                )
            })
            .collect()
    };
    c.insert("vias", vias);
    let zones: Vec<String> = {
        let ecs = world.ecs_mut();
        let mut q = ecs.query::<(
            &Zone,
            Option<&StitchPitch>,
            Option<&Hatch>,
            Option<&cypcb_world::components::BendRadius>,
        )>();
        q.iter(ecs)
            .map(|(z, s, h, r)| {
                format!(
                    "{:?}|{:?}|{}|{:?}|{:?}|{:?}|{:?}|{:?}",
                    z.kind,
                    z.bounds,
                    z.layer_mask,
                    z.net.map(|n| net(n.0)),
                    z.name,
                    s.map(|s| s.0),
                    h,
                    r.map(|r| r.0)
                )
            })
            .collect()
    };
    c.insert("zones", zones);
    let outline = world
        .board_entity()
        .and_then(|e| world.ecs().get::<BoardOutline>(e))
        .map(|o| o.points.iter().map(|p| format!("{p:?}")).collect())
        .unwrap_or_default();
    c.insert("outline_points", outline);
    let texts: Vec<String> = {
        let ecs = world.ecs_mut();
        let mut q = ecs.query::<&BoardText>();
        q.iter(ecs).map(|t| format!("{t:?}")).collect()
    };
    c.insert("texts", texts);
    let dims: Vec<String> = {
        let ecs = world.ecs_mut();
        let mut q = ecs.query::<&BoardDimension>();
        q.iter(ecs).map(|d| format!("{d:?}")).collect()
    };
    c.insert("dimensions", dims);
    let spanless = |value: serde_json::Value| without_spans(value).to_string();
    c.insert(
        "assertions",
        world
            .assertions()
            .iter()
            .map(|a| spanless(serde_json::to_value(&a.expression).expect("serializes")))
            .collect(),
    );
    c.insert(
        "diff_pairs",
        world
            .diff_pairs()
            .iter()
            .map(|p| spanless(serde_json::to_value(p).expect("serializes")))
            .collect(),
    );
    c.insert(
        "board",
        vec![format!(
            "{:?}|{:?}|{:?}|{:?}",
            world.board_info(),
            world.stackup(),
            world.fab(),
            world.teardrops()
        )],
    );
    for rows in c.values_mut() {
        rows.sort();
    }
    c
}

/// Rows on one side and not the other, as a multiset.
fn differing(a: &[String], b: &[String]) -> usize {
    let mut left: BTreeMap<&String, i64> = BTreeMap::new();
    for r in a {
        *left.entry(r).or_default() += 1;
    }
    for r in b {
        *left.entry(r).or_default() -= 1;
    }
    left.values().map(|n| n.unsigned_abs() as usize).sum()
}

/// A pad's layers as the language states them.
///
/// A drilled pad has no layer list in the language: it is on every copper
/// layer and opens the mask on both sides, and the mask gerber finds it
/// through its copper. KiCad's importer lists `TopMask` and `BottomMask` and,
/// on a four-layer board, only the outer copper - it reads `*.Cu` as two
/// layers - so a drilled pad is compared by what it is: drilled.
fn stated_layers(pad: &cypcb_world::footprint::PadDef) -> Vec<cypcb_world::Layer> {
    if pad.drill.is_some() {
        Vec::new()
    } else {
        pad.layers.clone()
    }
}

fn without_spans(mut value: serde_json::Value) -> serde_json::Value {
    fn strip(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Object(map) => {
                map.remove("span");
                map.values_mut().for_each(strip);
            }
            serde_json::Value::Array(items) => items.iter_mut().for_each(strip),
            _ => {}
        }
    }
    strip(&mut value);
    value
}

/// How the file carries each ECS component type, by its full path.
///
/// `Written`: the writer states it, and a board has as many after the trip as
/// before. `FromText`: made by the reader out of words the file already has -
/// a value's quantity, the kind a `component` line names, where in the text a
/// part was declared - so a board read from KiCad gains them and loses none.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Carried {
    Written,
    FromText,
}

const TYPES: &[(&str, Carried)] = &[
    ("cypcb_world::components::board::Board", Carried::Written),
    (
        "cypcb_world::components::board::BoardDimension",
        Carried::Written,
    ),
    (
        "cypcb_world::components::board::BoardOutline",
        Carried::Written,
    ),
    (
        "cypcb_world::components::board::BoardSize",
        Carried::Written,
    ),
    (
        "cypcb_world::components::board::BoardText",
        Carried::Written,
    ),
    ("cypcb_world::components::board::Fab", Carried::Written),
    (
        "cypcb_world::components::board::LayerStack",
        Carried::Written,
    ),
    ("cypcb_world::components::board::Stackup", Carried::Written),
    (
        "cypcb_world::components::board::Teardrops",
        Carried::Written,
    ),
    (
        "cypcb_world::components::electrical::LcscPart",
        Carried::Written,
    ),
    (
        "cypcb_world::components::electrical::NetConnections",
        Carried::Written,
    ),
    (
        "cypcb_world::components::electrical::NetId",
        Carried::Written,
    ),
    (
        "cypcb_world::components::electrical::PartSpec",
        Carried::Written,
    ),
    (
        "cypcb_world::components::electrical::RefDes",
        Carried::Written,
    ),
    (
        "cypcb_world::components::electrical::TypedValue",
        Carried::FromText,
    ),
    (
        "cypcb_world::components::electrical::Value",
        Carried::Written,
    ),
    (
        "cypcb_world::components::metadata::ComponentKind",
        Carried::FromText,
    ),
    ("cypcb_world::components::metadata::Name", Carried::Written),
    (
        "cypcb_world::components::metadata::SourceSpan",
        Carried::FromText,
    ),
    (
        "cypcb_world::components::physical::FootprintRef",
        Carried::Written,
    ),
    ("cypcb_world::components::physical::Side", Carried::Written),
    (
        "cypcb_world::components::position::Position",
        Carried::Written,
    ),
    (
        "cypcb_world::components::position::Rotation",
        Carried::Written,
    ),
    ("cypcb_world::components::trace::Curve", Carried::Written),
    ("cypcb_world::components::trace::Trace", Carried::Written),
    (
        "cypcb_world::components::trace::TraceNeck",
        Carried::Written,
    ),
    ("cypcb_world::components::trace::Via", Carried::Written),
    (
        "cypcb_world::components::zone::BendRadius",
        Carried::Written,
    ),
    ("cypcb_world::components::zone::Hatch", Carried::Written),
    ("cypcb_world::components::zone::Stitched", Carried::Written),
    (
        "cypcb_world::components::zone::StitchPitch",
        Carried::Written,
    ),
    ("cypcb_world::components::zone::Zone", Carried::Written),
];

/// Entities per ECS component type, whatever the type is.
fn by_type(world: &BoardWorld) -> BTreeMap<String, usize> {
    let ecs = world.ecs();
    let mut out = BTreeMap::new();
    for archetype in ecs.archetypes().iter() {
        if archetype.is_empty() {
            continue;
        }
        for id in archetype.components() {
            let name = ecs
                .components()
                .get_info(id)
                .map(|info| info.name().to_string())
                .unwrap_or_default();
            *out.entry(name).or_default() += archetype.len();
        }
    }
    out
}

/// Every board that loads, before and after the trip.
fn trips() -> Vec<(String, BoardWorld, BoardWorld)> {
    let mut out = Vec::new();
    for path in boards() {
        let name = name_of(&path);
        if CANNOT_BE_ASKED.contains(&name.as_str()) {
            continue;
        }
        let mut before =
            load(&path).unwrap_or_else(|| panic!("{name} does not load and is not excused"));
        let written = board_as_dsl(&mut before);
        let after = load_dsl(&written)
            .unwrap_or_else(|e| panic!("{name}: the saved file does not load: {e}"));
        out.push((name, before, after));
    }
    out
}

#[test]
fn every_board_comes_back_the_board_it_was() {
    let mut failures = String::new();
    let mut boards = 0;
    for (name, mut before, mut after) in trips() {
        boards += 1;
        let (a, b) = (content(&mut before), content(&mut after));
        for (kind, rows) in &a {
            let empty = Vec::new();
            let other = b.get(kind).unwrap_or(&empty);
            if differing(rows, other) > 0 {
                let lost: Vec<&String> =
                    rows.iter().filter(|r| !other.contains(r)).take(2).collect();
                let got: Vec<&String> =
                    other.iter().filter(|r| !rows.contains(r)).take(2).collect();
                failures.push_str(&format!(
                    "{name} {kind}:\n  before {lost:?}\n  after  {got:?}\n"
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{failures}");
    assert_eq!(boards, boards_listed() - CANNOT_BE_ASKED.len());
}

fn boards_listed() -> usize {
    boards().len()
}

#[test]
fn every_kind_the_model_holds_is_one_the_writer_carries() {
    let mut failures = String::new();
    for (name, before, after) in trips() {
        let (a, b) = (by_type(&before), by_type(&after));
        for kind in a.keys().chain(b.keys()) {
            let (x, y) = (
                a.get(kind).copied().unwrap_or(0),
                b.get(kind).copied().unwrap_or(0),
            );
            match TYPES.iter().find(|(known, _)| known == kind).map(|(_, how)| *how) {
                None => failures.push_str(&format!(
                    "{name}: {kind} is in the model and nobody has said how a saved file carries it\n"
                )),
                Some(Carried::Written) if x != y => {
                    failures.push_str(&format!("{name}: {kind} {x} before, {y} after\n"))
                }
                Some(Carried::FromText) if y < x => {
                    failures.push_str(&format!("{name}: {kind} {x} before, {y} after\n"))
                }
                Some(_) => {}
            }
        }
    }
    assert!(failures.is_empty(), "{failures}");
}

#[test]
fn every_courtyard_that_is_not_centred_comes_back_where_it_was() {
    // The count is the guard against a check that checks nothing: a KiCad
    // footprint's origin is often pin 1, and these are the courtyards on the
    // six benchmark boards that do not sit on their origin.
    let mut off_centre = 0;
    for (name, mut before, mut after) in trips() {
        if !name.ends_with(".kicad_pcb") {
            continue;
        }
        let courtyards = |world: &mut BoardWorld| -> Vec<(String, cypcb_core::Rect)> {
            let library = world.footprints().clone();
            let ecs = world.ecs_mut();
            let mut query = ecs.query::<(&RefDes, &FootprintRef)>();
            let mut rows: Vec<(String, cypcb_core::Rect)> = query
                .iter(ecs)
                .filter_map(|(refdes, fp)| {
                    library
                        .get(&fp.0)
                        .or_else(|| library.get(base_name(&fp.0)))
                        .map(|f| (refdes.0.clone(), f.courtyard))
                })
                .collect();
            rows.sort_by(|a, b| a.0.cmp(&b.0));
            rows
        };
        let (a, b) = (courtyards(&mut before), courtyards(&mut after));
        assert_eq!(a, b, "{name}");
        off_centre += a
            .iter()
            .filter(|(_, rect)| rect.center() != cypcb_core::Point::ORIGIN)
            .count();
    }
    assert_eq!(off_centre, 21, "the benchmark's KiCad boards changed");
}

fn courtyard_rows(world: &mut BoardWorld) -> usize {
    let rules = cypcb_drc::DesignRules::default();
    cypcb_drc::run_drc(world, &rules)
        .violations
        .iter()
        .filter(|v| v.kind == cypcb_drc::ViolationKind::CourtyardClearance)
        .count()
}

/// The library with every courtyard moved onto its footprint's origin - what a
/// save that drops `at` hands back.
fn centred(library: &FootprintLibrary) -> FootprintLibrary {
    let mut out = library.clone();
    for (key, footprint) in library.iter() {
        let mut moved = footprint.clone();
        moved.name = key.to_string();
        moved.courtyard = cypcb_core::Rect::from_center_size(
            cypcb_core::Point::ORIGIN,
            (moved.courtyard.width(), moved.courtyard.height()),
        );
        out.register(moved);
    }
    out
}

#[test]
fn the_courtyard_rule_finds_the_same_overlaps_after_a_save() {
    // No benchmark board has two courtyards close enough to be reported, so
    // the rule gives none before and none after a save whether the courtyards
    // moved or not. This builds the case that tells them apart: `led_blink`'s
    // header, whose courtyard runs down from pin 1, with another part moved
    // until the rule reports the real courtyard and not the centred one.
    let path = repo_root().join("tests/fixtures/benchmark/led_blink.kicad_pcb");
    let mut world = load_kicad(&path).expect("led_blink loads");
    assert_eq!(
        courtyard_rows(&mut world),
        0,
        "the board as drawn overlaps nothing"
    );

    let parts: Vec<(cypcb_world::Entity, String, cypcb_core::Point)> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<(
            cypcb_world::Entity,
            &RefDes,
            &cypcb_world::components::Position,
        )>();
        query
            .iter(ecs)
            .map(|(e, r, p)| (e, r.0.clone(), p.0))
            .collect()
    };
    let header = parts.iter().find(|(_, r, _)| r == "J1").expect("J1").2;
    let real = world.footprints().clone();
    let moved_centred = centred(&real);

    let mut found = None;
    'search: for (entity, refdes, _) in parts.iter().filter(|(_, r, _)| r != "J1") {
        for step_x in -16..=16 {
            for step_y in -16..=16 {
                let at = cypcb_core::Point::new(
                    cypcb_core::Nm(header.x.0 + step_x * 500_000),
                    cypcb_core::Nm(header.y.0 + step_y * 500_000),
                );
                world
                    .ecs_mut()
                    .entity_mut(*entity)
                    .insert(cypcb_world::components::Position(at));
                let with_real = courtyard_rows(&mut world);
                world.set_footprints(moved_centred.clone());
                let with_centred = courtyard_rows(&mut world);
                world.set_footprints(real.clone());
                if with_real > 0 && with_real != with_centred {
                    found = Some((refdes.clone(), with_real, with_centred));
                    break 'search;
                }
            }
        }
    }
    let (refdes, with_real, with_centred) =
        found.expect("no place where the courtyard's position changes what the rule reports");

    let written = board_as_dsl(&mut world);
    let mut after = load_dsl(&written).expect("the saved file loads");
    assert_eq!(
        courtyard_rows(&mut after),
        with_real,
        "{refdes} beside J1: {with_real} rows before the save, {with_centred} with the courtyards centred"
    );
}

/// A small board to hang one thing on that the language cannot say.
fn small_board() -> BoardWorld {
    load_dsl(
        "version 1\nboard b {\n    size 20mm x 20mm\n    layers 2\n}\n\
         component R1 resistor \"0402\" {\n    at 5mm, 5mm\n}\n\
         component R2 resistor \"0402\" {\n    at 15mm, 5mm\n}\n\
         net DP {\n    R1.1\n}\nnet DM {\n    R2.1\n}\n\
         diffpair USB {\n    DP\n    DM\n}\n",
    )
    .expect("the small board loads")
}

fn not_written(world: &mut BoardWorld) -> Vec<String> {
    board_as_dsl_reporting(world).not_written
}

#[test]
fn a_whole_board_leaves_nothing_out() {
    // The control for the five below, and for the corpus: none of the boards
    // above holds anything the language cannot say.
    assert_eq!(not_written(&mut small_board()), Vec::<String>::new());
    for (name, mut before, _) in trips() {
        assert_eq!(not_written(&mut before), Vec::<String>::new(), "{name}");
    }
}

#[test]
fn a_zone_on_layers_the_language_cannot_name_is_reported() {
    let mut world = small_board();
    let bounds = cypcb_core::Rect::new(
        cypcb_core::Point::from_mm(1.0, 1.0),
        cypcb_core::Point::from_mm(4.0, 4.0),
    );
    world.spawn_entity((Zone::keepout(bounds, 0b100),));
    world.spawn_entity((Zone::keepout(bounds, 0b1000),));
    assert_eq!(
        not_written(&mut world),
        vec!["2 zone(s) not written: on a set of layers other than top, bottom or all".to_string()]
    );
}

#[test]
fn a_pair_whose_net_needs_quotes_is_reported() {
    let mut world = small_board();
    let mut pairs = world.diff_pairs().to_vec();
    pairs[0].positive.value = "D+".to_string();
    world.set_diff_pairs(pairs);
    assert_eq!(
        not_written(&mut world),
        vec!["1 diff pair(s) not written: a net name that needs quotes, which diffpair does not take"
            .to_string()]
    );
}

#[test]
fn a_via_with_a_ring_of_its_own_is_reported() {
    let mut world = small_board();
    let net = world.nets().next().map(|(id, _)| id).expect("a net");
    let mut own = Via::new(cypcb_core::Point::from_mm(10.0, 10.0), net);
    own.outer_diameter = cypcb_core::Nm::from_mm(0.45);
    world.spawn_entity((own, net));
    world.spawn_entity((Via::new(cypcb_core::Point::from_mm(12.0, 10.0), net), net));
    assert_eq!(
        not_written(&mut world),
        vec!["1 via(s) written with a ring of twice the drill instead of their own".to_string()]
    );
}

#[test]
fn an_autorouted_trace_is_reported() {
    let mut world = small_board();
    let net: NetId = world.nets().next().map(|(id, _)| id).expect("a net");
    let mut trace = Trace::new(net);
    trace.add_segment(cypcb_world::components::trace::TraceSegment::new(
        cypcb_core::Point::from_mm(5.0, 5.0),
        cypcb_core::Point::from_mm(15.0, 5.0),
    ));
    trace.source = TraceSource::Autorouted;
    world.spawn_entity((trace, net));
    assert_eq!(
        not_written(&mut world),
        vec!["1 autorouted trace(s) written as drawn by hand".to_string()]
    );
}

#[test]
fn a_text_off_the_legend_is_reported() {
    let mut world = small_board();
    world.spawn_entity((BoardText {
        content: "REV A".to_string(),
        position: cypcb_core::Point::from_mm(10.0, 15.0),
        layer: cypcb_world::Layer::TopCopper,
        height: BoardText::DEFAULT_HEIGHT,
    },));
    assert_eq!(
        not_written(&mut world),
        vec!["1 text(s) written on the top legend instead of their own layer".to_string()]
    );
}

#[test]
fn from_kicad_says_what_it_could_not_write() {
    // `led_blink` with one via whose ring is not twice its drill, which the
    // language cannot say yet. The control is the board without it.
    let dir = std::env::temp_dir().join(format!("from-kicad-says-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    let board =
        std::fs::read_to_string(repo_root().join("tests/fixtures/benchmark/led_blink.kicad_pcb"))
            .expect("led_blink reads");
    let board = board.trim_end();
    let with_ring = format!(
        "{}  (via (at 120 115) (size 0.45) (drill 0.2) (layers \"F.Cu\" \"B.Cu\") (net 1))\n)\n",
        board.strip_suffix(')').expect("the board closes")
    );
    let run = |name: &str, text: &str| -> String {
        let input = dir.join(name);
        std::fs::write(&input, text).expect("the board is written");
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_cypcb"))
            .arg("from-kicad")
            .arg(&input)
            .output()
            .expect("the binary runs");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stderr).to_string()
    };
    let said = run("with_ring.kicad_pcb", &with_ring);
    let plain = run("plain.kicad_pcb", board);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        said.contains(
            "Warning: 1 via(s) written with a ring of twice the drill instead of their own"
        ),
        "{said}"
    );
    assert!(!plain.contains("written"), "{plain}");
}
