//! Export command implementation.
//!
//! Generates manufacturing files (Gerber, Excellon, BOM, CPL) from a .cypcb file.

use std::path::PathBuf;

use clap::Args;
use miette::{IntoDiagnostic, Result, WrapErr};

use cypcb_export::presets::from_name;
use cypcb_export::{run_export, ExportJob};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::sync_ast_to_world;
use cypcb_world::BoardWorld;

/// Export a .cypcb file to manufacturing files.
#[derive(Args)]
pub struct ExportCommand {
    /// Input .cypcb file
    #[arg(value_name = "FILE")]
    input: PathBuf,

    /// Output directory (default: ./output)
    #[arg(short, long, default_value = "output")]
    output: PathBuf,

    /// Which house the files are cut for: what it wants them called, in what
    /// format, with which layers.
    ///
    /// Not design rules, and no longer spelled `--preset` for that reason.
    /// `cypcb check --preset` names what a house can **etch** and knows ten
    /// fabs; this names what a house wants **shipped** and knows two. Which
    /// rules the board is checked against on the way out comes from the board
    /// itself - `board b { fab oshpark }` - and never from this flag.
    ///
    /// No short form: `-h` is help, and `-p` reading as `--preset` on one
    /// subcommand and as this on another is the confusion the rename removes.
    #[arg(long, default_value = "jlcpcb")]
    house: String,

    /// Skip assembly files (BOM, CPL)
    #[arg(long)]
    no_assembly: bool,

    /// Only list files that would be generated
    #[arg(long)]
    dry_run: bool,

    /// Write the files even when the board has copper touching copper.
    ///
    /// A short is not a quality judgement - the board cannot work, and a
    /// fabricator will make it anyway because the files say so. Every other
    /// violation is a warning; this one stops the export until a person says
    /// otherwise.
    #[arg(long)]
    force: bool,
}

impl ExportCommand {
    /// The house's file conventions, or an error that explains the two lists.
    ///
    /// These were both called `--preset` until the flag was renamed, and the
    /// two lists are still different lengths: `cypcb check --preset` takes a
    /// **design-rule** table - what a house can etch - and knows ten. This
    /// takes a **file convention** - what a house wants the Gerbers called, in
    /// what coordinate format, with which layers - and only two have been
    /// written down. A reader who checks a board against `oshpark` and then
    /// cannot export for it deserves to be told why rather than just told no.
    fn resolve_house(&self) -> Result<cypcb_export::presets::ExportPreset> {
        from_name(&self.house).ok_or_else(|| {
            miette::miette!(
                "'{}' is not a house this command can cut files for. Two are \
                 written down: jlcpcb, pcbway. They say what a fabricator \
                 wants the files called and in what format.\n\n\
                 That is a different list from `cypcb check --preset`, which \
                 takes design rules - what a house can etch - and knows more \
                 names including oshpark. A board can be checked against a \
                 house this command cannot yet write files for, and it is the \
                 board's own `fab` line that decides which rules it is checked \
                 against here.\n\n\
                 Export with `--house jlcpcb` or `--house pcbway`; the copper \
                 is the same either way, only the file names and the \
                 coordinate format differ.",
                self.house
            )
        })
    }

    /// Run the export command.
    pub fn run(&self) -> Result<()> {
        // A KiCad board goes to the importer. `export` is the command whose
        // output goes to a fabricator, so a board this project can read is a
        // board this project should be able to send.
        if crate::board_source::is_kicad(&self.input) {
            let loaded = crate::board_source::load_kicad(&self.input)?;
            return self.export_board(loaded.world, loaded.library);
        }

        // Read input file
        let source = std::fs::read_to_string(&self.input)
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to read {}", self.input.display()))?;

        // Before anything else. Until 2026-08-08 this was checked after the
        // file was read, parsed, imported, synced and warned about - so a
        // mistyped preset spent the whole build to say one word.
        self.resolve_house()?;

        eprintln!("Exporting {}...", self.input.display());

        // Parse source
        let result = cypcb_parser::parse(&source);

        if result.has_errors() {
            for err in result.errors {
                eprintln!("{:?}", miette::Report::new(err));
            }
            return Err(miette::miette!("Parse errors in input file"));
        }

        let ast = result.value;

        // Bring in whatever the file imports, resolved against its own
        // directory - the same way `check`, `route` and `score` do it. Export
        // was the one command that skipped this, so a design built from a
        // block library checked clean and then could not be made: every
        // `use Divider ...` came back as `unknown module: 'Divider'`.
        let mut import_errors = Vec::new();
        let ast = cypcb_parser::resolve_imports(&ast, &self.input, &mut import_errors);
        for error in &import_errors {
            eprintln!("Import error: {error}");
        }

        // Build world from AST
        eprintln!("Building board model...");
        let mut world = BoardWorld::new();
        let mut library = FootprintLibrary::new();
        let sync_result = sync_ast_to_world(&ast, &source, &mut world, &mut library);

        if !sync_result.errors.is_empty() {
            for err in &sync_result.errors {
                eprintln!("{:?}", miette::Report::new(err.clone()));
            }
            return Err(miette::miette!("Semantic errors in design"));
        }

        // Warnings are what the board did not say and what was assumed
        // instead. Only `check` printed them until 2026-08-08, so a board whose
        // size was assumed exported at that size in silence - and `export` is
        // the command whose output goes to a fab.
        for warning in &sync_result.warnings {
            eprintln!("{:?}", miette::Report::new(warning.clone()));
        }

        self.export_board(world, library)
    }

