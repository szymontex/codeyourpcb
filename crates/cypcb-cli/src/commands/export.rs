//! Export command implementation.
//!
//! Generates manufacturing files (Gerber, Excellon, BOM, CPL) from a .cypcb file.

use std::path::PathBuf;

use clap::Args;
use miette::{IntoDiagnostic, Result, WrapErr};

use cypcb_parser::CypcbParser;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::sync_ast_to_world;
use cypcb_world::BoardWorld;
use cypcb_export::presets::from_name;
use cypcb_export::{ExportJob, run_export};

/// Export a .cypcb file to manufacturing files.
#[derive(Args)]
pub struct ExportCommand {
    /// Input .cypcb file
    #[arg(value_name = "FILE")]
    input: PathBuf,

    /// Output directory (default: ./output)
    #[arg(short, long, default_value = "output")]
    output: PathBuf,

    /// Manufacturer preset (jlcpcb, pcbway)
    #[arg(short, long, default_value = "jlcpcb")]
    preset: String,

    /// Skip assembly files (BOM, CPL)
    #[arg(long)]
    no_assembly: bool,

    /// Only list files that would be generated
    #[arg(long)]
    dry_run: bool,
}

impl ExportCommand {
    /// Run the export command.
    pub fn run(&self) -> Result<()> {
        // Read input file
        let source = std::fs::read_to_string(&self.input)
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to read {}", self.input.display()))?;

        eprintln!("Exporting {}...", self.input.display());

        // Parse source
        let mut parser = CypcbParser::new();
        let result = parser.parse(&source);

        if result.has_errors() {
            for err in result.errors {
                eprintln!("{:?}", miette::Report::new(err));
            }
            return Err(miette::miette!("Parse errors in input file"));
        }

        let ast = result.value;

        // Build world from AST
        eprintln!("Building board model...");
        let mut world = BoardWorld::new();
        let library = FootprintLibrary::new();
        let sync_result = sync_ast_to_world(&ast, &source, &mut world, &library);

        if !sync_result.errors.is_empty() {
            for err in &sync_result.errors {
                eprintln!("Semantic error: {}", err);
            }
            return Err(miette::miette!("Semantic errors in design"));
        }

        // Look up preset
        let mut preset = from_name(&self.preset)
            .ok_or_else(|| {
                miette::miette!(
                    "Unknown preset '{}'. Available presets: jlcpcb, pcbway",
                    self.preset
                )
            })?;

        if self.no_assembly {
            preset.assembly = false;
        }

        eprintln!("Preset: {}", preset.name);

        // Determine board name from input file
        let board_name = self
            .input
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("board")
            .to_string();

        // Create export job
        let job = ExportJob {
            source_path: self.input.clone(),
            output_dir: self.output.clone(),
            preset: preset.clone(),
            board_name: board_name.clone(),
        };

        // Dry run: list files that would be generated
        if self.dry_run {
            eprintln!("\nFiles that would be generated:");
            eprintln!();

            if preset.layers.top_copper {
                eprintln!("  output/gerber/{}{}", board_name, preset.file_naming.top_copper);
            }
            if preset.layers.bottom_copper {
                eprintln!("  output/gerber/{}{}", board_name, preset.file_naming.bottom_copper);
            }
            if preset.layers.top_mask {
                eprintln!("  output/gerber/{}{}", board_name, preset.file_naming.top_mask);
            }
            if preset.layers.bottom_mask {
                eprintln!("  output/gerber/{}{}", board_name, preset.file_naming.bottom_mask);
            }
            if preset.layers.top_silk {
                eprintln!("  output/gerber/{}{}", board_name, preset.file_naming.top_silk);
            }
            if preset.layers.bottom_silk {
                eprintln!("  output/gerber/{}{}", board_name, preset.file_naming.bottom_silk);
            }
            if preset.layers.top_paste {
                eprintln!("  output/gerber/{}{}", board_name, preset.file_naming.top_paste);
            }
            if preset.layers.bottom_paste {
                eprintln!("  output/gerber/{}{}", board_name, preset.file_naming.bottom_paste);
            }
            if preset.layers.outline {
                eprintln!("  output/gerber/{}{}", board_name, preset.file_naming.outline);
            }
            if preset.layers.drill {
                eprintln!("  output/drill/{}{}", board_name, preset.file_naming.drill_pth);
            }
            if preset.assembly {
                eprintln!("  output/assembly/{}{}", board_name, preset.file_naming.bom);
                eprintln!("  output/assembly/{}.json", board_name);
                eprintln!("  output/assembly/{}{}", board_name, preset.file_naming.cpl);
            }

            eprintln!();
            return Ok(());
        }

        // Run export
        eprintln!();
        eprintln!("Generating Gerber files...");

        let export_result = run_export(&job, &mut world, &library)
            .into_diagnostic()
            .wrap_err("Export failed")?;

        // Print results
        eprintln!();
        for file in &export_result.files {
            let size_kb = file.size_bytes as f64 / 1024.0;
            eprintln!(
                "  [OK] {} ({:.1} KB) - {}",
                file.path.display(),
                size_kb,
                file.file_type
            );
        }

        let total_size: u64 = export_result.files.iter().map(|f| f.size_bytes).sum();
        let total_size_kb = total_size as f64 / 1024.0;

        eprintln!();
        eprintln!(
            "Export complete: {} files, {:.1} KB total ({} ms)",
            export_result.files.len(),
            total_size_kb,
            export_result.duration_ms
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_export_command_construction() {
        let cmd = ExportCommand {
            input: PathBuf::from("test.cypcb"),
            output: PathBuf::from("output"),
            preset: "jlcpcb".to_string(),
            no_assembly: false,
            dry_run: false,
        };

        assert_eq!(cmd.preset, "jlcpcb");
        assert!(!cmd.no_assembly);
    }

    #[test]
    fn test_preset_lookup() {
        let preset = from_name("jlcpcb").unwrap();
        assert_eq!(preset.name, "JLCPCB 2-Layer");
    }

    #[test]
    fn test_unknown_preset_error() {
        let result = from_name("unknown");
        assert!(result.is_none());
    }
}
