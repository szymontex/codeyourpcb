---
phase: 01-foundation
plan: 09
type: execute
wave: 5
depends_on: ["01-08"]
files_modified:
  - crates/cypcb-cli/src/main.rs
  - crates/cypcb-cli/src/commands/mod.rs
  - crates/cypcb-cli/src/commands/parse.rs
  - crates/cypcb-cli/src/commands/check.rs
  - crates/cypcb-cli/Cargo.toml
autonomous: true

must_haves:
  truths:
    - "CLI can parse .cypcb files and output JSON"
    - "Parse errors display with source snippets"
    - "Exit codes indicate success/failure"
  artifacts:
    - path: "crates/cypcb-cli/src/main.rs"
      provides: "CLI entry point with clap"
      contains: "#[derive(Parser)]"
    - path: "crates/cypcb-cli/src/commands/parse.rs"
      provides: "Parse command implementation"
      exports: ["ParseCommand"]
  key_links:
    - from: "crates/cypcb-cli/src/commands/parse.rs"
      to: "cypcb_parser::CypcbParser"
      via: "use statement"
      pattern: "use cypcb_parser"
    - from: "crates/cypcb-cli/src/commands/parse.rs"
      to: "cypcb_world::sync_ast_to_world"
      via: "function call"
      pattern: "sync_ast_to_world"
---

<objective>
Implement the CLI with parse and check commands for headless operation.

Purpose: Provide command-line interface for parsing .cypcb files and outputting JSON representation of the board model.

Output: Working `cypcb parse` and `cypcb check` commands.
</objective>

<execution_context>
@~/.claude/get-shit-done/workflows/execute-plan.md
@~/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/phases/01-foundation/01-RESEARCH.md

Success criteria from roadmap:
- "CLI can parse file and output JSON representation"
- Error messages with line/column info (DEV-03)

CLI should support:
- cypcb parse <file.cypcb> [--output json|ast]
- cypcb check <file.cypcb>
</context>

<tasks>

<task type="auto">
  <name>Task 1: Set up CLI structure with clap</name>
  <files>
    crates/cypcb-cli/src/main.rs
    crates/cypcb-cli/src/commands/mod.rs
    crates/cypcb-cli/Cargo.toml
  </files>
  <action>
Update Cargo.toml:
```toml
[dependencies]
cypcb-parser = { path = "../cypcb-parser" }
cypcb-world = { path = "../cypcb-world" }
clap = { workspace = true }
miette = { workspace = true }
serde_json = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = "0.3"

[[bin]]
name = "cypcb"
path = "src/main.rs"
```

Create main.rs with clap derive:
```rust
use clap::{Parser, Subcommand};
use miette::Result;

mod commands;

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
}

fn main() -> Result<()> {
    // Initialize tracing for debug output
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Parse(cmd) => cmd.run(),
        Commands::Check(cmd) => cmd.run(),
    }
}
```

Create commands/mod.rs:
```rust
mod parse;
mod check;

pub use parse::ParseCommand;
pub use check::CheckCommand;
```
  </action>
  <verify>
`cargo build -p cypcb-cli` compiles
`cargo run -p cypcb-cli -- --help` shows usage
  </verify>
  <done>CLI structure set up with clap</done>
</task>

<task type="auto">
  <name>Task 2: Implement parse and check commands</name>
  <files>
    crates/cypcb-cli/src/commands/parse.rs
    crates/cypcb-cli/src/commands/check.rs
  </files>
  <action>
Create parse.rs:
```rust
use clap::Args;
use miette::{IntoDiagnostic, Result, WrapErr};
use std::path::PathBuf;
use cypcb_parser::CypcbParser;
use cypcb_world::{BoardWorld, FootprintLibrary, sync_ast_to_world};

#[derive(Args)]
pub struct ParseCommand {
    /// Input .cypcb file
    #[arg(value_name = "FILE")]
    file: PathBuf,

    /// Output format
    #[arg(short, long, default_value = "json")]
    output: OutputFormat,
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum OutputFormat {
    Json,
    Ast,
}

impl ParseCommand {
    pub fn run(&self) -> Result<()> {
        let source = std::fs::read_to_string(&self.file)
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to read {}", self.file.display()))?;

        let mut parser = CypcbParser::new();
        let ast = parser.parse(&source).map_err(|errors| {
            // Convert parse errors to miette report
            // For now, just show first error
            errors.into_iter().next().unwrap()
        })?;

        match self.output {
            OutputFormat::Ast => {
                let json = serde_json::to_string_pretty(&ast).into_diagnostic()?;
                println!("{}", json);
            }
            OutputFormat::Json => {
                let mut world = BoardWorld::new();
                let lib = FootprintLibrary::new();
                let result = sync_ast_to_world(&ast, &source, &mut world, &lib);

                // Report any sync errors
                for err in &result.errors {
                    eprintln!("{:?}", miette::Report::new(err.clone()));
                }

                // Output world as JSON
                let board_json = world.to_json().into_diagnostic()?;
                println!("{}", board_json);
            }
        }

        Ok(())
    }
}
```