    /// Everything that happens once a board exists, whichever file it came
    /// from.
    fn export_board(&self, mut world: BoardWorld, library: FootprintLibrary) -> Result<()> {
        let mut preset = self.resolve_house()?;

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
            // The preset's name is a fabricator's profile, not a count of the
            // board's copper. "JLCPCB 2-Layer" over a four-layer board reads
            // like a contradiction unless the stack is stated beside it.
            if let Some((_, stack)) = world.board_info() {
                if stack.count > 2 {
                    eprintln!(
                        "Board stack: {} copper layers ({} inner)",
                        stack.count,
                        stack.count - 2
                    );
                }
            }

            eprintln!("\nFiles that would be generated:");
            eprintln!();

            if preset.layers.top_copper {
                eprintln!(
                    "  output/gerber/{}{}",
                    board_name, preset.file_naming.top_copper
                );
            }
            if preset.layers.bottom_copper {
                eprintln!(
                    "  output/gerber/{}{}",
                    board_name, preset.file_naming.bottom_copper
                );
            }
            // Inner copper comes from the board, not the preset - the same
            // rule the export itself follows. Listing the preset alone
            // promised a two-layer set for a four-layer board, which is the
            // sentence a person reads before spending money.
            let inner_count = world
                .board_info()
                .map(|(_, stack)| stack.count.saturating_sub(2))
                .unwrap_or(0)
                .max(preset.layers.inner_copper.len() as u8);
            for index in 0..inner_count {
                eprintln!(
                    "  output/gerber/{}{}",
                    board_name,
                    cypcb_export::inner_layer_suffix(preset.file_naming.top_copper, index + 1)
                );
            }

            if preset.layers.top_mask {
                eprintln!(
                    "  output/gerber/{}{}",
                    board_name, preset.file_naming.top_mask
                );
            }
            if preset.layers.bottom_mask {
                eprintln!(
                    "  output/gerber/{}{}",
                    board_name, preset.file_naming.bottom_mask
                );
            }
            if preset.layers.top_silk {
                eprintln!(
                    "  output/gerber/{}{}",
                    board_name, preset.file_naming.top_silk
                );
            }
            if preset.layers.bottom_silk {
                eprintln!(
                    "  output/gerber/{}{}",
                    board_name, preset.file_naming.bottom_silk
                );
            }
            if preset.layers.top_paste {
                eprintln!(
                    "  output/gerber/{}{}",
                    board_name, preset.file_naming.top_paste
                );
            }
            if preset.layers.bottom_paste {
                eprintln!(
                    "  output/gerber/{}{}",
                    board_name, preset.file_naming.bottom_paste
                );
            }
            if preset.layers.outline {
                eprintln!(
                    "  output/gerber/{}{}",
                    board_name, preset.file_naming.outline
                );
            }
            if preset.layers.drill {
                eprintln!(
                    "  output/drill/{}{}",
                    board_name, preset.file_naming.drill_pth
                );
            }
            if preset.assembly {
                eprintln!("  output/assembly/{}{}", board_name, preset.file_naming.bom);
                eprintln!("  output/assembly/{}.json", board_name);
                eprintln!("  output/assembly/{}{}", board_name, preset.file_naming.cpl);
            }

            eprintln!();
            return Ok(());
        }

