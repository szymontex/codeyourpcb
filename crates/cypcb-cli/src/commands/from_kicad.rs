//! Reading a KiCad board back out as a design in this language.

use std::path::PathBuf;

use clap::Args;
use cypcb_drc::PresetRules;
use miette::{IntoDiagnostic, Result, WrapErr};

/// Write a KiCad `.kicad_pcb` board out as a `.cypcb` design.
///
/// The mirror of `to-kicad`, and the half that was missing. A KiCad board could
/// already be checked, routed, scored and exported by this tool - `check`,
/// `route` and `export` all take one - but it could not be *edited*, because
/// nothing turned it into the text this language is. So the one thing the
/// project exists for, a board you read and change as source, was the one thing
/// a KiCad user could not get.
#[derive(Args)]
pub struct FromKicadCommand {
    /// Input .kicad_pcb file
    file: PathBuf,

    /// Where to write the design (default: the input file with a .cypcb suffix)
    #[arg(short, long)]
    output: Option<PathBuf>,
}

impl FromKicadCommand {
    pub fn run(self) -> Result<()> {
        let parsed = cypcb_kicad::parse_kicad_pcb(&self.file)
            .map_err(|error| miette::miette!("{error}"))
            .wrap_err_with(|| format!("Failed to read {}", self.file.display()))?;

        let mut world = parsed.world;

        // The importer hands routed copper back as `reference_routes` rather
        // than as entities on the board - it is what the router is measured
        // against, not part of the model. Nothing put it into the world, so
        // the first version of this command wrote a design with every trace
        // missing and said nothing: led_blink went in with one segment and came
        // out with none.
        if let Some(routes) = parsed.reference_routes {
            cypcb_router::apply_routes(&mut world, &routes);
        }

        // The house the board was written for, out of the project file beside
        // it. A `.kicad_pcb` has no field for a fabricator, so `to-kicad` puts
        // the name in the `.kicad_pro` it writes; without reading it back, a
        // blind-via board came home graded against the default table and
        // reported two `via-span` violations for holes its own house drills.
        //
        // Silent when there is no project file, when it is not ours, or when
        // it names nothing: each of those is a board that named no house.
        if world.fab().is_none() {
            let project = self.file.with_extension("kicad_pro");
            if let Ok(text) = std::fs::read_to_string(&project) {
                if let Some(fab) = cypcb_kicad::fab_of_project(&text) {
                    eprintln!(
                        "The house comes from {}: this board is checked against {fab}.",
                        project.display()
                    );
                    world.set_fab(cypcb_world::components::Fab(fab));
                }
            }
        }

        // The rest of that project file: eight numbers KiCad enforces, which
        // this language has no way to state. A board names a fab and a net
        // states its own figures; nothing writes a rule table per board. So
        // the numbers are read, compared against the table this design will
        // actually be checked against, and the ones that disagree are named -
        // a project set up by hand to a tighter clearance than its house
        // publishes is a fact about the board, and dropping it silently is how
        // the fab used to be lost.
        {
            let project = self.file.with_extension("kicad_pro");
            if let Some(stated) = std::fs::read_to_string(&project)
                .ok()
                .and_then(|text| cypcb_kicad::rules_of_project(&text))
            {
                let preset = crate::preset_choice::resolve(None, &world)?;
                let table = preset.rules();
                let checked = |name: &str| -> Option<cypcb_core::Nm> {
                    match name {
                        "minimum clearance" => Some(table.min_clearance),
                        "minimum track width" => Some(table.min_trace_width),
                        "minimum via diameter" => Some(table.min_via_diameter),
                        "minimum drill" => Some(table.min_drill_size),
                        "minimum hole to hole" => Some(table.min_hole_to_hole),
                        "minimum edge clearance" => Some(table.min_edge_clearance),
                        "minimum silkscreen clearance" => Some(table.min_silk_clearance),
                        "minimum annular ring" => Some(table.min_annular_ring),
                        _ => None,
                    }
                };
                let mut differing: Vec<String> = Vec::new();
                for (name, value) in stated.named() {
                    let Some(ours) = checked(name) else { continue };
                    if value != ours {
                        differing.push(format!(
                            "{name} {:.3}mm against {:.3}mm",
                            value.to_mm(),
                            ours.to_mm()
                        ));
                    }
                }
                if !differing.is_empty() {
                    eprintln!(
                        "Warning: {} states rules this language cannot ({}): a board names a \
                         fab and a net states its own figures, so the design is checked against \
                         {} instead.",
                        project.display(),
                        differing.join(", "),
                        preset.name()
                    );
                }
            }
        }

        let source = cypcb_world::dsl::board_as_dsl(&mut world);

        let output = self
            .output
            .unwrap_or_else(|| self.file.with_extension("cypcb"));
        std::fs::write(&output, &source)
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to write {}", output.display()))?;

        let components = source
            .lines()
            .filter(|l| l.starts_with("component "))
            .count();
        let nets = source.lines().filter(|l| l.starts_with("net ")).count();
        let footprints = source
            .lines()
            .filter(|l| l.starts_with("footprint "))
            .count();
        println!(
            "Wrote {} ({} bytes): {components} component(s), {nets} net(s), \
             {footprints} footprint definition(s)",
            output.display(),
            source.len()
        );

        // What the board carried and the design does not.
        //
        // The importer refuses a pour it cannot state - an L cut around a
        // connector is not a rectangle, and a bounding box would put copper
        // where the shape was drawn to avoid it - and it says why. The browser
        // engine has printed those reasons since they existed; this command
        // did not, so a board came through the command line one ground plane
        // short and nothing said so. Same for a stackup this reader will not
        // guess at.
        for refusal in parsed
            .metadata
            .zone_refusals
            .iter()
            .chain(parsed.metadata.stackup_refusals.iter())
        {
            eprintln!("Warning: {refusal}");
        }

        // A design that will not read back is not an import, and the cost of
        // finding out here rather than on the user's next command is one parse.
        let reread = cypcb_parser::parse(&source);
        if let Some(first) = reread.errors.first() {
            // A pad named rather than numbered used to be reported here as a
            // gap in the language: `pad_definition` took a number, and a USB-C
            // receptacle's pads are called A1, B4, S1. The language takes both
            // a name and a quoted name now, and the writer quotes what has to
            // be quoted, so a board like that imports like any other. What is
            // left is the honest message: a file this writer produced and this
            // reader refuses is a defect in the writer.
            return Err(miette::miette!("{first}")).wrap_err_with(|| {
                format!(
                    "{} was written but does not parse - this is a defect in the writer, \
                     not in your board",
                    output.display()
                )
            });
        }

        Ok(())
    }
}
