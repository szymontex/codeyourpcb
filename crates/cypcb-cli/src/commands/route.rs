//! Route command implementation.
//!
//! The built-in router does the work. FreeRouting is still reachable - this
//! command exports DSN and reads SES back - but only when a run names its jar,
//! because it is a Java program this binary cannot supply.

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use clap::Args;
use miette::{IntoDiagnostic, Result, WrapErr};

use cypcb_router::{
    apply_routes, export_dsn, FreeRoutingRunner, RoutingConfig, RoutingError, RoutingProgress,
    RoutingResult,
};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::sync_ast_to_world;
use cypcb_world::{BoardWorld, NetConnections};

/// The heading the FreeRouting-only options are printed under.
///
/// They read as general settings for `cypcb route` and are not: the built-in
/// router runs unless a jar is named, and a timeout or a pass count for a
/// program that is never started is a flag that looks like an instruction the
/// tool followed. `--timeout` cannot be refused - it carries a default, so
/// nothing tells a user who typed it from one who did not - which is the more
/// reason to print it where it belongs.
const FREEROUTING_OPTIONS: &str = "FreeRouting (opt-in: name a jar)";

/// Route a .cypcb file with the built-in autorouter.
#[derive(Args)]
pub struct RouteCommand {
    /// Input .cypcb file
    #[arg(value_name = "FILE")]
    pub file: PathBuf,

    /// Output .routes file (default: input.routes)
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Path to freerouting.jar (can also set FREEROUTING_JAR env var).
    ///
    /// Naming a jar is what asks for FreeRouting. Without it the built-in
    /// router does the work and none of the options under this heading mean
    /// anything - three of the four are refused rather than ignored.
    #[arg(long, help_heading = FREEROUTING_OPTIONS)]
    pub freerouting: Option<PathBuf>,

    /// Timeout in seconds (default: 300)
    #[arg(long, default_value = "300", help_heading = FREEROUTING_OPTIONS)]
    pub timeout: u64,

    /// Maximum routing passes
    #[arg(long, help_heading = FREEROUTING_OPTIONS)]
    pub max_passes: Option<u32>,

    /// Dry run: export DSN only, don't run FreeRouting
    #[arg(long, help_heading = FREEROUTING_OPTIONS)]
    pub dry_run: bool,

    /// Route with the built-in PathFinder autorouter and write the result
    /// back as `.cypcb` trace blocks.
    ///
    /// This is what a run does anyway. The flag is kept because it reads as an
    /// instruction and because scripts carry it: the default sat on
    /// FreeRouting while D1 - which router this project bets on - was open,
    /// and D1 closed on 2026-08-09 in favour of the in-house router. A command
    /// that needs a Java jar nobody has is not a default.
    #[arg(long)]
    pub in_house: bool,

    /// Fabrication rules to route and check against.
    ///
    /// The router and the DRC report both used JLCPCB whatever the design was
    /// for, so a board meant for another house was routed to the wrong
    /// clearances and then measured against them - two wrongs that agree.
    #[arg(long)]
    pub preset: Option<String>,

    /// Route the board several ways, score each and keep the best.
    ///
    /// This is what `--in-house` does anyway; the flag is kept because it
    /// reads as an instruction and because it turns the in-house router on by
    /// itself.
    #[arg(long)]
    pub variants: bool,

    /// Route once, with the default settings, instead of keeping the best of
    /// several.
    ///
    /// Eight routing settings have been measured on this project's benchmark
    /// boards and none is best everywhere, so the router asks the board rather
    /// than guessing - at roughly eight times the wall clock. On
    /// `examples/blink.cypcb` that is 0.06s against 0.9s, and 9 violations
    /// with 6 shorts against 5 with 3.
    ///
    /// What it costs is worth stating. Re-measured on `multi_ic` on
    /// 2026-08-08, release build: one default run gives **291 DRC violations
    /// with 187 shorts** in 5.88s, best-of-eight gives **165 with 86** in
    /// 86.03s, and `PathFinder Default` ranks **sixth of the eight** on that
    /// board. This flag buys wall clock with copper - roughly twice the shorts
    /// there. Use it when the wait matters more than the board.
    ///
    /// An earlier note here said the default came fourth on every benchmark
    /// board and quoted `multi_ic` at 248/106 against 317/166. Those numbers
    /// predate the fixture repairs - the comma that put two parts 50mm off the
    /// board, and the header that ran past its edge - so they describe a board
    /// that no longer exists.
    #[arg(long)]
    pub fast: bool,
}

