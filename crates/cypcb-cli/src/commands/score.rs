//! Score command implementation.
//!
//! Routes a .cypcb board and outputs a quality score breakdown as JSON.
//! Uses the `score_board()` function from `cypcb_autoroute::scoring` to
//! compute all 7 routing quality metrics.

use clap::Args;
use miette::{IntoDiagnostic, Result, WrapErr};
use std::path::PathBuf;

use cypcb_autoroute::scoring::{score_board, ScoreWeights};
use cypcb_autoroute::{route_board, AutorouteConfig};
use cypcb_drc::DesignRules;
use cypcb_parser::CypcbParser;
use cypcb_router::apply_routes;
use cypcb_rules::presets::{PresetRuleSet, RulesPreset};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::sync_ast_to_world;
use cypcb_world::BoardWorld;

/// Score a routed .cypcb file — routes the board and prints quality metrics as JSON.
#[derive(Args)]
pub struct ScoreCommand {
    /// Input .cypcb file
    #[arg(value_name = "FILE")]
    pub file: PathBuf,

    /// Use custom weights (future-proofing — currently uses equal defaults)
    #[arg(long, hide = true)]
    pub weights: Option<String>,
}

impl ScoreCommand {
    /// Run the score command.
    pub fn run(&self) -> Result<()> {
        // Read input file
        let source = std::fs::read_to_string(&self.file)
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to read {}", self.file.display()))?;

        // Parse source
        let mut parser = CypcbParser::new();
        let result = parser.parse(&source);

        if result.has_errors() {
            for err in result.errors {
                eprintln!("{:?}", miette::Report::new(err));
            }
            return Err(miette::miette!("Parse errors in {}", self.file.display()));
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
        let mut world = BoardWorld::new();
        let mut library = FootprintLibrary::new();
        let sync_result = sync_ast_to_world(&ast, &source, &mut world, &mut library);

        if !sync_result.errors.is_empty() {
            for err in &sync_result.errors {
                eprintln!("Semantic error: {}", err);
            }
            return Err(miette::miette!(
                "Semantic errors in {}",
                self.file.display()
            ));
        }

        // Build rules (JLCPCB 2-layer default)
        let preset = RulesPreset::from_name("jlcpcb")
            .ok_or_else(|| miette::miette!("Failed to load JLCPCB preset rules"))?;
        let rules = PresetRuleSet::new(preset);

        // Route the board
        let config = AutorouteConfig::default();
        let routing_result = route_board(&mut world, &library, &rules, &config);

        // Apply routes to world (spawns Trace and Via entities)
        apply_routes(&mut world, &routing_result);

        // Rebuild spatial index with traces for accurate scoring
        world.rebuild_spatial_index_from_library(&library);

        // Score the routed board
        let weights = ScoreWeights::default();
        let drc_rules = DesignRules::jlcpcb_2layer();
        let score = score_board(&mut world, &drc_rules, &weights);

        // Output as pretty JSON
        let json = serde_json::to_string_pretty(&score)
            .into_diagnostic()
            .wrap_err("Failed to serialize RoutingScore to JSON")?;

        println!("{json}");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_score_command_parses() {
        use clap::Parser;

        #[derive(Parser)]
        struct TestCli {
            #[command(flatten)]
            score: ScoreCommand,
        }

        let cli = TestCli::parse_from(["test", "design.cypcb"]);
        assert_eq!(cli.score.file, PathBuf::from("design.cypcb"));
        assert!(cli.score.weights.is_none());
    }
}
