//! Reading a KiCad board back out as a design in this language.

use std::path::PathBuf;

use clap::Args;
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
