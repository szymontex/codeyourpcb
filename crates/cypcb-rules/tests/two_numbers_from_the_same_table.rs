//! A via built to a fab's own land minimum, on that fab's own smallest drill.
//!
//! `cargo test -p cypcb-rules --test two_numbers_from_the_same_table -- --nocapture`
//!
//! D6 made `min_pad_size` load-bearing: it is the floor under a via's land and
//! a through-hole pad's, checked by `ViaDiameterRule` and `PadLandRule`. The
//! ring around that land is a separate number, `min_via_annular_ring`, checked
//! by `AnnularRingRule`. Both come from the same capability table and nothing
//! had ever asked whether they can be satisfied at once.
//!
//! They cannot always. A via at exactly the stated land, drilled at exactly
//! the stated minimum, has a ring of `(land - drill) / 2` - and where that
//! falls under the stated ring, a board can pass one rule and fail the other
//! while using nothing but the fab's own published numbers.
//!
//! This measures the gap for every preset that publishes a land and pins it.
//! The point is not that a shortfall is a defect - two rows of a capability
//! table are usually written about different processes - but that a user who
//! hits it deserves to find it written down rather than discover it from two
//! rules disagreeing about a board built to spec.
//!
//! Only JLCPCB publishes one. The first version of this test measured all ten
//! presets and reported four with a tension; the other three were 2000nm each,
//! and that turned out to be an artefact rather than a finding - their
//! `min_pad_size` was `min_drill_size + 2 * min_annular_ring` rounded to two
//! decimals, so the "shortfall" was the rounding step and nothing else. A
//! derived land cannot disagree with the ring it was derived from.

use cypcb_rules::presets::RulesPreset;

/// The ring left when a via is built at the published land on the stated drill.
///
/// `None` where the fab published no land: there is no second number to
/// disagree with the first, which is the whole question this file asks.
fn implied_ring_nm(preset: RulesPreset) -> Option<i64> {
    let c = preset.constraints();
    Some((c.min_pad_size?.raw() - c.min_via_drill.raw()) / 2)
}

#[test]
fn the_land_and_the_ring_are_measured_against_each_other() {
    let mut short: Vec<(String, i64)> = Vec::new();

    println!(
        "\n{:<26} {:>8} {:>8} {:>8} {:>10} {:>10}",
        "preset", "land", "drill", "ring", "implied", "shortfall"
    );
    for preset in RulesPreset::all() {
        let c = preset.constraints();
        let required = c.min_via_annular_ring.raw();
        let Some(implied) = implied_ring_nm(*preset) else {
            println!(
                "{:<26} {:>8} {:>8} {:>8} {:>10} {:>10}",
                format!("{preset:?}"),
                "-",
                c.min_via_drill.raw(),
                required,
                "-",
                "derived"
            );
            continue;
        };
        let shortfall = required - implied;

        println!(
            "{:<26} {:>8} {:>8} {:>8} {:>10} {:>10}",
            format!("{preset:?}"),
            c.min_pad_size.expect("a land, or the branch above").raw(),
            c.min_via_drill.raw(),
            required,
            implied,
            shortfall.max(0)
        );

        if shortfall > 0 {
            short.push((format!("{preset:?}"), shortfall));
        }
    }
    println!();

    // Nothing may be short by a whole tenth of a millimetre: that would be a
    // table somebody mistyped rather than two process rows disagreeing.
    for (name, shortfall) in &short {
        assert!(
            *shortfall < 100_000,
            "{name} is short by {shortfall}nm, which is too far to be two rows \
             of the same table meaning different things"
        );
    }

    // Pinned, so a preset edit that changes which fabs have the tension shows
    // up here rather than in a user's board. The numbers are nanometres.
    let mut named: Vec<String> = short
        .iter()
        .map(|(name, gap)| format!("{name} {gap}"))
        .collect();
    named.sort();
    assert_eq!(
        named,
        vec!["JlcpcbStandard2Layer 27000".to_string()],
        "which presets cannot satisfy both of their own numbers at once has \
         changed; if a table was corrected, update this list and say which"
    );
}

#[test]
fn jlcpcb_standard_is_the_one_worth_knowing_about() {
    // The only preset whose two numbers disagree by more than a rounding step,
    // and the default this project ships. A 0.5mm land on a 0.3mm drill leaves
    // 0.1mm of ring where the same table asks for 0.127mm: `PadLandRule` says
    // yes and `AnnularRingRule` says no, about a via built to spec.
    //
    // Not a bug to fix here. It is the reason D6's next action - reading each
    // fab's published page for the row that pairs a hole with a diameter - is
    // worth doing, and this is the measurement that says where to start.
    let preset = RulesPreset::JlcpcbStandard2Layer;
    let c = preset.constraints();

    assert_eq!(
        c.min_pad_size.map(|land| land.raw()),
        Some(500_000),
        "0.5mm land, published rather than derived - it is the only one"
    );
    assert_eq!(c.min_via_drill.raw(), 300_000, "0.3mm drill");
    assert_eq!(c.min_via_annular_ring.raw(), 127_000, "0.127mm ring");
    assert_eq!(
        implied_ring_nm(preset),
        Some(100_000),
        "0.1mm of ring is what is left"
    );

    // The three that used to appear beside it are gone, and their absence is
    // the finding rather than a loosened test. Each carried a `min_pad_size`
    // of `min_drill_size + 2 * min_annular_ring` rounded to two decimals, so
    // the 2000nm they were short by was the rounding step. A land the checker
    // derives is derived from the ring and cannot fall under it - computed
    // here the way `DesignRules::from_constraints` computes it.
    for other in [
        RulesPreset::IpcClass2,
        RulesPreset::IpcClass3,
        RulesPreset::OshPark4Layer,
    ] {
        let c = other.constraints();
        assert_eq!(
            c.min_pad_size, None,
            "{other:?} publishes no land; the number it used to carry was derived"
        );
        let derived = c.min_drill_size.raw() + 2 * c.min_annular_ring.raw();
        assert_eq!(
            (derived - c.min_drill_size.raw()) / 2,
            c.min_annular_ring.raw(),
            "{other:?}: a derived land leaves exactly the ring it was derived from"
        );
    }
}
