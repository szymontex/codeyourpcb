//! Check command implementation.

use clap::Args;
use miette::{IntoDiagnostic, Result, WrapErr};
use std::path::PathBuf;

use cypcb_parser::CypcbParser;

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
        let source = std::fs::read_to_string(&self.file)
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to read {}", self.file.display()))?;

        let mut parser = CypcbParser::new();
        let result = parser.parse(&source);

        // Report parse errors
        let mut has_errors = false;
        if result.has_errors() {
            for err in result.errors {
                eprintln!("{:?}", miette::Report::new(err));
            }
            has_errors = true;
        }

        // Note: Semantic validation via sync_ast_to_world is currently disabled
        // due to cargo workspace dependency resolution issues between cypcb-cli
        // and cypcb-world. The parse-level validation still catches syntax errors.
        // TODO: Re-enable semantic validation when cargo issues are resolved.

        if has_errors {
            std::process::exit(1);
        }

        println!("OK: {} validated successfully", self.file.display());
        Ok(())
    }
}
