//! Plot a copper layer as PDF, for a person to print or to attach.
//!
//! SVG is for a screen and DXF is for a mechanical tool. PDF is the third
//! thing a plot is for: what a person attaches to a message, and what a house
//! prints and lays on the bench beside the board. KiCad plots to all three;
//! item 7 of the KiCad parity audit is the set, and this is the last of it.
//!
//! # No dependency
//!
//! A PDF is a handful of objects, one content stream, and a table saying where
//! each object starts. The drawing itself is the same walk over pads, tracks,
//! vias and dimensions that the SVG and DXF plotters do - `re` and `m`/`l`/`S`
//! where the SVG writes `<rect>` and `<line>`. Pulling in a PDF library to
//! write five objects would be a dependency to maintain for the sake of code
//! this file can hold.
//!
//! # The one thing that has to be exactly right
//!
//! The cross-reference table gives the byte offset of every object. A reader
//! seeks to those offsets; an offset one byte out is a file that opens in
//! nothing. They are counted from the bytes actually written rather than
//! predicted, and a test reads them back out of the finished file.
//!
//! # Units and the Y axis
//!
//! PDF user space is points - 72 to the inch - so every millimetre is
//! multiplied by 72/25.4 and nothing else is scaled: a page printed at 100% is
//! the board at size. PDF's Y grows upwards, which is the board's own
//! direction, so nothing is flipped here; the SVG plotter flips because SVG's
//! grows down.

use crate::gerber::copper::place_pad_millideg;
use cypcb_core::{Nm, Point};
use cypcb_world::components::trace::{Trace, Via};
use cypcb_world::components::{FootprintRef, Position, Rotation};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{BoardWorld, Layer, PadShape};

/// Millimetres as PDF points, with three decimals.
fn pt(value: Nm) -> String {
    format!("{:.3}", value.0 as f64 / 1_000_000.0 * 72.0 / 25.4)
}

/// A length in points, as a number rather than as text.
fn points(value: Nm) -> f64 {
    value.0 as f64 / 1_000_000.0 * 72.0 / 25.4
}

/// The colour a layer is drawn in, as PDF's own red-green-blue.
///
/// The same three colours the SVG plotter uses, so a board looked at on a
/// screen and the same board printed are recognisably the one board.
fn ink(layer: Layer) -> &'static str {
    match layer {
        Layer::TopCopper => "0.753 0.125 0.125",
        Layer::BottomCopper => "0.125 0.314 0.753",
        _ => "0.125 0.439 0.125",
    }
}

/// A filled circle, as the four Bezier curves every drawing format draws one
/// with. `0.5523` is the arc-to-Bezier constant for a quarter turn.
fn circle(out: &mut String, centre: Point, radius: Nm) {
    let (x, y) = (points(centre.x), points(centre.y));
    let r = points(radius);
    let k = r * 0.552_284_749_8;
    out.push_str(&format!("{:.3} {:.3} m\n", x + r, y));
    out.push_str(&format!(
        "{:.3} {:.3} {:.3} {:.3} {:.3} {:.3} c\n",
        x + r,
        y + k,
        x + k,
        y + r,
        x,
        y + r
    ));
    out.push_str(&format!(
        "{:.3} {:.3} {:.3} {:.3} {:.3} {:.3} c\n",
        x - k,
        y + r,
        x - r,
        y + k,
        x - r,
        y
    ));
    out.push_str(&format!(
        "{:.3} {:.3} {:.3} {:.3} {:.3} {:.3} c\n",
        x - r,
        y - k,
        x - k,
        y - r,
        x,
        y - r
    ));
    out.push_str(&format!(
        "{:.3} {:.3} {:.3} {:.3} {:.3} {:.3} c\n",
        x + k,
        y - r,
        x + r,
        y - k,
        x + r,
        y
    ));
    out.push_str("f\n");
}

