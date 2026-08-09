//! The two halves of a differential pair have to be the same length.
//!
//! A pair is two nets carrying one signal between them, and the receiver reads
//! the difference. Copper one half runs and the other does not arrives late -
//! that is skew, and past the fab's length-match tolerance it is a signal
//! problem no amount of retries will talk the board out of.
//!
//! `diffpair USB { USB_DP USB_DM }` parsed into the AST and was read by
//! nothing; `length_match_tolerance` sat in every fab preset and was read by
//! nothing either. This is both ends of that.
//!
//! What is measured is copper length per net, summed over every segment of
//! every trace on it. Not measured: the gap between the two halves, which is
//! the other half of a diff-pair rule and needs the router to place them
//! alongside each other first.

use cypcb_core::{Nm, Point};
use cypcb_world::components::trace::Trace;
use cypcb_world::components::NetId;
use cypcb_world::BoardWorld;

use crate::presets::DesignRules;
use crate::violation::DrcViolation;

use super::DrcRule;

/// Rule for checking the skew between the halves of a differential pair.
pub struct DiffPairSkewRule;

impl DrcRule for DiffPairSkewRule {
    fn name(&self) -> &'static str {
        "diff-pair-skew"
    }

    fn check(&self, world: &mut BoardWorld, rules: &DesignRules) -> Vec<DrcViolation> {
        let pairs = world.diff_pairs().to_vec();
        if pairs.is_empty() {
            return Vec::new();
        }

        // Copper on each net, and somewhere on it to point at.
        let mut length: std::collections::HashMap<NetId, Nm> = std::collections::HashMap::new();
        let mut somewhere: std::collections::HashMap<NetId, Point> =
            std::collections::HashMap::new();
        {
            let ecs = world.ecs_mut();
            let mut query = ecs.query::<&Trace>();
            for trace in query.iter(ecs) {
                for segment in &trace.segments {
                    let run = distance(segment.start, segment.end);
                    let total = length.entry(trace.net_id).or_insert(Nm(0));
                    *total = Nm(total.0 + run.0);
                    somewhere.entry(trace.net_id).or_insert(segment.start);
                }
            }
        }

        // Every violation points somewhere, and a claim about two nets is
        // about the board rather than about one part of it.
        let Some(board) = world.board_entity() else {
            return Vec::new();
        };
        let mut violations = Vec::new();

        for pair in &pairs {
            let halves = [&pair.positive, &pair.negative];
            let mut ids = Vec::new();
            let mut missing = Vec::new();
            for half in halves {
                match world.get_net(&half.value) {
                    Some(id) => ids.push(id),
                    None => missing.push(half.value.clone()),
                }
            }

            // A pair naming a net the design does not have is a typo, and a
            // typo here means the check silently never runs.
            if !missing.is_empty() {
                violations.push(DrcViolation::diff_pair_skew(
                    board,
                    format!(
                        "diffpair '{}' names {} which is not a net on this board",
                        pair.name.value,
                        missing.join(" or ")
                    ),
                    None,
                    None,
                    Point::new(Nm(0), Nm(0)),
                ));
                continue;
            }

            let positive = length.get(&ids[0]).copied().unwrap_or(Nm(0));
            let negative = length.get(&ids[1]).copied().unwrap_or(Nm(0));
            let skew = Nm((positive.0 - negative.0).abs());
            if skew <= rules.max_diff_pair_skew {
                continue;
            }

            let at = somewhere
                .get(&ids[0])
                .or_else(|| somewhere.get(&ids[1]))
                .copied()
                .unwrap_or(Point::new(Nm(0), Nm(0)));

            violations.push(DrcViolation::diff_pair_skew(
                board,
                format!(
                    "diffpair '{}': {} runs {:.3}mm and {} runs {:.3}mm",
                    pair.name.value,
                    pair.positive.value,
                    positive.to_mm(),
                    pair.negative.value,
                    negative.to_mm()
                ),
                Some(skew),
                Some(rules.max_diff_pair_skew),
                at,
            ));
        }

        violations
    }
}

fn distance(a: Point, b: Point) -> Nm {
    let dx = (a.x.0 - b.x.0) as f64;
    let dy = (a.y.0 - b.y.0) as f64;
    Nm(dx.hypot(dy).round() as i64)
}
