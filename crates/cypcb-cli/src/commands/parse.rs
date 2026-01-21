//! Parse command implementation.

use clap::Args;
use miette::{IntoDiagnostic, Result, WrapErr};
use std::path::PathBuf;

use cypcb_parser::CypcbParser;

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
        let source = std::fs::read_to_string(&self.file)
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to read {}", self.file.display()))?;

        let mut parser = CypcbParser::new();
        let result = parser.parse(&source);

        // Report parse errors
        if result.has_errors() {
            for err in result.errors {
                eprintln!("{:?}", miette::Report::new(err));
            }
            std::process::exit(1);
        }

        let ast = result.value;

        match self.output {
            OutputFormat::Ast => {
                // Output raw AST as JSON
                let json = serde_json::to_string_pretty(&ast).into_diagnostic()?;
                println!("{}", json);
            }
            OutputFormat::Json => {
                // For now, also output AST since world integration has cargo issues
                // This will be updated when the cargo resolver issue is fixed
                let json = serde_json::to_string_pretty(&ast).into_diagnostic()?;
                println!("{}", json);
                // Note: Full board model output requires fixing cypcb-world workspace dependency
            }
        }

        Ok(())
    }
}
