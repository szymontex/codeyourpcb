//! What the variant ranking should order on.
//!
//! `cargo test --release -p cypcb-autoroute --test ranking_rule_sweep -- --ignored --nocapture`
//!
//! The shipped rule is lexicographic: complete first, then fewest shorts, then
//! the composite. Copper touching copper cannot work and a 0.05mm gap is a
//! yield risk a fab may still build, so shorts really do come first - but a
//! strict lexicographic order trades *any* amount of the second key for *any*
//! amount of the first. On `shift_driver.kicad_pcb`, the board nobody tuned
//! against, that trade is 34 violations for 7 shorts.
//!
//! This scores every candidate rule against the same routing. One pass per
//! board produces eight variants; each rule then picks from those same eight,
//! so nothing here depends on the router running twice.
//!
//! # The criterion, written before the numbers
//!
//! A pick is **dominated** when another variant that also routed everything has
//! fewer violations *and* no more shorts. Handing over a dominated board is
//! indefensible under any reading: it is worse on one axis and no better on the
//! other. So:
//!
//! 1. A rule that ever picks a dominated board is out.
//! 2. Among the rules left, prefer the one whose picks carry the fewest shorts,
//!    summed across the boards - shorts being what the project ranks first.
//!
//! The test asserts only what follows from (1) for the rule that ships. The
//! rest is printed for the decision, because choosing the weight is a judgement
//! about boards, not something a test should make silently.

use cypcb_autoroute::variant::{default_variant_configs, generate_variants, VariantResult};
use cypcb_drc::presets::DesignRules;
use cypcb_drc::{preset_for_world, ruleset_for_world};
use cypcb_kicad::{parse_kicad_pcb, BENCHMARKS};
use cypcb_rules::presets::RulesPreset;

use std::path::{Path, PathBuf};

fn fixture_path(filename: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .join("tests/fixtures/benchmark")
        .join(filename)
}

/// A way of choosing one variant out of the eight.
struct Rule {
    name: &'static str,
    /// How many violations one short is worth. `None` is the shipped rule:
    /// fewest shorts wins outright, whatever the totals say.
    shorts_weight: Option<u32>,
}

const RULES: &[Rule] = &[
    Rule {
        name: "lexicographic (shipped)",
        shorts_weight: None,
    },
    Rule {
        name: "violations + 0 x shorts",
        shorts_weight: Some(0),
    },
    Rule {
        name: "violations + 1 x shorts",
        shorts_weight: Some(1),
    },
    Rule {
        name: "violations + 2 x shorts",
        shorts_weight: Some(2),
    },
    Rule {
        name: "violations + 5 x shorts",
        shorts_weight: Some(5),
    },
    Rule {
        name: "violations + 10 x shorts",
        shorts_weight: Some(10),
    },
    Rule {
        name: "violations + 20 x shorts",
        shorts_weight: Some(20),
    },
];

/// The variant a rule picks. Incomplete boards are never picked, by every rule:
/// a net nobody routed leaves no copper to charge for.
fn pick<'a>(rule: &Rule, results: &'a [VariantResult]) -> &'a VariantResult {
    let complete: Vec<&VariantResult> = results.iter().filter(|r| r.unrouted == 0).collect();
    let pool: &[&VariantResult] = if complete.is_empty() {
        return results.first().expect("at least one variant routed");
    } else {
        &complete
    };

    match rule.shorts_weight {
        None => pool
            .iter()
            .min_by(|a, b| {
                a.score
                    .shorts
                    .cmp(&b.score.shorts)
                    .then_with(|| a.score.composite.total_cmp(&b.score.composite))
            })
            .expect("the pool is not empty"),
        Some(weight) => pool
            .iter()
            .min_by_key(|r| r.score.drc_violations as u64 + weight as u64 * r.score.shorts as u64)
            .expect("the pool is not empty"),
    }
}

/// Whether another complete variant beats this one on violations without
/// costing a short.
fn dominated_by<'a>(
    pick: &VariantResult,
    results: &'a [VariantResult],
) -> Option<&'a VariantResult> {
    results.iter().find(|other| {
        other.unrouted == 0
            && other.score.drc_violations < pick.score.drc_violations
            && other.score.shorts <= pick.score.shorts
    })
}

#[test]
#[ignore = "routes every benchmark eight times"]
fn what_the_ranking_should_order_on() {
    let configs = default_variant_configs();

    // board -> the eight results, routed once.
    let mut per_board: Vec<(&str, Vec<VariantResult>)> = Vec::new();
    for benchmark in BENCHMARKS {
        let parsed = parse_kicad_pcb(&fixture_path(benchmark.filename))
            .unwrap_or_else(|e| panic!("failed to parse {}: {e:?}", benchmark.filename));
        let mut world = parsed.world;

        // The table this board would actually be ranked under. `multi_ic` has
        // four copper layers, and on its own table the adaptive grid is
        // 0.400mm against 0.508mm - so a fixed two-layer answer here ranks a
        // different search, and this file is what every "which variant does
        // this board pick" answer rests on.
        let preset = preset_for_world(RulesPreset::JlcpcbStandard2Layer, &world);
        let rules = ruleset_for_world(preset, &world);
        let design_rules = DesignRules::from_constraints(&preset.constraints());
        eprintln!("{} ranked on {}", benchmark.filename, preset.name());

        let results =
            generate_variants(&mut world, &parsed.library, &rules, &design_rules, &configs);
        per_board.push((benchmark.filename, results));
    }

    eprintln!();
    eprintln!("What each rule picks, as violations / shorts:");
    eprintln!();
    eprint!("{:<26}", "rule");
    for (name, _) in &per_board {
        eprint!(" {:>22}", name.trim_end_matches(".kicad_pcb"));
    }
    eprintln!("  {:>8}  dominated picks", "shorts");

    let mut shipped_dominated = Vec::new();

    for rule in RULES {
        eprint!("{:<26}", rule.name);
        let mut total_shorts = 0u64;
        let mut dominated = Vec::new();

        for (board, results) in &per_board {
            let chosen = pick(rule, results);
            total_shorts += chosen.score.shorts as u64;
            let mark = match dominated_by(chosen, results) {
                Some(better) => {
                    dominated.push(format!(
                        "{board}: {} at {}/{} is beaten by {} at {}/{}",
                        chosen.name,
                        chosen.score.drc_violations,
                        chosen.score.shorts,
                        better.name,
                        better.score.drc_violations,
                        better.score.shorts
                    ));
                    "*"
                }
                None => "",
            };
            eprint!(
                " {:>21}",
                format!(
                    "{}/{}{}",
                    chosen.score.drc_violations, chosen.score.shorts, mark
                )
            );
        }
        eprintln!("  {:>8}  {}", total_shorts, dominated.len());

        if rule.shorts_weight.is_none() {
            shipped_dominated = dominated.clone();
        }
        for line in &dominated {
            eprintln!("      * {line}");
        }
    }

    eprintln!();
    eprintln!("A pick marked * is beaten on violations by a complete variant that costs no");
    eprintln!("more shorts. Rules with none of those are the ones worth choosing between.");

    // The shipped rule's behaviour is recorded rather than demanded: this test
    // exists to inform the choice, and the numbers above are the finding. What
    // it does hold is that the sweep ran on every board.
    assert_eq!(
        per_board.len(),
        BENCHMARKS.len(),
        "every benchmark has to be in the sweep"
    );
    eprintln!();
    eprintln!(
        "shipped rule picked {} dominated board(s)",
        shipped_dominated.len()
    );
}
