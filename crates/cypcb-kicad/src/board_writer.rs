//! Writing a whole board out as a `.kicad_pcb`.
//!
//! A design written here is text a person edits; KiCad is where the rest of
//! the world's boards live. The importer has read `.kicad_pcb` for a long time
//! and `writer.rs` can put routed copper back into a file it came from, but
//! nothing could take a `.cypcb` design and produce a KiCad board - so a
//! design written in this language could not be opened by anyone who does not
//! use this tool.
//!
//! What is written: the board outline on `Edge.Cuts`, one footprint per part
//! with its pads and their nets, every trace as `(segment ...)`, every via as
//! `(via ...)`, and the net list they refer to. Not written: zones, silkscreen
//! text, 3D models, the setup block's dozens of fields. KiCad fills its own
//! defaults for what a file leaves out, and inventing values for them here
//! would be inventing a board.
//!
//! The file is checked the only way it can be without KiCad in the room: this
//! project's own importer reads it back and the board that comes out is
//! compared with the one that went in.

use std::fmt::Write as _;

use cypcb_core::Nm;
use cypcb_world::components::trace::{Trace, Via};
use cypcb_world::components::zone::{Zone, ZoneKind};
use cypcb_world::components::{
    FootprintRef, Layer, NetConnections, PadShape, Position, RefDes, Rotation, Value,
};
use cypcb_world::footprint::SilkShape;
use cypcb_world::BoardWorld;

/// The KiCad name for a copper layer.
fn copper_layer(layer: Layer) -> Option<String> {
    Some(match layer {
        Layer::TopCopper => "F.Cu".to_string(),
        Layer::BottomCopper => "B.Cu".to_string(),
        Layer::Inner(n) => format!("In{}.Cu", n + 1),
        _ => return None,
    })
}

