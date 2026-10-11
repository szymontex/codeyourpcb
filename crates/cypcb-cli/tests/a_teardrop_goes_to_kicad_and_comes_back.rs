//! A teardrop goes to KiCad as the zone KiCad keeps it as, and comes back.
//!
//! `cargo test -p cypcb-cli --test a_teardrop_goes_to_kicad_and_comes_back`
//!
//! `to-kicad` used to say the fillets a design asks for were not in the board
//! it wrote. KiCad keeps a teardrop as a zone on the layer and net of its track
//! (`createTeardrop` in `pcbnew/teardrop/teardrop.cpp`), and the writer now puts
//! ours there in the form `PCB_IO_KICAD_SEXPR::format( const ZONE* )` writes
//! one: `(attr (teardrop (type padvia)))`, no uuid and no name, a zero
//! clearance, full pad connection, the polygon as outline and as fill. Both
//! files were read from the KiCad master branch on 2026-10-11.
//!
//! What is held here: the zone has the parts KiCad writes and none it leaves
//! out, one per fillet; each layer numbers its zones the way
//! `TEARDROP_MANAGER::setTeardropPriorities` does, from 30000 up by decreasing
//! area, because zones of equal priority on different nets do not clear each
//! other; and the board read back from the file holds the same teardrops - the
//! count, each outline to the nanometre, the layer and the net.

use std::path::{Path, PathBuf};
use std::process::Command;

use cypcb_world::teardrop::teardrop_copper;
use cypcb_world::{sync_ast_to_world, BoardWorld};

fn example(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples")
        .join(name)
}

fn scratch(who: &str) -> cypcb_fixtures::ScratchPath {
    let home = cypcb_fixtures::scratch_dir(&format!("cypcb-teardrop-kicad-{who}"));
    let dir = home.join("out");
    std::fs::create_dir_all(&dir).expect("the directory is creatable");
    home.holding(dir)
}

/// The design, asking for teardrops, written beside the zone file.
fn design(who: &str) -> (PathBuf, String) {
    let source = std::fs::read_to_string(example("usb-diff-pair.cypcb")).expect("the example");
    let asked = source.replace("    fab jlcpcb", "    fab jlcpcb\n    teardrops");
    assert_ne!(asked, source, "the example carries the line this edits");
    let home = cypcb_fixtures::scratch_dir(&format!("cypcb-teardrop-kicad-design-{who}"));
    let path = home.join("design.cypcb");
    std::fs::write(&path, &asked).expect("the design is writable");
    // The scratch directory is removed when its handle drops; keep it for the
    // life of the test.
    std::mem::forget(home);
    (path, asked)
}

fn world_of(source: &str) -> BoardWorld {
    let parsed = cypcb_parser::parse(source);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors.first());
    let mut world = BoardWorld::new();
    let mut library = cypcb_world::footprint::FootprintLibrary::new();
    let result = sync_ast_to_world(&parsed.value, source, &mut world, &mut library);
    assert!(result.errors.is_empty(), "{:?}", result.errors.first());
    world.rebuild_spatial_index_from_library(&library);
    world
}

fn to_kicad(design: &Path, out: &Path) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .env("SOURCE_DATE_EPOCH", "0")
        .arg("to-kicad")
        .arg(design)
        .arg("-o")
        .arg(out)
        .output()
        .expect("the binary runs");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stderr).to_string()
}

/// The text of every `(zone ...)` block that is a teardrop.
fn teardrop_zones(board: &str) -> Vec<String> {
    let mut zones = Vec::new();
    let mut rest = board;
    while let Some(at) = rest.find("\n  (zone\n") {
        let tail = &rest[at + 1..];
        let end = tail.find("\n  )\n").map(|e| e + 5).unwrap_or(tail.len());
        let block = &tail[..end];
        if block.contains("(attr (teardrop") {
            zones.push(block.to_string());
        }
        // Keep the newline the next block is found by.
        rest = &tail[end - 1..];
    }
    zones
}

