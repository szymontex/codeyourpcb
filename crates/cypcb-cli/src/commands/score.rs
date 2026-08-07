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

    /// Fabrication rules to score against.
    ///
    /// A score is a count of violations against somebody's rules, so which
    /// rules decides the number. This used to be JLCPCB whatever the board was
    /// for.
    #[arg(long, default_value = "jlcpcb")]
    pub preset: String,
}

impl ScoreCommand {
    /// Run the score command.
    pub fn run(&self) -> Result<()> {
        // Read input file
        let source = std::fs::read_to_string(&self.file)
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to read {}", self.file.display()))?;

        // Parse source
        let result = cypcb_parser::parse(&source);

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
                eprintln!("{:?}", miette::Report::new(err.clone()));
            }
            return Err(miette::miette!(
                "Semantic errors in {}",
                self.file.display()
            ));
        }

        // The same warnings `check` prints: what the board did not say.
        for warning in &sync_result.warnings {
            eprintln!("{:?}", miette::Report::new(warning.clone()));
        }

        // Build rules (JLCPCB 2-layer default)
        let preset = RulesPreset::from_name(&self.preset).ok_or_else(|| {
            let available: Vec<&str> = RulesPreset::all().iter().map(|p| p.name()).collect();
            miette::miette!(
                "Unknown preset '{}'. Available presets: {}",
                self.preset,
                available.join(", ")
            )
        })?;
        let rules = PresetRuleSet::new(preset);

        // Score the board in front of us, and route only a board that has no
        // copper yet.
        //
        // This used to route unconditionally. A file that already carries
        // traces parses them as `Manual`, and `apply_routes` only clears
        // `Autorouted` ones - so scoring a routed board laid a second routing
        // on top of the first and measured the pile. Measured on
        // examples/blink.cypcb routed with `--in-house`: 11 violations from
        // `score` against 6 from `cypcb check` on the same file.
        let already_routed = {
            let ecs = world.ecs_mut();
            let mut query = ecs.query::<&cypcb_world::components::trace::Trace>();
            query.iter(ecs).count()
        };

        if already_routed == 0 {
            eprintln!("No traces in the file - routing it first.");
            let config = AutorouteConfig::default();
            let routing_result = route_board(&mut world, &library, &rules, &config);
            apply_routes(&mut world, &routing_result);
        } else {
            eprintln!("Scoring the {already_routed} trace(s) the file carries.");
        }

        // Rebuild spatial index with traces for accurate scoring
        world.rebuild_spatial_index_from_library(&library);

        // Score the routed board
        let weights = ScoreWeights::default();
        // The rules the board is being scored against, not a fixed fab.
        let drc_rules = DesignRules::from_constraints(&preset.constraints());
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