/// Millimetres, printed the way pcbnew writes them.
fn mm(nm: Nm) -> String {
    let value = nm.0 as f64 / 1_000_000.0;
    let text = format!("{value:.6}");
    let trimmed = text.trim_end_matches('0').trim_end_matches('.');
    if trimmed.is_empty() || trimmed == "-" {
        "0".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Where a pad sits once its component's placement is applied.
fn placed(
    origin: cypcb_core::Point,
    offset: cypcb_core::Point,
    rotation_millideg: i32,
) -> cypcb_core::Point {
    if rotation_millideg == 0 {
        return cypcb_core::Point::new(Nm(origin.x.0 + offset.x.0), Nm(origin.y.0 + offset.y.0));
    }
    let radians = (rotation_millideg as f64 / 1000.0).to_radians();
    let (sin, cos) = radians.sin_cos();
    let x = offset.x.0 as f64 * cos - offset.y.0 as f64 * sin;
    let y = offset.x.0 as f64 * sin + offset.y.0 as f64 * cos;
    cypcb_core::Point::new(
        Nm(origin.x.0 + x.round() as i64),
        Nm(origin.y.0 + y.round() as i64),
    )
}

/// KiCad's name for a pad shape.
///
/// A rounded rectangle is written as one, with the corner ratio the footprint
/// states; an oblong pad is KiCad's `oval`.
fn pad_shape(shape: PadShape) -> (&'static str, Option<u8>) {
    match shape {
        PadShape::Circle => ("circle", None),
        PadShape::Rect => ("rect", None),
        PadShape::RoundRect { corner_ratio } => ("roundrect", Some(corner_ratio)),
        PadShape::Oblong => ("oval", None),
    }
}

/// Write the board as a KiCad board file.
///
/// `generator` is what KiCad shows as the program that wrote the file.
pub fn write_board(world: &mut BoardWorld, generator: &str) -> String {
    let (size, stack) = world
        .board_info()
        .unwrap_or((cypcb_world::BoardSize::default(), Default::default()));
    let copper_layers = stack.count.max(2) as usize;

    // Nets, numbered the way KiCad numbers them: 0 is the unconnected net and
    // has to be there, or pcbnew refuses the file.
    let mut nets: Vec<(usize, String)> = vec![(0, String::new())];
    let mut net_number = std::collections::HashMap::new();
    for (id, name) in world.nets() {
        let number = nets.len();
        net_number.insert(id, number);
        nets.push((number, name.to_string()));
    }

    let mut out = String::new();
    let _ = writeln!(
        out,
        "(kicad_pcb (version 20221018) (generator \"{generator}\")"
    );
    let _ = writeln!(out, "  (general (thickness 1.6))");
    let _ = writeln!(out, "  (paper \"A4\")");

    // Layers. Copper first, in KiCad's order, then the technical layers every
    // board has.
    let _ = writeln!(out, "  (layers");
    let _ = writeln!(out, "    (0 \"F.Cu\" signal)");
    for inner in 1..copper_layers.saturating_sub(1) {
        let _ = writeln!(out, "    ({inner} \"In{inner}.Cu\" signal)");
    }
    let _ = writeln!(out, "    (31 \"B.Cu\" signal)");
    for (number, name) in [
        (36, "B.SilkS"),
        (37, "F.SilkS"),
        (38, "B.Mask"),
        (39, "F.Mask"),
        (44, "Edge.Cuts"),
    ] {
        let _ = writeln!(out, "    ({number} \"{name}\" user)");
    }
    let _ = writeln!(out, "  )");

    for (number, name) in &nets {
        let _ = writeln!(out, "  (net {number} \"{name}\")");
    }

    // The outline the design states, or the rectangle its size describes when
    // it states none. Writing the rectangle either way - which this did until
    // 2026-08-09 - sends a board with a cutout to KiCad as a plain rectangle,
    // and the Gerber exporter has honoured the real outline all along, so the
    // two files disagreed about the shape of the same board.
    let corners: Vec<(Nm, Nm)> = world
        .board_entity()
        .and_then(|entity| {
            world
                .ecs()
                .get::<cypcb_world::components::BoardOutline>(entity)
        })
        .map(|outline| outline.points.iter().map(|p| (p.x, p.y)).collect())
        .unwrap_or_else(|| {
            let (w, h) = (size.width, size.height);
            vec![(Nm(0), Nm(0)), (w, Nm(0)), (w, h), (Nm(0), h)]
        });
    for index in 0..corners.len() {
        let (x1, y1) = corners[index];
        let (x2, y2) = corners[(index + 1) % corners.len()];
        let _ = writeln!(
            out,
            "  (gr_line (start {} {}) (end {} {}) (stroke (width 0.1) (type solid)) (layer \"Edge.Cuts\"))",
            mm(x1),
            mm(y1),
            mm(x2),
            mm(y2)
        );
    }

    write_footprints(world, &net_number, &mut out);
    write_copper(world, &net_number, &mut out);
    write_zones(world, &net_number, &mut out);

    let _ = writeln!(out, ")");
    out
}

fn write_footprints(
    world: &mut BoardWorld,
    net_number: &std::collections::HashMap<cypcb_world::NetId, usize>,
    out: &mut String,
) {
    let library = world.footprints().clone();
    /// One part, read out of the world before the file is written.
    ///
    /// A named struct rather than a seven-element tuple: the tuple had grown
    /// to the point where the compiler complained about it, and a reader
    /// counting commas to find out which `String` is the footprint is one
    /// transposition away from a wrong board.
    struct Placed {
        refdes: String,
        footprint: String,
        position: cypcb_core::Point,
        rotation: i32,
        value: String,
        connections: Option<NetConnections>,
        on_bottom: bool,
    }

    let mut parts: Vec<Placed> = Vec::new();
    {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<(
            &RefDes,
            &Position,
            &Rotation,
            &FootprintRef,
            Option<&Value>,
            Option<&NetConnections>,
            Option<&cypcb_world::components::Side>,
        )>();
        for (refdes, position, rotation, footprint, value, connections, side) in query.iter(ecs) {
            parts.push(Placed {
                refdes: refdes.0.clone(),
                footprint: footprint.0.clone(),
                position: position.0,
                rotation: rotation.0,
                value: value.map(|v| v.0.clone()).unwrap_or_default(),
                connections: connections.cloned(),
                on_bottom: matches!(side, Some(cypcb_world::components::Side::Bottom)),
            });
        }
    }

    for part in parts {
        let Placed {
            refdes,
            footprint: footprint_name,
            position,
            rotation,
            value,
            connections,
            on_bottom,
        } = part;
        // KiCad states the face on the footprint and mirrors the geometry
        // itself, which is what this project's own KiCad reader relies on: a
        // back-side footprint in a real board file carries `(layer "B.Cu")`
        // and pad coordinates identical to a front-side one.
        //
        // So a bottom part is written from the footprint the design named,
        // not from the mirrored copy the world holds - writing the mirror as
        // well would flip it twice - and the layer names carry the side. It
        // used to write the mirrored copy under `(layer "F.Cu")`, under the
        // derived name `0402@bottom`, with its legend on the front: KiCad
        // opened a front-side part whose pads were on the back.
        let library_name = cypcb_world::footprint::base_name(&footprint_name).to_string();
        let geometry_name = if on_bottom {
            library_name.clone()
        } else {
            footprint_name.clone()
        };
        let Some(footprint) = library.get(&geometry_name) else {
            continue;
        };

        let (face, silk_layer, fab_layer) = if on_bottom {
            ("B.Cu", "B.SilkS", "B.Fab")
        } else {
            ("F.Cu", "F.SilkS", "F.Fab")
        };

        let _ = writeln!(
            out,
            "  (footprint \"cypcb:{library_name}\" (layer \"{face}\") (at {} {} {})",
            mm(position.x),
            mm(position.y),
            rotation as f64 / 1000.0
        );
        let _ = writeln!(
            out,
            "    (fp_text reference \"{refdes}\" (at 0 -1) (layer \"{silk_layer}\") (effects (font (size 1 1) (thickness 0.15))))"
        );
        let _ = writeln!(
            out,
            "    (fp_text value \"{value}\" (at 0 1) (layer \"{fab_layer}\") (effects (font (size 1 1) (thickness 0.15))))"
        );

        for pad in &footprint.pads {
            let (shape, corner_ratio) = pad_shape(pad.shape);
            let kind = if pad.is_smd() { "smd" } else { "thru_hole" };
            let layers = if pad.is_smd() {
                // The part's own face decides, because the geometry above is
                // the unmirrored footprint: its pads name front-side layers
                // whichever way round the part is soldered.
                let bottom = on_bottom || matches!(pad.layers.first(), Some(Layer::BottomCopper));
                if bottom {
                    "\"B.Cu\" \"B.Paste\" \"B.Mask\"".to_string()
                } else {
                    "\"F.Cu\" \"F.Paste\" \"F.Mask\"".to_string()
                }
            } else {
                "\"*.Cu\" \"*.Mask\"".to_string()
            };

            let net = connections
                .as_ref()
                .and_then(|c| c.pin_net(&pad.number))
                .and_then(|id| net_number.get(&id).map(|n| (*n, id)));

            let _ = write!(
                out,
                "    (pad \"{}\" {kind} {shape} (at {} {}) (size {} {}) (layers {layers})",
                pad.number,
                mm(pad.position.x),
                mm(pad.position.y),
                mm(pad.size.0),
                mm(pad.size.1),
            );
            if let Some(ratio) = corner_ratio {
                let _ = write!(out, " (roundrect_rratio {})", ratio as f64 / 100.0);
            }
            if let Some(drill) = pad.drill {
                let _ = write!(out, " (drill {})", mm(drill));
            }
            if let Some((number, id)) = net {
                let name = world.net_name(id).unwrap_or_default();
                let _ = write!(out, " (net {number} \"{name}\")");
            }
            let _ = writeln!(out, ")");
        }

        write_legend(footprint, silk_layer, out);

        let _ = writeln!(out, "  )");
    }
}

/// The part's legend, by the same rule the Gerber writer uses.
///
/// A footprint with artwork prints its artwork; one without prints its
/// courtyard outline, which is what tells a person where a part is when they
/// look at the board. `gerber/silk.rs` has drawn it that way since the
/// silkscreen was written, and the KiCad file carried **nothing** - measured on
/// one example board, 38 stroked segments in `F_SilkS.gbr` against 0 `fp_line`
/// in the `.kicad_pcb` for the same three parts.
///
/// The geometry is the footprint's own, unmirrored: a bottom part is written
/// under `(layer "B.Cu")` with `B.SilkS` here, and KiCad mirrors both from the
/// layer.
fn write_legend(footprint: &cypcb_world::footprint::Footprint, layer: &str, out: &mut String) {
    if !footprint.silk.is_empty() {
        for shape in &footprint.silk {
            match *shape {
                SilkShape::Segment { start, end, width } => {
                    let _ = writeln!(
                        out,
                        "    (fp_line (start {} {}) (end {} {}) (stroke (width {}) (type solid)) (layer \"{layer}\"))",
                        mm(start.x),
                        mm(start.y),
                        mm(end.x),
                        mm(end.y),
                        mm(width)
                    );
                }
                SilkShape::Circle {
                    centre,
                    radius,
                    width,
                } => {
                    let _ = writeln!(
                        out,
                        "    (fp_circle (center {} {}) (end {} {}) (stroke (width {}) (type solid)) (fill none) (layer \"{layer}\"))",
                        mm(centre.x),
                        mm(centre.y),
                        mm(cypcb_core::Nm(centre.x.0 + radius.0)),
                        mm(centre.y),
                        mm(width)
                    );
                }
            }
        }
        return;
    }

    // The courtyard, drawn as four lines rather than a rectangle primitive,
    // because that is what a footprint's outline is in a KiCad file.
    let (min, max) = (footprint.courtyard.min, footprint.courtyard.max);
    let corners = [
        (min.x, min.y, max.x, min.y),
        (max.x, min.y, max.x, max.y),
        (max.x, max.y, min.x, max.y),
        (min.x, max.y, min.x, min.y),
    ];
    for (x1, y1, x2, y2) in corners {
        let _ = writeln!(
            out,
            "    (fp_line (start {} {}) (end {} {}) (stroke (width 0.12) (type solid)) (layer \"{layer}\"))",
            mm(x1),
            mm(y1),
            mm(x2),
            mm(y2)
        );
    }
}

/// Copper pours and keepouts, as KiCad states them.
///
/// A ground plane is close to every two-layer board anybody makes, and this
/// file carried none: a design with `zone gnd_pour { ... }` exported eight
/// lines of geometry and no plane at all. The board opened in KiCad without
/// its ground, which is the same silence the Gerber writer had before the
/// pour was implemented.
///
/// The shape is the one this project's own KiCad reader accepts, and its
/// fixture is the authority for it: a net number and name, a layer, a filled
/// polygon of points. A zone spanning two layers becomes one `(zone ...)` per
/// layer, which is how KiCad stores it - `(layers ...)` on a zone is for
/// keepouts and rule areas rather than for a pour.
fn write_zones(
    world: &mut BoardWorld,
    net_number: &std::collections::HashMap<cypcb_world::NetId, usize>,
    out: &mut String,
) {
    let zones: Vec<Zone> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<&Zone>();
        query.iter(ecs).cloned().collect()
    };

    for zone in zones {
        let net = zone
            .net
            .and_then(|id| net_number.get(&id).copied())
            .unwrap_or(0);
        let net_name = zone
            .net
            .and_then(|id| world.net_name(id))
            .unwrap_or_default();

        for layer in copper_layers_of(zone.layer_mask) {
            let keepout = matches!(zone.kind, ZoneKind::Keepout);
            let _ = writeln!(out, "  (zone");
            let _ = writeln!(out, "    (net {net})");
            let _ = writeln!(out, "    (net_name \"{net_name}\")");
            let _ = writeln!(out, "    (layer \"{layer}\")");
            if let Some(name) = &zone.name {
                let _ = writeln!(out, "    (name \"{name}\")");
            }
            let _ = writeln!(out, "    (hatch edge 0.5)");
            if keepout {
                // A keepout is an area rule rather than copper: KiCad calls it
                // a zone with nothing allowed in it, and fills nothing.
                let _ = writeln!(
                    out,
                    "    (keepout (tracks not_allowed) (vias not_allowed) (pads not_allowed) (copperpour not_allowed) (footprints not_allowed))"
                );
            } else {
                let _ = writeln!(out, "    (connect_pads (clearance 0.5))");
                let _ = writeln!(out, "    (min_thickness 0.25)");
                let _ = writeln!(
                    out,
                    "    (fill yes (thermal_gap 0.5) (thermal_bridge_width 0.5))"
                );
            }
            let _ = writeln!(out, "    (polygon");
            let _ = writeln!(
                out,
                "      (pts (xy {} {}) (xy {} {}) (xy {} {}) (xy {} {}))",
                mm(zone.bounds.min.x),
                mm(zone.bounds.min.y),
                mm(zone.bounds.max.x),
                mm(zone.bounds.min.y),
                mm(zone.bounds.max.x),
                mm(zone.bounds.max.y),
                mm(zone.bounds.min.x),
                mm(zone.bounds.max.y),
            );
            let _ = writeln!(out, "    )");
            let _ = writeln!(out, "  )");
        }
    }
}

/// The copper layers a zone's mask names, in order from the top.
fn copper_layers_of(mask: u32) -> Vec<String> {
    let mut layers = Vec::new();
    if mask & 0b01 != 0 {
        layers.push("F.Cu".to_string());
    }
    if mask & 0b10 != 0 {
        layers.push("B.Cu".to_string());
    }
    for inner in 0..30u32 {
        if mask & (1 << (2 + inner)) != 0 {
            layers.push(format!("In{}.Cu", inner + 1));
        }
    }
    layers
}

fn write_copper(
    world: &mut BoardWorld,
    net_number: &std::collections::HashMap<cypcb_world::NetId, usize>,
    out: &mut String,
) {
    let traces: Vec<Trace> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<&Trace>();
        query.iter(ecs).cloned().collect()
    };
    for trace in traces {
        let Some(layer) = copper_layer(trace.layer) else {
            continue;
        };
        let net = net_number.get(&trace.net_id).copied().unwrap_or(0);
        for segment in &trace.segments {
            let _ = writeln!(
                out,
                "  (segment (start {} {}) (end {} {}) (width {}) (layer \"{layer}\") (net {net}))",
                mm(segment.start.x),
                mm(segment.start.y),
                mm(segment.end.x),
                mm(segment.end.y),
                mm(trace.width)
            );
        }
    }

    let vias: Vec<Via> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<&Via>();
        query.iter(ecs).cloned().collect()
    };
    for via in vias {
        let (Some(start), Some(end)) = (copper_layer(via.start_layer), copper_layer(via.end_layer))
        else {
            continue;
        };
        let net = net_number.get(&via.net_id).copied().unwrap_or(0);
        let _ = writeln!(
            out,
            "  (via (at {} {}) (size {}) (drill {}) (layers \"{start}\" \"{end}\") (net {net}))",
            mm(via.position.x),
            mm(via.position.y),
            mm(via.outer_diameter),
            mm(via.drill)
        );
    }
}

/// A pad's placed position, exposed for the round-trip test.
pub fn placed_pad(
    origin: cypcb_core::Point,
    offset: cypcb_core::Point,
    rotation_millideg: i32,
) -> cypcb_core::Point {
    placed(origin, offset, rotation_millideg)
}