impl RouteCommand {
    /// Run the route command.
    pub fn run(&self) -> Result<()> {
        let start_time = Instant::now();

        // A KiCad board routes in-house and is written back as a KiCad board.
        // The other route out of here appends `.cypcb` trace blocks to a copy
        // of the source, and appending DSL to a `(kicad_pcb ...)` file would
        // produce something neither reader can open.
        if crate::board_source::is_kicad(&self.file) {
            return self.route_kicad(start_time);
        }

        // Read input file
        let source = std::fs::read_to_string(&self.file)
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to read {}", self.file.display()))?;

        eprintln!("Parsing {}...", self.file.display());

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
        // directory. Errors are collected rather than fatal so the rest of the
        // design is still checked.
        let mut import_errors = Vec::new();
        let ast = cypcb_parser::resolve_imports(&ast, &self.file, &mut import_errors);
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

        // The same warnings `check` prints: what the board did not say.
        for warning in &sync_result.warnings {
            eprintln!("{:?}", miette::Report::new(warning.clone()));
        }

        // FreeRouting only when a run asks for it by name. `--dry-run` counts:
        // its whole output is a DSN file for somebody else's router.
        let freerouting_asked_for =
            !self.in_house && !self.variants && (self.freerouting.is_some() || self.dry_run);
        if !freerouting_asked_for {
            return self.route_in_house(&source, world, library, start_time);
        }

        // Determine output paths
        let dsn_path = self.file.with_extension("dsn");
        let ses_path = self.file.with_extension("ses");
        let routes_path = self
            .output
            .clone()
            .unwrap_or_else(|| self.file.with_extension("routes"));

        // Export to DSN
        eprintln!("Exporting to DSN format...");
        {
            let mut dsn_file = std::fs::File::create(&dsn_path)
                .into_diagnostic()
                .wrap_err("Failed to create DSN file")?;
            export_dsn(&mut world, &library, &mut dsn_file)
                .map_err(|e| miette::miette!("DSN export failed: {}", e))?;
        }
        eprintln!("  Created: {}", dsn_path.display());

        if self.dry_run {
            eprintln!("\nDry run complete. DSN file ready for manual routing.");
            eprintln!("To route manually:");
            eprintln!(
                "  java -jar freerouting.jar -de {} -do {}",
                dsn_path.display(),
                ses_path.display()
            );
            return Ok(());
        }

        // Find FreeRouting JAR
        let jar_path = self.find_freerouting_jar()?;

        // Build routing configuration
        let mut config = RoutingConfig::new(jar_path.clone()).with_timeout(self.timeout);
        if let Some(mp) = self.max_passes {
            config = config.with_max_passes(mp);
        }

        eprintln!("\nStarting FreeRouting...");
        eprintln!("  JAR: {}", jar_path.display());
        eprintln!("  Timeout: {} seconds", self.timeout);
        if let Some(mp) = self.max_passes {
            eprintln!("  Max passes: {}", mp);
        }

        // Create runner
        let runner = FreeRoutingRunner::new(config);

        // Set up Ctrl+C handler for cancellation
        let cancel_flag = runner.cancel_flag();
        ctrlc_cancel_setup(&cancel_flag);

        // Build net name to ID lookup
        let net_lookup = build_net_lookup(&mut world);

        // Run routing with progress output
        let routing_result = runner.route_with_progress(
            &dsn_path,
            &ses_path,
            &net_lookup,
            |progress: RoutingProgress| {
                print_progress(&progress);
            },
        );

        eprintln!(); // Newline after progress

        let result = match routing_result {
            Ok(result) => result,
            Err(RoutingError::Cancelled) => {
                eprintln!("Routing cancelled by user.");
                // Try to save partial results if SES exists
                if ses_path.exists() {
                    eprintln!(
                        "Partial results may be available in: {}",
                        ses_path.display()
                    );
                }
                return Ok(());
            }
            Err(RoutingError::Timeout(secs)) => {
                eprintln!("Routing timed out after {} seconds.", secs);
                if ses_path.exists() {
                    eprintln!(
                        "Partial results may be available in: {}",
                        ses_path.display()
                    );
                }
                return Err(miette::miette!("Routing timed out"));
            }
            // No Java is the second dead end, and the container this project
            // builds in walks into it: the jar is there, the runtime is not.
            // Say the same thing the missing-jar path says.
            Err(RoutingError::JavaNotFound) => {
                return Err(self.no_java_error());
            }
            Err(e) => {
                return Err(miette::miette!("Routing failed: {}", e));
            }
        };

        // Apply routes to world
        apply_routes(&mut world, &result);

        // Save routes file
        save_routes(&routes_path, &result)?;

        // Print summary
        let elapsed = start_time.elapsed();
        print_summary(&result, &routes_path, elapsed);

        Ok(())
    }

