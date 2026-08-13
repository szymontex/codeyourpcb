//! Turning a design's own net constraints into rules the router obeys.
//!
//! A `.cypcb` file can say `netclass Power [width 0.5mm clearance 0.4mm]`, and
//! until this module existed nothing carried that as far as the autorouter.
//! `PresetRuleSet` has had `with_overrides` and `set_net_override` since the
//! presets were written; every caller outside its own tests built the rule set
//! with `PresetRuleSet::new`, which is the fab preset and nothing else. The
//! router therefore optimised against one set of numbers while `cypcb check`
//! graded the result against another - the design's - and the difference
//! surfaced as violations on a board the router thought it had finished.
//!
//! The fab preset is a floor, not a target. A design asking for copper
//! narrower than the fabricator will image is a fault `MinTraceWidthRule`
//! reports; it is not licence to route below the floor, so a stated width or
//! clearance can only raise what the router uses, never lower it.
//!
//! What this buys is one-sided, and saying so is cheaper than a reader
//! discovering it. `ClearanceRule` resolves a pair of nets by folding both
//! their stated clearances with `max`, so mains copper is 3mm from a signal
//! whichever of the two is being measured. A rule set is consulted per net
//! being routed, so the mains net is planned at 3mm and the signal that comes
//! near it afterwards is still planned at the preset. The router obeys the
//! constraint on the net that declared it and not on its neighbours.
//!
//! What this does **not** read is `current`. A net declared `current 10A`
//! needs a width IPC-2221 can compute, and the answer depends on which layer
//! the copper ends up on - outer copper sheds heat into the air, inner copper
//! does not. The router picks the layer, so the width is not known when the
//! rule set is built. That is a real gap and it is written down in the tracker
//! rather than papered over with whichever of the two answers is convenient.

use std::collections::HashMap;

use cypcb_core::Nm;
use cypcb_rules::constraints::DesignConstraints;
use cypcb_rules::presets::{PresetRuleSet, RulesPreset};
use cypcb_world::BoardWorld;

/// The larger of two dimensions.
#[inline]
fn wider(floor: Nm, asked: Nm) -> Nm {
    if asked.raw() > floor.raw() {
        asked
    } else {
        floor
    }
}

/// Build a routing rule set that carries the design's per-net constraints.
///
/// Nets the design says nothing about are absent from the override map and
/// route on the preset, exactly as before. A net with a stated width or
/// clearance gets a copy of the preset with those two raised to what the
/// design asked for.
pub fn ruleset_for_world(preset: RulesPreset, world: &BoardWorld) -> PresetRuleSet {
    let base = preset.constraints();
    let mut overrides: HashMap<u32, DesignConstraints> = HashMap::new();

    for (net_id, _name) in world.nets() {
        let Some(stated) = world.net_constraints(net_id) else {
            continue;
        };
        if stated.width.is_none() && stated.clearance.is_none() {
            continue;
        }
        let mut wanted = base.clone();
        if let Some(width) = stated.width {
            wanted.min_trace_width = wider(base.min_trace_width, width);
        }
        if let Some(clearance) = stated.clearance {
            wanted.min_clearance = wider(base.min_clearance, clearance);
        }
        overrides.insert(net_id.id(), wanted);
    }

    PresetRuleSet::with_overrides(preset, overrides)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cypcb_rules::RoutingRuleSet;
    use cypcb_world::registry::NetConstraints;

    /// A world with one net, carrying whatever the design stated about it.
    fn world_with_net(name: &str, stated: NetConstraints) -> (BoardWorld, u32) {
        let mut world = BoardWorld::new();
        let id = world.intern_net(name);
        world.set_net_constraints(id, stated);
        (world, id.id())
    }

    #[test]
    fn a_stated_width_reaches_the_router() {
        let preset = RulesPreset::JlcpcbStandard2Layer;
        let (world, id) = world_with_net(
            "POWER",
            NetConstraints {
                width: Some(Nm::from_mm(0.5)),
                ..Default::default()
            },
        );
        let rules = ruleset_for_world(preset, &world);

        assert_eq!(
            rules.constraints_for_net(id).min_trace_width,
            Nm::from_mm(0.5),
            "the design asked for 0.5mm and the router has to be told"
        );
        // The preset's own minimum is what it always was for anything else.
        assert_eq!(
            rules.constraints_for_net(9999).min_trace_width,
            preset.constraints().min_trace_width,
            "a net the design says nothing about routes on the preset"
        );
    }

    #[test]
    fn a_stated_clearance_reaches_the_router() {
        let preset = RulesPreset::JlcpcbStandard2Layer;
        let (world, id) = world_with_net(
            "MAINS",
            NetConstraints {
                clearance: Some(Nm::from_mm(3.0)),
                ..Default::default()
            },
        );
        let rules = ruleset_for_world(preset, &world);

        assert_eq!(
            rules.constraints_for_net(id).min_clearance,
            Nm::from_mm(3.0)
        );
    }

    /// The fab preset is a floor. A design asking for copper thinner than the
    /// fabricator images is a fault the checker reports; the router does not
    /// get told to aim below what can be made.
    #[test]
    fn a_narrower_ask_than_the_fab_allows_does_not_lower_the_floor() {
        let preset = RulesPreset::JlcpcbStandard2Layer;
        let floor = preset.constraints().min_trace_width;
        let thinner = Nm::new(floor.raw() / 2);
        let (world, id) = world_with_net(
            "SIGNAL",
            NetConstraints {
                width: Some(thinner),
                clearance: Some(Nm::new(preset.constraints().min_clearance.raw() / 2)),
                ..Default::default()
            },
        );
        let rules = ruleset_for_world(preset, &world);
        let got = rules.constraints_for_net(id);

        assert_eq!(got.min_trace_width, floor);
        assert_eq!(got.min_clearance, preset.constraints().min_clearance);
    }

    /// `current` is deliberately not read here, and a test says so - otherwise
    /// the next reader has to guess whether it was forgotten or refused.
    #[test]
    fn current_alone_is_not_yet_a_width() {
        let preset = RulesPreset::JlcpcbStandard2Layer;
        let (world, id) = world_with_net(
            "COIL",
            NetConstraints {
                current_ma: Some(10_000.0),
                ..Default::default()
            },
        );
        let rules = ruleset_for_world(preset, &world);

        assert_eq!(
            rules.constraints_for_net(id).min_trace_width,
            preset.constraints().min_trace_width,
            "deriving a width from current needs the layer, which the router picks"
        );
    }
}
