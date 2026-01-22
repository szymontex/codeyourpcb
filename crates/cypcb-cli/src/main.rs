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

mod commands;

/// CodeYourPCB - Code-first PCB design
#[derive(Parser)]
#[command(name = "cypcb")]
#[command(about = "CodeYourPCB - Code-first PCB design")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Parse a .cypcb file and output the result
    Parse(commands::ParseCommand),
    /// Check a .cypcb file for errors
    Check(commands::CheckCommand),
    /// Route a .cypcb file using FreeRouting autorouter
    Route(commands::RouteCommand),
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Parse(cmd) => cmd.run(),
        Commands::Check(cmd) => cmd.run(),
        Commands::Route(cmd) => cmd.run(),
    }
}
