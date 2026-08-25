//! Writing a design out as a KiCad board.

use std::path::PathBuf;

use clap::Args;
use miette::{IntoDiagnostic, Result, WrapErr};

use cypcb_drc::{Preset, PresetRules};
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

    /// Manufacturer preset whose design rules the board is written with.
    ///
    /// Without one the file states no rules at all and KiCad checks the board
    /// against its own defaults - numbers with nothing to do with the fab this
    /// design was checked for. This was the last command in the CLI that never
    /// asked which fabricator a board is for.
    #[arg(short, long)]
    preset: Option<String>,
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

        // The rules the board is written with are the ones a person can ask
        // `cypcb check` for by the same name, so the two tools agree about
        // whether the board passes.
        let rules = match &self.preset {
            Some(name) => {
                let preset = Preset::from_name(name).ok_or_else(|| {
                    let available: Vec<&str> = Preset::all().iter().map(|p| p.name()).collect();
                    miette::miette!(
                        "Unknown preset '{}'. Available presets: {}",
                        name,
                        available.join(", ")
                    )
                })?;
                let rules = preset.rules();
                Some(cypcb_kicad::KicadDesignRules {
                    clearance: rules.min_clearance,
                    track_width: rules.min_trace_width,
                    via_diameter: rules.min_via_diameter,
                    via_drill: rules.min_via_drill,
                    mask_expansion: rules.solder_mask_expansion,
                    drill_size: rules.min_drill_size,
                    hole_to_hole: rules.min_hole_to_hole,
                    edge_clearance: rules.min_edge_clearance,
                    silk_clearance: rules.min_silk_clearance,
                    annular_ring: rules.min_annular_ring,
                })
            }
            None => None,
        };

        let board = cypcb_kicad::write_board_with_rules(&mut world, "cypcb", rules);

        let output = self
            .output
            .unwrap_or_else(|| self.file.with_extension("kicad_pcb"));
        std::fs::write(&output, &board)
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to write {}", output.display()))?;

        println!("Wrote {} ({} bytes)", output.display(), board.len());

        // What a `.kicad_pcb` has no place for.
        //
        // A design states which spans its fabricator drills - `drill Top to
        // Bottom`, `drill Top to Inner1` - and `ViaSpanRule` holds the board's
        // vias to that list. KiCad keeps no such list in the board file: a via
        // there carries its own two layers and the question of which spans the
        // build makes lives in the project's design rules, not in the
        // `(setup ...)` this writer fills. So the statement is dropped, and a
        // board read back from this file states none, which is the same as
        // saying every span is allowed. Said out loud rather than left for a
        // reader to find by checking the same board twice and getting two
        // answers.
        if let Some(pairs) = world
            .stackup()
            .map(|stackup| stackup.drill_pairs.clone())
            .filter(|pairs| !pairs.is_empty())
        {
            let named: Vec<String> = pairs
                .iter()
                .map(|pair| format!("{} to {}", pair.start, pair.end))
                .collect();
            eprintln!(
                "Warning: the drill spans this design states ({}) are not in the KiCad board: \
                 the format has no place for them, so a board read back from this file allows \
                 every span.",
                named.join(", ")
            );
        }

        // The part of the board that bends.
        //
        // `flex bend { ... }` is V8's rigid-flex vocabulary: the region a
        // rigid-flex build folds, which the stackup's coverlay and stiffener
        // are about. KiCad has no area for it - a zone there is copper or a
        // rule area, and this is neither - so it is dropped rather than
        // written as a pour with no net, which is what it used to be: one
        // netless zone per copper layer, 32 for a region stated on `all`.
        {
            let flex: Vec<String> = world
                .zones()
                .into_iter()
                .filter(|(_, zone)| {
                    matches!(zone.kind, cypcb_world::components::zone::ZoneKind::Flex)
                })
                .map(|(_, zone)| zone.name.clone().unwrap_or_else(|| "unnamed".to_string()))
                .collect();
            if !flex.is_empty() {
                eprintln!(
                    "Warning: the flexible region(s) this design states ({}) are not in the \
                     KiCad board: the format has no area for the part of a board that bends, \
                     so a board read back from this file is rigid throughout.",
                    flex.join(", ")
                );
            }
        }

        // The fab the board names.
        //
        // `board b { fab oshpark }` decides which table `cypcb check` grades
        // the board against, and a `.kicad_pcb` has no field for it - KiCad
        // keeps its constraints as numbers in the project file rather than as
        // a fabricator's name. `--preset` writes those numbers beside the
        // board; the name itself does not survive, so a design read back from
        // here is graded against the default table.
        if let Some(fab) = world.fab() {
            eprintln!(
                "Warning: the fabricator this design names ({fab}) is not in the KiCad board: \
                 a board read back from this file is checked against the default table."
            );
        }

        // What a net asks for.
        //
        // `net SIG [width 0.2mm clearance 0.25mm current 500mA impedance
        // 50ohm]` is four constraints, and three rules read them:
        // `MinTraceWidthRule`, `TraceCurrentRule` and `ImpedanceRule`. The
        // board file carries the net's *membership* and nothing about what it
        // asks for, so those three stop checking on a board read back from
        // here - silently, because a net with no constraints is a net nobody
        // constrained.
        //
        // The four do not share a fate, and the warning says which is which.
        // Read out of three `.kicad_pro` files KiCad itself wrote, in
        // `viewer/faebryk` and `viewer/kicad-tools`: a net class there carries
        // `clearance`, `track_width`, `via_diameter`, `via_drill` and the
        // diff-pair figures, and **no current and no impedance**. So width and
        // clearance could travel in the project file this command already
        // writes; the other two have nowhere in either file to go.
        //
        // What could not be measured is how a net is *assigned* to a class:
        // all three of those files carry `netclass_patterns: []` and
        // `netclass_assignments: null`, so the shape of an entry is not
        // visible in any of them. Writing one from memory is the kind of guess
        // this project keeps finding in its own history, so the classes are
        // not written until a file that uses them can be read.
        {
            let constrained: Vec<String> = world
                .nets()
                .filter_map(|(id, name)| {
                    let asks = world
                        .net_constraints(id)
                        .is_some_and(|c| c != Default::default());
                    asks.then(|| name.to_string())
                })
                .collect();
            if !constrained.is_empty() {
                eprintln!(
                    "Warning: what {} net(s) ask for ({}) is not in the KiCad board: it carries \
                     a net's membership and nothing else. Width and clearance have a home in the \
                     project file's net classes; current and impedance have none in either file, \
                     so the trace-current and impedance rules stop checking those nets.",
                    constrained.len(),
                    constrained.join(", ")
                );
            }
        }

        // The rules go in the project file beside the board, because that is
        // where KiCad reads them from. A board file stating them is a board
        // file KiCad refuses to open.
        if let Some(rules) = rules {
            let project = output.with_extension("kicad_pro");
            let stem = project
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("board");
            let text = cypcb_kicad::write_project(rules, stem);
            std::fs::write(&project, &text)
                .into_diagnostic()
                .wrap_err_with(|| format!("Failed to write {}", project.display()))?;

            println!(
                "Wrote {} ({} bytes) - open the board through this file, or KiCad checks it \
                 against its own defaults instead of {}'s",
                project.display(),
                text.len(),
                self.preset.as_deref().unwrap_or("the fab")
            );
        }

        Ok(())
    }
}
