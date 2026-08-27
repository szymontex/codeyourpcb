//! Reading a mechanical drawing's cutout as a board outline.

use std::path::PathBuf;

use clap::Args;
use miette::{IntoDiagnostic, Result, WrapErr};

/// Turn a DXF drawing's closed shape into a `.cypcb` design.
///
/// An enclosure is drawn in a mechanical tool and the board has to fit inside
/// it. Until now the way that fact reached a design was a person reading
/// coordinates off a drawing and typing them in, which is how a board ends up
/// a fraction out from the case it was made for. This reads them.
///
/// What comes out is a design that parses and checks on its own: the board
/// block with the size the shape needs, and the outline itself. The rest - the
/// parts, the nets, the copper - is the design's own work.
#[derive(Args)]
pub struct FromDxfCommand {
    /// Input .dxf drawing
    file: PathBuf,

    /// Which DXF layer holds the outline
    ///
    /// Without this, every layer is read and the largest closed shape wins: a
    /// drawing of a case holds the cutout and the holes in it, and the cutout
    /// is the big one. The run says which layer it took.
    #[arg(short, long)]
    layer: Option<String>,

    /// Where to write the design (default: the drawing with a .cypcb suffix)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Keep the drawing's own coordinates instead of moving the shape to the
    /// origin.
    ///
    /// A cutout drawn 400mm along a fixture is 400mm along it in the file. A
    /// board is measured from its own corner, so the shape is moved by default
    /// and the run says by how much.
    #[arg(long)]
    keep_origin: bool,
}

/// A number as the language reads it back: `10`, `0.5`, `1.27`.
fn mm(value: cypcb_core::Nm) -> String {
    let text = format!("{:.3}", value.0 as f64 / 1_000_000.0);
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// A board name the language accepts, out of a file name it may not.
fn board_name(file: &std::path::Path) -> String {
    let stem = file
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("board");
    let cleaned: String = stem
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '_'
            }
        })
        .collect();
    match cleaned.chars().next() {
        Some(first) if first.is_ascii_alphabetic() => cleaned,
        _ => format!("board_{cleaned}"),
    }
}

impl FromDxfCommand {
    pub fn run(self) -> Result<()> {
        let text = std::fs::read_to_string(&self.file)
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to read {}", self.file.display()))?;

        let outline = cypcb_export::dxf::read_outline(&text, self.layer.as_deref())
            .map_err(|error| miette::miette!("{error}"))
            .wrap_err_with(|| format!("Failed to read an outline from {}", self.file.display()))?;

        let min_x = outline.points.iter().map(|p| p.x.0).min().unwrap_or(0);
        let min_y = outline.points.iter().map(|p| p.y.0).min().unwrap_or(0);
        let max_x = outline.points.iter().map(|p| p.x.0).max().unwrap_or(0);
        let max_y = outline.points.iter().map(|p| p.y.0).max().unwrap_or(0);
        let (shift_x, shift_y) = if self.keep_origin {
            (0, 0)
        } else {
            (min_x, min_y)
        };

        let name = board_name(&self.file);
        let mut design = String::new();
        design.push_str(&format!(
            "// Outline read from {}, DXF layer `{}`.\n",
            self.file.display(),
            outline.layer
        ));
        design.push_str("version 1\n\n");
        design.push_str(&format!("board {name} {{\n"));
        design.push_str(&format!(
            "    size {}mm x {}mm\n",
            mm(cypcb_core::Nm(max_x - shift_x)),
            mm(cypcb_core::Nm(max_y - shift_y))
        ));
        design.push_str("    layers 2\n");
        design.push_str("}\n\noutline {\n");
        for point in &outline.points {
            design.push_str(&format!(
                "    point {}mm, {}mm\n",
                mm(cypcb_core::Nm(point.x.0 - shift_x)),
                mm(cypcb_core::Nm(point.y.0 - shift_y))
            ));
        }
        design.push_str("}\n");

        let output = self
            .output
            .clone()
            .unwrap_or_else(|| self.file.with_extension("cypcb"));
        std::fs::write(&output, &design)
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to write {}", output.display()))?;

        // What was read, and everything that was not: a drawing this took one
        // shape out of usually held several, and a person who cannot see which
        // cannot tell a cutout from a mounting hole.
        eprintln!(
            "Read {} points from layer `{}`, in {}.",
            outline.points.len(),
            outline.layer,
            outline.units
        );
        if outline.loops > 1 {
            eprintln!(
                "The drawing holds {} closed shapes; this is the largest. Name another with --layer.",
                outline.loops
            );
        }
        if !outline.skipped.is_empty() {
            let seen: Vec<String> = outline
                .skipped
                .iter()
                .map(|(kind, count)| format!("{count} {kind}"))
                .collect();
            eprintln!(
                "Passed over {}: this reads lines and polylines, and a curve is not either.",
                seen.join(", ")
            );
        }
        if (shift_x, shift_y) != (0, 0) {
            eprintln!(
                "Moved to the origin by {}mm, {}mm - keep the drawing's own numbers with --keep-origin.",
                mm(cypcb_core::Nm(-shift_x)),
                mm(cypcb_core::Nm(-shift_y))
            );
        }
        eprintln!("Wrote {}", output.display());
        Ok(())
    }
}
