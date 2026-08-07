//! The copper the router lays has to arrive in the fabrication files.
//!
//! `cargo test --release -p cypcb-autoroute --test routed_copper_reaches_the_files -- --ignored --nocapture`
//!
//! `cypcb-cli` checks this on one small board. The three benchmark fixtures
//! are the dense ones, they are routed in this crate already, and a trace that
//! reaches the board model and not the Gerber is a connection the fabricator
//! never makes - on a 900-segment board, one nobody would notice.

use std::path::{Path, PathBuf};

use cypcb_autoroute::{route_board, AutorouteConfig};
use cypcb_export::{run_export, ExportJob};
use cypcb_kicad::{parse_kicad_pcb, BENCHMARKS};
use cypcb_router::apply_routes;
use cypcb_rules::presets::{PresetRuleSet, RulesPreset};
use cypcb_world::components::trace::Trace;
use cypcb_world::components::Layer;

fn fixture_path(filename: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/fixtures/benchmark")
        .join(filename)
}

/// Every stroke a Gerber layer draws, as pairs of points in millimetres.
///
/// `D02` lifts the pen and moves; `D01` draws from wherever it is.
fn drawn_strokes(gerber: &str) -> Vec<((f64, f64), (f64, f64))> {
    let mut strokes = Vec::new();
    let mut pen: Option<(f64, f64)> = None;

    for line in gerber.lines() {
        let Some(point) = coordinates(line) else {
            continue;
        };
        if line.contains("D02") {
            pen = Some(point);
        } else if line.contains("D01") {
            if let Some(from) = pen {
                strokes.push((from, point));
            }
            pen = Some(point);
        }
    }
    strokes
}

fn coordinates(line: &str) -> Option<(f64, f64)> {
    Some((between(line, 'X')?, between(line, 'Y')?))
}

fn between(line: &str, marker: char) -> Option<f64> {
    let start = line.find(marker)? + 1;
    let rest = &line[start..];
    let end = rest
        .find(|c: char| !c.is_ascii_digit() && c != '-' && c != '.')
        .unwrap_or(rest.len());
    rest[..end].parse::<f64>().ok()
}

/// Whether a point sits on a drawn stroke, within a tenth of the narrowest
/// trace this project allows.
fn on_stroke(point: (f64, f64), stroke: ((f64, f64), (f64, f64))) -> bool {
    let ((x1, y1), (x2, y2)) = stroke;
    let (dx, dy) = (x2 - x1, y2 - y1);
    let length_squared = dx * dx + dy * dy;

    let (nx, ny) = if length_squared < f64::EPSILON {
        (x1, y1)
    } else {
        let t = (((point.0 - x1) * dx + (point.1 - y1) * dy) / length_squared).clamp(0.0, 1.0);
        (x1 + t * dx, y1 + t * dy)
    };

    let (ex, ey) = (point.0 - nx, point.1 - ny);
    (ex * ex + ey * ey).sqrt() < 0.0127
}