#[test]
fn each_fillet_is_one_zone_with_the_parts_kicad_writes() {
    let (path, source) = design("zone");
    let mut world = world_of(&source);
    let held = teardrop_copper(&mut world);
    assert!(held.len() >= 2, "the design holds fillets: {}", held.len());

    let out = scratch("zone");
    to_kicad(&path, &out.join("board.kicad_pcb"));
    let board = std::fs::read_to_string(out.join("board.kicad_pcb")).expect("written");
    let zones = teardrop_zones(&board);
    assert_eq!(zones.len(), held.len(), "one zone per fillet");

    for zone in &zones {
        for part in [
            "(attr (teardrop (type padvia)))",
            "(connect_pads yes (clearance 0))",
            // KiCad's zone minimum thickness, 0.0254mm.
            "(min_thickness 0.0254)",
            "(fill yes (island_removal_mode 1))",
            "(hatch none 0.1)",
            "(priority 3",
            "(filled_polygon",
        ] {
            assert!(zone.contains(part), "{part} in {zone}");
        }
        // What KiCad leaves out of a teardrop zone: its uuid, a name and the
        // thermal figures.
        for absent in ["(uuid", "(name", "thermal_gap", "thermal_bridge_width"] {
            assert!(!zone.contains(absent), "{absent} in {zone}");
        }
        // The polygon twice, once as the outline and once as the fill, the same.
        let pts: Vec<&str> = zone
            .lines()
            .filter(|l| l.trim_start().starts_with("(pts"))
            .collect();
        assert_eq!(pts.len(), 2, "{zone}");
        assert_eq!(pts[0].trim(), pts[1].trim(), "{zone}");
    }
}

#[test]
fn the_board_read_back_holds_the_same_teardrops() {
    let (path, source) = design("trip");
    let mut before = world_of(&source);
    let held = teardrop_copper(&mut before);
    assert!(held.len() >= 2);

    let out = scratch("trip");
    let file = out.join("board.kicad_pcb");
    to_kicad(&path, &file);

    let mut parsed = cypcb_kicad::parse_kicad_pcb(&file).expect("the board reads");
    assert_eq!(parsed.metadata.teardrop_count, held.len());
    let read = teardrop_copper(&mut parsed.world);
    assert_eq!(read.len(), held.len(), "the count");

    let describe = |world: &BoardWorld, copper: &cypcb_world::teardrop::TeardropCopper, net| {
        (
            copper.layer,
            world.net_name(net).unwrap_or_default().to_string(),
            copper
                .outline
                .iter()
                .map(|p| (p.x.0, p.y.0))
                .collect::<Vec<_>>(),
        )
    };
    let mut went: Vec<_> = held
        .iter()
        .map(|(_, c, n)| describe(&before, c, *n))
        .collect();
    let mut came: Vec<_> = read
        .iter()
        .map(|(_, c, n)| describe(&parsed.world, c, *n))
        .collect();
    went.sort_by_key(|(layer, net, outline)| {
        (layer.to_copper_mask(), net.clone(), outline.clone())
    });
    came.sort_by_key(|(layer, net, outline)| {
        (layer.to_copper_mask(), net.clone(), outline.clone())
    });
    assert_eq!(went, came, "layer, net and every corner, to the nanometre");
}

#[test]
fn from_kicad_says_the_language_cannot_state_them() {
    let (path, _) = design("said");
    let out = scratch("said");
    let file = out.join("board.kicad_pcb");
    to_kicad(&path, &file);

    let written = out.join("back.cypcb");
    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .arg("from-kicad")
        .arg(&file)
        .arg("-o")
        .arg(&written)
        .output()
        .expect("the binary runs");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let said = String::from_utf8_lossy(&output.stderr);
    assert!(
        said.contains("teardrop(s) read from the board not written"),
        "{said}"
    );
    assert!(written.exists(), "the board still arrives");
}

/// Three nets on two layers: two on the top copper, at two track widths, and
/// one on the bottom. A wider track makes a larger fillet.
const TWO_LAYERS: &str = "version 1

board priorities {
    size 40mm x 30mm
    layers 2
    teardrops
}

