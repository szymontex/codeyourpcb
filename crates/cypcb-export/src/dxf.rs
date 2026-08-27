//! Plot a copper layer as DXF, for a mechanical tool rather than a person.
//!
//! An SVG is a picture and a Gerber is a fabrication file. Neither is what a
//! mechanical engineer opens: an enclosure is drawn in a CAD tool, and the
//! question that tool asks of a board is where the copper, the holes and the
//! edge actually are. DXF is the format every one of those tools reads. Item 7
//! of the KiCad parity audit is that this plotted to none of them; SVG closed
//! the picture half and this closes the mechanical one.
//!
//! # Which DXF
//!
//! AC1009 - AutoCAD R12. It is the oldest version still universally read, it
//! needs no entity handles, and its entity set is enough for a board:
//! `LINE`, `CIRCLE`, `POLYLINE` with vertices, and `TEXT`. `LWPOLYLINE` would
//! be tidier and arrived in R14, which is a version some mechanical tools
//! still refuse; a heavier polyline every tool reads beats a lighter one some
//! tools do not.
//!
//! # The Y axis
//!
//! DXF's Y grows upwards, which is the board's own direction, so nothing is
//! flipped here. The SVG plotter flips because SVG's Y grows down; this one
//! writing the same numbers unchanged is the correct difference between them.
//!
//! # What a layer carries
//!
//! Copper on its own DXF layer, named the way the Gerber is - `F_Cu`, `B_Cu`,
//! `In1_Cu` - the board's edge on `Edge_Cuts`, and any dimensions on
//! `Dimensions`. A tool that wants the outline alone can switch the rest off,
//! which is the whole reason for putting them on separate layers.

use crate::gerber::copper::place_pad_millideg;
use cypcb_core::{Nm, Point};
use cypcb_world::components::trace::{Trace, Via};
use cypcb_world::components::{FootprintRef, Position, Rotation};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{BoardWorld, Layer, PadShape};

/// Millimetres, with three decimals - a micron, the same figure the SVG uses.
fn mm(value: Nm) -> String {
    format!("{:.3}", value.0 as f64 / 1_000_000.0)
}

/// One group: the code on its own line, then the value on the next.
///
/// Every line of a DXF file is half of one of these pairs. Writing them
/// through one function is what keeps a stray newline from shifting every
/// group after it, which is the failure this format is famous for.
fn group(out: &mut String, code: u16, value: &str) {
    out.push_str(&format!("{code}\n{value}\n"));
}

/// The DXF layer a copper layer is drawn on, named as its Gerber is.
fn layer_name(layer: Layer) -> String {
    match layer {
        Layer::TopCopper => "F_Cu".to_string(),
        Layer::BottomCopper => "B_Cu".to_string(),
        Layer::Inner(index) => format!("In{}_Cu", index + 1),
        other => format!("{other:?}"),
    }
}

/// A straight run of copper, at the width it runs at.
fn polyline(out: &mut String, layer: &str, points: &[Point], closed: bool, width: Option<Nm>) {
    if points.len() < 2 {
        return;
    }
    group(out, 0, "POLYLINE");
    group(out, 8, layer);
    // 66 announces that vertices follow, which R12 requires and later
    // versions ignore.
    group(out, 66, "1");
    group(out, 70, if closed { "1" } else { "0" });
    if let Some(width) = width {
        group(out, 40, &mm(width));
        group(out, 41, &mm(width));
    }
    for point in points {
        group(out, 0, "VERTEX");
        group(out, 8, layer);
        group(out, 10, &mm(point.x));
        group(out, 20, &mm(point.y));
        group(out, 30, "0.0");
    }
    group(out, 0, "SEQEND");
    group(out, 8, layer);
}

/// A pad, a via ring or a drilled hole.
fn circle(out: &mut String, layer: &str, centre: Point, radius: Nm) {
    group(out, 0, "CIRCLE");
    group(out, 8, layer);
    group(out, 10, &mm(centre.x));
    group(out, 20, &mm(centre.y));
    group(out, 30, "0.0");
    group(out, 40, &mm(radius));
}

/// A rectangle turned about its own centre, as four corners.
fn rectangle(centre: Point, width: Nm, height: Nm, millideg: i32) -> Vec<Point> {
    let turn = (millideg as f64 / 1000.0).to_radians();
    let (sin, cos) = turn.sin_cos();
    let half_x = width.0 as f64 / 2.0;
    let half_y = height.0 as f64 / 2.0;
    [
        (-half_x, -half_y),
        (half_x, -half_y),
        (half_x, half_y),
        (-half_x, half_y),
    ]
    .iter()
    .map(|(x, y)| Point {
        x: Nm(centre.x.0 + (x * cos - y * sin).round() as i64),
        y: Nm(centre.y.0 + (x * sin + y * cos).round() as i64),
    })
    .collect()
}

