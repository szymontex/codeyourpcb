//! A via is a hole somebody has to drill, and not every span is drillable.
//!
//! A board is drilled and plated once per lamination cycle. A through hole is
//! drilled after the last press and reaches everything; a **blind** via
//! reaches an outer layer and stops inside; a **buried** one touches neither
//! face. The last two mean the board is drilled and plated more than once,
//! with a press in between, so a house prices them separately and many refuse
//! them outright.
//!
//! Two questions, and the design can be wrong about either:
//!
//! - **Does this house drill them at all?** `blind_vias_allowed` and
//!   `buried_vias_allowed` have been in every fab table since the tables were
//!   written and were dropped before they reached any rule - so a flag every
//!   house sets checked nothing, exactly the way
//!   `castellated_holes_allowed` did until the stackup rule learned to read
//!   it.
//! - **Is this span one of the ones this build makes?** A design that lists
//!   its drill pairs - `drill Top to Inner2` - is stating which cycle drills
//!   what. A via whose span is not on the list is a hole this build does not
//!   make, whatever the house is capable of.
//!
//! A design that lists no pairs is asked only the first question. Saying
//! nothing is not the same as saying "every span is fine", but this rule
//! cannot tell the difference between a design that has not thought about it
//! and one that means every span - so it stays quiet and leaves the fab
//! table's answer standing.

use cypcb_world::components::trace::Via;
use cypcb_world::components::Layer;
use cypcb_world::BoardWorld;

use crate::presets::DesignRules;
use crate::violation::DrcViolation;

use super::DrcRule;

/// What kind of hole a span is on a board with this many copper layers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Span {
    /// Touches both faces: drilled after the last press.
    Through,
    /// Touches one face and stops inside.
    Blind,
    /// Touches neither face.
    Buried,
}

/// Where a layer sits in the copper sequence, top first.
///
/// `Layer::Inner` is zero-based and the copper sequence is not: the first
/// inner layer is copper entry 1, which is the off-by-one this project has
/// shipped three times.
fn copper_index(layer: Layer, copper_count: usize) -> Option<usize> {
    match layer {
        Layer::TopCopper => Some(0),
        Layer::BottomCopper => copper_count.checked_sub(1),
        Layer::Inner(n) => {
            let index = usize::from(n) + 1;
            (index < copper_count).then_some(index)
        }
        _ => None,
    }
}

fn span_of(via: &Via, copper_count: usize) -> Option<Span> {
    let start = copper_index(via.start_layer, copper_count)?;
    let end = copper_index(via.end_layer, copper_count)?;
    let (top, bottom) = (start.min(end), start.max(end));
    let last = copper_count.checked_sub(1)?;
    Some(match (top == 0, bottom == last) {
        (true, true) => Span::Through,
        (false, false) => Span::Buried,
        _ => Span::Blind,
    })
}

/// Rule for checking that a via's span is a hole the build drills.
pub struct ViaSpanRule;

impl DrcRule for ViaSpanRule {
    fn name(&self) -> &'static str {
        "via-span"
    }

    fn check(&self, world: &mut BoardWorld, rules: &DesignRules) -> Vec<DrcViolation> {
        let Some((_, layer_stack)) = world.board_info() else {
            return Vec::new();
        };
        let copper_count = usize::from(layer_stack.count);
        // A two-layer board has one span and it is the through hole. Nothing
        // here can be wrong, and asking would report every via on every
        // ordinary board.
        if copper_count < 3 {
            return Vec::new();
        }
        let pairs = world
            .stackup()
            .map(|stackup| stackup.drill_pairs.clone())
            .unwrap_or_default();

        let vias: Vec<(bevy_ecs::entity::Entity, Via)> = {
            let ecs = world.ecs_mut();
            let mut query = ecs.query::<(bevy_ecs::entity::Entity, &Via)>();
            query
                .iter(ecs)
                .map(|(entity, via)| (entity, *via))
                .collect()
        };

        let mut violations = Vec::new();
        for (entity, via) in vias {
            let Some(span) = span_of(&via, copper_count) else {
                continue;
            };
            let (start, end) = (via.start_layer, via.end_layer);

            let refused = match span {
                Span::Through => None,
                Span::Blind if !rules.blind_vias_allowed => Some("blind"),
                Span::Buried if !rules.buried_vias_allowed => Some("buried"),
                _ => None,
            };
            if let Some(word) = refused {
                violations.push(DrcViolation::via_span(
                    entity,
                    format!(
                        "a via from {start} to {end} is a {word} via and this table does not \
                         drill them: a hole that stops inside the board is drilled in its own \
                         lamination cycle"
                    ),
                    via.position,
                ));
                continue;
            }

            // The design's own list, when it states one. A pair that is not on
            // it is a hole this build does not make, whatever the house can do.
            if !pairs.is_empty() && !pairs.iter().any(|pair| pair.covers(start, end)) {
                let listed: Vec<String> = pairs
                    .iter()
                    .map(|pair| format!("{} to {}", pair.start, pair.end))
                    .collect();
                violations.push(DrcViolation::via_span(
                    entity,
                    format!(
                        "a via from {start} to {end} is not a span this build drills: the \
                         stackup states {}",
                        listed.join(", ")
                    ),
                    via.position,
                ));
            }
        }
        violations
    }
}
