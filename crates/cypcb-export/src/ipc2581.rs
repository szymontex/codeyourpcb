//! The handoff file a modern fabricator reads: IPC-2581.
//!
//! Gerber says what to etch. It does not say what the board is - the netlist,
//! the stackup, which holes a build drills, which layer is which - and every
//! one of those travels beside the copper as a separate file a person has to
//! keep together with it. IPC-2581 is one XML document that carries the lot,
//! and row 10 of the KiCad parity audit is that this project wrote none.
//!
//! # Why this and not ODB++
//!
//! Recorded in the tracker on 2026-08-28 with the measurements behind it: both
//! are accepted by the one house of the two here that accepts either, IPC-2581
//! is a single document against a published schema where ODB++ is a directory
//! tree normally shipped as an archive, and IPC-2581 is published by IPC while
//! ODB++ belongs to one competitor.
//!
//! # What this writes, and what it does not
//!
//! The document's frame: who sent it, what wrote it, what units it is in, what
//! layers the board has, and the board's own outline. The features - pads,
//! tracks, vias, pours - are the bulk of the format and are the next slice;
//! this file is the part every one of them hangs off, and it is worth being
//! right before anything hangs off it.
//!
//! The section order is the schema's, not a preference: an IPC-2581 document
//! whose sections are out of order is rejected by a validator even when every
//! fact in it is true. `Content`, then `LogisticHeader`, then `HistoryRecord`,
//! then `Ecad` - and inside `Ecad`, `CadHeader` before `CadData`, and inside
//! that, every `Layer` before the `Step`.

use crate::gerber::copper::place_pad_millideg;
use cypcb_core::{Nm, Point};
use cypcb_world::components::{FootprintRef, Position, Rotation};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{BoardWorld, Layer, PadShape};
use std::collections::BTreeMap;
use std::fmt::Write;

/// Millimetres, which is what the document says its units are.
fn mm(value: Nm) -> String {
    format!("{:.3}", value.0 as f64 / 1_000_000.0)
}

/// A name the schema accepts.
///
/// `qualifiedNameType` is `([a-zA-Z][a-zA-Z0-9_\-]*)(:[a-zA-Z][a-zA-Z0-9_\-]*)*`,
/// so a board called `2-layer test` cannot be written as it stands: every
/// character outside that set becomes an underscore, and a name that does not
/// start with a letter gets one.
fn qualified(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' || character == '-' {
                character
            } else {
                '_'
            }
        })
        .collect();
    match cleaned.chars().next() {
        Some(first) if first.is_ascii_alphabetic() => cleaned,
        _ => format!("b_{cleaned}"),
    }
}

/// The name and the three things the schema wants said about each layer.
fn copper_layers(count: usize) -> Vec<(String, &'static str)> {
    let mut layers = vec![("F_Cu".to_string(), "TOP")];
    for index in 0..count.saturating_sub(2) {
        layers.push((format!("In{}_Cu", index + 1), "INTERNAL"));
    }
    layers.push(("B_Cu".to_string(), "BOTTOM"));
    layers
}

/// Write this board as an IPC-2581 document, stamped with the moment it was
/// written.
///
/// The stamp lives here rather than in the caller for the same reason every
/// other exporter's does: a fabricator asks when the files were cut, and the
/// answer should not depend on which command asked for them.
pub fn export_ipc2581_now(world: &mut BoardWorld, library: &FootprintLibrary) -> String {
    export_ipc2581(
        world,
        library,
        &chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%z").to_string(),
    )
}