/// A rectangle turned about its own centre, as a filled path.
fn turned_rectangle(out: &mut String, centre: Point, width: Nm, height: Nm, millideg: i32) {
    let turn = (millideg as f64 / 1000.0).to_radians();
    let (sin, cos) = turn.sin_cos();
    let half_x = points(width) / 2.0;
    let half_y = points(height) / 2.0;
    let (cx, cy) = (points(centre.x), points(centre.y));
    let corners = [
        (-half_x, -half_y),
        (half_x, -half_y),
        (half_x, half_y),
        (-half_x, half_y),
    ];
    for (index, (x, y)) in corners.iter().enumerate() {
        let px = cx + x * cos - y * sin;
        let py = cy + x * sin + y * cos;
        out.push_str(&format!(
            "{:.3} {:.3} {}\n",
            px,
            py,
            if index == 0 { "m" } else { "l" }
        ));
    }
    out.push_str("h\nf\n");
}

/// Plot one copper layer of this board as a PDF page.
///
/// Returns the whole file. An empty board still produces a page with its
/// outline on it, for the same reason the SVG does: a plot of nothing answers
/// "what is on In2".
pub fn plot_layer(world: &mut BoardWorld, library: &FootprintLibrary, layer: Layer) -> String {
    let (size, _) = world
        .board_info()
        .unwrap_or((cypcb_world::components::BoardSize::new(Nm(0), Nm(0)), {
            cypcb_world::components::LayerStack::new(2)
        }));
    let name = world.board_name().unwrap_or("board").to_string();
    let colour = ink(layer);

    let mut stream = String::new();

    // The board's edge, so a printed page shows where the board stops.
    stream.push_str("0.251 0.251 0.251 RG\n0.283 w\n");
    stream.push_str(&format!(
        "0 0 {} {} re\nS\n",
        pt(size.width),
        pt(size.height)
    ));

    stream.push_str(&format!("{colour} rg\n{colour} RG\n"));

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
                PadShape::Circle => circle(&mut stream, centre, Nm(width.0 / 2)),
                // A rounded corner is drawn as the rectangle it fits inside:
                // on a printed page at board size the radius is a tenth of a
                // millimetre, and the pad is being looked at rather than made.
                _ => turned_rectangle(&mut stream, centre, width, height, rotation),
            }
        }
    }

    // Tracks: one stroked line per segment, at the width that segment runs at,
    // with the round ends copper has at a corner.
    type PlottedTrace = (Trace, Option<cypcb_world::components::trace::Curve>);
    let traces: Vec<PlottedTrace> = {
        let mut query = world
            .ecs_mut()
            .query::<(&Trace, Option<&cypcb_world::components::trace::Curve>)>();
        query
            .iter(world.ecs())
            .filter(|(trace, _)| trace.layer == layer)
            .map(|(trace, curve)| (trace.clone(), curve.copied()))
            .collect()
    };
    stream.push_str("1 J\n1 j\n");
    for (trace, curve) in traces {
        // A curve is drawn as the curve it is. PDF has no arc operator and
        // does not need one: four Beziers approximate a whole circle to about
        // one part in a thousand, which is finer than the chords the checker
        // reads and far finer than a printer resolves.
        if let Some(curve) = curve {
            if let Some(first) = trace.segments.first() {
                let dx = (first.start.x.0 - curve.centre.x.0) as f64;
                let dy = (first.start.y.0 - curve.centre.y.0) as f64;
                let radius = (dx * dx + dy * dy).sqrt();
                let start = dy.atan2(dx);
                let sweep = (curve.sweep_millideg as f64 / 1000.0).to_radians();
                // A quarter turn at a time: the approximation is good to a
                // part in a thousand there and worse as the piece grows.
                let pieces = (sweep.abs() / std::f64::consts::FRAC_PI_2).ceil().max(1.0);
                let step = sweep / pieces;
                // The control-point distance for one piece of that angle.
                let handle = 4.0 / 3.0 * (step / 4.0).tan();
                let (cx, cy) = (points(curve.centre.x), points(curve.centre.y));
                let radius_pt = radius / 1_000_000.0 * 72.0 / 25.4;
                let on = |angle: f64| (cx + radius_pt * angle.cos(), cy + radius_pt * angle.sin());

                stream.push_str(&format!("{} w\n", pt(trace.width)));
                let (sx, sy) = on(start);
                stream.push_str(&format!("{sx:.3} {sy:.3} m\n"));
                for piece in 0..pieces as usize {
                    let from = start + step * piece as f64;
                    let to = from + step;
                    let (x1, y1) = on(from);
                    let (x2, y2) = on(to);
                    stream.push_str(&format!(
                        "{:.3} {:.3} {:.3} {:.3} {x2:.3} {y2:.3} c\n",
                        x1 - handle * radius_pt * from.sin(),
                        y1 + handle * radius_pt * from.cos(),
                        x2 + handle * radius_pt * to.sin(),
                        y2 - handle * radius_pt * to.cos(),
                    ));
                }
                stream.push_str("S\n");
                continue;
            }
        }
        for segment in &trace.segments {
            let width = segment.width.unwrap_or(trace.width);
            stream.push_str(&format!(
                "{} w\n{} {} m\n{} {} l\nS\n",
                pt(width),
                pt(segment.start.x),
                pt(segment.start.y),
                pt(segment.end.x),
                pt(segment.end.y)
            ));
        }
    }

    // Vias: the ring, and the hole through it in the colour of the paper.
    let vias: Vec<Via> = {
        let mut query = world.ecs_mut().query::<&Via>();
        query.iter(world.ecs()).cloned().collect()
    };
    for via in vias {
        circle(&mut stream, via.position, Nm(via.outer_diameter.0 / 2));
        stream.push_str("1 1 1 rg\n");
        circle(&mut stream, via.position, Nm(via.drill.0 / 2));
        stream.push_str(&format!("{colour} rg\n"));
    }

    // The measurements, for the person holding the printed page: the line
    // stood off what it measures, a witness line back to each end, and the
    // figure the two ends give.
    let dimensions: Vec<cypcb_world::components::BoardDimension> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<&cypcb_world::components::BoardDimension>();
        query.iter(ecs).copied().collect()
    };
    if !dimensions.is_empty() {
        stream.push_str("0.376 0.376 0.376 RG\n0.376 0.376 0.376 rg\n0.142 w\n");
    }
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
        for (from, to) in [(a, b), (dimension.from, a), (dimension.to, b)] {
            stream.push_str(&format!(
                "{} {} m\n{} {} l\nS\n",
                pt(from.x),
                pt(from.y),
                pt(to.x),
                pt(to.y)
            ));
        }
        // One millimetre of type, which is the height a designator prints at.
        stream.push_str(&format!(
            "BT\n/F1 2.835 Tf\n{} {} Td\n({:.3}mm) Tj\nET\n",
            pt(Nm((a.x.0 + b.x.0) / 2)),
            pt(Nm((a.y.0 + b.y.0) / 2)),
            dimension.length().0 as f64 / 1_000_000.0
        ));
    }

    // Five objects: the catalogue, the page tree, the page, its content
    // stream, and the one font a dimension needs.
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Contents 4 0 R \
             /Resources << /Font << /F1 5 0 R >> >> >>",
            pt(size.width),
            pt(size.height)
        ),
        format!(
            "<< /Length {} >>\nstream\n{}endstream",
            stream.len(),
            stream
        ),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
    ];

    let mut out = String::new();
    out.push_str("%PDF-1.4\n");
    // The title is a comment rather than a document-information object: it is
    // for a person reading the file, and one more object is one more offset.
    out.push_str(&format!("% {name} - {layer:?}\n"));

    // Offsets are counted from the bytes already written, never predicted.
    let mut offsets = Vec::with_capacity(objects.len());
    for (index, object) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.push_str(&format!("{} 0 obj\n{}\nendobj\n", index + 1, object));
    }

    let xref_at = out.len();
    out.push_str(&format!("xref\n0 {}\n", objects.len() + 1));
    // Every entry is exactly twenty bytes, the free head included.
    out.push_str("0000000000 65535 f \n");
    for offset in &offsets {
        out.push_str(&format!("{offset:010} 00000 n \n"));
    }
    out.push_str(&format!(
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
        objects.len() + 1,
        xref_at
    ));
    out
}