    /// Find the FreeRouting JAR file.
    fn find_freerouting_jar(&self) -> Result<PathBuf> {
        // Check explicit path first
        if let Some(ref path) = self.freerouting {
            if path.exists() {
                return Ok(path.clone());
            }
            return Err(self.no_jar_error(Some(path)));
        }

        // Check environment variable (already parsed by clap, but check explicitly)
        if let Ok(env_path) = std::env::var("FREEROUTING_JAR") {
            let path = PathBuf::from(&env_path);
            if path.exists() {
                return Ok(path);
            }
        }

        // Check common locations
        let common_paths = [
            PathBuf::from("freerouting.jar"),
            PathBuf::from("./freerouting.jar"),
            dirs::home_dir()
                .map(|h| h.join(".local/share/freerouting/freerouting.jar"))
                .unwrap_or_default(),
            dirs::home_dir()
                .map(|h| h.join("freerouting/freerouting.jar"))
                .unwrap_or_default(),
            PathBuf::from("/usr/local/share/freerouting/freerouting.jar"),
            PathBuf::from("/opt/freerouting/freerouting.jar"),
        ];

        for path in &common_paths {
            if path.exists() {
                return Ok(path.clone());
            }
        }

        Err(self.no_jar_error(None))
    }

    /// The way out of every FreeRouting dead end: the router already in this
    /// binary.
    ///
    /// `cypcb route` needs a Java runtime and a jar the project does not ship.
    /// Miss either and the command used to hand back a download link, while
    /// `--in-house` sat one flag away, compiled in, needing neither. Both dead
    /// ends print this line first now.
    fn in_house_way_out(&self) -> String {
        format!(
            "No jar and no Java needed - this binary has its own autorouter:\n  \
             cypcb route {} --in-house",
            self.file.display()
        )
    }

    /// What to print when there is no jar to run.
    ///
    /// `looked_at` is the path the user named, when they named one.
    fn no_jar_error(&self, looked_at: Option<&Path>) -> miette::Report {
        let opening = match looked_at {
            Some(path) => format!("FreeRouting JAR not found at: {}", path.display()),
            None => "FreeRouting JAR not found.".to_string(),
        };
        let way_out = self.in_house_way_out();

        miette::miette!(
            "{opening}\n\n{way_out}\n\n\
             To use FreeRouting instead:\n  \
             1. Download from https://github.com/freerouting/freerouting/releases\n  \
             2. Either:\n     \
             - Set the FREEROUTING_JAR environment variable\n     \
             - Use the --freerouting flag\n     \
             - Place freerouting.jar in the current directory",
        )
    }

