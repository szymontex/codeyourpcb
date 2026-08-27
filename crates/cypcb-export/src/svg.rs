//! Plot a copper layer as SVG, for a person rather than for a fabricator.
//!
//! Gerber is what a house reads and nothing in this tool drew a layer anybody
//! could look at without a Gerber viewer: no picture for a review, none for a
//! document, none for a web page. KiCad plots to SVG, PDF, DXF and more; item 7
//! of the KiCad parity audit is that this plotted to none of them.
//!
//! SVG first because it is text: a test can read what was drawn line by line,
//! which a raster cannot offer and a PDF makes hard.
//!
//! # What the picture is
//!
//! One copper layer at a time, in millimetres, with the board's own outline
//! around it. Pads are filled shapes, tracks are stroked polylines of their own
//! width with round ends - which is what copper does at a corner - and vias are
//! rings. Nothing is scaled: one millimetre of board is one unit of the SVG
//! user space, so a viewer that prints at 100% prints the board at size.
//!
//! # The Y axis
//!
//! A board's Y grows upwards and an SVG's grows downwards. Everything is drawn
//! in board coordinates inside one group that flips the axis, rather than every
//! shape negating its own Y - one place to be wrong instead of six.

use crate::gerber::copper::place_pad_millideg;
use cypcb_core::{Nm, Point};
use cypcb_world::components::trace::{Trace, Via};
use cypcb_world::components::{FootprintRef, Position, Rotation};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{BoardWorld, Layer, PadShape};

/// Millimetres, as SVG user units, with three decimals - a micron.
fn mm(value: Nm) -> String {
    format!("{:.3}", value.0 as f64 / 1_000_000.0)
}

/// The colour a layer is drawn in.
///
/// Copper on the top is red and on the bottom blue, which is what every board
/// tool has done since the two were drawn on film; an inner layer takes a
/// green so it cannot be mistaken for either.
fn colour(layer: Layer) -> &'static str {
    match layer {
        Layer::TopCopper => "#c02020",
        Layer::BottomCopper => "#2050c0",
        _ => "#207020",
    }
}

