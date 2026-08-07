//! Parse command implementation.

use clap::Args;
use miette::{IntoDiagnostic, Result, WrapErr};
use serde::Serialize;
use std::path::PathBuf;

use cypcb_world::components::trace::{Trace, Via};
use cypcb_world::components::zone::Zone;
use cypcb_world::components::{FootprintRef, NetConnections};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{sync_ast_to_world, BoardWorld, Position, RefDes, Rotation, Value};

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

/// The board a file describes, after imports are resolved and the AST is
/// turned into the model every other command works on.
///
/// This is what `-o json` promises. It said so from the first release and
/// printed the AST instead, which is a different thing in every way that
/// matters: the AST has no footprint geometry, no resolved nets, and nothing
/// an import brought in.
#[derive(Serialize)]
struct BoardModel {
    board: Option<BoardModelInfo>,
    components: Vec<ComponentModel>,
    nets: Vec<NetModel>,
    traces: Vec<TraceModel>,
    vias: Vec<ViaModel>,
    zones: Vec<ZoneModel>,
}

#[derive(Serialize)]
struct BoardModelInfo {
    name: String,
    width_nm: i64,
    height_nm: i64,
    layers: u8,
}

#[derive(Serialize)]
struct ComponentModel {
    refdes: String,
    value: String,
    footprint: String,
    /// Whether the footprint the component names is one the model could find.
    footprint_known: bool,
    x_nm: i64,
    y_nm: i64,
    rotation_deg: f64,
    pins: Vec<PinModel>,
}

#[derive(Serialize)]
struct PinModel {
    pin: String,
    net: String,
}

#[derive(Serialize)]
struct NetModel {
    name: String,
    id: u32,
    width_nm: Option<i64>,
    clearance_nm: Option<i64>,
    current_ma: Option<f64>,
}

#[derive(Serialize)]
struct TraceModel {
    net: String,
    layer: String,
    width_nm: i64,
    locked: bool,
    segments: Vec<SegmentModel>,
}

#[derive(Serialize)]
struct SegmentModel {
    start_x_nm: i64,
    start_y_nm: i64,
    end_x_nm: i64,
    end_y_nm: i64,
}

#[derive(Serialize)]
struct ViaModel {
    net: String,
    x_nm: i64,
    y_nm: i64,
    drill_nm: i64,
    diameter_nm: i64,
    from_layer: String,
    to_layer: String,
}

#[derive(Serialize)]
struct ZoneModel {
    name: Option<String>,
    kind: String,
    net: Option<String>,
    layer_mask: u32,
    min_x_nm: i64,
    min_y_nm: i64,
    max_x_nm: i64,
    max_y_nm: i64,
}

impl ParseCommand {
    /// Run the parse command.
    pub fn run(&self) -> Result<()> {
        let source = std::fs::read_to_string(&self.file)
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to read {}", self.file.display()))?;

        let result = cypcb_parser::parse(&source);

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
                let json = serde_json::to_string_pretty(&ast).into_diagnostic()?;
                println!("{}", json);
            }
            OutputFormat::Json => {
                // The same three steps `check` takes, so the two commands
                // cannot disagree about what a file says: resolve what it
                // imports, sync the AST into the world, then read the world.
                let mut import_errors = Vec::new();
                let ast = cypcb_parser::resolve_imports(&ast, &self.file, &mut import_errors);
                for error in &import_errors {
                    eprintln!("Import error: {error}");
                }

                let mut world = BoardWorld::new();
                let mut library = FootprintLibrary::new();
                let sync_result = sync_ast_to_world(&ast, &source, &mut world, &mut library);
                if !sync_result.errors.is_empty() {
                    for err in &sync_result.errors {
                        eprintln!("Semantic error: {}", err);
                    }
                    std::process::exit(1);
                }

                let model = board_model(&mut world, &library);
                let json = serde_json::to_string_pretty(&model).into_diagnostic()?;
                println!("{}", json);
            }
        }

        Ok(())
    }
}

