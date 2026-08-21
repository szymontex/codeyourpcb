//! Is the variant each board picks the best point near it, or just the best
//! point somebody tried?
//!
//! `cargo test --release -p cypcb-autoroute --test is_the_best_variant_a_local_optimum -- --ignored --nocapture`
//!
//! `docs/routing.md` has recorded for several fires that the variants are
//! guesses at one question: each is a point in a space of seven knobs, chosen
//! because somebody measured that point, and nothing has ever looked between
//! them. Adding a twelfth point moved `multi_ic` by 140 violations, which is
//! the argument for looking made in numbers.
//!
//! This is the cheapest form of looking. Take the variant each board's own
//! ranking picks, change **one knob at a time**, and hand the winner and its
//! neighbours to `generate_variants` together so they are ranked by the same
//! rule the router uses - complete first, then fewest shorts, then composite.
//!
//! Two outcomes and both are worth having. If the shipped winner stays first,
//! it is a local optimum on this neighbourhood and the twelve points are
//! better chosen than "guesses" suggests. If a neighbour beats it, that
//! neighbour is a free improvement nobody had tried, and the case for a real
//! search stops being theoretical.

use std::path::Path;

use cypcb_autoroute::variant::{default_variant_configs, generate_variants, VariantConfig};
use cypcb_drc::{preset_for_world, ruleset_for_world, DesignRules};
use cypcb_kicad::{parse_kicad_pcb, BENCHMARKS};
use cypcb_rules::presets::RulesPreset;
use cypcb_world::BoardWorld;

fn fixture_path(filename: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/fixtures/benchmark")
        .join(filename)
}

/// Each board's measured noise band, violations and shorts, from
/// `via_price_sweep::how_much_of_the_price_is_noise`.
///
/// The ranking and the band answer different questions and this file needs
/// both. The ranking says which point the router would hand over; the band
/// says whether the difference is the negotiation going differently. A
/// neighbour the ranking prefers by less than the band is not an improvement,
/// and a diagnostic that printed only the first would read as though it were.
fn band(filename: &str) -> (i64, i64) {
    match filename {
        "led_blink.kicad_pcb" => (0, 0),
        "stm32_breakout.kicad_pcb" => (59, 61),
        "multi_ic.kicad_pcb" => (65, 56),
        "shift_driver.kicad_pcb" => (17, 8),
        "qfp_fanout.kicad_pcb" => (57, 44),
        "plane_board.kicad_pcb" => (0, 0),
        _ => (0, 0),
    }
}

/// The fab table this board would actually be graded against.
///
/// `cypcb check` resolves the sibling that matches the board, so `multi_ic`
/// reads against `jlcpcb_standard_4layer` and the other five against the
/// two-layer table. This harness had its own two-layer answer for all six,
/// which asks about a board nobody ships.
fn table_for(world: &BoardWorld) -> RulesPreset {
    preset_for_world(RulesPreset::JlcpcbStandard2Layer, world)
}

/// One-knob neighbours of a variant, named after what was changed.
///
/// Deliberately one at a time: a neighbourhood that moves two knobs cannot say
/// which move mattered, and this vector has spent enough fires on instruments
/// whose result could not be attributed.
fn neighbours(base: &VariantConfig) -> Vec<VariantConfig> {
    let mut out = Vec::new();
    let mut add = |suffix: &str, mut c: VariantConfig| {
        c.name = format!("{} [{}]", base.name, suffix);
        out.push(c);
    };

    for value in [0.0, 0.25, 1.0] {
        if (value - base.via_ring_penalty).abs() > f64::EPSILON {
            let mut c = base.clone();
            c.via_ring_penalty = value;
            add(&format!("via_ring {value}"), c);
        }
    }
    for value in [0.0, 5.0, 20.0] {
        if (value - base.foreign_pad_penalty).abs() > f64::EPSILON {
            let mut c = base.clone();
            c.foreign_pad_penalty = value;
            add(&format!("foreign_pad {value}"), c);
        }
    }
    for value in [0.0, 10.0] {
        if (value - base.clearance_barrier).abs() > f64::EPSILON {
            let mut c = base.clone();
            c.clearance_barrier = value;
            add(&format!("barrier {value}"), c);
        }
    }
    for value in [1.0, 1.25] {
        if (value - base.heuristic_weight).abs() > f64::EPSILON {
            let mut c = base.clone();
            c.heuristic_weight = value;
            add(&format!("weight {value}"), c);
        }
    }
    for value in [2u16, 3, 4] {
        if value != base.pad_zone_margin_cells {
            let mut c = base.clone();
            c.pad_zone_margin_cells = value;
            add(&format!("margin {value}"), c);
        }
    }
    {
        let mut c = base.clone();
        c.pad_zone_blocks_foreign_copper = !base.pad_zone_blocks_foreign_copper;
        add("pad gate flipped", c);
    }
    {
        let mut c = base.clone();
        c.reserve_trace_footprint = !base.reserve_trace_footprint;
        add("reservation flipped", c);
    }
    out
}

