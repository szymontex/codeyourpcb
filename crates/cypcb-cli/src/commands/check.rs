//! Check command implementation.

use clap::Args;
use miette::{IntoDiagnostic, Result, WrapErr};
use std::collections::BTreeMap;
use std::path::PathBuf;

use cypcb_drc::{run_drc, Preset, PresetRules};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::sync_ast_to_world;
use cypcb_world::BoardWorld;

/// Check a .cypcb file for errors.
#[derive(Args)]
pub struct CheckCommand {
    /// Input .cypcb file
    #[arg(value_name = "FILE")]
    pub file: PathBuf,

    /// Manufacturer preset for design rules.
    ///
    /// Absent means the board decides: `board b { fab oshpark }`. Absent from
    /// both is JLCPCB, which is what it has always been.
    #[arg(short, long)]
    pub preset: Option<String>,

    /// Check syntax and semantics only, skip design rule check
    #[arg(long)]
    pub no_drc: bool,
}

/// The 1-based line a byte offset falls on.
///
/// The span a component carries is a byte range - the line it sits on is not
/// known where that span is built, so it is worked out here against the source
/// that produced it.
fn line_of(source: &str, offset: usize) -> usize {
    source[..offset.min(source.len())].matches('\n').count() + 1
}

impl CheckCommand {
    /// Run the check command.
    pub fn run(&self) -> Result<()> {
        // A KiCad board goes to the importer, and everything below this point
        // is the same for both. Without this the file went to the DSL parser
        // and came back with `Missing a definition` pointing at `(kicad_pcb`,
        // which told the reader nothing about what was wrong.
        if crate::board_source::is_kicad(&self.file) {
            let loaded = crate::board_source::load_kicad(&self.file)?;
            return self.check_board(loaded.world, &loaded.source);
        }

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
            eprintln!("{:?}", miette::Report::new(warning.clone()));
        }

        self.check_board(world, &source)
    }

    /// Everything that happens once a board exists, whichever file it came
    /// from.
    fn check_board(&self, mut world: BoardWorld, source: &str) -> Result<()> {
        // A file with no board is not a board that passed.
        //
        // `examples/v2-interfaces.cypcb` declares four interfaces and nothing
        // else, and this command answered `OK: passed DRC against
        // jlcpcb_2layer in 0ms`. Every rule skips quietly when there is no
        // board size - `EdgeClearanceRule` says so in its own doc - so the
        // checker ran, checked nothing, and reported a pass. A design whose
        // board block failed to parse, or whose import did not resolve, got
        // the same green line as a board that was actually checked.
        if world.board_entity().is_none() {
            let parts = {
                let ecs = world.ecs_mut();
                let mut query = ecs.query::<&cypcb_world::components::RefDes>();
                query.iter(ecs).count()
            };
            if parts > 0 {
                return Err(miette::miette!(
                    "{} places {parts} component(s) and declares no board. \
                     Nothing can be checked against a board that is not there.",
                    self.file.display()
                ));
            }
            println!(
                "{} declares no board and places no components: nothing was checked.",
                self.file.display()
            );
            return Ok(());
        }

        if self.no_drc {
            println!(
                "OK: {} parsed and validated (DRC skipped)",
                self.file.display()
            );
            return Ok(());
        }

        // Design rule check
        let preset = crate::preset_choice::resolve(self.preset.as_deref(), &world)?;

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

            // Where in the file, when the model can say. A violation is found
            // in board coordinates, and a coordinate is not something a reader
            // can search a text file for - the definition it belongs to is.
            // `path:line:` is also what an editor and a terminal both know how
            // to jump to.
            let where_written = world
                .get::<cypcb_world::components::SourceSpan>(violation.entity)
                .map(|span| line_of(source, span.start_byte))
                .map(|line| format!("{}:{}: ", self.file.display(), line))
                .unwrap_or_default();

            eprintln!(
                "  {}{} at ({:.3}mm, {:.3}mm): {}",
                where_written,
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
        //
        // Selected by what the number measures, not only by its value. The
        // filter used to be "actual is zero", which caught the first rule that
        // reported a different zero-width thing: a paste stencil web of 0.000mm
        // is a torn stencil, not copper touching copper, and counting it here
        // made the line say something untrue about the board.
        let shorts = drc
            .violations
            .iter()
            .filter(|violation| {
                matches!(violation.kind, cypcb_drc::ViolationKind::Clearance)
                    && violation.actual == Some(cypcb_core::Nm::ZERO)
            })
            .count();
        if shorts > 0 {
            eprintln!("  copper touching copper at 0.00mm: {}", shorts);
        }

        report_rules_the_fab_never_stated(&preset, &drc.violations, &preset.rules());

        std::process::exit(1);
    }
}

/// Say which of the reported rules the fab never stated.
///
/// Three assembly-side rules have no counterpart in a fab's routing table, so
/// when the preset does not state one the checker derives it: a via pad is the
/// drill plus two annular rings, silk clearance follows the silk width, and
/// courtyard clearance takes a conservative IPC-style value. Every preset but
/// `prototype` leaves all three unstated today.
///
/// A number this tool chose and a number the fab published read exactly the
/// same in a violation - `0.25mm required` - and a person deciding whether to
/// change their board deserves to know which they are looking at. Printed only
/// for the rules that actually reported something, because a note on every run
/// about rules nothing broke is noise nobody reads.
fn report_rules_the_fab_never_stated(
    preset: &Preset,
    violations: &[cypcb_drc::DrcViolation],
    rules: &cypcb_drc::DesignRules,
) {
    use cypcb_drc::ViolationKind;

    let constraints = preset.constraints();
    let derived: [(ViolationKind, &str, bool, cypcb_core::Nm); 4] = [
        (
            ViolationKind::ViaDiameter,
            "via diameter",
            constraints.min_via_diameter.is_none(),
            rules.min_via_diameter,
        ),
        (
            ViolationKind::SilkClearance,
            "silkscreen clearance",
            constraints.min_silk_clearance.is_none(),
            rules.min_silk_clearance,
        ),
        (
            ViolationKind::CourtyardClearance,
            "courtyard clearance",
            constraints.min_courtyard_clearance.is_none(),
            rules.min_courtyard_clearance,
        ),
        (
            ViolationKind::PadLand,
            "minimum pad size",
            constraints.min_pad_size.is_none(),
            rules.min_pad_size,
        ),
    ];

    let mut said_anything = false;
    for (kind, what, is_derived, value) in derived {
        if !is_derived || !violations.iter().any(|v| v.kind == kind) {
            continue;
        }
        if !said_anything {
            eprintln!("Notes:");
            said_anything = true;
        }
        eprintln!(
            "  {} does not state a {what}. The {:.3}mm above is this tool's own value, not the fab's.",
            preset.name(),
            value.to_mm()
        );
    }
}
