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
//! This measures the gap for every preset and pins it. The point is not that a
//! shortfall is a defect - two rows of a capability table are usually written
//! about different processes - but that a user who hits it deserves to find it
//! written down rather than discover it from two rules disagreeing about a
//! board built to spec.

use cypcb_rules::presets::RulesPreset;

/// The ring left when a via is built at the stated land on the stated drill.
fn implied_ring_nm(preset: RulesPreset) -> i64 {
    let c = preset.constraints();
    (c.min_pad_size.raw() - c.min_via_drill.raw()) / 2
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
        let implied = implied_ring_nm(*preset);
        let required = c.min_via_annular_ring.raw();
        let shortfall = required - implied;

        println!(
            "{:<26} {:>8} {:>8} {:>8} {:>10} {:>10}",
            format!("{preset:?}"),
            c.min_pad_size.raw(),
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
        vec![
            "IpcClass2 2000".to_string(),
            "IpcClass3 2000".to_string(),
            "JlcpcbStandard2Layer 27000".to_string(),
            "OshPark4Layer 2000".to_string(),
        ],
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

    assert_eq!(c.min_pad_size.raw(), 500_000, "0.5mm land");
    assert_eq!(c.min_via_drill.raw(), 300_000, "0.3mm drill");
    assert_eq!(c.min_via_annular_ring.raw(), 127_000, "0.127mm ring");
    assert_eq!(
        implied_ring_nm(preset),
        100_000,
        "0.1mm of ring is what is left"
    );

    // The other three are within 2000nm, which is a rounding of mils into
    // millimetres rather than a disagreement.
    for other in [
        RulesPreset::IpcClass2,
        RulesPreset::IpcClass3,
        RulesPreset::OshPark4Layer,
    ] {
        let gap = other.constraints().min_via_annular_ring.raw() - implied_ring_nm(other);
        assert_eq!(gap, 2_000, "{other:?} is a rounding, not a disagreement");
    }
}
