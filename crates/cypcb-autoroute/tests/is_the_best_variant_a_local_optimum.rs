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
use cypcb_drc::DesignRules;
use cypcb_kicad::{parse_kicad_pcb, BENCHMARKS};
use cypcb_rules::presets::{PresetRuleSet, RulesPreset};

fn fixture_path(filename: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/fixtures/benchmark")
        .join(filename)
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

#[test]
#[ignore = "diagnostic: one-knob neighbourhood around each board's chosen variant"]
fn does_a_one_knob_neighbour_beat_the_variant_the_board_picks() {
    let rules = PresetRuleSet::new(RulesPreset::from_name("jlcpcb").unwrap());
    let design_rules = DesignRules::jlcpcb_2layer();
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
            "=== {} - {} points around `{}` in {:.1}s ===",
            benchmark.filename,
            local.len(),
            winner_name,
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
            eprintln!("  -> BEATEN by `{}`", best.name);
            improved.push(format!("{}: {}", benchmark.filename, best.name));
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
        eprintln!("The twelve points are better chosen than 'guesses' suggests, and a");
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
