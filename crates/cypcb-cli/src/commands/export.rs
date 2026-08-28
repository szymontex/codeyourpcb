//! Export command implementation.
//!
//! Generates manufacturing files (Gerber, Excellon, BOM, CPL) from a .cypcb file.

use std::path::PathBuf;

use clap::Args;
use miette::{IntoDiagnostic, Result, WrapErr};

use cypcb_export::presets::from_name;
use cypcb_export::{run_export_with, ExportJob};
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

    /// Fillet every join where a track meets a pad.
    ///
    /// A track meeting a pad at a right angle is where the copper tears when
    /// the board is drilled or flexed, and a teardrop is the standard answer -
    /// KiCad has drawn them since 7.0. Off by default, because a board that
    /// has been fabricated before must keep receiving the copper it received
    /// last time until somebody asks for the change.
    ///
    /// The ratios are KiCad's defaults: the fillet runs half a pad's size
    /// along the track and leaves the pad at nine tenths of its width.
    #[arg(long)]
    teardrops: bool,

    /// Also write the IPC-D-356A netlist a bare-board tester reads.
    ///
    /// Before anything is soldered, a fabricator probes the board and checks
    /// that every point which should be connected is, and that no two which
    /// should not be are. The tester needs the design's own answer to compare
    /// against, and this file carries it - one 80-column record per pad and
    /// via, written into `netlist/` beside the Gerbers.
    #[arg(long)]
    ipc356: bool,

    /// Also plot every copper layer as SVG, for a person to look at.
    ///
    /// Gerber is what a fabricator reads; this is the picture for a review, a
    /// document or a web page. One file per copper layer in `plot/`, drawn in
    /// millimetres at size, with the board's outline around it.
    #[arg(long)]
    svg: bool,

    /// Also plot every copper layer as DXF, for a mechanical tool to read.
    ///
    /// An enclosure is drawn in a CAD tool, and the question that tool asks of
    /// a board is where the copper, the holes and the edge are. Same files as
    /// `--svg`, in `plot/`, on layers named as the Gerbers are.
    #[arg(long)]
    dxf: bool,

    /// Also plot every copper layer as PDF, to print or to attach.
    ///
    /// What a person sends in a message and what a house lays on the bench
    /// beside the board. Same files as `--svg`, in `plot/`, one page per layer
    /// at the board's own size.
    #[arg(long)]
    pdf: bool,

    /// Also write the IPC-2581 handoff document.
    ///
    /// One XML file carrying what Gerber cannot: which layer is which, the
    /// board's own outline, and - as the format's feature sections land - the
    /// netlist and the stackup beside the copper rather than in a folder of
    /// files a person has to keep together.
    #[arg(long)]
    ipc2581: bool,
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
            // Counted and named from the one list rather than written out
            // here: this message used to hold four hand-written copies of a
            // list of two, and a third house would have made all four wrong.
            let houses: Vec<&str> = cypcb_export::presets::house_names().collect();
            miette::miette!(
                "'{}' is not a house this command can cut files for. {} are \
                 written down: {}. They say what a fabricator \
                 wants the files called and in what format.\n\n\
                 That is a different list from `cypcb check --preset`, which \
                 takes design rules - what a house can etch - and knows more \
                 names including oshpark. A board can be checked against a \
                 house this command cannot yet write files for, and it is the \
                 board's own `fab` line that decides which rules it is checked \
                 against here.\n\n\
                 Export with {}; the copper \
                 is the same either way, only the file names and the \
                 coordinate format differ.",
                self.house,
                houses.len(),
                houses.join(", "),
                houses
                    .iter()
                    .map(|house| format!("`--house {house}`"))
                    .collect::<Vec<_>>()
                    .join(" or "),
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

            // The prose stays on stderr and the paths go to stdout, because
            // the paths are the answer: `export --dry-run board.cypcb > set.txt`
            // wrote an empty file while the whole listing went to the stream
            // reserved for diagnostics.
            eprintln!("\nFiles that would be generated:");
            eprintln!();

            // Under the directory this run was given, not under the default.
            // The listing said `output/...` whatever `--output` named, so a
            // person reading it before spending money was told the wrong
            // paths by the flag whose whole job is to say what a run writes.
            let root = self.output.display().to_string();

            if preset.layers.top_copper {
                println!(
                    "  {root}/gerber/{}{}",
                    board_name, preset.file_naming.top_copper
                );
            }
            if preset.layers.bottom_copper {
                println!(
                    "  {root}/gerber/{}{}",
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
                println!(
                    "  {root}/gerber/{}{}",
                    board_name,
                    cypcb_export::inner_layer_suffix(preset.file_naming.top_copper, index + 1)
                );
            }

            if preset.layers.top_mask {
                println!(
                    "  {root}/gerber/{}{}",
                    board_name, preset.file_naming.top_mask
                );
            }
            if preset.layers.bottom_mask {
                println!(
                    "  {root}/gerber/{}{}",
                    board_name, preset.file_naming.bottom_mask
                );
            }
            if preset.layers.top_silk {
                println!(
                    "  {root}/gerber/{}{}",
                    board_name, preset.file_naming.top_silk
                );
            }
            if preset.layers.bottom_silk {
                println!(
                    "  {root}/gerber/{}{}",
                    board_name, preset.file_naming.bottom_silk
                );
            }
            if preset.layers.top_paste {
                println!(
                    "  {root}/gerber/{}{}",
                    board_name, preset.file_naming.top_paste
                );
            }
            if preset.layers.bottom_paste {
                println!(
                    "  {root}/gerber/{}{}",
                    board_name, preset.file_naming.bottom_paste
                );
            }
            if preset.layers.outline {
                println!(
                    "  {root}/gerber/{}{}",
                    board_name, preset.file_naming.outline
                );
            }
            if preset.layers.drill {
                println!(
                    "  {root}/drill/{}{}",
                    board_name, preset.file_naming.drill_pth
                );
            }
            if preset.assembly {
                println!("  {root}/assembly/{}{}", board_name, preset.file_naming.bom);
                println!("  {root}/assembly/{}.json", board_name);
                println!("  {root}/assembly/{}{}", board_name, preset.file_naming.cpl);
            }

            // The Gerber job file, which the listing left out: a real run
            // writes one whenever it wrote a Gerber or a drill file, and it
            // sits at the root of the set rather than inside `gerber/` because
            // it describes both. Thirteen names listed against fourteen files
            // written, and the missing one is the file a fabricator's software
            // opens first.
            if preset.layers.top_copper || preset.layers.bottom_copper || preset.layers.drill {
                println!("  {root}/{board_name}-job.gbrjob");
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

            let shorts = cypcb_drc::shorts(&report.violations);

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

        // What the board itself asked for wins: a design that states its
        // ratios is more specific than a flag that states none, and a person
        // running the command should not have to repeat what the file says.
        // The flag turns them on for a board that is silent.
        let teardrops = world
            .teardrops()
            .map(|asked| cypcb_world::teardrop::TeardropRatios {
                length: asked.length,
                width: asked.width,
            })
            .or_else(|| {
                self.teardrops
                    .then(cypcb_world::teardrop::TeardropRatios::default)
            });
        let export_result = run_export_with(&job, &mut world, &library, teardrops)
            .into_diagnostic()
            .wrap_err("Export failed")?;

        // The netlist a bare-board tester reads, when it was asked for. It is
        // written here rather than inside the job because it is not part of
        // the file set a house receives by default: a board that has been
        // fabricated before keeps getting exactly the files it got before.
        if self.ipc356 {
            let (netlist, netlist_warnings) =
                cypcb_export::ipc356::export_ipc356(&mut world, &library, &job.board_name);
            let netlist_dir = job.output_dir.join("netlist");
            std::fs::create_dir_all(&netlist_dir)
                .into_diagnostic()
                .wrap_err("Creating the netlist directory failed")?;
            let netlist_path = netlist_dir.join(format!("{}-IPC-D-356.ipc", job.board_name));
            std::fs::write(&netlist_path, &netlist)
                .into_diagnostic()
                .wrap_err("Writing the IPC-D-356 netlist failed")?;
            // Anything the format could not hold is said here rather than left
            // in a file a tester will read as fact.
            for warning in netlist_warnings {
                eprintln!("Warning: {warning}");
            }
            eprintln!(
                "  [OK] {} ({:.1} KB) - IPC-D-356A netlist",
                netlist_path.display(),
                netlist.len() as f64 / 1024.0
            );
        }

        // The handoff document, when it was asked for. Same rule again: a
        // house receives what it received last month unless a flag says
        // otherwise.
        if self.ipc2581 {
            let handoff_dir = job.output_dir.join("handoff");
            std::fs::create_dir_all(&handoff_dir)
                .into_diagnostic()
                .wrap_err("Creating the handoff directory failed")?;
            // The tolerance is the fab's, so it comes from the fab the board
            // names rather than from the flag that decides file naming.
            let published = world
                .fab()
                .and_then(cypcb_rules::presets::RulesPreset::from_name)
                .map(|preset| preset.constraints());
            let house = cypcb_export::ipc2581::HouseTolerances {
                thickness_percent: published
                    .as_ref()
                    .and_then(|c| c.board_thickness_tolerance_percent),
                thickness_thin: published
                    .as_ref()
                    .and_then(|c| c.board_thickness_tolerance_thin),
                hole_plus: published.as_ref().and_then(|c| c.hole_tolerance_plus),
                hole_minus: published.as_ref().and_then(|c| c.hole_tolerance_minus),
            };
            let (document, handoff_warnings) =
                cypcb_export::ipc2581::export_ipc2581_now(&mut world, &library, house);
            // Before the file is announced, so a person reads what it could
            // not say before they read that it was written.
            for warning in handoff_warnings {
                eprintln!("Warning: {warning}");
            }
            let path = handoff_dir.join(format!("{}.xml", job.board_name));
            std::fs::write(&path, &document)
                .into_diagnostic()
                .wrap_err("Writing the handoff document failed")?;
            eprintln!(
                "  [OK] {} ({:.1} KB) - IPC-2581 handoff",
                path.display(),
                document.len() as f64 / 1024.0
            );
        }

        // The picture, when it was asked for. Same rule as the netlist: the
        // file set a house receives does not change unless somebody says so.
        if self.svg || self.dxf || self.pdf {
            let plot_dir = job.output_dir.join("plot");
            std::fs::create_dir_all(&plot_dir)
                .into_diagnostic()
                .wrap_err("Creating the plot directory failed")?;
            let layer_count = world
                .board_info()
                .map(|(_, stack)| stack.count)
                .unwrap_or(2);
            let mut layers = vec![
                (cypcb_world::Layer::TopCopper, "F_Cu".to_string()),
                (cypcb_world::Layer::BottomCopper, "B_Cu".to_string()),
            ];
            for index in 0..layer_count.saturating_sub(2) {
                layers.push((
                    cypcb_world::Layer::Inner(index),
                    format!("In{}_Cu", index + 1),
                ));
            }
            for (layer, suffix) in layers {
                if self.svg {
                    let drawing = cypcb_export::svg::plot_layer(&mut world, &library, layer);
                    let path = plot_dir.join(format!("{}-{}.svg", job.board_name, suffix));
                    std::fs::write(&path, &drawing)
                        .into_diagnostic()
                        .wrap_err("Writing the plot failed")?;
                    eprintln!(
                        "  [OK] {} ({:.1} KB) - {} plot",
                        path.display(),
                        drawing.len() as f64 / 1024.0,
                        suffix
                    );
                }
                if self.dxf {
                    let drawing = cypcb_export::dxf::plot_layer(&mut world, &library, layer);
                    let path = plot_dir.join(format!("{}-{}.dxf", job.board_name, suffix));
                    std::fs::write(&path, &drawing)
                        .into_diagnostic()
                        .wrap_err("Writing the plot failed")?;
                    eprintln!(
                        "  [OK] {} ({:.1} KB) - {} drawing",
                        path.display(),
                        drawing.len() as f64 / 1024.0,
                        suffix
                    );
                }
                if self.pdf {
                    let page = cypcb_export::pdf::plot_layer(&mut world, &library, layer);
                    let path = plot_dir.join(format!("{}-{}.pdf", job.board_name, suffix));
                    std::fs::write(&path, &page)
                        .into_diagnostic()
                        .wrap_err("Writing the plot failed")?;
                    eprintln!(
                        "  [OK] {} ({:.1} KB) - {} page",
                        path.display(),
                        page.len() as f64 / 1024.0,
                        suffix
                    );
                }
            }
        }

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
            teardrops: false,
            ipc356: false,
            svg: false,
            dxf: false,
            pdf: false,
            ipc2581: false,
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
