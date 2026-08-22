//! What the stack actually delivers, against what the net asked for.
//!
//! A net can say `impedance 90ohm`. Whether it gets 90 depends on how wide the
//! trace is, how far it sits from its reference plane and what is in between -
//! and until now nothing compared the two. `require_impedance_control` has
//! been a flag in `cypcb-rules` with no code behind it since it was written.
//!
//! # The tolerance, and why it is not tighter
//!
//! A miss is reported when it exceeds **10%** of the target. That is not a
//! house style: the microstrip form this uses is quoted at 5-7% accuracy, so a
//! threshold under that would report the equation's own error as a defect on
//! the board. Ten percent is the smallest round figure above it, and it is
//! also the tolerance controlled-impedance boards are ordinarily quoted to, so
//! a miss this rule reports is a miss a fabricator would also call one.
//!
//! # What it will not do
//!
//! Say nothing and pass. A layer whose surroundings the stack cannot describe
//! is **reported as not checked**, once per net and layer. That covers an
//! inner layer which is not centred between its planes, a dielectric stating
//! no `dk`, and a stack stating no thickness. A controlled-impedance net that
//! quietly goes unchecked is the failure this rule exists to prevent, and it
//! looks exactly like a pass.

use std::collections::BTreeSet;

use cypcb_core::{Nm, Point};
use cypcb_world::components::trace::Trace;
use cypcb_world::components::{CopperEnvironment, NetId};
use cypcb_world::{BoardWorld, Layer};

use crate::presets::DesignRules;
use crate::violation::DrcViolation;

use super::DrcRule;

/// How far off target a trace has to be before it is worth saying so.
///
/// Hundredths of a percent, so the comparison stays in integers.
const TOLERANCE_PERCENT_X100: u64 = 1_000;

/// Rule for checking a net's stated impedance against what the stack gives it.
pub struct ImpedanceRule;

impl DrcRule for ImpedanceRule {
    fn name(&self) -> &'static str {
        "impedance"
    }

    fn check(&self, world: &mut BoardWorld, _rules: &DesignRules) -> Vec<DrcViolation> {
        let Some(stackup) = world.stackup().cloned() else {
            // A board that describes no stack cannot be asked what it
            // delivers. That is reported by `stackup`'s own rule rather than
            // repeated per trace here.
            return Vec::new();
        };
        let copper_count = stackup.copper_count();

        let traces: Vec<(bevy_ecs::entity::Entity, NetId, Layer, Nm, Point)> = {
            let ecs = world.ecs_mut();
            let mut query = ecs.query::<(bevy_ecs::entity::Entity, &Trace)>();
            query
                .iter(ecs)
                .filter_map(|(entity, trace)| {
                    Some((
                        entity,
                        trace.net_id,
                        trace.layer,
                        trace.width,
                        midpoint(trace)?,
                    ))
                })
                .collect()
        };

        let mut violations = Vec::new();
        // One "not checked" per net and layer rather than one per trace: a net
        // is routed in many segments and the reason is the same for all of
        // them.
        let mut said: BTreeSet<(u32, String)> = BTreeSet::new();

        for (entity, net_id, layer, width, at) in traces {
            let Some(target) = world
                .net_constraints(net_id)
                .and_then(|c| c.impedance_ohms_x100)
            else {
                continue;
            };
            let net_name = world.net_name(net_id).unwrap_or("unnamed").to_string();
            let Some(index) = copper_index(layer, copper_count) else {
                continue;
            };

            let computed = stackup
                .environment_of(index)
                .and_then(|environment| impedance_of(environment, width));

            let Some(computed) = computed else {
                if said.insert((net_id.0, format!("{layer}"))) {
                    let mut violation = DrcViolation::impedance(entity, at);
                    violation.message = format!(
                        "net '{net_name}' asks for {} and this stack cannot be asked what it delivers on {layer}: \
                         the layer is not centred between two planes of the same dielectric, or the stack states no thickness or no dk for it. \
                         Not checked - not passed",
                        format_ohms(target)
                    );
                    violations.push(violation);
                }
                continue;
            };

            let off_by = computed.abs_diff(target) as u64 * 10_000 / u64::from(target);
            if off_by > TOLERANCE_PERCENT_X100 {
                let mut violation = DrcViolation::impedance(entity, at);
                violation.message = format!(
                    "net '{net_name}' asks for {} and a {:.3}mm trace on {layer} gives {} - {:.1}% off. \
                     IPC-2141's closed form, which is quoted at 5-7%: check a controlled-impedance stack against your fabricator's own calculator",
                    format_ohms(target),
                    width.raw() as f64 / 1_000_000.0,
                    format_ohms(computed),
                    off_by as f64 / 100.0,
                );
                violations.push(violation);
            }
        }

        violations
    }
}

/// Which entry of the stack's copper sequence a layer is.
///
/// **`Layer::Inner` is zero-based**: `sync.rs` maps the language's `Inner1` to
/// `Layer::Inner(0)`, `job.rs` names that one `In1`, and the KiCad writer
/// spells it `In1.Cu`. The stack's copper sequence is not - its first entry is
/// the top layer - so the first inner layer is copper entry **1** and the
/// number is offset rather than used as it stands.
///
/// This read `Inner(n)` as entry `n` when it was written, which put every
/// trace on the first inner layer against the **top layer's** surroundings.
/// It survived its own tests because a symmetric stack gives neighbouring
/// layers the same answer - the third time that has hidden an index error in
/// this project.
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

/// The impedance a trace of this width gets in these surroundings.
fn impedance_of(environment: CopperEnvironment, width: Nm) -> Option<u32> {
    match environment {
        CopperEnvironment::Microstrip {
            height,
            dk_x1000,
            copper,
        } => cypcb_calc::microstrip_ohms_x100(width, height, copper, dk_x1000),
        CopperEnvironment::Stripline {
            plate_separation,
            dk_x1000,
            copper,
        } => cypcb_calc::stripline_ohms_x100(width, plate_separation, copper, dk_x1000),
    }
}

/// A point on the trace to report the violation at.
fn midpoint(trace: &Trace) -> Option<Point> {
    let segment = trace.segments.get(trace.segments.len() / 2)?;
    Some(Point::new(
        Nm((segment.start.x.raw() + segment.end.x.raw()) / 2),
        Nm((segment.start.y.raw() + segment.end.y.raw()) / 2),
    ))
}

/// Hundredths of an ohm, printed the way a designer states one.
fn format_ohms(ohms_x100: u32) -> String {
    if ohms_x100.is_multiple_of(100) {
        format!("{}ohm", ohms_x100 / 100)
    } else {
        format!("{:.2}ohm", f64::from(ohms_x100) / 100.0)
    }
}