    /// What to print when the jar is there but nothing can run it.
    fn no_java_error(&self) -> miette::Report {
        let way_out = self.in_house_way_out();

        miette::miette!(
            "Java not found, and FreeRouting is a Java program.\n\n{way_out}\n\n\
             To use FreeRouting instead, install a Java 21+ runtime and put \
             `java` on PATH.",
        )
    }
}

/// Build a lookup map from net names to NetIds.
fn build_net_lookup(world: &mut BoardWorld) -> HashMap<String, cypcb_world::NetId> {
    let mut lookup = HashMap::new();

    // The world provides net_name() to look up names by ID
    // We need to collect all net IDs first, then build the reverse lookup
    let mut net_ids = std::collections::HashSet::new();

    {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<&NetConnections>();

        for net_conn in query.iter(ecs) {
            for conn in net_conn.iter() {
                net_ids.insert(conn.net);
            }
        }
    }

    // Now build the lookup map using net_name()
    for net_id in net_ids {
        if let Some(name) = world.net_name(net_id) {
            lookup.insert(name.to_string(), net_id);
        }
    }

    lookup
}

/// Set up Ctrl+C handler for cancellation.
fn ctrlc_cancel_setup(cancel_flag: &std::sync::Arc<std::sync::atomic::AtomicBool>) {
    let flag = std::sync::Arc::clone(cancel_flag);

    // Note: We can't use ctrlc crate without adding it as a dependency
    // For now, rely on process termination
    // A proper implementation would use ctrlc::set_handler
    let _ = flag;

    // Signal handler would be:
    // ctrlc::set_handler(move || {
    //     flag.store(true, Ordering::SeqCst);
    //     eprintln!("\nCancelling routing...");
    // }).ok();
}

/// Print routing progress to stderr.
fn print_progress(progress: &RoutingProgress) {
    eprint!(
        "\rRouting... Pass {}: {} routed, {} unrouted ({} sec)    ",
        progress.pass, progress.routed, progress.unrouted, progress.elapsed_secs
    );
    std::io::stderr().flush().ok();
}

/// Save routing results to a .routes file.
fn save_routes(path: &PathBuf, result: &RoutingResult) -> Result<()> {
    use cypcb_router::calculate_metrics;

    let metrics = calculate_metrics(result);

    let mut file = std::fs::File::create(path)
        .into_diagnostic()
        .wrap_err("Failed to create routes file")?;

    // Simple text format for routes file
    writeln!(file, "# CodeYourPCB Routes File").into_diagnostic()?;
    writeln!(file, "# Generated by cypcb route command").into_diagnostic()?;
    writeln!(file, "# This file is regenerable - can be gitignored").into_diagnostic()?;
    writeln!(file).into_diagnostic()?;
    writeln!(file, "version 1").into_diagnostic()?;
    writeln!(file).into_diagnostic()?;
    writeln!(file, "# Routing metrics").into_diagnostic()?;
    writeln!(file, "segments {}", result.routes.len()).into_diagnostic()?;
    writeln!(file, "vias {}", metrics.via_count).into_diagnostic()?;
    writeln!(file, "total_length_nm {}", metrics.total_length.0).into_diagnostic()?;
    writeln!(file).into_diagnostic()?;

    // Write route segments
    writeln!(file, "# Route segments: net_id layer width_nm x1 y1 x2 y2").into_diagnostic()?;
    for segment in &result.routes {
        writeln!(
            file,
            "segment {} {:?} {} {} {} {} {}",
            segment.net_id.0,
            segment.layer,
            segment.width.0,
            segment.start.x.0,
            segment.start.y.0,
            segment.end.x.0,
            segment.end.y.0
        )
        .into_diagnostic()?;
    }

    writeln!(file).into_diagnostic()?;

    // Write vias
    writeln!(file, "# Vias: net_id x y drill_nm start_layer end_layer").into_diagnostic()?;
    for via in &result.vias {
        writeln!(
            file,
            "via {} {} {} {} {:?} {:?}",
            via.net_id.0,
            via.position.x.0,
            via.position.y.0,
            via.drill.0,
            via.start_layer,
            via.end_layer
        )
        .into_diagnostic()?;
    }

    Ok(())
}