Create check.rs:
```rust
use clap::Args;
use miette::{IntoDiagnostic, Result, WrapErr};
use std::path::PathBuf;
use cypcb_parser::CypcbParser;
use cypcb_world::{BoardWorld, FootprintLibrary, sync_ast_to_world};

#[derive(Args)]
pub struct CheckCommand {
    /// Input .cypcb file
    #[arg(value_name = "FILE")]
    file: PathBuf,
}

impl CheckCommand {
    pub fn run(&self) -> Result<()> {
        let source = std::fs::read_to_string(&self.file)
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to read {}", self.file.display()))?;

        let mut parser = CypcbParser::new();
        let ast = match parser.parse(&source) {
            Ok(ast) => ast,
            Err(errors) => {
                for err in errors {
                    eprintln!("{:?}", miette::Report::new(err));
                }
                std::process::exit(1);
            }
        };

        let mut world = BoardWorld::new();
        let lib = FootprintLibrary::new();
        let result = sync_ast_to_world(&ast, &source, &mut world, &lib);

        let mut has_errors = false;
        for err in result.errors {
            eprintln!("{:?}", miette::Report::new(err));
            has_errors = true;
        }

        for warn in result.warnings {
            eprintln!("Warning: {}", warn);
        }

        if has_errors {
            std::process::exit(1);
        }

        println!("OK: {} validated successfully", self.file.display());
        Ok(())
    }
}
```

Add to_json() method to BoardWorld (or create a separate serialization).
  </action>
  <verify>
Create test file examples/blink.cypcb and run:
`cargo run -p cypcb-cli -- parse examples/blink.cypcb`
`cargo run -p cypcb-cli -- check examples/blink.cypcb`
  </verify>
  <done>Parse and check commands implemented</done>
</task>

<task type="auto">
  <name>Task 3: Create example file and add BoardWorld JSON serialization</name>
  <files>
    examples/blink.cypcb
    crates/cypcb-world/src/world.rs
  </files>
  <action>
Create examples/blink.cypcb:
```cypcb
// Simple LED blink circuit
version 1

board blink {
    size 30mm x 20mm
    layers 2
}

component R1 resistor "0402" {
    value "330"
    at 10mm, 10mm
}

component LED1 led "0603" {
    at 20mm, 10mm
}

component J1 connector "pin_header_1x2" {
    at 5mm, 10mm
}

net VCC {
    J1.1
    R1.1
}

net LED_SIGNAL {
    R1.2
    LED1.1
}

net GND {
    J1.2
    LED1.2
}
```

Add JSON serialization to BoardWorld:
```rust
impl BoardWorld {
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        #[derive(Serialize)]
        struct BoardJson {
            board: Option<BoardInfo>,
            components: Vec<ComponentInfo>,
            nets: Vec<NetInfo>,
        }

        #[derive(Serialize)]
        struct BoardInfo {
            name: String,
            width_nm: i64,
            height_nm: i64,
            layers: u8,
        }

        // Query and serialize...
        serde_json::to_string_pretty(&json)
    }
}
```
  </action>
  <verify>
`cargo run -p cypcb-cli -- parse examples/blink.cypcb` outputs valid JSON
`cargo run -p cypcb-cli -- check examples/blink.cypcb` prints "OK"
JSON contains board, components, and nets
  </verify>
  <done>Example file works with CLI</done>
</task>

</tasks>

<verification>
- `cypcb parse examples/blink.cypcb` outputs JSON with board model
- `cypcb parse examples/blink.cypcb --output ast` outputs raw AST
- `cypcb check examples/blink.cypcb` validates without errors
- Parse errors show source code snippets with miette
- Exit code 0 on success, 1 on error
</verification>

<success_criteria>
1. CLI parses .cypcb files to JSON
2. Error messages include source snippets and line/column
3. Check command validates and reports issues
4. Example file demonstrates all Phase 1 features
5. Exit codes are correct (0 success, 1 error)
</success_criteria>

<output>
After completion, create `.planning/phases/01-foundation/01-09-SUMMARY.md`
</output>
