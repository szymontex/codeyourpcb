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
use cypcb_world::registry::NetConstraints;

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
/// What this project keeps in a `.kicad_pro` that KiCad has no field for.
///
/// A `.kicad_pcb` carries a board's shape and its copper and nothing about the
/// house it was written for or what a net asks of a trace. Both were dropped on
/// the way out and announced as losses; both ride here instead, under a key of
/// this project's own so nothing pretends to be KiCad's.
#[derive(Debug, Default, Clone)]
pub struct ProjectExtras {
    /// The fabricator the board named.
    pub fab: Option<String>,
    /// What each net asks for, by net name.
    pub nets: Vec<(String, NetConstraints)>,
}

/// One net's figures as the project file states them.
fn net_json(name: &str, constraints: &NetConstraints) -> Option<String> {
    let mut fields: Vec<String> = Vec::new();
    if let Some(width) = constraints.width {
        fields.push(format!("\"width_nm\": {}", width.raw()));
    }
    if let Some(clearance) = constraints.clearance {
        fields.push(format!("\"clearance_nm\": {}", clearance.raw()));
    }
    if let Some(current) = constraints.current_ma {
        fields.push(format!("\"current_ma\": {current}"));
    }
    if let Some(impedance) = constraints.impedance_ohms_x100 {
        fields.push(format!("\"impedance_ohms_x100\": {impedance}"));
    }
    if let Some(neck) = constraints.neck {
        fields.push(format!("\"neck_width_nm\": {}", neck.width.raw()));
        fields.push(format!("\"neck_length_nm\": {}", neck.length.raw()));
    }
    (!fields.is_empty()).then(|| format!("      \"{name}\": {{ {} }}", fields.join(", ")))
}

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
fn cypcb_section(extras: &ProjectExtras) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(name) = &extras.fab {
        parts.push(format!("    \"fab\": \"{name}\""));
    }
    let nets: Vec<String> = extras
        .nets
        .iter()
        .filter_map(|(name, constraints)| net_json(name, constraints))
        .collect();
    if !nets.is_empty() {
        parts.push(format!("    \"nets\": {{\n{}\n    }}", nets.join(",\n")));
    }
    if parts.is_empty() {
        return String::new();
    }
    format!("  \"cypcb\": {{\n{}\n  }},\n", parts.join(",\n"))
}

/// What each net asks for, out of a project file this project wrote.
///
/// Empty for a file KiCad wrote, which has no such key, and for a design whose
/// nets stated nothing.
pub fn nets_of_project(text: &str) -> Vec<(String, NetConstraints)> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(text) else {
        return Vec::new();
    };
    let Some(nets) = value.get(FAB_KEY).and_then(|ours| ours.get("nets")) else {
        return Vec::new();
    };
    let Some(nets) = nets.as_object() else {
        return Vec::new();
    };
    let nm = |row: &serde_json::Value, key: &str| -> Option<Nm> { row.get(key)?.as_i64().map(Nm) };
    nets.iter()
        .map(|(name, row)| {
            let neck = match (nm(row, "neck_width_nm"), nm(row, "neck_length_nm")) {
                (Some(width), Some(length)) => {
                    Some(cypcb_world::components::trace::TraceNeck { width, length })
                }
                _ => None,
            };
            (
                name.clone(),
                NetConstraints {
                    width: nm(row, "width_nm"),
                    clearance: nm(row, "clearance_nm"),
                    current_ma: row.get("current_ma").and_then(|v| v.as_f64()),
                    impedance_ohms_x100: row
                        .get("impedance_ohms_x100")
                        .and_then(|v| v.as_u64())
                        .map(|v| v as u32),
                    neck,
                },
            )
        })
        .collect()
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

/// The design rules a `.kicad_pro` states, in nanometres.
///
/// Every field is optional because a project file states what somebody set:
/// KiCad writes the keys it knows and a person editing one by hand may leave
/// any of them out.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ProjectRules {
    pub clearance: Option<Nm>,
    pub track_width: Option<Nm>,
    pub via_diameter: Option<Nm>,
    pub drill_size: Option<Nm>,
    pub hole_to_hole: Option<Nm>,
    pub edge_clearance: Option<Nm>,
    pub silk_clearance: Option<Nm>,
    pub annular_ring: Option<Nm>,
}

impl ProjectRules {
    /// Each rule with the name a person reads, in the order KiCad writes them.
    pub fn named(&self) -> Vec<(&'static str, Nm)> {
        [
            ("minimum clearance", self.clearance),
            ("minimum track width", self.track_width),
            ("minimum via diameter", self.via_diameter),
            ("minimum drill", self.drill_size),
            ("minimum hole to hole", self.hole_to_hole),
            ("minimum edge clearance", self.edge_clearance),
            ("minimum silkscreen clearance", self.silk_clearance),
            ("minimum annular ring", self.annular_ring),
        ]
        .into_iter()
        .filter_map(|(name, value)| value.map(|value| (name, value)))
        .collect()
    }
}