/// Print routing summary.
fn print_summary(result: &RoutingResult, routes_path: &Path, elapsed: std::time::Duration) {
    use cypcb_router::calculate_metrics;

    let metrics = calculate_metrics(result);
    let total_length_mm = metrics.total_length.0 as f64 / 1_000_000.0;

    eprintln!("\nRouting complete!");
    eprintln!("  Status: {:?}", result.status);
    eprintln!("  Segments: {}", result.routes.len());
    eprintln!("  Vias: {}", metrics.via_count);
    eprintln!("  Total length: {:.2} mm", total_length_mm);
    eprintln!("  Time: {:.2} seconds", elapsed.as_secs_f64());
    eprintln!("\n  Routes saved to: {}", routes_path.display());
}

/// Home directory helper (inline implementation to avoid dependency).
mod dirs {
    use std::path::PathBuf;

    pub fn home_dir() -> Option<PathBuf> {
        std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from)
    }
}

impl RouteCommand {
    /// Route with the project's own autorouter and write the traces back as
    /// `.cypcb` source.
    ///
    /// This is the round trip the README promises - "traces persist as
    /// readable DSL code" - and until now nothing on the command line did it:
    /// `route` shells out to a Java jar, and `score` routes only to print
    /// numbers. The output is a new file rather than an edit in place, because
    /// a router is not something to point at someone's source without asking.
    /// Route a KiCad board and write it back as one.
    ///
    /// The loop a KiCad user has is: draw it in KiCad, route it, open it in
    /// KiCad. The last step needed a writer, and what it writes is narrow on
    /// purpose - the `(segment ...)` and `(via ...)` forms routing produces,
    /// inserted into a copy of the original file. Everything this project
    /// models loosely or not at all is carried through byte for byte.
    fn route_kicad(&self, start_time: Instant) -> Result<()> {
        self.refuse_freerouting_only_flags("a .kicad_pcb board is always routed in-house")?;

        use cypcb_autoroute::{route_board, AutorouteConfig};

        let source = std::fs::read_to_string(&self.file)
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to read {}", self.file.display()))?;

        let parsed = cypcb_kicad::parse_kicad_pcb(&self.file)
            .map_err(|e| miette::miette!("{e}"))
            .wrap_err_with(|| format!("Failed to read KiCad board {}", self.file.display()))?;
        for refusal in &parsed.metadata.zone_refusals {
            eprintln!("warning: {refusal}");
        }

        let library = parsed.library.clone();
        let mut world = parsed.world;
        world.set_footprints(library.clone());
        world.rebuild_spatial_index_from_library(&library);

        let preset = crate::preset_choice::resolve(self.preset.as_deref(), &world)?;
        let rules = cypcb_drc::ruleset_for_world(preset, &world);

        // Best-of-N unless the caller asked for speed, exactly as a `.cypcb`
        // board is routed. This branch used to call `route_board` with
        // `AutorouteConfig::default()` whatever the flags said, so `--variants`
        // and `--fast` were accepted and ignored on every KiCad board, and the
        // setting it always used is the one measured **fourth of eight on
        // every benchmark board**. Two commands, one router, one behaviour.
        eprintln!("Routing {}...", self.file.display());
        let result = if self.fast {
            route_board(&mut world, &library, &rules, &AutorouteConfig::default())
        } else {
            self.route_variants(&mut world, &library, &rules)?
        };

        if result.routes.is_empty() {
            return Err(miette::miette!(
                "The router produced nothing to write ({:?})",
                result.status
            ));
        }

        // A board KiCad 10 saved declares no nets, so there is no number for a
        // segment to carry. Give the file a table rather than refusing to
        // write: the nets are known - the importer interned them from the
        // pads - and the numbering only has to be self-consistent within the
        // file it is written into.
        let (numbers, declare) = if parsed.net_numbers.is_empty() {
            let mut numbers = std::collections::HashMap::new();
            let mut declare = Vec::new();
            let mut named: Vec<(cypcb_world::NetId, String)> = world
                .nets()
                .map(|(id, name)| (id, name.to_string()))
                .collect();
            named.sort_by(|a, b| a.1.cmp(&b.1));
            // Zero is KiCad's unconnected net and is always the empty name.
            let mut next = 1;
            for (id, name) in named {
                let number = if name.is_empty() { 0 } else { next };
                if number != 0 {
                    next += 1;
                }
                numbers.insert(id, number);
                declare.push((number, name));
            }
            declare.sort_by_key(|(number, _)| *number);
            if !declare.iter().any(|(number, _)| *number == 0) {
                declare.insert(0, (0, String::new()));
            }
            eprintln!(
                "The board declares no nets - KiCad 10 writes none - so {} are written into the \
                 routed copy.",
                declare.len()
            );
            (numbers, declare)
        } else {
            (parsed.net_numbers.clone(), Vec::new())
        };

        let routed = cypcb_kicad::writer::append_routing_declaring(
            &source,
            &result,
            &numbers,
            parsed.board_origin_mm,
            &declare,
        )
        .map_err(|e| miette::miette!("{e}"))?;

        let out_path = self
            .output
            .clone()
            .unwrap_or_else(|| self.file.with_extension("routed.kicad_pcb"));
        std::fs::write(&out_path, &routed)
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to write {}", out_path.display()))?;

        eprintln!(
            "Wrote {} ({} segments, {} vias) in {:.2}s",
            out_path.display(),
            result.routes.len(),
            result.vias.len(),
            start_time.elapsed().as_secs_f64()
        );
        Ok(())
    }

