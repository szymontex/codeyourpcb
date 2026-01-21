//! Parse command implementation.

use clap::Args;
use miette::Result;
use std::path::PathBuf;

/// Parse a .cypcb file and output the result.
#[derive(Args)]
pub struct ParseCommand {
    /// Input .cypcb file
    #[arg(value_name = "FILE")]
    pub file: PathBuf,

    /// Output format
    #[arg(short, long, default_value = "json")]
    pub output: OutputFormat,
}

/// Output format for the parse command.
#[derive(Clone, Copy, clap::ValueEnum)]
pub enum OutputFormat {
    /// Output board model as JSON
    Json,
    /// Output raw AST as JSON
    Ast,
}

impl ParseCommand {
    /// Run the parse command.
    pub fn run(&self) -> Result<()> {
        // TODO: Implement in Task 2
        println!("Parse command - file: {}", self.file.display());
        Ok(())
    }
}