component J1 connector \"PIN-HDR-1x2\" {
    at 4mm, 6mm
    rotate 90
}
component J2 connector \"PIN-HDR-1x2\" {
    at 20mm, 6mm
    rotate 90
}
component J3 connector \"PIN-HDR-1x2\" {
    at 4mm, 14mm
    rotate 90
}
component J4 connector \"PIN-HDR-1x2\" {
    at 20mm, 14mm
    rotate 90
}
component J5 connector \"PIN-HDR-1x2\" {
    at 4mm, 22mm
    rotate 90
}
component J6 connector \"PIN-HDR-1x2\" {
    at 20mm, 22mm
    rotate 90
}

net A { J1.1 J2.1 }
net B { J3.1 J4.1 }
net C { J5.1 J6.1 }

trace A { from J1.1 to J2.1 layer Top width 0.2mm }
trace B { from J3.1 to J4.1 layer Top width 0.3mm }
trace C { from J5.1 to J6.1 layer Bottom width 0.25mm }
";

/// The layer, the priority and twice the area of every teardrop zone, the area
/// read from the written polygon.
fn priorities(board: &str) -> Vec<(String, u32, i128)> {
    teardrop_zones(board)
        .iter()
        .map(|zone| {
            let after = |key: &str| zone.split(key).nth(1).expect(key);
            let layer = after("(layer \"").split('"').next().unwrap().to_string();
            let priority = after("(priority ")
                .split(')')
                .next()
                .unwrap()
                .parse()
                .expect("a number");
            let pts = zone
                .lines()
                .find(|l| l.trim_start().starts_with("(pts"))
                .expect("the polygon");
            let corners: Vec<(f64, f64)> = pts
                .split("(xy ")
                .skip(1)
                .map(|c| {
                    let mut n = c.trim_end_matches([')', ' ']).split(' ');
                    (
                        n.next().unwrap().parse().unwrap(),
                        n.next().unwrap().parse().unwrap(),
                    )
                })
                .collect();
            let mut sum = 0.0;
            for i in 0..corners.len() {
                let (a, b) = (corners[i], corners[(i + 1) % corners.len()]);
                sum += a.0 * b.1 - b.0 * a.1;
            }
            // Square micrometres, so the sum compares as an integer.
            (layer, priority, (sum.abs() * 1.0e6).round() as i128)
        })
        .collect()
}

#[test]
fn each_layer_numbers_its_zones_from_30000_by_decreasing_area() {
    let home = cypcb_fixtures::scratch_dir("cypcb-teardrop-kicad-priority");
    let design = home.join("design.cypcb");
    std::fs::write(&design, TWO_LAYERS).expect("the design is writable");
    let out = scratch("priority");
    let file = out.join("board.kicad_pcb");
    to_kicad(&design, &file);
    let board = std::fs::read_to_string(&file).expect("written");

    let written = priorities(&board);
    for (layer, want) in [("F.Cu", 4usize), ("B.Cu", 2)] {
        let mut here: Vec<_> = written.iter().filter(|(l, _, _)| l == layer).collect();
        assert_eq!(
            here.len(),
            want,
            "{layer} holds {want} fillets: {written:?}"
        );
        here.sort_by_key(|(_, priority, _)| *priority);
        let numbers: Vec<u32> = here.iter().map(|(_, p, _)| *p).collect();
        let expected: Vec<u32> = (30000..30000 + want as u32).collect();
        assert_eq!(
            numbers, expected,
            "{layer} counts up from 30000 with no gap"
        );
        for pair in here.windows(2) {
            assert!(
                pair[0].2 >= pair[1].2,
                "{layer}: priority {} has area {}, priority {} has {}",
                pair[0].1,
                pair[0].2,
                pair[1].1,
                pair[1].2
            );
        }
    }
    // The two widths on the top are two sizes, so the order is not the order of
    // the file: the larger pair is first.
    let mut top: Vec<_> = written.iter().filter(|(l, _, _)| l == "F.Cu").collect();
    top.sort_by_key(|(_, priority, _)| *priority);
    assert!(
        top[0].2 > top[3].2,
        "the fillets on the top differ in size: {top:?}"
    );
}
