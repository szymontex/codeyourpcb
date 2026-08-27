//! CodeYourPCB CLI - Command-line interface for parsing and validating .cypcb files.
//!
//! # Commands
//!
//! - `cypcb parse <file>` - Parse a .cypcb design, print the model as JSON
//! - `cypcb check <file>` - Check a .cypcb or .kicad_pcb board for errors
//! - `cypcb route <file>` - Route a .cypcb or .kicad_pcb board in-house
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
//! # Route a design
//! cypcb route examples/blink.cypcb
//!
//! # Route it through FreeRouting instead, by naming the jar it needs
//! cypcb route examples/blink.cypcb --freerouting /path/to/freerouting.jar
//! ```

use clap::{Parser, Subcommand};
use miette::Result;

mod board_source;
mod commands;
mod preset_choice;

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
    /// Parse a .cypcb design and print the model as JSON (see parse-kicad)
    Parse(commands::ParseCommand),
    /// Check a .cypcb or .kicad_pcb board for errors
    Check(commands::CheckCommand),
    /// Route a .cypcb or .kicad_pcb board with the built-in autorouter
    ///
    /// Thirteen variants are routed and the best is kept: each is a different
    /// cost model, they are scored on the board in front of them, and the
    /// winner is named on the way out - `Chose PathFinder High-Density`. That
    /// is the search, and it is what makes a run take as long as it does.
    ///
    /// `--fast` routes once with the shipped defaults instead, which is the
    /// same thing `score` does. Two commands routing one board therefore
    /// answer differently unless both are told to run once, and this page and
    /// `score`'s say so rather than leaving a reader to compare two numbers
    /// that were never the same measurement.
    Route(commands::RouteCommand),
    /// Export a .cypcb or .kicad_pcb board to manufacturing files
    ///
    /// Every file written carries the moment it was written, so two exports of
    /// one board never compare equal byte for byte: a gerber and a drill file
    /// carry `TF.CreationDate`, the job file a `CreationDate` field, and the
    /// assembly JSON an `export_date` to the nanosecond. Measured by exporting
    /// one board twice a second apart - all fifteen files differ, each by that
    /// line and nothing else.
    ///
    /// It is written because a fabricator asks when the files were cut, KiCad
    /// writes it too, and Ucamco's specification lists the attribute as
    /// optional rather than unwanted. There is no flag to leave it out, so
    /// anything comparing two exports has to drop the stamp itself.
    Export(commands::ExportCommand),
    /// Parse a KiCad .kicad_pcb file and output metadata as JSON
    ParseKicad(commands::ParseKicadCommand),

    /// Write a KiCad .kicad_pcb board out as a .cypcb design
    ///
    /// A `.kicad_pro` beside the board is read when there is one. A
    /// `.kicad_pcb` has no field for the fabricator a board was written for or
    /// for what a net asks of a trace, so `to-kicad` puts both in the project
    /// file under a key of this project's own and this reads them back - keep
    /// the pair together or the design comes home graded against the default
    /// table with its nets asking for nothing.
    ///
    /// The rules that project file states - the eight numbers KiCad enforces -
    /// are read too and compared against the table this design will be checked
    /// against; the ones that disagree are named, because this language states
    /// rules per fab and per net rather than per board. A project file KiCad
    /// wrote has no such key and is read for those numbers alone.
    FromKicad(commands::FromKicadCommand),
    /// Read a DXF drawing's cutout as a .cypcb board outline
    ///
    /// An enclosure is drawn in a mechanical tool and the board has to fit
    /// inside it. Until this, the way that fact reached a design was a person
    /// reading coordinates off a drawing and typing them in. The largest
    /// closed shape wins unless `--layer` names one, the shape is moved to the
    /// origin unless `--keep-origin` says otherwise, and every entity passed
    /// over is named - a curve is not a line or a polyline, and this reads
    /// only those two.
    FromDxf(commands::FromDxfCommand),
    /// Route a .cypcb or .kicad_pcb board and print quality metrics as JSON
    ///
    /// The shipped defaults, once - no variant search. `route` ranks thirteen
    /// cost models and keeps the winner, so its board is usually the better
    /// one and its numbers are not these numbers; `route --fast` is the run
    /// that matches this one. Scoring measures a board rather than searching
    /// for one, which is why the defaults are the honest thing to measure.
    ///
    /// A board that already carries copper is measured as it stands rather
    /// than routed again, and the run says which it did.
    Score(commands::ScoreCommand),
    /// Write a .cypcb design out as a KiCad .kicad_pcb board
    ///
    /// Two files come out, not one: the board, and a `.kicad_pro` beside it
    /// carrying what a `.kicad_pcb` has no field for. The design rules go
    /// there because that is where KiCad reads them; the fabricator the board
    /// names and what each net asks for go there under a key of this
    /// project's own, so `from-kicad` can read them back. **Keep the pair
    /// together**: a board opened or re-read without its project file is
    /// checked against the default table with its nets asking for nothing.
    ///
    /// KiCad ignores the key it does not know and drops it if it saves the
    /// project itself, so the pairing survives a trip through these two
    /// commands rather than through the editor. What the format cannot carry
    /// at all - the drill spans a build makes, a flexible region - is
    /// announced as it is dropped.
    ToKicad(commands::ToKicadCommand),
    /// Check a .cypcb or .kicad_pcb board, then again every time it changes
    Watch(commands::WatchCommand),
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
        Commands::FromKicad(cmd) => cmd.run(),
        Commands::FromDxf(cmd) => cmd.run(),
        Commands::ToKicad(cmd) => cmd.run(),
        Commands::Watch(cmd) => cmd.run(),
        Commands::Score(cmd) => cmd.run(),
    }
}
