//! Does the KiCad file state the relief the board was checked against?
//!
//! `cargo test -p cypcb-cli --test the_kicad_file_states_the_relief_the_preset_ordered -- --nocapture`
//!
//! The writer used to state `(thermal_gap 0.5) (thermal_bridge_width 0.5)` as
//! two literals in its own source. Both shipped export presets order 0.254 mm
//! for the same two figures and the Gerber path honours them, so one command
//! produced two files for one board whose reliefs differed by a factor of about
//! two, and neither file mentioned the other.
//!
//! The round-trip test this project already had could not find it. The writer's
//! own header says why: the file is read back by this project's importer and
//! compared with what went in, so a closed loop cannot catch a shape both halves
//! agree on. A constant is exactly that shape - the writer states it, the parser
//! ignores it, the comparison passes, and the disagreement exists only between
//! our file and the fabricator's reading of it.
//!
//! So these assertions do not close a loop. They compare what the writer emits
//! against the preset it was handed, and the expected numbers are read from the
//! preset rather than typed here: changing the fab table changes the file, or
//! this test fails.

use std::path::Path;

use cypcb_kicad::{parse_kicad_pcb, write_board_with_rules, KicadDesignRules};
use cypcb_rules::presets::RulesPreset;

fn plane_board() -> cypcb_world::BoardWorld {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/fixtures/benchmark/plane_board.kicad_pcb");
    parse_kicad_pcb(&path).expect("the fixture parses").world
}

/// The two figures under test, taken from the fab table rather than written out.
fn jlcpcb() -> (cypcb_core::Nm, cypcb_core::Nm, KicadDesignRules) {
    let rules = RulesPreset::JlcpcbStandard2Layer.constraints();
    (
        rules.thermal_relief_gap,
        rules.thermal_relief_spoke_width,
        KicadDesignRules {
            clearance: rules.min_clearance,
            track_width: rules.min_trace_width,
            via_diameter: rules
                .min_via_diameter
                .unwrap_or(cypcb_core::Nm::from_mm(0.6)),
            via_drill: rules.min_via_drill,
            mask_expansion: rules.solder_mask_expansion,
            drill_size: rules.min_drill_size,
            hole_to_hole: rules.min_hole_to_hole,
            edge_clearance: rules.min_edge_clearance,
            silk_clearance: rules
                .min_silk_clearance
                .unwrap_or(cypcb_core::Nm::from_mm(0.15)),
            annular_ring: rules.min_annular_ring,
            thermal_relief_gap: rules.thermal_relief_gap,
            thermal_relief_spoke_width: rules.thermal_relief_spoke_width,
        },
    )
}

#[test]
fn the_relief_written_is_the_relief_the_fab_table_orders() {
    let (gap, spoke, rules) = jlcpcb();
    let mut world = plane_board();
    let file = write_board_with_rules(&mut world, "cypcb", Some(rules));

    let wanted = format!(
        "(fill yes (thermal_gap {}) (thermal_bridge_width {}))",
        gap.to_mm(),
        spoke.to_mm()
    );
    println!("expected from the fab table: {wanted}");

    assert!(
        file.contains(&wanted),
        "the file does not state the relief the preset ordered.\n  wanted: {wanted}\n  \
         the fill line it wrote: {:?}",
        file.lines()
            .find(|line| line.contains("(fill "))
            .unwrap_or("none")
    );

    // The control that names the defect rather than the fix: the old literal
    // must be gone. Without this the assertion above would pass on a writer
    // that emitted both.
    assert!(
        !file.contains("thermal_gap 0.5"),
        "the 0.5mm literal is still in the file"
    );
}

#[test]
fn a_board_with_no_chosen_rules_states_no_relief_at_all() {
    // The other half of the same principle, and the one the export command
    // already applies to the whole setup node: rules nobody chose are worse
    // than none, because KiCad believes them. With no preset the fill node
    // carries no numbers and KiCad fills its own defaults.
    let mut world = plane_board();
    let file = write_board_with_rules(&mut world, "cypcb", None);

    let fill = file
        .lines()
        .find(|line| line.contains("(fill "))
        .expect("a copper pour is still filled");
    println!("with no rules chosen: {}", fill.trim());

    assert!(
        !file.contains("thermal_gap"),
        "a board that chose no fab table must not be handed a relief figure"
    );
    assert!(!file.contains("thermal_bridge_width"), "nor a spoke width");
}

#[test]
fn the_two_figures_are_not_the_same_number_by_accident() {
    // The control on the test itself. Both figures happen to be 0.254 mm in
    // every shipped preset, so an assertion on the pair would pass against a
    // writer that emitted one of them twice. This pins the substitution to two
    // separate fields by changing one of them.
    let (gap, _spoke, mut rules) = jlcpcb();
    rules.thermal_relief_spoke_width = cypcb_core::Nm::from_mm(0.3);
    let mut world = plane_board();
    let file = write_board_with_rules(&mut world, "cypcb", Some(rules));

    let wanted = format!(
        "(fill yes (thermal_gap {}) (thermal_bridge_width 0.3))",
        gap.to_mm()
    );
    assert!(
        file.contains(&wanted),
        "the two figures have to come from two fields.\n  wanted: {wanted}\n  got: {:?}",
        file.lines()
            .find(|line| line.contains("(fill "))
            .unwrap_or("none")
    );
}
