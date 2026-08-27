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

// ---------------------------------------------------------------------------
// Reading one back
// ---------------------------------------------------------------------------
//
// The other direction, and the one a mechanical engineer actually hands over:
// an enclosure's cutout arrives as a DXF and is retyped as `outline` points by
// hand, which is how a board ends up a fraction out from the case it has to
// fit. The reader takes the entity set the writer above produces, so the two
// agree by construction - and it takes `LWPOLYLINE` as well, because every
// tool newer than R14 writes cutouts with it.

/// What a DXF drawing had to say about a board's edge.
#[derive(Debug, Clone)]
pub struct DxfOutline {
    /// The closed loop, in the drawing's own coordinates.
    pub points: Vec<Point>,
    /// The DXF layer it came from.
    pub layer: String,
    /// What a drawing unit turned out to be.
    pub units: &'static str,
    /// How many closed loops the drawing held, this one included.
    pub loops: usize,
    /// Entity kinds that were passed over, and how many of each.
    pub skipped: Vec<(String, usize)>,
}

/// One group pair: the code, and the value that followed it.
type Pair = (u16, String);

/// Split a DXF into its pairs, or say which line stopped it.
///
/// Every line of the format is half of a pair, so a file with an odd number of
/// lines or a non-numeric code is a file whose every entity after that point
/// would be read as something else. Saying so beats guessing.
fn read_pairs(text: &str) -> Result<Vec<Pair>, String> {
    let lines: Vec<&str> = text.lines().map(|line| line.trim()).collect();
    // A trailing newline leaves one empty line, which is not half of a pair.
    let lines: Vec<&str> = match lines.last() {
        Some(&"") => lines[..lines.len() - 1].to_vec(),
        _ => lines,
    };
    if !lines.len().is_multiple_of(2) {
        return Err(format!(
            "the file has {} lines, and every line of a DXF is half of a pair",
            lines.len()
        ));
    }
    let mut pairs = Vec::with_capacity(lines.len() / 2);
    for (index, chunk) in lines.chunks(2).enumerate() {
        let code = chunk[0].parse::<u16>().map_err(|_| {
            format!(
                "line {} should be a group code and is `{}`",
                index * 2 + 1,
                chunk[0]
            )
        })?;
        pairs.push((code, chunk[1].to_string()));
    }
    Ok(pairs)
}

/// The value of the first `code` in an entity's pairs.
fn value(entity: &[Pair], code: u16) -> Option<&str> {
    entity
        .iter()
        .find(|(found, _)| *found == code)
        .map(|(_, value)| value.as_str())
}

/// A closed loop of points, and the layer it was drawn on.
struct Loop {
    points: Vec<Point>,
    layer: String,
}

/// Twice the signed area, which is all that is needed to rank loops by size.
fn double_area(points: &[Point]) -> f64 {
    let mut sum = 0.0;
    for index in 0..points.len() {
        let a = points[index];
        let b = points[(index + 1) % points.len()];
        sum += a.x.0 as f64 * b.y.0 as f64 - b.x.0 as f64 * a.y.0 as f64;
    }
    sum.abs()
}