#[test]
#[ignore = "slow: routes and exports all three benchmark fixtures"]
fn what_the_router_lays_is_what_the_fabricator_gets() {
    let mut missing: Vec<String> = Vec::new();
    let mut endpoints_checked = 0usize;
    let mut segments_total = 0usize;

    for benchmark in BENCHMARKS {
        let parsed = parse_kicad_pcb(&fixture_path(benchmark.filename))
            .unwrap_or_else(|e| panic!("{}: {e:?}", benchmark.filename));
        let mut world = parsed.world;
        let library = parsed.library;

        let rules = PresetRuleSet::new(RulesPreset::from_name("jlcpcb").expect("the preset"));
        let routing = route_board(&mut world, &library, &rules, &AutorouteConfig::default());
        assert!(
            routing.route_count() > 0,
            "{}: the router produced nothing to check",
            benchmark.filename
        );
        apply_routes(&mut world, &routing);
        world.rebuild_spatial_index_from_library(&library);

        let label = benchmark.filename.trim_end_matches(".kicad_pcb");
        let dir = std::env::temp_dir().join(format!("cypcb-routed-{label}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a temp directory");

        let job = ExportJob {
            source_path: PathBuf::from(benchmark.filename),
            output_dir: dir.clone(),
            preset: cypcb_export::presets::from_name("jlcpcb").expect("the jlcpcb preset"),
            board_name: label.to_string(),
        };
        run_export(&job, &mut world, &library)
            .unwrap_or_else(|e| panic!("{} failed to export: {e:?}", benchmark.filename));

        let read = |marker: &str| -> Vec<((f64, f64), (f64, f64))> {
            walk(&dir)
                .into_iter()
                .find(|path| path.to_string_lossy().ends_with(marker))
                .map(|path| drawn_strokes(&std::fs::read_to_string(path).expect("a layer")))
                .unwrap_or_else(|| panic!("{}: no {marker} exported", benchmark.filename))
        };
        let top = read("F_Cu.gbr");
        let bottom = read("B_Cu.gbr");
        // A four-layer board routes on its inner pair, and those files exist
        // only since the exporter started reading the board's own stack.
        let inner: Vec<Vec<((f64, f64), (f64, f64))>> = (1..=4)
            .map(|n| {
                walk(&dir)
                    .into_iter()
                    .find(|path| path.to_string_lossy().ends_with(&format!("In{n}_Cu.gbr")))
                    .map(|path| drawn_strokes(&std::fs::read_to_string(path).expect("a layer")))
                    .unwrap_or_default()
            })
            .collect();

        let routed: Vec<(Layer, (f64, f64), (f64, f64))> = {
            let ecs = world.ecs_mut();
            let mut query = ecs.query::<&Trace>();
            query
                .iter(ecs)
                .flat_map(|trace| {
                    trace.segments.iter().map(move |segment| {
                        (
                            trace.layer,
                            (
                                segment.start.x.0 as f64 / 1_000_000.0,
                                segment.start.y.0 as f64 / 1_000_000.0,
                            ),
                            (
                                segment.end.x.0 as f64 / 1_000_000.0,
                                segment.end.y.0 as f64 / 1_000_000.0,
                            ),
                        )
                    })
                })
                .collect()
        };
        segments_total += routed.len();

        for (layer, start, end) in &routed {
            let strokes = match layer {
                Layer::BottomCopper => &bottom,
                Layer::Inner(index) => inner.get(*index as usize).unwrap_or(&top),
                _ => &top,
            };
            for point in [start, end] {
                endpoints_checked += 1;
                if !strokes.iter().any(|stroke| on_stroke(*point, *stroke)) {
                    missing.push(format!(
                        "{}: a routed end at ({:.3}mm, {:.3}mm) on {layer:?} has no copper under it",
                        benchmark.filename, point.0, point.1
                    ));
                }
            }
        }

        eprintln!(
            "{}: {} segments routed, {} endpoints on drawn copper",
            benchmark.filename,
            routed.len(),
            routed.len() * 2
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    // A board's worth of missing copper prints as thousands of lines, and the
    // first few say the same thing as all of them.
    if missing.len() > 20 {
        missing.truncate(20);
        missing.push("... and more; the list is cut at twenty".to_string());
    }
    assert!(
        missing.is_empty(),
        "the copper does not follow the routing:\n{}",
        missing.join("\n")
    );
    assert!(
        endpoints_checked >= 2 * segments_total,
        "only {endpoints_checked} endpoints checked against {segments_total} segments"
    );
    eprintln!("{endpoints_checked} routed endpoints found on copper actually drawn");
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(walk(&path));
        } else {
            out.push(path);
        }
    }
    out
}

#[test]
#[ignore = "diagnostic: routes the four-layer fixture and counts via spans"]
fn which_layers_the_router_joins_with_a_via() {
    // The claim on record was that the router places through vias only. The
    // pieces say otherwise - `postprocess` builds each `ViaPlacement` from the
    // layer transition that produced it, and `apply_routes` copies the pair
    // onto the `Via` - so this counts what actually lands on a four-layer
    // board rather than repeating the claim.
    use std::collections::BTreeMap;

    let parsed = parse_kicad_pcb(&fixture_path("multi_ic.kicad_pcb")).expect("the fixture parses");
    let mut world = parsed.world;
    let library = parsed.library;

    let rules = PresetRuleSet::new(RulesPreset::from_name("jlcpcb").expect("the preset"));
    let routing = route_board(&mut world, &library, &rules, &AutorouteConfig::default());
    assert!(routing.via_count() > 0, "no vias to look at");

    let mut spans: BTreeMap<String, usize> = BTreeMap::new();
    for via in &routing.vias {
        *spans
            .entry(format!("{:?} to {:?}", via.start_layer, via.end_layer))
            .or_default() += 1;
    }

    eprintln!();
    eprintln!("=== multi_ic.kicad_pcb via spans ===");
    for (span, count) in &spans {
        eprintln!("  {count:>4}  {span}");
    }

    assert_eq!(
        spans.values().sum::<usize>(),
        routing.via_count(),
        "every via should be accounted for"
    );
}
