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

    /// What to print: a report for a person, or JSON for a program.
    ///
    /// The verdict has been an exit code and a page of prose since the first
    /// release, so a CI job could fail a build on "some rule broke" and never
    /// on *which* rule. `-o json` prints the same violations the browser
    /// receives - kind, place, what was measured and what was required - on
    /// stdout, with the prose left on stderr.
    #[arg(short, long, default_value = "text")]
    pub output: ReportFormat,
}

/// What `check` prints.
#[derive(Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ReportFormat {
    /// A report for a person, on stderr, with the verdict in the exit code.
    Text,
    /// The violations as JSON, on stdout.
    Json,
}

impl CheckCommand {
    /// One JSON document on stdout, and nothing else.
    ///
    /// Every other line this command writes goes to stderr, so a program can
    /// read the report without filtering prose out of it.
    fn print_json(&self, report: &serde_json::Value) -> Result<()> {
        let text = serde_json::to_string_pretty(report).into_diagnostic()?;
        println!("{text}");
        Ok(())
    }
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
            if self.output == ReportFormat::Json {
                self.print_json(&serde_json::json!({
                    "file": self.file.display().to_string(),
                    "preset": serde_json::Value::Null,
                    "checked": false,
                    "violations": [],
                    "summary": {},
                    "ok": true,
                }))?;
                return Ok(());
            }
            println!(
                "{} declares no board and places no components: nothing was checked.",
                self.file.display()
            );
            return Ok(());
        }

        if self.no_drc {
            if self.output == ReportFormat::Json {
                self.print_json(&serde_json::json!({
                    "file": self.file.display().to_string(),
                    "preset": serde_json::Value::Null,
                    "checked": false,
                    "violations": [],
                    "summary": {},
                    "ok": true,
                }))?;
                return Ok(());
            }
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
            if self.output == ReportFormat::Json {
                self.print_json(&serde_json::json!({
                    "file": self.file.display().to_string(),
                    "preset": preset.name(),
                    "checked": true,
                    "violations": [],
                    "summary": {},
                    "ok": true,
                }))?;
                return Ok(());
            }
            println!(
                "OK: {} passed DRC against {} in {}ms",
                self.file.display(),
                preset.name(),
                drc.duration_ms
            );
            return Ok(());
        }

        // A program asked, so it gets every row rather than the grouped
        // listing below: the prose collapses a pair of traces running beside
        // each other into their worst contact, which is the right thing to
        // read and the wrong thing to count against.
        if self.output == ReportFormat::Json {
            let mut summary: BTreeMap<String, usize> = BTreeMap::new();

            // Worst first, so a job reading the file finds the board's real
            // problem at the top rather than whichever rule the registry ran
            // first. The order is by how far under its rule each row is, as a
            // fraction of what the rule asked for: a short measures nothing at
            // all and comes out 1.0, a 0.100mm trace against a 0.127mm floor
            // 0.213. Rules that measure no distance - an unrouted pin, an
            // assertion - keep their order at the end, because a number
            // invented for them would sort them among the ones that have one.
            let mut ranked: Vec<&cypcb_drc::DrcViolation> = drc.violations.iter().collect();
            ranked.sort_by(|left, right| {
                let key = |violation: &cypcb_drc::DrcViolation| {
                    cypcb_drc::shortfall(violation).unwrap_or(-1.0)
                };
                key(right)
                    .partial_cmp(&key(left))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            let violations: Vec<serde_json::Value> = ranked
                .iter()
                .map(|violation| {
                    *summary.entry(violation.kind.to_string()).or_insert(0) += 1;
                    let line = world
                        .get::<cypcb_world::components::SourceSpan>(violation.entity)
                        .map(|span| line_of(source, span.start_byte));
                    serde_json::json!({
                        "kind": violation.kind.to_string(),
                        "message": violation.message,
                        "x_mm": violation.location.x.to_mm(),
                        "y_mm": violation.location.y.to_mm(),
                        "actual_mm": violation.actual.map(|value| value.to_mm()),
                        "required_mm": violation.required.map(|value| value.to_mm()),
                        "line": line,
                    })
                })
                .collect();
            self.print_json(&serde_json::json!({
                "file": self.file.display().to_string(),
                "preset": preset.name(),
                "checked": true,
                "violations": violations,
                "summary": summary,
                "ok": false,
            }))?;
            std::process::exit(1);
        }

        eprintln!(
            "{} DRC violation(s) against {}:",
            drc.violations.len(),
            preset.name()
        );

        // One contact, one row. `ClearanceRule` compares pairs of *segments*
        // and a trace is a polyline, so two traces running beside each other
        // for 10mm are one fault and as many rows as they have segments in
        // that stretch. Measured on the shipped boards: 759 rows for 484
        // contacts, and 24 rows for a single `U1 <-> trace 'GND'` on
        // `qfp_fanout`. A designer reading that sees one problem two dozen
        // times.
        //
        // **The counts do not change.** The header, the per-kind summary and
        // the shorts line are row counts and stay row counts, because every
        // published number in this project - the ratchets, the noise bands,
        // every sweep table - is a count of rows, and a display change that
        // quietly moved them would be a re-baseline pretending to be a tidy-up.
        // What changes is which rows are printed and a note saying how many
        // more there were.
        //
        // Only clearance is grouped. The other kinds report per feature and
        // two of their messages being equal is two faults, not one seen twice.
        let mut worst_of_pair: BTreeMap<(String, String), (usize, usize)> = BTreeMap::new();
        for (index, violation) in drc.violations.iter().enumerate() {
            if violation.kind != cypcb_drc::ViolationKind::Clearance {
                continue;
            }
            let key = (
                violation.kind.to_string(),
                cypcb_drc::pair_of(&violation.message).to_string(),
            );
            let gap = violation.actual.map(|a| a.raw()).unwrap_or(i64::MAX);
            match worst_of_pair.get_mut(&key) {
                None => {
                    worst_of_pair.insert(key, (index, 1));
                }
                Some((best, count)) => {
                    *count += 1;
                    let best_gap = drc.violations[*best]
                        .actual
                        .map(|a| a.raw())
                        .unwrap_or(i64::MAX);
                    if gap < best_gap {
                        *best = index;
                    }
                }
            }
        }

        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        for (index, violation) in drc.violations.iter().enumerate() {
            *counts.entry(violation.kind.to_string()).or_insert(0) += 1;

            let same_pair = worst_of_pair.get(&(
                violation.kind.to_string(),
                cypcb_drc::pair_of(&violation.message).to_string(),
            ));
            if let Some((best, _)) = same_pair {
                if *best != index {
                    continue;
                }
            }

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
            if let Some((_, count)) = same_pair {
                if *count > 1 {
                    eprintln!(
                        "      and {} more place(s) where the same two touch; this is the worst",
                        count - 1
                    );
                }
            }

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

        // Reconcile the two numbers a reader is now looking at. The listing
        // prints one clearance row per contact and the summary counts rows, so
        // on a routed board they disagree - `qfp_fanout` lists 166 and counts
        // 291 - and a reader with no line explaining that is left to work out
        // which of the two is the board.
        //
        // Said only when they differ, because on a board where every contact
        // is reported once the sentence would be noise. The counts themselves
        // do not move: they are what the ratchets, the noise bands and every
        // table in `docs/routing.md` are made of.
        let clearance_rows = counts.get("clearance").copied().unwrap_or(0);
        let contacts = worst_of_pair.len();
        if clearance_rows > contacts {
            eprintln!(
                "  ({clearance_rows} clearance rows describe {contacts} contacts; \
                 one row is listed per contact, the worst of its group)"
            );
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
        let shorts = cypcb_drc::shorts(&drc.violations);
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

    // Whose table this is, before anything about individual rules. Seven of
    // the eleven presets are a fabricator's own published page; three are this
    // tool's reading of IPC, which is not a public document; one is this
    // tool's own choice. A reader told a board is out of spec deserves to know
    // which of those three said so.
    if let Some(caveat) = preset.provenance().caveat(preset.name()) {
        eprintln!("Notes:");
        said_anything = true;
        eprintln!("  {caveat}");
    }

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