/// Write this board as an IPC-2581 document.
///
/// `now` is the timestamp the document carries, passed in rather than read
/// here so a test can write the same board twice and compare the two files.
pub fn export_ipc2581(world: &mut BoardWorld, library: &FootprintLibrary, now: &str) -> String {
    let (size, stack) = world.board_info().unwrap_or((
        cypcb_world::components::BoardSize::new(Nm(0), Nm(0)),
        cypcb_world::components::LayerStack::new(2),
    ));
    let board = qualified(world.board_name().unwrap_or("board"));
    let layers = copper_layers(stack.count as usize);

    // Every pad the board has, on which copper layer, and what shape it is.
    //
    // The shapes are collected before anything is written because the format
    // wants them at the top: a pad in the features section is a reference to a
    // dictionary entry, so the dictionary has to know every shape the board
    // uses before the first pad is placed. That is the one structural
    // difference from every exporter here so far, which write geometry where
    // they meet it.
    let mut shapes: BTreeMap<String, String> = BTreeMap::new();
    let mut placed: Vec<(String, Point, i32, String)> = Vec::new();
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
            for (layer_name, side) in &layers {
                let layer = match *side {
                    "TOP" => Layer::TopCopper,
                    "BOTTOM" => Layer::BottomCopper,
                    _ => continue,
                };
                if !pad.layers.contains(&layer) {
                    continue;
                }
                let (width, height) = pad.size;
                // A round pad is a circle and everything else is drawn as the
                // rectangle it fits inside. IPC-2581 has primitives for an
                // oval and a rounded rectangle; this writes neither yet, and
                // says so rather than calling a rounded pad round.
                let (id, primitive) = match pad.shape {
                    PadShape::Circle => (
                        format!("circle_{}", mm(Nm(width.0)).replace('.', "_")),
                        format!("<Circle diameter=\"{}\"/>", mm(width)),
                    ),
                    _ => (
                        format!(
                            "rect_{}x{}",
                            mm(Nm(width.0)).replace('.', "_"),
                            mm(Nm(height.0)).replace('.', "_")
                        ),
                        format!(
                            "<RectCenter width=\"{}\" height=\"{}\"/>",
                            mm(width),
                            mm(height)
                        ),
                    ),
                };
                shapes.insert(id.clone(), primitive);
                placed.push((
                    layer_name.clone(),
                    place_pad_millideg(position, pad.position, rotation),
                    rotation,
                    id,
                ));
            }
        }
    }

    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str("<IPC-2581 revision=\"C\">\n");

    // What is in the file, and who is speaking.
    out.push_str("  <Content roleRef=\"cypcb\">\n");
    out.push_str("    <FunctionMode mode=\"FABRICATION\" level=\"1\"/>\n");
    let _ = writeln!(out, "    <StepRef name=\"{board}\"/>");
    for (name, _) in &layers {
        let _ = writeln!(out, "    <LayerRef name=\"{name}\"/>");
    }
    out.push_str("    <LayerRef name=\"Edge_Cuts\"/>\n");
    if !shapes.is_empty() {
        out.push_str("    <DictionaryStandard units=\"MILLIMETER\">\n");
        for (id, primitive) in &shapes {
            let _ = writeln!(
                out,
                "      <EntryStandard id=\"{id}\">{primitive}</EntryStandard>"
            );
        }
        out.push_str("    </DictionaryStandard>\n");
    }
    out.push_str("  </Content>\n");

    // The people the format expects. A design written in a text file has no
    // buyer and no address, and inventing either would be a fact a fabricator
    // could act on: the sender is the tool, and it says so.
    out.push_str("  <LogisticHeader>\n");
    out.push_str("    <Role id=\"cypcb\" roleFunction=\"SENDER\"/>\n");
    out.push_str("    <Enterprise id=\"cypcb\" name=\"cypcb\" code=\"NONE\"/>\n");
    out.push_str("    <Person name=\"cypcb\" enterpriseRef=\"cypcb\" roleRef=\"cypcb\"/>\n");
    out.push_str("  </LogisticHeader>\n");

    // What wrote it, and when. `SELFTEST` is the honest certification status:
    // this writer has not been through anybody's conformance suite.
    let _ = writeln!(
        out,
        "  <HistoryRecord number=\"1.0\" origination=\"{now}\" software=\"cypcb\" \
         lastChange=\"{now}\">"
    );
    out.push_str(
        "    <FileRevision fileRevisionId=\"1.0\" comment=\"written by cypcb\">\n\
         \x20     <SoftwarePackage name=\"cypcb\" vendor=\"cypcb\" revision=\"",
    );
    out.push_str(env!("CARGO_PKG_VERSION"));
    out.push_str("\">\n");
    out.push_str("        <Certification certificationStatus=\"SELFTEST\"/>\n");
    out.push_str("      </SoftwarePackage>\n");
    out.push_str("    </FileRevision>\n");
    out.push_str("  </HistoryRecord>\n");

    // The board itself.
    let _ = writeln!(out, "  <Ecad name=\"{board}\">");
    out.push_str("    <CadHeader units=\"MILLIMETER\"/>\n");
    out.push_str("    <CadData>\n");
    for (name, side) in &layers {
        let _ = writeln!(
            out,
            "      <Layer name=\"{name}\" layerFunction=\"CONDUCTOR\" side=\"{side}\" \
             polarity=\"POSITIVE\"/>"
        );
    }
    out.push_str(
        "      <Layer name=\"Edge_Cuts\" layerFunction=\"BOARD_OUTLINE\" side=\"NONE\" \
         polarity=\"POSITIVE\"/>\n",
    );

    let _ = writeln!(out, "      <Step name=\"{board}\">");
    out.push_str("        <Datum x=\"0\" y=\"0\"/>\n");
    out.push_str("        <Profile>\n          <Polygon>\n");
    // The outline the board states, or the rectangle it is.
    let outline: Vec<cypcb_core::Point> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<&cypcb_world::components::BoardOutline>();
        query
            .iter(ecs)
            .next()
            .map(|outline| outline.points.clone())
            .unwrap_or_else(|| {
                vec![
                    cypcb_core::Point::new(Nm(0), Nm(0)),
                    cypcb_core::Point::new(size.width, Nm(0)),
                    cypcb_core::Point::new(size.width, size.height),
                    cypcb_core::Point::new(Nm(0), size.height),
                ]
            })
    };
    let first = outline
        .first()
        .copied()
        .unwrap_or(cypcb_core::Point::ORIGIN);
    let _ = writeln!(
        out,
        "            <PolyBegin x=\"{}\" y=\"{}\"/>",
        mm(first.x),
        mm(first.y)
    );
    for point in outline.iter().skip(1) {
        let _ = writeln!(
            out,
            "            <PolyStepSegment x=\"{}\" y=\"{}\"/>",
            mm(point.x),
            mm(point.y)
        );
    }
    // A profile is a closed contour, so it comes back to where it began.
    let _ = writeln!(
        out,
        "            <PolyStepSegment x=\"{}\" y=\"{}\"/>",
        mm(first.x),
        mm(first.y)
    );
    out.push_str("          </Polygon>\n        </Profile>\n");

    // The copper, one section per layer. A layer with nothing on it gets no
    // section rather than an empty one: the schema wants at least one `Set`
    // inside a `LayerFeature`, so an empty section would be a document that
    // fails validation for saying nothing.
    for (layer_name, _) in &layers {
        let on_layer: Vec<&(String, Point, i32, String)> = placed
            .iter()
            .filter(|(layer, _, _, _)| layer == layer_name)
            .collect();
        if on_layer.is_empty() {
            continue;
        }
        let _ = writeln!(out, "        <LayerFeature layerRef=\"{layer_name}\">");
        out.push_str("          <Set padUsage=\"TERMINATION\">\n");
        for (_, centre, rotation, id) in on_layer {
            out.push_str("            <Pad>\n");
            // Rotation is a non-negative number in this format, and a pad
            // turned by -90 degrees is the same copper as one turned by 270.
            let turn = ((*rotation as f64 / 1000.0) % 360.0 + 360.0) % 360.0;
            let _ = writeln!(out, "              <Xform rotation=\"{turn:.3}\"/>");
            let _ = writeln!(
                out,
                "              <Location x=\"{}\" y=\"{}\"/>",
                mm(centre.x),
                mm(centre.y)
            );
            let _ = writeln!(out, "              <StandardPrimitiveRef id=\"{id}\"/>");
            out.push_str("            </Pad>\n");
        }
        out.push_str("          </Set>\n");
        out.push_str("        </LayerFeature>\n");
    }

    out.push_str("      </Step>\n");
    out.push_str("    </CadData>\n");
    out.push_str("  </Ecad>\n");
    out.push_str("</IPC-2581>\n");
    out
}