/// Plot one copper layer of this board.
///
/// Returns the SVG document. An empty board still produces a document with its
/// outline, because a plot of nothing is a useful answer to "what is on In2".
pub fn plot_layer(world: &mut BoardWorld, library: &FootprintLibrary, layer: Layer) -> String {
    let (size, _) = world
        .board_info()
        .unwrap_or((cypcb_world::components::BoardSize::new(Nm(0), Nm(0)), {
            cypcb_world::components::LayerStack::new(2)
        }));
    let name = world.board_name().unwrap_or("board").to_string();
    let ink = colour(layer);

    let mut body = String::new();

    // Pads, drawn where the component's rotation puts them.
    let parts: Vec<(cypcb_core::Point, i32, String)> = {
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
            let turn = rotation as f64 / 1000.0;
            let transform = format!(
                " transform=\"rotate({:.3} {} {})\"",
                -turn,
                mm(centre.x),
                mm(centre.y)
            );
            match pad.shape {
                PadShape::Circle => body.push_str(&format!(
                    "  <circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{ink}\"/>\n",
                    mm(centre.x),
                    mm(centre.y),
                    mm(Nm(width.0 / 2))
                )),
                PadShape::Oblong => body.push_str(&format!(
                    "  <rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" fill=\"{ink}\"{transform}/>\n",
                    mm(Nm(centre.x.0 - width.0 / 2)),
                    mm(Nm(centre.y.0 - height.0 / 2)),
                    mm(width),
                    mm(height),
                    mm(Nm(width.0.min(height.0) / 2))
                )),
                PadShape::RoundRect { corner_ratio } => body.push_str(&format!(
                    "  <rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" fill=\"{ink}\"{transform}/>\n",
                    mm(Nm(centre.x.0 - width.0 / 2)),
                    mm(Nm(centre.y.0 - height.0 / 2)),
                    mm(width),
                    mm(height),
                    mm(Nm(width.0.min(height.0) * i64::from(corner_ratio) / 100))
                )),
                PadShape::Rect => body.push_str(&format!(
                    "  <rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{ink}\"{transform}/>\n",
                    mm(Nm(centre.x.0 - width.0 / 2)),
                    mm(Nm(centre.y.0 - height.0 / 2)),
                    mm(width),
                    mm(height)
                )),
            }
        }
    }

    // Tracks: one polyline per segment, at the width that segment runs at.
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
            body.push_str(&format!(
                "  <line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{ink}\" stroke-width=\"{}\" stroke-linecap=\"round\"/>\n",
                mm(segment.start.x),
                mm(segment.start.y),
                mm(segment.end.x),
                mm(segment.end.y),
                mm(width)
            ));
        }
    }

    // Vias: the ring, and the hole through it.
    let vias: Vec<Via> = {
        let mut query = world.ecs_mut().query::<&Via>();
        query.iter(world.ecs()).cloned().collect()
    };
    for via in vias {
        body.push_str(&format!(
            "  <circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{ink}\"/>\n",
            mm(via.position.x),
            mm(via.position.y),
            mm(Nm(via.outer_diameter.0 / 2))
        ));
        body.push_str(&format!(
            "  <circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"#ffffff\"/>\n",
            mm(via.position.x),
            mm(via.position.y),
            mm(Nm(via.drill.0 / 2))
        ));
    }

    // The measurements, drawn for the person looking at the plot and for
    // nobody else: a dimension is documentation, and a fabricator printing one
    // would put `30.000mm` across the finished board.
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
        // Beside what it measures, not on top of it.
        let (nx, ny) = (-dy / run, dx / run);
        let shift = dimension.offset.0 as f64;
        let shifted = |point: Point| Point {
            x: Nm(point.x.0 + (nx * shift).round() as i64),
            y: Nm(point.y.0 + (ny * shift).round() as i64),
        };
        let a = shifted(dimension.from);
        let b = shifted(dimension.to);

        body.push_str(&format!(
            "  <line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#606060\" stroke-width=\"0.050\"/>\n",
            mm(a.x), mm(a.y), mm(b.x), mm(b.y)
        ));
        // The witness lines, from the thing measured out to the dimension.
        for (end, shifted_end) in [(dimension.from, a), (dimension.to, b)] {
            body.push_str(&format!(
                "  <line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#606060\" stroke-width=\"0.050\"/>\n",
                mm(end.x), mm(end.y), mm(shifted_end.x), mm(shifted_end.y)
            ));
        }
        // The figure, upright in the reader's frame rather than the board's:
        // the whole drawing is inside a flipped group, so the text is flipped
        // back here or it reads mirrored.
        let middle_x = (a.x.0 + b.x.0) / 2;
        let middle_y = (a.y.0 + b.y.0) / 2;
        body.push_str(&format!(
            "  <text x=\"{}\" y=\"{}\" font-size=\"1\" fill=\"#606060\" text-anchor=\"middle\" \
             transform=\"translate(0 {}) scale(1 -1)\">{:.3}mm</text>\n",
            mm(Nm(middle_x)),
            mm(Nm(-middle_y)),
            mm(Nm(2 * middle_y)),
            dimension.length().0 as f64 / 1_000_000.0
        ));
    }

    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}mm\" height=\"{}mm\" viewBox=\"0 0 {} {}\">\n",
        mm(size.width),
        mm(size.height),
        mm(size.width),
        mm(size.height)
    ));
    out.push_str(&format!("  <title>{name} - {layer:?}</title>\n"));
    // One flip for the whole drawing: board Y up, SVG Y down.
    out.push_str(&format!(
        "  <g transform=\"translate(0 {}) scale(1 -1)\">\n",
        mm(size.height)
    ));
    out.push_str(&format!(
        "  <rect x=\"0\" y=\"0\" width=\"{}\" height=\"{}\" fill=\"none\" stroke=\"#404040\" stroke-width=\"0.100\"/>\n",
        mm(size.width),
        mm(size.height)
    ));
    out.push_str(&body);
    out.push_str("  </g>\n");
    out.push_str("</svg>\n");
    out
}
