//! The copper already in a KiCad file arrived offset by the whole board
//! origin.
//!
//! `cargo test -p cypcb-kicad --test imported_copper_lands_on_the_board`
//!
//! Every pad had its position taken relative to the board's own corner -
//! `parse_footprint` subtracts `board_origin` - and `parse_segment` and
//! `parse_via` did not, because neither was given it. KiCad lays boards out on
//! a sheet, so that corner is almost never at the file's origin: `led_blink`
//! has its outline at (95, 55) and its traces at (110, 62). They imported onto
//! a 40 x 30mm board at 110mm and 62mm.
//!
//! What that looked like from outside, before:
//!
//! ```text
//! edge-clearance at (105.000mm, 72.770mm): trace 'GND': 0.00mm actual, 0.30mm required
//! edge-clearance at (110.000mm, 70.000mm): via 'VCC':   0.00mm actual, 0.30mm required
//! ...
//! Summary: edge-clearance: 5, unrouted-pin: 12
//! ```
//!
//! Five pieces of copper hanging off the edge of a board they were nowhere
//! near, and twelve pins reported unreached because the copper meant to reach
//! them was 95mm away. After: no edge violations and eight unreached pins, the
//! four difference being pins the file's own traces do connect.
//!
//! This was invisible for as long as it was because the importer's copper only
//! ever fed benchmarks, and the benchmark harness routes from scratch.

use cypcb_kicad::parse_kicad_pcb;

use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .join("tests/fixtures/benchmark")
        .join(name)
}

#[test]
fn every_imported_trace_and_via_is_inside_the_board() {
    // One fixture carries copper in the file now. Three did until 2026-08-08,
    // and what they carried was straight lines between part centres, crossing
    // every package on the way - 1, 13 and 32 shorts before anything was
    // routed. `led_blink` keeps one real trace instead: R1 pad 2 to D1 pad 1,
    // both on LED_ANODE.
    for name in ["led_blink.kicad_pcb"] {
        let parsed = parse_kicad_pcb(&fixture(name)).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        let routes = parsed
            .reference_routes
            .unwrap_or_else(|| panic!("{name} carries copper and none was read"));
        let (size, _) = parsed
            .world
            .board_info()
            .unwrap_or_else(|| panic!("{name} states no board"));
        let (w, h) = (size.width.to_mm(), size.height.to_mm());

        assert!(
            !routes.routes.is_empty(),
            "{name} has segments in the file and none reached the model"
        );

        for segment in &routes.routes {
            for (label, point) in [("start", segment.start), ("end", segment.end)] {
                let (x, y) = (point.x.to_mm(), point.y.to_mm());
                assert!(
                    (-1.0..=w + 1.0).contains(&x) && (-1.0..=h + 1.0).contains(&y),
                    "{name}: a trace {label} is at ({x:.3}mm, {y:.3}mm) on a \
                     board {w} x {h}mm. The board's corner is not at the file's \
                     origin, and this copper kept the file's coordinates."
                );
            }
        }

        for via in &routes.vias {
            let (x, y) = (via.position.x.to_mm(), via.position.y.to_mm());
            assert!(
                (-1.0..=w + 1.0).contains(&x) && (-1.0..=h + 1.0).contains(&y),
                "{name}: a via is at ({x:.3}mm, {y:.3}mm) on a board {w} x {h}mm"
            );
        }
    }
}

#[test]
fn imported_copper_sits_where_the_pads_it_joins_are() {
    // Inside the outline is necessary and not sufficient: copper shifted by
    // some other amount would still land on the board and connect nothing.
    // `led_blink`'s trace runs from (120.48, 65) to (124.25, 65) in a file
    // whose board corner is at (95, 55), so it belongs at (25.48, 10) to
    // (29.25, 10).
    let parsed = parse_kicad_pcb(&fixture("led_blink.kicad_pcb")).expect("led_blink parses");
    let routes = parsed.reference_routes.expect("led_blink carries copper");

    let first = routes
        .routes
        .iter()
        .find(|segment| (segment.start.x.to_mm() - 25.48).abs() < 0.001)
        .unwrap_or_else(|| {
            panic!(
                "no segment starts at 25.48mm; got {:?}",
                routes
                    .routes
                    .iter()
                    .map(|s| (s.start.x.to_mm(), s.start.y.to_mm()))
                    .collect::<Vec<_>>()
            )
        });

    assert!((first.start.y.to_mm() - 10.0).abs() < 0.001, "{first:?}");
    assert!((first.end.x.to_mm() - 29.25).abs() < 0.001, "{first:?}");
    assert!((first.end.y.to_mm() - 10.0).abs() < 0.001, "{first:?}");
}