/// The rules a `.kicad_pro` states, whoever wrote it.
///
/// `board.design_settings.rules` is where KiCad keeps what its design rule
/// check enforces, and it is the half of a project file this language has no
/// way to say: a board states a fab and a net states its own figures, and
/// neither is a rule table written out per board.
pub fn rules_of_project(text: &str) -> Option<ProjectRules> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    let rules = value.get("board")?.get("design_settings")?.get("rules")?;
    let read = |key: &str| -> Option<Nm> {
        let mm = rules.get(key)?.as_f64()?;
        (mm.is_finite() && mm >= 0.0).then(|| Nm::from_mm(mm))
    };
    let found = ProjectRules {
        clearance: read("min_clearance"),
        track_width: read("min_track_width"),
        via_diameter: read("min_via_diameter"),
        drill_size: read("min_through_hole_diameter"),
        hole_to_hole: read("min_hole_to_hole"),
        edge_clearance: read("min_copper_edge_clearance"),
        silk_clearance: read("min_silk_clearance"),
        annular_ring: read("min_via_annular_width"),
    };
    (found != ProjectRules::default()).then_some(found)
}

pub fn write_project(rules: KicadDesignRules, stem: &str, extras: &ProjectExtras) -> String {
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
        cypcb = cypcb_section(extras),
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
        let text = write_project(jlcpcb_shaped(), "board", &ProjectExtras::default());

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
        let text = write_project(jlcpcb_shaped(), "board", &ProjectExtras::default());
        let class = text
            .split("\"classes\"")
            .nth(1)
            .expect("the project states a net class");

        assert!(class.contains("\"track_width\": 0.127"), "{class}");
        assert!(class.contains("\"clearance\": 0.127"), "{class}");
    }

    #[test]
    fn the_file_names_itself() {
        let text = write_project(jlcpcb_shaped(), "my-board", &ProjectExtras::default());
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
        let named = write_project(
            jlcpcb_shaped(),
            "board",
            &ProjectExtras {
                fab: Some("pcbway".to_string()),
                nets: Vec::new(),
            },
        );
        assert!(named.contains("\"fab\": \"pcbway\""), "{named}");
        assert_eq!(fab_of_project(&named).as_deref(), Some("pcbway"));

        // A board that named no house says nothing, and neither does a project
        // file KiCad wrote itself.
        let anonymous = write_project(jlcpcb_shaped(), "board", &ProjectExtras::default());
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

    /// The rules come back out of the file they were written into.
    #[test]
    fn the_numbers_are_read_back() {
        let text = write_project(jlcpcb_shaped(), "board", &ProjectExtras::default());
        let read = rules_of_project(&text).expect("the file states rules");
        assert_eq!(read.clearance, Some(jlcpcb_shaped().clearance));
        assert_eq!(read.annular_ring, Some(jlcpcb_shaped().annular_ring));
        assert_eq!(read.named().len(), 8, "{read:?}");

        // A file with no rules section, and one that is not JSON at all.
        assert_eq!(rules_of_project("{\"board\": {}}"), None);
        assert_eq!(rules_of_project("not json"), None);
    }

    /// What a net asks for rides in the same key, and comes back out.
    #[test]
    fn a_nets_figures_ride_in_the_project_file() {
        let asking = NetConstraints {
            width: Some(Nm::from_mm(0.2)),
            clearance: Some(Nm::from_mm(0.25)),
            current_ma: Some(500.0),
            impedance_ohms_x100: Some(5000),
            neck: Some(cypcb_world::components::trace::TraceNeck {
                width: Nm::from_mm(0.15),
                length: Nm::from_mm(1.0),
            }),
        };
        let text = write_project(
            jlcpcb_shaped(),
            "board",
            &ProjectExtras {
                fab: None,
                nets: vec![("SIG".to_string(), asking)],
            },
        );
        serde_json::from_str::<serde_json::Value>(&text)
            .unwrap_or_else(|error| panic!("still JSON: {error}\n{text}"));

        let read = nets_of_project(&text);
        assert_eq!(read.len(), 1, "{read:?}");
        assert_eq!(read[0].0, "SIG");
        assert_eq!(read[0].1, asking, "every figure comes back");

        // A net that asks for nothing is not written, and a file KiCad wrote
        // has no such key at all.
        let silent = write_project(
            jlcpcb_shaped(),
            "board",
            &ProjectExtras {
                fab: None,
                nets: vec![("GND".to_string(), NetConstraints::default())],
            },
        );
        assert!(!silent.contains("cypcb"), "{silent}");
        assert!(nets_of_project(&silent).is_empty());
        assert!(nets_of_project("{\"board\": {}}").is_empty());
    }
}
