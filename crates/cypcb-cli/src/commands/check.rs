//! Check command implementation.

use clap::Args;
use miette::Result;
use std::path::PathBuf;

/// Check a .cypcb file for errors.
#[derive(Args)]
pub struct CheckCommand {
    /// Input .cypcb file
    #[arg(value_name = "FILE")]
    pub file: PathBuf,
}

impl CheckCommand {
    /// Run the check command.
    pub fn run(&self) -> Result<()> {
        // TODO: Implement in Task 2
        println!("Check command - file: {}", self.file.display());
        Ok(())
    }
}