/// Read the board edge out of a DXF drawing.
///
/// `layer` names the DXF layer to read; without one, every layer is
/// considered and the largest closed loop wins - a drawing of a case holds the
/// cutout and the holes in it, and the cutout is the big one.
///
/// The loop comes back in the drawing's own coordinates. Moving it to the
/// origin is the caller's decision, because a board placed against a fixture
/// may want the drawing's own numbers.
pub fn read_outline(text: &str, layer: Option<&str>) -> Result<DxfOutline, String> {
    let pairs = read_pairs(text)?;

    // A DXF number carries no unit of its own. `$INSUNITS` is the drawing
    // saying which it meant; a drawing that says nothing is read as
    // millimetres, which is what every board tool writes.
    let mut scale = 1_000_000.0;
    let mut units = "millimetres";
    for (index, (code, value)) in pairs.iter().enumerate() {
        if *code == 9 && value == "$INSUNITS" {
            if let Some((70, setting)) = pairs.get(index + 1) {
                if setting.trim() == "1" {
                    scale = 25_400_000.0;
                    units = "inches";
                }
            }
        }
    }
    let point_at = |x: f64, y: f64| Point {
        x: Nm((x * scale).round() as i64),
        y: Nm((y * scale).round() as i64),
    };
    let number = |entity: &[Pair], code: u16| -> f64 {
        value(entity, code)
            .and_then(|text| text.parse::<f64>().ok())
            .unwrap_or(0.0)
    };

    // Only the entities section: a drawing's blocks and tables carry the same
    // entity names and are not the drawing.
    let start = pairs
        .windows(2)
        .position(|window| {
            window[0] == (0, "SECTION".to_string()) && window[1] == (2, "ENTITIES".to_string())
        })
        .ok_or_else(|| "the file has no ENTITIES section".to_string())?;
    let entities_end = pairs
        .iter()
        .skip(start)
        .position(|pair| *pair == (0, "ENDSEC".to_string()))
        .map(|offset| start + offset)
        .unwrap_or(pairs.len());
    let body = &pairs[start + 2..entities_end];

    // Split into entities: a `0` pair starts one and ends the one before it.
    let mut entities: Vec<(String, Vec<Pair>)> = Vec::new();
    for pair in body {
        if pair.0 == 0 {
            entities.push((pair.1.clone(), Vec::new()));
        } else if let Some(last) = entities.last_mut() {
            last.1.push(pair.clone());
        }
    }

    let mut loops: Vec<Loop> = Vec::new();
    let mut segments: Vec<(String, Point, Point)> = Vec::new();
    let mut skipped: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();

    let mut index = 0;
    while index < entities.len() {
        let (kind, entity) = &entities[index];
        let on = value(entity, 8).unwrap_or("0").to_string();
        match kind.as_str() {
            // R12: the vertices are entities of their own, up to a SEQEND.
            "POLYLINE" => {
                let closed = number(entity, 70) as i64 & 1 == 1;
                let mut points = Vec::new();
                index += 1;
                while index < entities.len() && entities[index].0 == "VERTEX" {
                    let vertex = &entities[index].1;
                    points.push(point_at(number(vertex, 10), number(vertex, 20)));
                    index += 1;
                }
                if index < entities.len() && entities[index].0 == "SEQEND" {
                    index += 1;
                }
                if closed && points.len() >= 3 {
                    loops.push(Loop { points, layer: on });
                }
                continue;
            }
            // R14 and later: the vertices are pairs inside the one entity.
            "LWPOLYLINE" => {
                let closed = number(entity, 70) as i64 & 1 == 1;
                let mut points = Vec::new();
                let mut x = None;
                for (code, text) in entity {
                    match code {
                        10 => x = text.parse::<f64>().ok(),
                        20 => {
                            if let (Some(found), Ok(y)) = (x.take(), text.parse::<f64>()) {
                                points.push(point_at(found, y));
                            }
                        }
                        _ => {}
                    }
                }
                if closed && points.len() >= 3 {
                    loops.push(Loop { points, layer: on });
                }
            }
            // A cutout drawn as loose lines is the ordinary case, and the
            // lines arrive in whatever order the tool drew them.
            "LINE" => segments.push((
                on,
                point_at(number(entity, 10), number(entity, 20)),
                point_at(number(entity, 11), number(entity, 21)),
            )),
            "VERTEX" | "SEQEND" => {}
            other => {
                *skipped.entry(other.to_string()).or_insert(0) += 1;
            }
        }
        index += 1;
    }

    // Loose lines into loops: take one, follow whichever unused line starts or
    // ends where the last one stopped, and keep the ring if it closes.
    let mut used = vec![false; segments.len()];
    for seed in 0..segments.len() {
        if used[seed] {
            continue;
        }
        let on = segments[seed].0.clone();
        let start_point = segments[seed].1;
        let mut points = vec![start_point];
        let mut head = segments[seed].2;
        used[seed] = true;
        loop {
            points.push(head);
            let next = (0..segments.len()).find(|candidate| {
                !used[*candidate]
                    && segments[*candidate].0 == on
                    && (segments[*candidate].1 == head || segments[*candidate].2 == head)
            });
            let Some(next) = next else { break };
            used[next] = true;
            head = if segments[next].1 == head {
                segments[next].2
            } else {
                segments[next].1
            };
            if head == start_point {
                break;
            }
        }
        if head == start_point && points.len() >= 3 {
            loops.push(Loop { points, layer: on });
        }
    }

    let considered: Vec<&Loop> = loops
        .iter()
        .filter(|found| layer.is_none_or(|wanted| found.layer == wanted))
        .collect();
    let best = considered
        .iter()
        .max_by(|a, b| {
            double_area(&a.points)
                .partial_cmp(&double_area(&b.points))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .ok_or_else(|| {
            let seen: Vec<String> = skipped
                .iter()
                .map(|(kind, count)| format!("{count} {kind}"))
                .collect();
            match (layer, seen.is_empty()) {
                (Some(wanted), _) => format!("no closed shape on layer `{wanted}`"),
                (None, true) => "no closed shape in the drawing".to_string(),
                (None, false) => format!(
                    "no closed shape in the drawing - it holds {}, which this reads none of",
                    seen.join(", ")
                ),
            }
        })?;

    Ok(DxfOutline {
        points: best.points.clone(),
        layer: best.layer.clone(),
        units,
        loops: considered.len(),
        skipped: skipped.into_iter().collect(),
    })
}
