//! Check command implementation.

use clap::Args;
use miette::{IntoDiagnostic, Result, WrapErr};
use std::collections::BTreeMap;
use std::path::PathBuf;

use cypcb_drc::{run_drc, Preset};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::sync_ast_to_world;
use cypcb_world::BoardWorld;

/// Check a .cypcb file for errors.
#[derive(Args)]
pub struct CheckCommand {
    /// Input .cypcb file
    #[arg(value_name = "FILE")]
    pub file: PathBuf,

    /// Manufacturer preset for design rules
    #[arg(short, long, default_value = "jlcpcb")]
    pub preset: String,

    /// Check syntax and semantics only, skip design rule check
    #[arg(long)]
    pub no_drc: bool,
}

impl CheckCommand {
    /// Run the check command.
    pub fn run(&self) -> Result<()> {
        let source = std::fs::read_to_string(&self.file)
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to read {}", self.file.display()))?;

        let result = cypcb_parser::parse(&source);

        // Report parse errors
        if result.has_errors() {
            for err in result.errors {
                eprintln!("{:?}", miette::Report::new(err));
            }
            std::process::exit(1);
        }

        let ast = result.value;

        // Bring in whatever the file imports, resolved against its own
        // directory. Errors are collected rather than fatal so the rest of the
        // design is still checked.
        let mut import_errors = Vec::new();
        let ast = cypcb_parser::resolve_imports(&ast, &self.file, &mut import_errors);
        for error in &import_errors {
            eprintln!("Import error: {error}");
        }

        // Semantic validation: build the board model from the AST.
        let mut world = BoardWorld::new();
        let mut library = FootprintLibrary::new();
        let sync_result = sync_ast_to_world(&ast, &source, &mut world, &mut library);

        if !sync_result.errors.is_empty() {
            for err in &sync_result.errors {
                eprintln!("{:?}", miette::Report::new(err.clone()));
            }
            std::process::exit(1);
        }

        for warning in &sync_result.warnings {
            eprintln!("Warning: {}", warning);
        }

        if self.no_drc {
            println!(
                "OK: {} parsed and validated (DRC skipped)",
                self.file.display()
            );
            return Ok(());
        }

        // Design rule check
        let preset = Preset::from_name(&self.preset).ok_or_else(|| {
            let available: Vec<&str> = Preset::all().iter().map(|p| p.name()).collect();
            miette::miette!(
                "Unknown preset '{}'. Available presets: {}",
                self.preset,
                available.join(", ")
            )
        })?;

        let drc = run_drc(&mut world, &preset.rules());

        if drc.violations.is_empty() {
            println!(
                "OK: {} passed DRC against {} in {}ms",
                self.file.display(),
                preset.name(),
                drc.duration_ms
            );
            return Ok(());
        }

        eprintln!(
            "{} DRC violation(s) against {}:",
            drc.violations.len(),
            preset.name()
        );

        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        for violation in &drc.violations {
            *counts.entry(violation.kind.to_string()).or_insert(0) += 1;
            eprintln!(
                "  {} at ({:.3}mm, {:.3}mm): {}",
                violation.kind,
                violation.location.x.to_mm(),
                violation.location.y.to_mm(),
                violation.message
            );

            // Some faults are a place and some are a piece of copper. A pour
            // island reported as a coordinate points at the middle of a plane,
            // which looks like every other part of the plane - the size and
            // the corners are what a person can act on.
            if let Some(area) = violation.area {
                eprintln!(
                    "      copper {:.3}mm x {:.3}mm, from ({:.3}mm, {:.3}mm) to ({:.3}mm, {:.3}mm)",
                    (area.max.x - area.min.x).to_mm(),
                    (area.max.y - area.min.y).to_mm(),
                    area.min.x.to_mm(),
                    area.min.y.to_mm(),
                    area.max.x.to_mm(),
                    area.max.y.to_mm(),
                );
            }
        }

        eprintln!("Summary:");
        for (kind, count) in &counts {
            eprintln!("  {}: {}", kind, count);
        }

        // A count on its own reads the same whether the board shorts or runs
        // 0.01mm under spec. The first cannot work; the second is a yield risk
        // a fab may still build, and a person deciding whether to send the
        // files needs to know which they have.
        let shorts = drc
            .violations
            .iter()
            .filter(|violation| violation.actual == Some(cypcb_core::Nm::ZERO))
            .count();
        if shorts > 0 {
            eprintln!("  copper touching copper at 0.00mm: {}", shorts);
        }

        std::process::exit(1);
    }
}
