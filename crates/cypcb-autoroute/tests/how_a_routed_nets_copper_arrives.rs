//! Does a routed net's copper really arrive in 1.2mm pieces?
//!
//! `cargo test --release -p cypcb-autoroute --test how_a_routed_nets_copper_arrives -- --ignored --nocapture`
//!
//! `what_a_declared_neck_costs` found that a declared neck draws **0.0mm** on
//! every benchmark, because `Trace::apply_neck` refuses a run with no room for
//! two necks and `stm32_breakout` has 185 runs over 899 segments - about 1.2mm
//! each. Two readings fit that and they need different fixes:
//!
//! 1. The segments of one path are adjacent in `RoutingResult::routes` but
//!    their endpoints do not match, so `Trace::runs` cuts a chain that is
//!    really continuous. The runs are an artefact and the chaining test is
//!    wrong.
//! 2. The router genuinely emits short disconnected pieces, and a neck belongs
//!    at the ends of a **net's** copper rather than a run's.
//!
//! This prints what separates them: how many breaks there are, and how many of
//! those breaks are two segments that share a point in some *other* order -
//! reversed, or joined start-to-start. A break of that kind is a chain nobody
//! oriented; a break with no shared point at all is a genuine gap.

use std::collections::BTreeMap;
use std::path::Path;

use cypcb_autoroute::{route_board, AutorouteConfig};
use cypcb_core::Point;
use cypcb_drc::{preset_for_world, ruleset_for_world};
use cypcb_kicad::parse_kicad_pcb;
use cypcb_rules::presets::RulesPreset;

fn fixture_path(filename: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/fixtures/benchmark")
        .join(filename)
}

/// How two consecutive segments of one net and layer meet, if they do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Join {
    /// `previous.end == next.start`: what `Trace::runs` counts as one chain.
    Chained,
    /// They share a point, but not that way round.
    SharesAPointReversed,
    /// No shared endpoint: a genuine gap between two pieces of copper.
    Apart,
}

fn join(previous: (Point, Point), next: (Point, Point)) -> Join {
    let (ps, pe) = previous;
    let (ns, ne) = next;
    if pe == ns {
        Join::Chained
    } else if pe == ne || ps == ns || ps == ne {
        Join::SharesAPointReversed
    } else {
        Join::Apart
    }
}

#[test]
#[ignore = "diagnostic: how the router's segments arrive, per net and layer"]
fn how_a_routed_nets_copper_arrives() {
    for benchmark in cypcb_kicad::BENCHMARKS {
        let parsed =
            parse_kicad_pcb(&fixture_path(benchmark.filename)).expect("the fixture parses");
        let mut world = parsed.world;
        let library = parsed.library;
        let preset = preset_for_world(RulesPreset::JlcpcbStandard2Layer, &world);
        let rules = ruleset_for_world(preset, &world);
        let result = route_board(&mut world, &library, &rules, &AutorouteConfig::default());

        // Grouped exactly as `apply_routes` groups them, in the same order.
        let mut groups: BTreeMap<(u32, String), Vec<(Point, Point)>> = BTreeMap::new();
        for segment in &result.routes {
            groups
                .entry((segment.net_id.id(), format!("{:?}", segment.layer)))
                .or_default()
                .push((segment.start, segment.end));
        }

        let mut counts: BTreeMap<Join, usize> = BTreeMap::new();
        let mut segments = 0usize;
        for pieces in groups.values() {
            segments += pieces.len();
            for pair in pieces.windows(2) {
                *counts.entry(join(pair[0], pair[1])).or_insert(0) += 1;
            }
        }

        let chained = counts.get(&Join::Chained).copied().unwrap_or(0);
        let reversed = counts
            .get(&Join::SharesAPointReversed)
            .copied()
            .unwrap_or(0);
        let apart = counts.get(&Join::Apart).copied().unwrap_or(0);
        let breaks = reversed + apart;

        eprintln!();
        eprintln!(
            "=== {} - {} segments in {} net/layer groups ===",
            benchmark.filename,
            segments,
            groups.len()
        );
        eprintln!("  consecutive pairs that chain end-to-start: {chained}");
        eprintln!("  pairs that share a point some other way:   {reversed}");
        eprintln!("  pairs with no shared endpoint at all:      {apart}");
        eprintln!(
            "  so {} of {} breaks are orientation, {} are real gaps",
            reversed, breaks, apart
        );

        // How long a run actually is, in millimetres. The last run of this
        // work asserted "about 1.2mm" from segments per run times one grid
        // step, which assumes a segment is one step - the smoother merges
        // collinear ones, so that was an inference and not a measurement.
        let mut lengths: Vec<f64> = Vec::new();
        for pieces in groups.values() {
            let mut current = 0f64;
            for (index, (start, end)) in pieces.iter().enumerate() {
                if index > 0 && join(pieces[index - 1], (*start, *end)) != Join::Chained {
                    lengths.push(current);
                    current = 0.0;
                }
                let dx = end.x.to_mm() - start.x.to_mm();
                let dy = end.y.to_mm() - start.y.to_mm();
                current += (dx * dx + dy * dy).sqrt();
            }
            lengths.push(current);
        }
        lengths.sort_by(|a, b| a.partial_cmp(b).expect("no NaN in a length"));
        let total: f64 = lengths.iter().sum();
        let median = lengths[lengths.len() / 2];
        let over_2mm = lengths.iter().filter(|l| **l > 2.0).count();
        eprintln!(
            "  run length mm: min {:.2}, median {:.2}, max {:.2}, mean {:.2}",
            lengths[0],
            median,
            lengths[lengths.len() - 1],
            total / lengths.len() as f64
        );
        eprintln!(
            "  runs long enough for two 1mm necks: {} of {}",
            over_2mm,
            lengths.len()
        );
    }
}
