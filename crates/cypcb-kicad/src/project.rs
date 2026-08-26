//! The project file that carries a board's design rules into KiCad.
//!
//! A `.kicad_pcb` holds the board. The numbers KiCad checks it against - the
//! narrowest trace, the smallest gap, the smallest hole - live in the
//! `.kicad_pro` beside it, under `board.design_settings.rules`. Nothing in the
//! board file states them, and this project spent one commit believing
//! otherwise: `--preset` wrote a `(setup (rules ...))` node into the board, and
//! KiCad 10.0.5 answered
//!
//! ```text
//! Failed to load board: Unexpected rules in 'blink.kicad_pcb', line 6, offset 6.
//! ```
//!
//! so the flag whose whole purpose was to make the two tools agree produced a
//! file the other tool would not open at all.
//!
//! Measured, on `routing-test` routed and exported: without a project file
//! KiCad reports 8 `track_width` and 1 `clearance` violations against its own
//! factory defaults of 0.2mm; with one carrying JLCPCB's published 0.127mm,
//! it reports none of either. The remaining warnings are silkscreen overlap
//! and unmatched library footprints, neither of which is a design rule.

use cypcb_core::Nm;

use crate::board_writer::KicadDesignRules;

/// Millimetres as JSON writes them.
fn mm(nm: Nm) -> String {
    let value = nm.0 as f64 / 1_000_000.0;
    let text = format!("{value:.6}");
    let trimmed = text.trim_end_matches('0').trim_end_matches('.');
    if trimmed.is_empty() || trimmed == "-" {
        "0".to_string()
    } else {
        trimmed.to_string()
    }
}

/// The `.kicad_pro` for a board written with `rules`.
///
/// `stem` is the file name without its extension - KiCad stores it in `meta`
/// and expects it to match the file it is read from.
///
/// Two places carry the numbers, because KiCad reads them for two different
/// questions. `board.design_settings.rules` is what the design rule check
/// enforces; `net_settings.classes` is what the editor hands you when you draw
/// a new trace. A project stating only the first passes DRC and then offers a
/// default 0.2mm trace on a board whose fab tops out at 0.127mm.
/// The house the board was written for, in a key KiCad does not read.
///
/// A `.kicad_pcb` has no field for a fabricator's name, and losing it costs
/// the board its table: a design read back from KiCad is graded against the
/// default one, which on a blind-via board means two `via-span` violations for
/// holes the house it was written for drills every day. The numbers already
/// travel in this file; the name travels with them, under a key of this
/// project's own so nothing here pretends to be KiCad's.
///
/// KiCad ignores what it does not know, and drops it if it saves the project
/// itself - so this survives a round trip through these two commands rather
/// than through the editor.
fn cypcb_section(fab: Option<&str>) -> String {
    match fab {
        Some(name) => format!("  \"cypcb\": {{\n    \"fab\": \"{name}\"\n  }},\n"),
        None => String::new(),
    }
}

/// The name this project files a board's house under.
pub const FAB_KEY: &str = "cypcb";

/// The fabricator a `.kicad_pro` written by this project names, if it names one.
///
/// Reads only the key this project writes. A project file KiCad wrote has no
/// such key and gets `None`, which is the same answer as a board that never
/// named a house.
pub fn fab_of_project(text: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    value
        .get(FAB_KEY)?
        .get("fab")?
        .as_str()
        .map(|name| name.to_string())
}

