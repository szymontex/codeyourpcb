//! CLI command for parsing KiCad .kicad_pcb files.

use std::path::PathBuf;

use clap::Args;
use miette::{Context, IntoDiagnostic, Result};

/// Parse a KiCad .kicad_pcb file and output metadata as JSON.
#[derive(Args)]
pub struct ParseKicadCommand {
    /// Path to the .kicad_pcb file to parse.
    file: PathBuf,
}

impl ParseKicadCommand {
    pub fn run(self) -> Result<()> {
        let result = cypcb_kicad::parse_kicad_pcb(&self.file)
            .map_err(|e| miette::miette!("{e}"))
            .wrap_err_with(|| {
                format!("Failed to parse KiCad PCB file: {}", self.file.display())
            })?;

        let json = serde_json::to_string_pretty(&result.metadata)
            .into_diagnostic()
            .wrap_err("Failed to serialize metadata to JSON")?;

        println!("{json}");
        Ok(())
    }
}