        // What the checker says about the board before anyone makes it.
        //
        // `check` and `export` were two commands with nothing joining them, so
        // a shorted board exported without a word. Most violations stay a
        // warning: whether to make a board with a gap 0.01mm under spec is the
        // designer's call, and a fab will make whatever the files say. Copper
        // touching copper is not that call - the board cannot work - so it
        // stops here until `--force` says otherwise.
        {
            use cypcb_drc::{run_drc, PresetRules};
            // The board decides, exactly as it does for `cypcb check`, so the
            // two commands measure the same board against the same table.
            //
            // This used to read `--preset`, which on this command names a
            // **file convention** and not a design rule set. The two lists
            // overlap on `jlcpcb` and `pcbway` and nowhere else, so a board
            // written for OSHPark was checked against JLCPCB on the way out
            // however it was exported - `--preset oshpark` is refused by
            // `resolve_preset` long before this line - and `--preset pcbway`
            // silently changed which rules a board was measured against
            // without anybody asking for that.
            let rules = crate::preset_choice::resolve(None, &world)?.rules();
            world.rebuild_spatial_index_from_library(&library);
            let report = run_drc(&mut world, &rules);

            let shorts = report
                .violations
                .iter()
                .filter(|violation| violation.actual == Some(cypcb_core::Nm::ZERO))
                .count();

            if !report.violations.is_empty() {
                eprintln!();
                eprintln!(
                    "Warning: exporting a board with {} DRC violation(s). Run `cypcb check {}` to read them.",
                    report.violations.len(),
                    self.input.display()
                );
            }

            if shorts > 0 {
                if self.force {
                    eprintln!(
                        "Forcing: {} of them are copper touching copper, and the files are being written anyway.",
                        shorts
                    );
                } else {
                    return Err(miette::miette!(
                        "{} of the violations are copper touching copper. \
                         The board cannot work as drawn - fix them, or pass --force to write the files anyway.",
                        shorts
                    ));
                }
            }
        }

        // A file with no board is a library, not a design. Saying so beats
        // `NoBoardSize`, which is the error the exporter raises three layers
        // down and which reads like a missing setting rather than a file that
        // was never meant to be made.
        if world.board_info().is_none() {
            return Err(miette::miette!(
                "{} declares no board, so there is nothing to make from it. \
                 A file of modules, footprints or interfaces is a library - \
                 import it from a design that has a `board` block.",
                self.input.display()
            ));
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

        // What was already in the output directory and is not ours.
        //
        // Exporting a second board into a directory that still holds the first
        // leaves both, and the whole directory is what gets zipped and sent.
        // Measured: `four-layer` then `blink` into one directory gives 20
        // Gerbers for two different boards, including In1 and In2 copper that
        // belongs to neither the 2-layer board nor anything the fabricator was
        // asked for. Overwriting the same board's own files is ordinary and
        // stays silent; copper from a different board does not.
        report_foreign_files(&export_result, &self.output);

        let total_size: u64 = export_result.files.iter().map(|f| f.size_bytes).sum();
        let total_size_kb = total_size as f64 / 1024.0;

        eprintln!();
        eprintln!(
            "Export complete: {} files, {:.1} KB total ({} ms)",
            export_result.files.len(),
            total_size_kb,
            export_result.duration_ms
        );

        // What the export passed over. These are not errors - the files are
        // written and a fabricator will make what they say - which is exactly
        // why they have to be said out loud.
        if !export_result.warnings.is_empty() {
            eprintln!();
            for warning in &export_result.warnings {
                eprintln!("Warning: {warning}");
            }
        }

        Ok(())
    }
}

/// Warn about fabrication files in the output directory that this export did
/// not write.
///
/// The output directory is the unit somebody zips and sends, so a file left
/// there by an earlier export of a different board travels with this one. This
/// only looks at the directories the job wrote into, and only at the
/// extensions a fabricator reads.
fn report_foreign_files(result: &cypcb_export::ExportResult, output: &std::path::Path) {
    use std::collections::BTreeSet;

    let ours: BTreeSet<std::path::PathBuf> = result.files.iter().map(|f| f.path.clone()).collect();
    let dirs: BTreeSet<std::path::PathBuf> = result
        .files
        .iter()
        .filter_map(|f| f.path.parent().map(|p| p.to_path_buf()))
        .collect();

    let mut strangers: Vec<String> = Vec::new();
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if ours.contains(&path) || !path.is_file() {
                continue;
            }
            let is_fab_file = path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| matches!(e, "gbr" | "drl" | "xln" | "csv" | "json" | "gm1"));
            if is_fab_file {
                strangers.push(
                    path.strip_prefix(output)
                        .unwrap_or(&path)
                        .display()
                        .to_string(),
                );
            }
        }
    }

    if strangers.is_empty() {
        return;
    }
    strangers.sort();

    eprintln!();
    eprintln!(
        "Warning: {} file(s) in {} were not written by this export and will \
         travel with it:",
        strangers.len(),
        output.display()
    );
    for name in strangers.iter().take(8) {
        eprintln!("  {name}");
    }
    if strangers.len() > 8 {
        eprintln!("  ... and {} more", strangers.len() - 8);
    }
    eprintln!("Delete them, or export into a directory of its own.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_export_command_construction() {
        let cmd = ExportCommand {
            input: PathBuf::from("test.cypcb"),
            output: PathBuf::from("output"),
            house: "jlcpcb".to_string(),
            no_assembly: false,
            dry_run: false,
            force: false,
        };

        assert_eq!(cmd.house, "jlcpcb");
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