pub fn write_project(rules: KicadDesignRules, stem: &str, fab: Option<&str>) -> String {
    format!(
        r#"{{
{cypcb}  "board": {{
    "design_settings": {{
      "rules": {{
        "min_clearance": {clearance},
        "min_track_width": {track_width},
        "min_via_diameter": {via_diameter},
        "min_through_hole_diameter": {drill_size},
        "min_hole_to_hole": {hole_to_hole},
        "min_copper_edge_clearance": {edge_clearance},
        "min_silk_clearance": {silk_clearance},
        "min_via_annular_width": {annular_ring}
      }}
    }}
  }},
  "net_settings": {{
    "classes": [
      {{
        "name": "Default",
        "clearance": {clearance},
        "track_width": {track_width},
        "via_diameter": {via_diameter},
        "via_drill": {via_drill}
      }}
    ],
    "meta": {{
      "version": 5
    }}
  }},
  "meta": {{
    "filename": "{stem}.kicad_pro",
    "version": 3
  }}
}}
"#,
        cypcb = cypcb_section(fab),
        clearance = mm(rules.clearance),
        track_width = mm(rules.track_width),
        via_diameter = mm(rules.via_diameter),
        via_drill = mm(rules.via_drill),
        drill_size = mm(rules.drill_size),
        hole_to_hole = mm(rules.hole_to_hole),
        edge_clearance = mm(rules.edge_clearance),
        silk_clearance = mm(rules.silk_clearance),
        annular_ring = mm(rules.annular_ring),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jlcpcb_shaped() -> KicadDesignRules {
        KicadDesignRules {
            clearance: Nm::from_mm(0.127),
            track_width: Nm::from_mm(0.127),
            via_diameter: Nm::from_mm(0.45),
            via_drill: Nm::from_mm(0.3),
            mask_expansion: Nm::from_mm(0.05),
            drill_size: Nm::from_mm(0.3),
            hole_to_hole: Nm::from_mm(0.5),
            edge_clearance: Nm::from_mm(0.2),
            silk_clearance: Nm::from_mm(0.15),
            annular_ring: Nm::from_mm(0.13),
        }
    }

    #[test]
    fn the_numbers_are_the_fabs_own() {
        let text = write_project(jlcpcb_shaped(), "board", None);

        assert!(text.contains("\"min_track_width\": 0.127"), "{text}");
        assert!(text.contains("\"min_clearance\": 0.127"), "{text}");
        assert!(text.contains("\"min_hole_to_hole\": 0.5"), "{text}");
        assert!(
            text.contains("\"min_copper_edge_clearance\": 0.2"),
            "{text}"
        );
    }

    #[test]
    fn the_editors_default_trace_is_the_same_number() {
        // A project that passes DRC and then hands you a trace the fab cannot
        // make is the same disagreement in a different place.
        let text = write_project(jlcpcb_shaped(), "board", None);
        let class = text
            .split("\"classes\"")
            .nth(1)
            .expect("the project states a net class");

        assert!(class.contains("\"track_width\": 0.127"), "{class}");
        assert!(class.contains("\"clearance\": 0.127"), "{class}");
    }

    #[test]
    fn the_file_names_itself() {
        let text = write_project(jlcpcb_shaped(), "my-board", None);
        assert!(
            text.contains("\"filename\": \"my-board.kicad_pro\""),
            "{text}"
        );
    }

    /// The house rides in the project file, and comes back out of it.
    ///
    /// A `.kicad_pcb` has no field for a fabricator, so a board round-tripped
    /// through KiCad used to be graded against the default table - two
    /// `via-span` violations on a blind-via board the house it was written for
    /// drills without blinking.
    #[test]
    fn the_house_rides_in_the_project_file() {
        let named = write_project(jlcpcb_shaped(), "board", Some("pcbway"));
        assert!(named.contains("\"fab\": \"pcbway\""), "{named}");
        assert_eq!(fab_of_project(&named).as_deref(), Some("pcbway"));

        // A board that named no house says nothing, and neither does a project
        // file KiCad wrote itself.
        let anonymous = write_project(jlcpcb_shaped(), "board", None);
        assert!(!anonymous.contains("cypcb"), "{anonymous}");
        assert_eq!(fab_of_project(&anonymous), None);
        assert_eq!(fab_of_project("{\"board\": {}}"), None);

        // Both forms are still JSON, which is the half a typo in the key would
        // break silently.
        for text in [named, anonymous] {
            serde_json::from_str::<serde_json::Value>(&text)
                .unwrap_or_else(|error| panic!("the project file is JSON: {error}\n{text}"));
        }
    }
}