/// The yardstick is the board's own table, and one benchmark proves it matters.
///
/// `multi_ic` has four copper layers. Ranked on the two-layer table it puts
/// `PathFinder Eager Pads Priced Ring` first at 172 / 84; ranked on its own
/// table it puts `PathFinder Eager Pads` first at 371 / 128, which is what
/// `cypcb route --variants` prints. A harness that ranks under a table the tool
/// would not use answers about a board nobody ships, so which table each
/// benchmark resolves to is pinned here rather than left to a diagnostic that
/// only fails when it finds an improvement.
#[test]
fn each_benchmark_is_graded_on_the_table_its_own_layer_count_asks_for() {
    let mut seen: Vec<(&str, &str)> = Vec::new();
    for benchmark in BENCHMARKS {
        let parsed = parse_kicad_pcb(&fixture_path(benchmark.filename))
            .unwrap_or_else(|e| panic!("Failed to parse {}: {:?}", benchmark.filename, e));
        seen.push((benchmark.filename, table_for(&parsed.world).name()));
    }

    assert!(
        seen.contains(&("multi_ic.kicad_pcb", "jlcpcb_standard_4layer")),
        "multi_ic is the four-layer benchmark and the harness must grade it on \
         the four-layer table; got {seen:?}"
    );
    for (filename, table) in &seen {
        if *filename == "multi_ic.kicad_pcb" {
            continue;
        }
        assert_eq!(
            *table, "jlcpcb_standard_2layer",
            "{filename} is a two-layer board and should resolve to the two-layer table"
        );
    }
}

#[test]
#[ignore = "diagnostic: one-knob neighbourhood around each board's chosen variant"]
fn does_a_one_knob_neighbour_beat_the_variant_the_board_picks() {
    let shipped = default_variant_configs();

    // Six boards times a dozen points is more than one sitting, and the
    // `multi_ic` winner carries the barrier so every point in its
    // neighbourhood builds the clearance field. `CYPCB_BOARDS=plane,shift`
    // runs the cheap ones; unset runs all six and takes minutes.
    let only = std::env::var("CYPCB_BOARDS").unwrap_or_default();
    let wanted: Vec<&str> = only.split(',').filter(|s| !s.is_empty()).collect();

    let mut improved = Vec::new();
    let mut skipped = Vec::new();

    for benchmark in BENCHMARKS {
        if !wanted.is_empty() && !wanted.iter().any(|w| benchmark.filename.contains(w)) {
            skipped.push(benchmark.filename);
            continue;
        }
        // Round one: what the board picks today.
        let parsed = parse_kicad_pcb(&fixture_path(benchmark.filename))
            .unwrap_or_else(|e| panic!("Failed to parse {}: {:?}", benchmark.filename, e));
        let mut world = parsed.world;
        let preset = table_for(&world);
        let rules = ruleset_for_world(preset, &world);
        let design_rules = DesignRules::from_constraints(&preset.constraints());
        let ranked =
            generate_variants(&mut world, &parsed.library, &rules, &design_rules, &shipped);
        let winner_name = ranked
            .first()
            .expect("every board ranks something")
            .name
            .clone();
        let winner = shipped
            .iter()
            .find(|c| c.name == winner_name)
            .expect("the winner is one of the configs handed in")
            .clone();

        // Round two: the winner against its own neighbourhood, ranked by the
        // same rule. The winner is included so the comparison is made by
        // `generate_variants` rather than across two of its runs.
        let mut field = vec![winner.clone()];
        field.extend(neighbours(&winner));

        let parsed = parse_kicad_pcb(&fixture_path(benchmark.filename))
            .unwrap_or_else(|e| panic!("Failed to parse {}: {:?}", benchmark.filename, e));
        let mut world = parsed.world;
        let started = std::time::Instant::now();
        let local = generate_variants(&mut world, &parsed.library, &rules, &design_rules, &field);
        let elapsed = started.elapsed();

        eprintln!();
        eprintln!(
            "=== {} - {} points around `{}` on {} in {:.1}s ===",
            benchmark.filename,
            local.len(),
            winner_name,
            preset.name(),
            elapsed.as_secs_f64()
        );
        for (rank, r) in local.iter().take(5).enumerate() {
            eprintln!(
                "  {}. {:<44} drc {:>4} ({:>3} shorts), {} unrouted",
                rank + 1,
                r.name,
                r.score.drc_violations,
                r.score.shorts,
                r.unrouted
            );
        }

        let best = local.first().expect("the neighbourhood ranks something");
        if best.name == winner_name {
            eprintln!("  -> the shipped point is a local optimum on this neighbourhood");
        } else {
            // The ranking preferred it. Whether that is an improvement is the
            // band's question, and the two disagree often enough that printing
            // only the first would invite the wrong conclusion.
            let here = local
                .iter()
                .find(|r| r.name == winner_name)
                .expect("the winner was handed in with its own neighbourhood");
            let (bv, bs) = band(benchmark.filename);
            let dv = best.score.drc_violations as i64 - here.score.drc_violations as i64;
            let ds = best.score.shorts as i64 - here.score.shorts as i64;
            let outside = dv.abs() > bv || ds.abs() > bs;

            eprintln!(
                "  -> the ranking prefers `{}` ({dv:+} violations, {ds:+} shorts, band {bv} / {bs})",
                best.name
            );
            if outside {
                eprintln!("     and the move is OUTSIDE the band: a real improvement");
                improved.push(format!("{}: {}", benchmark.filename, best.name));
            } else {
                eprintln!("     but the move is inside the band: the negotiation going");
                eprintln!("     differently, not a better setting");
            }
        }
    }

    eprintln!();
    if !skipped.is_empty() {
        // Named, because a diagnostic that quietly measured two boards and
        // printed a verdict would read as a verdict about six.
        eprintln!("Not run this time: {}", skipped.join(", "));
        eprintln!();
    }
    if improved.is_empty() {
        eprintln!("No board has a one-knob neighbour that beats what it already picks.");
        eprintln!(
            "The {} points are better chosen than 'guesses' suggests, and a",
            shipped.len()
        );
        eprintln!("search would have to look further than one knob to find anything.");
    } else {
        eprintln!("Beaten on {} board(s):", improved.len());
        for line in &improved {
            eprintln!("  {line}");
        }
        eprintln!();
        eprintln!("Each of those is a point nobody had tried. Before adopting one, check");
        eprintln!("it against that board's own noise band - a move inside the band is the");
        eprintln!("negotiation going differently, not a better setting.");
    }
}