    /// Refuse options this run cannot honour, instead of swallowing them.
    ///
    /// `--dry-run` says "export DSN only, don't run FreeRouting". On a KiCad
    /// board it routed the whole thing and wrote the board anyway; on a
    /// `--in-house` run the same. `--freerouting` names a jar that is never
    /// started, `--max-passes` a number never read. A flag that is accepted
    /// and ignored is worse than one that does not exist: it reads as an
    /// instruction the tool followed.
    ///
    /// `--timeout` is not in this list, and cannot be: it carries a default
    /// value, so there is no way to tell a user who typed `--timeout 300` from
    /// one who typed nothing.
    fn refuse_freerouting_only_flags(&self, why: &str) -> Result<()> {
        let mut named: Vec<&str> = Vec::new();
        if self.dry_run {
            named.push("--dry-run");
        }
        if self.freerouting.is_some() {
            named.push("--freerouting");
        }
        if self.max_passes.is_some() {
            named.push("--max-passes");
        }
        if named.is_empty() {
            return Ok(());
        }

        Err(miette::miette!(
            "{} {} only to routing through FreeRouting, and {why}. Drop the \
             flag, or name a jar with --freerouting to route a .cypcb board \
             through FreeRouting.",
            named.join(" and "),
            if named.len() == 1 { "applies" } else { "apply" },
        ))
    }

