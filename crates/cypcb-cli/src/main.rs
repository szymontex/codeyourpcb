//! CodeYourPCB CLI - Command-line interface for parsing and validating .cypcb files.
//!
//! # Commands
//!
//! - `cypcb parse <file>` - Parse a .cypcb file and output JSON
//! - `cypcb check <file>` - Validate a .cypcb file and report errors
//! - `cypcb route <file>` - Route a .cypcb file using FreeRouting autorouter
//!
//! # Examples
//!
//! ```bash
//! # Parse and output board model as JSON
//! cypcb parse examples/blink.cypcb
//!
//! # Parse and output raw AST
//! cypcb parse examples/blink.cypcb --output ast
//!
//! # Validate a design file
//! cypcb check examples/blink.cypcb
//!
//! # Route a design using FreeRouting
//! cypcb route examples/blink.cypcb --freerouting /path/to/freerouting.jar
//! ```

use clap::{Parser, Subcommand};
use miette::Result;

mod board_source;
mod commands;

/// CodeYourPCB - Code-first PCB design
#[derive(Parser)]
#[command(name = "cypcb")]
#[command(about = "CodeYourPCB - Code-first PCB design")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Show what the router and the readers are doing.
    ///
    /// The crates this binary is built from carry 76 `tracing` calls - which
    /// net could not be routed, how many iterations the router took, which
    /// variant was skipped and why - and no command installed a subscriber, so
    /// every one of them went nowhere. `RUST_LOG` did nothing either.
    ///
    /// Once for `info`, twice for `debug`, three times for `trace`. `RUST_LOG`
    /// overrides this when it is set, so a reader who knows the syntax can ask
    /// for one crate: `RUST_LOG=cypcb_autoroute=debug`.
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    verbose: u8,
}

#[derive(Subcommand)]
enum Commands {
    /// Parse a .cypcb file and output the result
    Parse(commands::ParseCommand),
    /// Check a .cypcb file for errors
    Check(commands::CheckCommand),
    /// Route a .cypcb file using FreeRouting autorouter
    Route(commands::RouteCommand),
    /// Export a .cypcb file to manufacturing files
    Export(commands::ExportCommand),
    /// Parse a KiCad .kicad_pcb file and output metadata as JSON
    ParseKicad(commands::ParseKicadCommand),
    /// Score a routed .cypcb file — routes and prints quality metrics as JSON
    Score(commands::ScoreCommand),
}

/// Send the crates' `tracing` output somewhere a person can read it.
///
/// Warnings are on by default and go to stderr, so a variant that failed and
/// was dropped from the ranking, or a net the router gave up on, says so
/// instead of leaving a hole in the output. Anything louder is asked for.
///
/// Stdout carries the answer - the JSON of `score` and `parse`, the report of
/// `check` - and nothing here may touch it.
fn init_logging(verbose: u8) {
    use tracing_subscriber::filter::EnvFilter;

    let filter = match std::env::var("RUST_LOG") {
        Ok(value) if !value.trim().is_empty() => EnvFilter::new(value),
        _ => EnvFilter::new(match verbose {
            0 => "warn",
            1 => "info",
            2 => "debug",
            _ => "trace",
        }),
    };

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_target(false)
        .without_time()
        .init();
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    init_logging(cli.verbose);

    match cli.command {
        Commands::Parse(cmd) => cmd.run(),
        Commands::Check(cmd) => cmd.run(),
        Commands::Route(cmd) => cmd.run(),
        Commands::Export(cmd) => cmd.run(),
        Commands::ParseKicad(cmd) => cmd.run(),
        Commands::Score(cmd) => cmd.run(),
    }
}
