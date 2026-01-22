//! Route command implementation.
//!
//! This command exports a .cypcb file to DSN format, runs FreeRouting,
//! and imports the resulting routes.

use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

use clap::Args;
use miette::{IntoDiagnostic, Result, WrapErr};

use cypcb_parser::CypcbParser;
use cypcb_router::{
    apply_routes, export_dsn, FreeRoutingRunner, RoutingConfig, RoutingError, RoutingProgress,
    RoutingResult,
};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::sync_ast_to_world;
use cypcb_world::{BoardWorld, NetConnections};

/// Route a .cypcb file using FreeRouting autorouter.
#[derive(Args)]
pub struct RouteCommand {
    /// Input .cypcb file
    #[arg(value_name = "FILE")]
    pub file: PathBuf,

    /// Output .routes file (default: input.routes)
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Path to freerouting.jar (can also set FREEROUTING_JAR env var)
    #[arg(long)]
    pub freerouting: Option<PathBuf>,

    /// Timeout in seconds (default: 300)
    #[arg(long, default_value = "300")]
    pub timeout: u64,

    /// Maximum routing passes
    #[arg(long)]
    pub max_passes: Option<u32>,

    /// Dry run: export DSN only, don't run FreeRouting
    #[arg(long)]
    pub dry_run: bool,
}

impl RouteCommand {
    /// Run the route command.
    pub fn run(&self) -> Result<()> {
        let start_time = Instant::now();

        // Read input file
        let source = std::fs::read_to_string(&self.file)
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to read {}", self.file.display()))?;

        eprintln!("Parsing {}...", self.file.display());

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
        let library = FootprintLibrary::default();
        let sync_result = sync_ast_to_world(&ast, &source, &mut world, &library);

        if !sync_result.errors.is_empty() {
            for err in &sync_result.errors {
                eprintln!("Semantic error: {}", err);
            }
            return Err(miette::miette!("Semantic errors in design"));
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
            eprintln!("  java -jar freerouting.jar -de {} -do {}", dsn_path.display(), ses_path.display());
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
                    eprintln!("Partial results may be available in: {}", ses_path.display());
                }
                return Ok(());
            }
            Err(RoutingError::Timeout(secs)) => {
                eprintln!("Routing timed out after {} seconds.", secs);
                if ses_path.exists() {
                    eprintln!("Partial results may be available in: {}", ses_path.display());
                }
                return Err(miette::miette!("Routing timed out"));
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
            return Err(miette::miette!(
                "FreeRouting JAR not found at: {}\n\nTo install FreeRouting:\n  1. Download from https://github.com/freerouting/freerouting/releases\n  2. Set FREEROUTING_JAR environment variable or use --freerouting flag",
                path.display()
            ));
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

        Err(miette::miette!(
            "FreeRouting JAR not found.\n\nTo install FreeRouting:\n  1. Download from https://github.com/freerouting/freerouting/releases\n  2. Either:\n     - Set FREEROUTING_JAR environment variable\n     - Use --freerouting flag\n     - Place freerouting.jar in current directory"
        ))
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
fn print_summary(result: &RoutingResult, routes_path: &PathBuf, elapsed: std::time::Duration) {
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
}