    fn route_in_house(
        &self,
        source: &str,
        mut world: BoardWorld,
        library: FootprintLibrary,
        start_time: Instant,
    ) -> Result<()> {
        self.refuse_freerouting_only_flags(
            "--in-house and --variants ask for the built-in router",
        )?;

        use cypcb_autoroute::{route_board, AutorouteConfig};
        use cypcb_router::apply_routes;
        use cypcb_router::types::RoutingStatus;

        let preset = crate::preset_choice::resolve(self.preset.as_deref(), &world)?;
        let rules = cypcb_drc::ruleset_for_world(preset, &world);

        // Best-of-N unless the caller asked for speed. Routing once was the
        // default until 2026-08-07 and it is measurably not the best the
        // router can do: on examples/blink.cypcb one run gives 9 violations
        // with 6 shorts and best-of-eight gives 5 with 3, for 0.06s against
        // 0.9s.
        let result = if self.fast {
            eprintln!("Routing with the built-in autorouter...");
            route_board(&mut world, &library, &rules, &AutorouteConfig::default())
        } else {
            self.route_variants(&mut world, &library, &rules)?
        };

        match result.status {
            RoutingStatus::Failed { ref reason } => {
                return Err(miette::miette!("Routing failed: {reason}"));
            }
            RoutingStatus::Partial { unrouted_count } => {
                eprintln!("Warning: {unrouted_count} connection(s) could not be routed");
            }
            RoutingStatus::Complete => {}
        }

        apply_routes(&mut world, &result);

        // What the checker will say about the file we are about to write. The
        // scorer's number ranks candidates during the search; this one is the
        // board, measured the way `cypcb check` measures it, and the two have
        // disagreed by a factor of six.
        {
            use cypcb_drc::{run_drc, DesignRules};
            world.rebuild_spatial_index_from_library(&library);
            let report = run_drc(
                &mut world,
                &DesignRules::from_constraints(&preset.constraints()),
            );
            let shorts = report
                .violations
                .iter()
                .filter(|violation| violation.actual == Some(cypcb_core::Nm::ZERO))
                .count();
            if shorts > 0 {
                eprintln!(
                    "DRC on the routed board: {} violations, {} of them copper touching copper",
                    report.violations.len(),
                    shorts
                );
            } else {
                eprintln!(
                    "DRC on the routed board: {} violations, none of them touching",
                    report.violations.len()
                );
            }
        }

        let traces = cypcb_world::dsl::traces_as_dsl(&mut world);
        if traces.is_empty() {
            return Err(miette::miette!("The router produced nothing to write"));
        }

        // Append to a copy of the source, so the design is unchanged and the
        // traces are readable underneath it.
        let routed_path = self
            .output
            .clone()
            .unwrap_or_else(|| self.file.with_extension("routed.cypcb"));
        let mut out = source.to_string();
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str("\n// Traces below were produced by `cypcb route --in-house`.\n");
        out.push_str(&traces);

        std::fs::write(&routed_path, &out)
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to write {}", routed_path.display()))?;

        // A blind or buried via costs several times what a through hole costs to
        // make, and a four-layer board can collect them without anyone asking
        // for one - measured on multi_ic, 14 of 26. The number belongs beside
        // the via count, before the files are sent anywhere.
        let buried = result
            .vias
            .iter()
            .filter(|via| {
                !matches!(
                    (via.start_layer, via.end_layer),
                    (
                        cypcb_world::Layer::TopCopper,
                        cypcb_world::Layer::BottomCopper
                    ) | (
                        cypcb_world::Layer::BottomCopper,
                        cypcb_world::Layer::TopCopper
                    )
                )
            })
            .count();
        if buried > 0 {
            eprintln!(
                "{} of the vias are blind or buried: they join layers that are not the two faces, and cost more to make.",
                buried
            );
        }

        eprintln!(
            "Wrote {} ({} segments, {} vias) in {:.2}s",
            routed_path.display(),
            result.routes.len(),
            result.vias.len(),
            start_time.elapsed().as_secs_f64()
        );
        Ok(())
    }