/// Plot one copper layer of this board as a DXF drawing.
///
/// Returns the whole file. An empty board still produces one, with its edge on
/// `Edge_Cuts`: a drawing of nothing is a useful answer to "what is on In2",
/// and a mechanical tool asking for the outline gets it either way.
pub fn plot_layer(world: &mut BoardWorld, library: &FootprintLibrary, layer: Layer) -> String {
    let (size, _) = world
        .board_info()
        .unwrap_or((cypcb_world::components::BoardSize::new(Nm(0), Nm(0)), {
            cypcb_world::components::LayerStack::new(2)
        }));
    let copper = layer_name(layer);

    let mut body = String::new();

    // The edge, before the copper: a tool that reads only the first entity it
    // understands gets the board's own extent.
    polyline(
        &mut body,
        "Edge_Cuts",
        &[
            Point { x: Nm(0), y: Nm(0) },
            Point {
                x: size.width,
                y: Nm(0),
            },
            Point {
                x: size.width,
                y: size.height,
            },
            Point {
                x: Nm(0),
                y: size.height,
            },
        ],
        true,
        None,
    );

    // Pads, where the component's rotation puts them.
    let parts: Vec<(Point, i32, String)> = {
        let mut query = world
            .ecs_mut()
            .query::<(&Position, &Rotation, &FootprintRef)>();
        query
            .iter(world.ecs())
            .map(|(position, rotation, footprint)| {
                (position.0, rotation.0, footprint.as_str().to_string())
            })
            .collect()
    };
    for (position, rotation, footprint_name) in parts {
        let Some(footprint) = library.get(&footprint_name) else {
            continue;
        };
        for pad in &footprint.pads {
            if !pad.layers.contains(&layer) {
                continue;
            }
            let centre = place_pad_millideg(position, pad.position, rotation);
            let (width, height) = pad.size;
            match pad.shape {
                // Every rounded corner a board has - oblong, rounded
                // rectangle - is drawn as the rectangle it fits inside. A
                // mechanical tool asks how much room the pad needs, and the
                // corner radius does not change the answer.
                PadShape::Circle => circle(&mut body, &copper, centre, Nm(width.0 / 2)),
                _ => polyline(
                    &mut body,
                    &copper,
                    &rectangle(centre, width, height, rotation),
                    true,
                    None,
                ),
            }
        }
    }

    // Tracks: one polyline per segment, carrying the width that segment runs
    // at as the polyline's own width.
    let traces: Vec<Trace> = {
        let mut query = world.ecs_mut().query::<&Trace>();
        query
            .iter(world.ecs())
            .filter(|trace| trace.layer == layer)
            .cloned()
            .collect()
    };
    for trace in traces {
        for segment in &trace.segments {
            let width = segment.width.unwrap_or(trace.width);
            polyline(
                &mut body,
                &copper,
                &[segment.start, segment.end],
                false,
                Some(width),
            );
        }
    }

    // Vias: the ring, and the hole through it. The hole is on its own layer
    // because a mechanical tool asking where to clear a boss wants the drilled
    // holes and not the copper around them.
    let vias: Vec<Via> = {
        let mut query = world.ecs_mut().query::<&Via>();
        query.iter(world.ecs()).cloned().collect()
    };
    for via in vias {
        circle(
            &mut body,
            &copper,
            via.position,
            Nm(via.outer_diameter.0 / 2),
        );
        circle(&mut body, "Drill", via.position, Nm(via.drill.0 / 2));
    }

    // The measurements, on their own layer and drawn the same way the SVG
    // draws them: the line stood off what it measures, a witness line back to
    // each end, and the figure the ends give.
    let dimensions: Vec<cypcb_world::components::BoardDimension> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<&cypcb_world::components::BoardDimension>();
        query.iter(ecs).copied().collect()
    };
    for dimension in &dimensions {
        let dx = (dimension.to.x.0 - dimension.from.x.0) as f64;
        let dy = (dimension.to.y.0 - dimension.from.y.0) as f64;
        let run = (dx * dx + dy * dy).sqrt();
        if run <= 0.0 {
            continue;
        }
        let (nx, ny) = (-dy / run, dx / run);
        let shift = dimension.offset.0 as f64;
        let shifted = |point: Point| Point {
            x: Nm(point.x.0 + (nx * shift).round() as i64),
            y: Nm(point.y.0 + (ny * shift).round() as i64),
        };
        let a = shifted(dimension.from);
        let b = shifted(dimension.to);
        polyline(&mut body, "Dimensions", &[a, b], false, None);
        polyline(&mut body, "Dimensions", &[dimension.from, a], false, None);
        polyline(&mut body, "Dimensions", &[dimension.to, b], false, None);

        group(&mut body, 0, "TEXT");
        group(&mut body, 8, "Dimensions");
        group(&mut body, 10, &mm(Nm((a.x.0 + b.x.0) / 2)));
        group(&mut body, 20, &mm(Nm((a.y.0 + b.y.0) / 2)));
        group(&mut body, 30, "0.0");
        group(&mut body, 40, "1.0");
        group(
            &mut body,
            1,
            &format!("{:.3}mm", dimension.length().0 as f64 / 1_000_000.0),
        );
    }

    let mut out = String::new();
    group(&mut out, 0, "SECTION");
    group(&mut out, 2, "HEADER");
    group(&mut out, 9, "$ACADVER");
    group(&mut out, 1, "AC1009");
    // Millimetres. A DXF number carries no unit of its own, so a drawing that
    // does not say this is a drawing a tool may read as inches.
    group(&mut out, 9, "$INSUNITS");
    group(&mut out, 70, "4");
    group(&mut out, 0, "ENDSEC");

    group(&mut out, 0, "SECTION");
    group(&mut out, 2, "ENTITIES");
    out.push_str(&body);
    group(&mut out, 0, "ENDSEC");
    group(&mut out, 0, "EOF");
    out
}