/// Read the board out of the world, in the order the file's own entities were
/// spawned so two runs of the same file print the same bytes.
fn board_model(world: &mut BoardWorld, library: &FootprintLibrary) -> BoardModel {
    let board = world.board_info().map(|(size, stack)| BoardModelInfo {
        name: world.board_name().unwrap_or("").to_string(),
        width_nm: size.width.0,
        height_nm: size.height.0,
        layers: stack.count,
    });

    let net_name = |world: &BoardWorld, id: cypcb_world::NetId| -> String {
        world.net_name(id).unwrap_or("").to_string()
    };

    let mut nets: Vec<NetModel> = world
        .nets()
        .map(|(id, name)| (id, name.to_string()))
        .collect::<Vec<_>>()
        .into_iter()
        .map(|(id, name)| {
            let constraints = world.net_constraints(id);
            NetModel {
                name,
                id: id.id(),
                width_nm: constraints.as_ref().and_then(|c| c.width).map(|w| w.0),
                clearance_nm: constraints.as_ref().and_then(|c| c.clearance).map(|c| c.0),
                current_ma: constraints.as_ref().and_then(|c| c.current_ma),
            }
        })
        .collect();
    nets.sort_by_key(|net| net.id);

    let components: Vec<ComponentModel> = {
        let mut query = world.ecs_mut().query::<(
            &RefDes,
            &Value,
            &Position,
            &Rotation,
            &FootprintRef,
            Option<&NetConnections>,
        )>();
        let rows: Vec<_> = query
            .iter(world.ecs())
            .map(
                |(refdes, value, position, rotation, footprint, connections)| {
                    (
                        refdes.as_str().to_string(),
                        value.as_str().to_string(),
                        footprint.as_str().to_string(),
                        position.0,
                        rotation.to_degrees(),
                        connections
                            .map(|c| {
                                c.iter()
                                    .map(|pin| (pin.pin.clone(), pin.net))
                                    .collect::<Vec<_>>()
                            })
                            .unwrap_or_default(),
                    )
                },
            )
            .collect();

        let mut components: Vec<ComponentModel> = rows
            .into_iter()
            .map(
                |(refdes, value, footprint, position, rotation_deg, pins)| ComponentModel {
                    footprint_known: library.get(&footprint).is_some(),
                    refdes,
                    value,
                    footprint,
                    x_nm: position.x.0,
                    y_nm: position.y.0,
                    rotation_deg,
                    pins: pins
                        .into_iter()
                        .map(|(pin, net)| PinModel {
                            pin,
                            net: net_name(world, net),
                        })
                        .collect(),
                },
            )
            .collect();
        components.sort_by(|a, b| a.refdes.cmp(&b.refdes));
        components
    };

    let traces: Vec<TraceModel> = {
        let mut query = world.ecs_mut().query::<&Trace>();
        let rows: Vec<Trace> = query.iter(world.ecs()).cloned().collect();
        rows.into_iter()
            .map(|trace| TraceModel {
                net: net_name(world, trace.net_id),
                layer: format!("{:?}", trace.layer),
                width_nm: trace.width.0,
                locked: trace.locked,
                segments: trace
                    .segments
                    .iter()
                    .map(|segment| SegmentModel {
                        start_x_nm: segment.start.x.0,
                        start_y_nm: segment.start.y.0,
                        end_x_nm: segment.end.x.0,
                        end_y_nm: segment.end.y.0,
                    })
                    .collect(),
            })
            .collect()
    };

    let vias: Vec<ViaModel> = {
        let mut query = world.ecs_mut().query::<&Via>();
        let rows: Vec<Via> = query.iter(world.ecs()).cloned().collect();
        rows.into_iter()
            .map(|via| ViaModel {
                net: net_name(world, via.net_id),
                x_nm: via.position.x.0,
                y_nm: via.position.y.0,
                drill_nm: via.drill.0,
                diameter_nm: via.outer_diameter.0,
                from_layer: format!("{:?}", via.start_layer),
                to_layer: format!("{:?}", via.end_layer),
            })
            .collect()
    };

    let zone_rows: Vec<Zone> = world.zones().into_iter().map(|(_, zone)| zone).collect();
    let zones: Vec<ZoneModel> = zone_rows
        .into_iter()
        .map(|zone| ZoneModel {
            name: zone.name.clone(),
            kind: format!("{:?}", zone.kind),
            net: zone.net.map(|id| net_name(world, id)),
            layer_mask: zone.layer_mask,
            min_x_nm: zone.bounds.min.x.0,
            min_y_nm: zone.bounds.min.y.0,
            max_x_nm: zone.bounds.max.x.0,
            max_y_nm: zone.bounds.max.y.0,
        })
        .collect();

    BoardModel {
        board,
        components,
        nets,
        traces,
        vias,
        zones,
    }
}