    /// Route every variant, report how each scored, and return the winner.
    ///
    /// The viewer has had this since variants existed; the command line routed
    /// one way and called it the answer. A board that needs a different
    /// setting than the default got a worse board depending on which front end
    /// its owner happened to use.
    fn route_variants(
        &self,
        world: &mut BoardWorld,
        library: &FootprintLibrary,
        rules: &cypcb_rules::presets::PresetRuleSet,
    ) -> Result<cypcb_router::types::RoutingResult> {
        use cypcb_autoroute::variant::{default_variant_configs, generate_variants};
        use cypcb_drc::DesignRules;

        let configs = default_variant_configs();
        eprintln!("Routing {} variants and keeping the best...", configs.len());

        // The fab the board is for, which is what every variant is scored
        // against. This used to read `constraints_for_net(rules, 0)` - net 0,
        // whichever net that happens to be - as a stand-in for the preset,
        // which was harmless only while no net had an override. It has one
        // now: a design saying `netclass Mains [clearance 3mm]` would have
        // scored every net on the board against 3mm because a mains net
        // interned first. The preset is the preset, so ask it.
        let design_rules = DesignRules::from_constraints(&rules.preset().constraints());
        let results = generate_variants(world, library, rules, &design_rules, &configs);

        let best = results
            .first()
            .ok_or_else(|| miette::miette!("Every routing variant failed"))?;

        // Ranked best first, so the list reads as the decision it made.
        for (rank, entry) in results.iter().enumerate() {
            eprintln!(
                "  {}. {:<28} composite {:>10.1}, {} DRC violations \
                 ({} shorts, {} clearance contacts), {} vias, {:.1}s",
                rank + 1,
                entry.name,
                entry.score.composite,
                entry.score.drc_violations,
                entry.score.shorts,
                entry.score.clearance_contacts,
                entry.score.via_count,
                entry.elapsed_ms as f64 / 1000.0,
            );
        }
        eprintln!("Chose {}", best.name);

        Ok(cypcb_router::types::RoutingResult::complete(
            best.routes.clone(),
            best.vias.clone(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_route_command_parses() {
        // Test that command args parse correctly
        use clap::Parser;

        #[derive(Parser)]
        struct TestCli {
            #[command(flatten)]
            route: RouteCommand,
        }

        let cli = TestCli::parse_from(["test", "design.cypcb"]);
        assert_eq!(cli.route.file, PathBuf::from("design.cypcb"));
        assert_eq!(cli.route.timeout, 300);
        assert!(cli.route.output.is_none());
    }

    #[test]
    fn test_route_command_with_options() {
        use clap::Parser;

        #[derive(Parser)]
        struct TestCli {
            #[command(flatten)]
            route: RouteCommand,
        }

        let cli = TestCli::parse_from([
            "test",
            "design.cypcb",
            "--output",
            "custom.routes",
            "--timeout",
            "600",
            "--max-passes",
            "10",
            "--dry-run",
        ]);

        assert_eq!(cli.route.output, Some(PathBuf::from("custom.routes")));
        assert_eq!(cli.route.timeout, 600);
        assert_eq!(cli.route.max_passes, Some(10));
        assert!(cli.route.dry_run);
    }

    #[test]
    fn test_route_command_freerouting_flag() {
        use clap::Parser;

        #[derive(Parser)]
        struct TestCli {
            #[command(flatten)]
            route: RouteCommand,
        }

        let cli = TestCli::parse_from([
            "test",
            "design.cypcb",
            "--freerouting",
            "/path/to/freerouting.jar",
        ]);

        assert_eq!(
            cli.route.freerouting,
            Some(PathBuf::from("/path/to/freerouting.jar"))
        );
    }

    /// Neither missing piece is a dead end - every one of them names the way
    /// out, and the way out is a command the reader can paste.
    #[test]
    fn every_freerouting_dead_end_names_the_router_that_needs_nothing() {
        use clap::Parser;

        #[derive(Parser)]
        struct TestCli {
            #[command(flatten)]
            route: RouteCommand,
        }

        let cli = TestCli::parse_from(["test", "examples/blink.cypcb"]);

        let dead_ends = [
            ("no jar anywhere", cli.route.no_jar_error(None)),
            (
                "the jar the user named",
                cli.route
                    .no_jar_error(Some(Path::new("/nowhere/freerouting.jar"))),
            ),
            ("no Java", cli.route.no_java_error()),
        ];

        for (what_is_missing, report) in dead_ends {
            let message = report.to_string();
            assert!(
                message.contains("cypcb route examples/blink.cypcb --in-house"),
                "`{what_is_missing}` has to offer a command the reader can paste, got:\n{message}"
            );
        }

        let named = cli
            .route
            .no_jar_error(Some(Path::new("/nowhere/freerouting.jar")))
            .to_string();
        assert!(
            named.contains("/nowhere/freerouting.jar"),
            "a named path still has to say where it looked, got:\n{named}"
        );
    }
}
