//! A declared stackup has to describe the board the rest of the design says.
//!
//! `stackup { copper 0.035mm prepreg 0.2mm copper 0.035mm }` is the design
//! telling a fabricator what to press together. Both parsers have read it
//! since the board block started refusing what it does not recognise, and
//! until now nothing consumed it - so a stackup that contradicted the layer
//! count written three lines above it went to the fab unread.
//!
//! Two contradictions are worth reporting, and both are answerable from the
//! design alone:
//!
//! - The stackup describes a different number of copper layers than `layers N`
//!   does. The exporter writes one Gerber per copper layer from the count, so
//!   the files and the build instructions disagree about what the board is.
//! - Two copper layers sit against each other with no dielectric between them.
//!   Two foils pressed together are one thicker foil, so this describes a
//!   board whose layers are shorted to each other by construction.
//!
//! - The board asks for a process the fab's table refuses. Castellated pads
//!   are the first: `stackup { pads castellated }` is the design asking for
//!   plated holes cut in half at the outline, and a table that says the house
//!   does not make them is the answer. That flag had been in
//!   `DesignConstraints` since the tables were written and was dropped before
//!   it reached any rule, so it checked nothing - and no board could state the
//!   want either, which is why the gap survived.
//!
//! Not reported: total thickness against anything. What a fab supports is fab
//! data this project does not have, and inventing a range would be worse than
//! staying quiet - the thickness is put in the message instead, where a person
//! who does know can read it.

use cypcb_core::{Nm, Point};
use cypcb_world::components::{Stackup, StackupLayerKind};
use cypcb_world::BoardWorld;

use crate::presets::DesignRules;
use crate::violation::DrcViolation;

use super::DrcRule;

/// Rule for checking a declared stackup against the rest of the design.
pub struct StackupRule;

impl DrcRule for StackupRule {
    fn name(&self) -> &'static str {
        "stackup"
    }

    fn check(&self, world: &mut BoardWorld, rules: &DesignRules) -> Vec<DrcViolation> {
        // Most designs state no stackup and take the fab's, which is not a
        // fault and is the reason this rule reports nothing on most boards.
        let Some(stackup) = world.stackup().cloned() else {
            return Vec::new();
        };
        let Some((_, layer_stack)) = world.board_info() else {
            return Vec::new();
        };
        let Some(board) = world.board_entity() else {
            return Vec::new();
        };

        let at = Point::new(Nm(0), Nm(0));
        let mut violations = Vec::new();

        let declared = stackup.copper_count();
        let counted = usize::from(layer_stack.count);
        if declared != counted {
            violations.push(DrcViolation::stackup(
                board,
                format!(
                    "board says {counted} copper layers and the stackup describes {declared}{}",
                    thickness_note(&stackup)
                ),
                at,
            ));
        }

        // A process the design asks for and this house does not do. The
        // message names both sides, because a designer reading it has two
        // ways out: drop the request, or send the board to a house that
        // does it.
        if stackup.castellated_pads && !rules.castellated_holes_allowed {
            violations.push(DrcViolation::stackup(
                board,
                "the stackup asks for castellated pads and this table does not \
                 make them: a plated hole cut in half at the outline is a \
                 process a house either offers or does not"
                    .to_string(),
                at,
            ));
        }

        // A stiffener a house does not bond.
        //
        // A stiffener is a sheet of FR4, polyimide or steel laminated under the
        // rigid part of a flex board, and a fabricator bonds the sheets it
        // stocks rather than any figure a design asks for. JLCPCB publishes
        // three lists of thickness options; a table that has read none says
        // nothing here, because a design held to a figure nobody published is
        // worse than a design held to nothing.
        for layer in stackup
            .layers
            .iter()
            .filter(|layer| matches!(layer.kind, StackupLayerKind::Stiffener))
        {
            let (Some(thickness), Some(material)) = (layer.thickness, layer.material.as_ref())
            else {
                continue;
            };
            let asked_for = normalised(material);
            let Some((_, stocked)) = rules
                .stiffener_thickness_um
                .iter()
                .find(|(named, _)| *named == asked_for)
            else {
                // A material this house publishes no list for. The design may
                // well be right, and this rule has nothing to measure it with.
                continue;
            };
            if stocked
                .iter()
                .any(|um| Nm(i64::from(*um) * 1_000) == thickness)
            {
                continue;
            }
            violations.push(DrcViolation::stackup(
                board,
                format!(
                    "the stackup asks for a {:.3}mm {material} stiffener and this house bonds \
                     {}: a stiffener is a sheet the fabricator stocks rather than a figure a \
                     design picks",
                    thickness.to_mm(),
                    stocked
                        .iter()
                        .map(|um| format!("{:.3}mm", f64::from(*um) / 1_000.0))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                at,
            ));
        }

        for first in stackup.copper_touching_copper() {
            violations.push(DrcViolation::stackup(
                board,
                format!(
                    "copper layer {} and layer {} touch with no dielectric between them{}",
                    first + 1,
                    next_copper_after(&stackup, first) + 1,
                    surface_note(&stackup, first)
                ),
                at,
            ));
        }

        violations
    }
}

/// A material name as the fab table spells it: lower case, nothing but
/// letters and digits.
///
/// The design writes what a datasheet writes - `"FR4"`, `"Stainless Steel"`,
/// `"PI"` - and a table that only matched one spelling would hold half the
/// designs that state a stiffener to nothing at all.
fn normalised(material: &str) -> String {
    material
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// The stackup's total thickness, for the message, when every layer states one.
fn thickness_note(stackup: &Stackup) -> String {
    match stackup.total_thickness() {
        Some(total) => format!(" ({:.3}mm of material)", total.to_mm()),
        None => String::new(),
    }
}

/// The index of the copper layer that pairs with the one at `first`.
fn next_copper_after(stackup: &Stackup, first: usize) -> usize {
    stackup
        .layers
        .iter()
        .enumerate()
        .skip(first + 1)
        .find(|(_, layer)| layer.kind == StackupLayerKind::Copper)
        .map(|(index, _)| index)
        .unwrap_or(first)
}

/// Name what is between the two, when something is.
///
/// `copper mask copper` is a likelier mistake than `copper copper`, and saying
/// which layer was expected to separate them is what turns the report into an
/// edit.
fn surface_note(stackup: &Stackup, first: usize) -> String {
    let between: Vec<&str> = stackup
        .layers
        .iter()
        .take(next_copper_after(stackup, first))
        .skip(first + 1)
        .map(|layer| layer.kind.as_str())
        .collect();

    if between.is_empty() {
        String::new()
    } else {
        format!(
            " - {} is a surface finish, not a dielectric",
            between.join(" ")
        )
    }
}
