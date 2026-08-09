//! Writing a design out as a KiCad board.

use std::path::PathBuf;

use clap::Args;
use miette::{IntoDiagnostic, Result, WrapErr};

use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::sync_ast_to_world;
use cypcb_world::BoardWorld;

/// Write a `.cypcb` design out as a `.kicad_pcb`.
///
/// The mirror of `parse-kicad`. A design written in this language could be
/// checked, routed and turned into fabrication files, and could not be opened
/// by anybody who does not use this tool - which is most people who make
/// boards.
#[derive(Args)]
pub struct ToKicadCommand {
    /// Input .cypcb file
    file: PathBuf,

    /// Where to write the board (default: the input file with a .kicad_pcb suffix)
    #[arg(short, long)]
    output: Option<PathBuf>,
}

impl ToKicadCommand {
    pub fn run(self) -> Result<()> {
        let source = std::fs::read_to_string(&self.file)
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to read {}", self.file.display()))?;

        let parsed = cypcb_parser::parse(&source);
        if let Some(first) = parsed.errors.first() {
            return Err(miette::miette!("{first}"))
                .wrap_err_with(|| format!("{} does not parse", self.file.display()));
        }

        // Same resolution every other command does: a design may be split
        // across files, and what it imports is part of the board.
        let mut import_errors = Vec::new();
        let ast = cypcb_parser::resolve_imports(&parsed.value, &self.file, &mut import_errors);
        for error in &import_errors {
            eprintln!("Import error: {error}");
        }

        let mut world = BoardWorld::new();
        let mut library = FootprintLibrary::new();
        let sync = sync_ast_to_world(&ast, &source, &mut world, &mut library);
        if !sync.errors.is_empty() {
            for error in &sync.errors {
                eprintln!("{:?}", miette::Report::new(error.clone()));
            }
            std::process::exit(1);
        }

        let board = cypcb_kicad::write_board(&mut world, "cypcb");

        let output = self
            .output
            .unwrap_or_else(|| self.file.with_extension("kicad_pcb"));
        std::fs::write(&output, &board)
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to write {}", output.display()))?;

        println!("Wrote {} ({} bytes)", output.display(), board.len());
        Ok(())
    }
}
