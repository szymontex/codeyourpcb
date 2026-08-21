//! Writing a routed board back out as `.cypcb` source.
//!
//! The project's promise is that traces persist as readable code rather than
//! as a binary the user cannot inspect. This is the half that makes that true:
//! a routed `BoardWorld` becomes `trace` and `via` blocks a designer can read,
//! edit and put under version control.
//!
//! Lives here rather than in the WASM engine because the command line needs it
//! too, and one implementation is the point.
//!
//! # What this cannot say yet
//!
//! The writer can only be as complete as the language. Two things a `Via`
//! carries have no syntax, so they are rebuilt from defaults on the way back
//! in rather than read:
//!
//! - **its outer diameter.** Rebuilt as twice the drill, which is what both
//!   the router and `sync` assume, so nothing is lost today and a via with a
//!   deliberate ring would be. Proposed syntax when it matters:
//!   `via 10mm,12mm drill 0.3mm diameter 0.6mm`.
//!
//! Which layers a via joins is written now - `layers Top to Inner1` - and left
//! off when the via goes through, which is what a via with no stated pair
//! means.

use std::collections::BTreeMap;
use std::fmt::Write;

use crate::components::trace::{Trace, Via};
use crate::components::Layer;
use crate::world::BoardWorld;

/// Split a trace's segments into runs that actually join end to end.
///
/// Returns one slice per chain. A segment whose start is not the previous
/// segment's end begins a new one.
fn contiguous_runs(
    segments: &[crate::components::trace::TraceSegment],
) -> Vec<&[crate::components::trace::TraceSegment]> {
    let mut runs = Vec::new();
    let mut start = 0usize;
    for i in 1..segments.len() {
        if segments[i].start != segments[i - 1].end {
            runs.push(&segments[start..i]);
            start = i;
        }
    }
    if start < segments.len() {
        runs.push(&segments[start..]);
    }
    runs
}

/// Format millimetres so a round trip is exact.
///
/// Six decimal places is 1nm resolution, which guarantees nm -> mm string ->
/// parse -> nm returns the value it started with.
fn format_mm(mm: f64) -> String {
    format!("{:.6}", mm)
}

/// Render every trace and via on the board as `.cypcb` source.
///
/// Returns an empty string for a board with neither, so a caller can tell
/// "nothing routed" from "here is the routing".
/// Whether a name can be written into the language and read back out.
///
/// The grammar's `identifier` is `[a-zA-Z_][a-zA-Z0-9_]*` and there is no
/// quoted form, so a net called `VBUS+`, `D+` or `D-` - which is every USB
/// design there is - cannot be named at all. `from-kicad` interns whatever a
/// KiCad file carries, so a world can hold such a net even though a designer
/// could never have typed one.
///
/// Writing it anyway produces a file this project's own parser rejects, which
/// on the viewer's save path means work the user cannot reopen. Until the
/// grammar grows a quoted form, the writer omits what it cannot spell and says
/// so in the file - the same choice already made for copper pours a few lines
/// down, and for the same reason: a stated gap beats a silent one.
pub(crate) fn is_writable_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphabetic() || first == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// A pad name written so the readers get back what the model holds.
///
/// A bare number and a bare identifier are written as they are. Anything else
/// needs quotes, which is the form the grammar accepts for exactly this case:
/// `A1+`, a name starting with a digit and carrying a letter, a name with a
/// space in it. Writing such a name bare would produce a file this project
/// cannot read back, which is the round trip failing silently.
pub(crate) fn pad_name_as_written(name: &str) -> String {
    let bare_number = !name.is_empty() && name.chars().all(|c| c.is_ascii_digit());
    if bare_number || is_writable_identifier(name) {
        name.to_string()
    } else {
        format!("{:?}", name)
    }
}

/// A net name written so the readers get back what the model holds.
///
/// The same question a pad name asks, with one difference: a net is never a
/// bare number, so an identifier is the only unquoted form. `VBUS+`, `3V3` and
/// `D-` all come back quoted, which is what the grammar accepts and what
/// `net_name` was added for.
pub(crate) fn net_name_as_written(name: &str) -> String {
    if is_writable_identifier(name) {
        name.to_string()
    } else {
        format!("{:?}", name)
    }
}

pub fn traces_as_dsl(world: &mut BoardWorld) -> String {
    // Collect all traces grouped by net name
    let trace_data: Vec<Trace> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<&Trace>();
        query.iter(ecs).cloned().collect()
    };

    // Collect all vias grouped by net name
    let via_data: Vec<Via> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<&Via>();
        query.iter(ecs).copied().collect()
    };

    if trace_data.is_empty() && via_data.is_empty() {
        return String::new();
    }

    // Group traces by net name, using BTreeMap for deterministic ordering
    let mut net_traces: BTreeMap<String, Vec<&Trace>> = BTreeMap::new();
    let mut net_vias: BTreeMap<String, Vec<&Via>> = BTreeMap::new();

    for trace in &trace_data {
        let net_name = world
            .net_name(trace.net_id)
            .unwrap_or("unknown")
            .to_string();
        net_traces.entry(net_name).or_default().push(trace);
    }

    for via in &via_data {
        let net_name = world.net_name(via.net_id).unwrap_or("unknown").to_string();
        net_vias.entry(net_name).or_default().push(via);
    }

    // Collect all net names
    let mut all_nets: Vec<String> = net_traces.keys().cloned().collect();
    for net in net_vias.keys() {
        if !all_nets.contains(net) {
            all_nets.push(net.clone());
        }
    }
    all_nets.sort();

    // Every net is written. A name the identifier rule refuses is quoted
    // rather than dropped: the language grew `net_name` for exactly this, so
    // the copper on `VBUS+` no longer has to be left behind to keep the file
    // readable. This used to filter those nets out and print a comment naming
    // them, which was the honest answer while the grammar had no quoted form.

    let mut output = String::with_capacity(4096);

    for net_name in &all_nets {
        let traces = net_traces.get(net_name);
        let vias = net_vias.get(net_name);

        if traces.is_none() && vias.is_none() {
            continue;
        }

        // For each trace on this net, emit a separate trace block
        if let Some(traces) = traces {
            for trace in traces {
                let _ = writeln!(output, "trace {} {{", net_name_as_written(net_name));

                // Layer
                let layer_str = match trace.layer {
                    Layer::TopCopper => "Top",
                    Layer::BottomCopper => "Bottom",
                    // One-based in the language, zero-based in the model:
                    // `Layer::Inner(0)` is the first inner layer and the
                    // grammar calls it `Inner1`. Writing the raw number
                    // produced `layer Inner0`, which does not parse, so a
                    // routed four-layer board could not be read back at all.
                    Layer::Inner(n) => {
                        let _ = writeln!(output, "    layer Inner{}", n + 1);
                        ""
                    }
                    _ => "Top",
                };
                if !layer_str.is_empty() {
                    let _ = writeln!(output, "    layer {}", layer_str);
                }

                // Width
                let width_mm = trace.width.to_mm();
                let _ = writeln!(output, "    width {}mm", format_mm(width_mm));

                // Path - one polyline per contiguous run of segments.
                //
                // A `Trace` holds every segment a net has on a layer, and a net
                // with more than two pads branches: the segment list is a set
                // of chains, not one chain. Writing it as a single `path`
                // draws a straight line from the end of one branch to the
                // start of the next, which is copper that was never routed.
                // Measured on examples/blink.cypcb: 2 DRC violations in the
                // routed board, 13 in the file it was written to.
                for run in contiguous_runs(&trace.segments) {
                    let _ = write!(output, "    path ");
                    let first = run[0];
                    let _ = write!(
                        output,
                        "{}mm,{}mm",
                        format_mm(first.start.x.to_mm()),
                        format_mm(first.start.y.to_mm())
                    );
                    for seg in run {
                        let _ = write!(
                            output,
                            " -> {}mm,{}mm",
                            format_mm(seg.end.x.to_mm()),
                            format_mm(seg.end.y.to_mm())
                        );
                    }
                    let _ = writeln!(output);
                }

                // Locked
                if trace.locked {
                    let _ = writeln!(output, "    locked");
                }

                let _ = writeln!(output, "}}");
                let _ = writeln!(output);
            }
        }

        // Vias that are not associated with a trace (standalone vias)
        // These get their own trace block
        if let Some(vias) = vias {
            for via in vias {
                let _ = writeln!(output, "trace {} {{", net_name_as_written(net_name));
                let _ = writeln!(
                    output,
                    "    via {}mm,{}mm drill {}mm{}",
                    format_mm(via.position.x.to_mm()),
                    format_mm(via.position.y.to_mm()),
                    format_mm(via.drill.to_mm()),
                    via_span_suffix(via)
                );
                if via.locked {
                    let _ = writeln!(output, "    locked");
                }
                let _ = writeln!(output, "}}");
                let _ = writeln!(output);
            }
        }
    }

    // Trim trailing newline
    while output.ends_with('\n') {
        output.pop();
    }
    // Add exactly one trailing newline
    output.push('\n');

    output
}

/// ` layers Top to Inner1`, or nothing at all for a via that goes through.
///
/// Written only when it says something: a through via with the pair spelled
/// out is noise in every file, and its absence already means through.
fn via_span_suffix(via: &crate::components::trace::Via) -> String {
    use crate::components::Layer;

    if via.start_layer == Layer::TopCopper && via.end_layer == Layer::BottomCopper {
        return String::new();
    }
    format!(
        " layers {} to {}",
        layer_keyword(via.start_layer),
        layer_keyword(via.end_layer)
    )
}

/// A layer as a `trace` block writes it.
fn layer_keyword(layer: crate::components::Layer) -> String {
    use crate::components::Layer;

    match layer {
        Layer::TopCopper => "Top".to_string(),
        Layer::BottomCopper => "Bottom".to_string(),
        Layer::Inner(n) => format!("Inner{}", n + 1),
        other => format!("{other:?}"),
    }
}

/// The kind word the language wants for a part, taken from its reference.
///
/// KiCad does not record what a part *is*. A board file has a reference
/// designator, a value and a footprint name, and nothing that says "resistor" -
/// so an import either states a kind or the file it writes will not parse.
///
/// The reference designator prefix is where that lives, and it is a convention
/// rather than a guess: `R` for resistors, `C` for capacitors, `U` for
/// integrated circuits, and so on, the same letters every schematic has used
/// for decades. Where the prefix is not one of them the answer is `generic`,
/// which the language has for exactly this - a part whose kind nobody stated.
/// Inventing one from the footprint name would be inventing a fact about the
/// board.
pub fn kind_from_refdes(refdes: &str) -> &'static str {
    let letters: String = refdes
        .chars()
        .take_while(|c| c.is_ascii_alphabetic())
        .flat_map(char::to_uppercase)
        .collect();
    match letters.as_str() {
        "R" | "RN" | "RV" | "VR" => "resistor",
        "C" | "CP" => "capacitor",
        "L" | "FB" => "inductor",
        "U" | "IC" => "ic",
        "LED" | "DS" => "led",
        "D" | "CR" => "diode",
        "Q" | "T" => "transistor",
        "J" | "P" | "CN" | "CON" => "connector",
        "Y" | "X" | "XTAL" => "crystal",
        _ => "generic",
    }
}

/// Quote a string the way the language reads one back.
fn quoted(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Render a whole board as `.cypcb` source.
///
/// The counterpart to `cypcb to-kicad`, and the half that makes a KiCad board
/// something you can edit rather than only something the tools will accept: a
/// `BoardWorld` - whatever it was read from - becomes the text a person keeps
/// under version control.
///
/// # What this writes, and what it cannot
///
/// Written: the board and its layer count, a declared outline when the board is
/// not the rectangle its size describes, a `footprint` definition for every
/// footprint the design uses that is not built in, one `component` per part
/// with its value, placement, rotation and side, a `net` block per net listing
/// the pins on it, and the routed copper through [`traces_as_dsl`].
///
/// Not written, because the language has no syntax for it: copper pours and
/// keepouts. A board carrying them loses them here, and the writer says so in
/// a comment at the top of the file rather than dropping them silently.
pub fn board_as_dsl(world: &mut BoardWorld) -> String {
    use crate::components::{
        FootprintRef, NetConnections, Position, RefDes, Rotation, Side, Value,
    };

    let (size, stack) = world
        .board_info()
        .unwrap_or((crate::BoardSize::default(), Default::default()));

    // The parts, read out before anything borrows the world again.
    struct Part {
        refdes: String,
        footprint: String,
        value: String,
        position: cypcb_core::Point,
        rotation: i32,
        on_bottom: bool,
        connections: Option<NetConnections>,
    }
    let mut parts: Vec<Part> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<(
            &RefDes,
            &Position,
            &Rotation,
            &FootprintRef,
            Option<&Value>,
            Option<&NetConnections>,
            Option<&Side>,
        )>();
        query
            .iter(ecs)
            .map(
                |(refdes, position, rotation, footprint, value, connections, side)| Part {
                    refdes: refdes.0.clone(),
                    footprint: footprint.0.clone(),
                    value: value.map(|v| v.0.clone()).unwrap_or_default(),
                    position: position.0,
                    rotation: rotation.0,
                    on_bottom: matches!(side, Some(Side::Bottom)),
                    connections: connections.cloned(),
                },
            )
            .collect()
    };
    // Written in the order a person reads them, not in whatever order the ECS
    // happens to hold: a diff between two imports of the same board should be
    // empty, and entity order is not a promise the ECS makes.
    parts.sort_by(|a, b| a.refdes.cmp(&b.refdes));

    let zone_count = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<&crate::components::zone::Zone>();
        query.iter(ecs).count()
    };

    let outline: Option<Vec<cypcb_core::Point>> = world
        .board_entity()
        .and_then(|entity| world.ecs().get::<crate::components::BoardOutline>(entity))
        .map(|outline| outline.points.clone());

    // The fabricator the design named, carried straight back out. A board that
    // named none writes none: this writer's job is to return what it was given,
    // and inventing a fab here would make every round trip claim a choice the
    // source never made.
    let fab: Option<String> = world.fab().map(|fab| fab.to_string());

    // The layers a fabricator is expected to press together, likewise carried
    // straight back out. This block was read, checked against the layer count
    // and then dropped on the way out, so a design that said how it wanted to
    // be built lost that on its first save through the editor - and the number
    // it feeds, `Stackup::total_thickness`, is the depth every plated hole is
    // drilled through and the figure a fab quotes against.
    let stackup: Option<crate::components::Stackup> = world.stackup().cloned();

    let name = world.board_name().unwrap_or("board").to_string();
    let safe_name: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();

    let mut out = String::new();
    if zone_count > 0 {
        let _ = writeln!(
            out,
            "// {zone_count} copper pour(s) on the source board are not written: the language\n\
             // has no syntax for one yet, so they would be invented rather than kept."
        );
        let _ = writeln!(out);
    }
    let _ = writeln!(out, "version 1");
    let _ = writeln!(out);
    let _ = writeln!(out, "board {safe_name} {{");
    let _ = writeln!(
        out,
        "    size {}mm x {}mm",
        format_mm(size.width.0 as f64 / 1e6),
        format_mm(size.height.0 as f64 / 1e6)
    );
    let _ = writeln!(out, "    layers {}", stack.count.max(2));
    if let Some(stackup) = &stackup {
        let _ = writeln!(out, "    stackup {{");
        for layer in &stackup.layers {
            // A layer that stated no thickness is written without one. The
            // alternative - filling in a plausible foil or prepreg - would
            // turn a gap in the design into a number the fab is quoted on.
            match layer.thickness {
                Some(thickness) => {
                    let _ = writeln!(
                        out,
                        "        {} {}mm",
                        layer.kind.as_str(),
                        format_mm(thickness.0 as f64 / 1e6)
                    );
                }
                None => {
                    let _ = writeln!(out, "        {}", layer.kind.as_str());
                }
            }
        }
        let _ = writeln!(out, "    }}");
    }
    if let Some(fab) = fab {
        let _ = writeln!(out, "    fab {fab}");
    }
    let _ = writeln!(out, "}}");

    // An outline is only worth stating when the board is not the rectangle its
    // size already describes.
    if let Some(points) = outline {
        let corners = [
            cypcb_core::Point::new(cypcb_core::Nm(0), cypcb_core::Nm(0)),
            cypcb_core::Point::new(size.width, cypcb_core::Nm(0)),
            cypcb_core::Point::new(size.width, size.height),
            cypcb_core::Point::new(cypcb_core::Nm(0), size.height),
        ];
        let is_plain_rectangle = points.len() == 4 && points.iter().all(|p| corners.contains(p));
        if !is_plain_rectangle && points.len() >= 3 {
            let _ = writeln!(out);
            let _ = writeln!(out, "outline {{");
            for point in &points {
                let _ = writeln!(
                    out,
                    "    point {}mm, {}mm",
                    format_mm(point.x.0 as f64 / 1e6),
                    format_mm(point.y.0 as f64 / 1e6)
                );
            }
            let _ = writeln!(out, "}}");
        }
    }

    // Footprint definitions, for every footprint the board uses that a fresh
    // library does not already have. A KiCad board names parts things like
    // `Package_QFP:LQFP-48_7x7mm_P0.5mm`, which no built-in library has, so
    // without this the file names pads nobody can resolve.
    // A footprint definition takes a bare identifier - `footprint USB_ANCHOR {`
    // - and a KiCad footprint is named `cypcb:USB_ANCHOR` or
    // `Package_QFP:LQFP-48_7x7mm_P0.5mm`, which is neither bare nor an
    // identifier. The library prefix goes, and anything the grammar will not
    // take becomes an underscore. Names that collide after that are given a
    // number rather than silently merged, because two footprints wearing one
    // name is two parts with the wrong pads.
    fn as_identifier(name: &str) -> String {
        let bare = name.rsplit(':').next().unwrap_or(name);
        let mut out: String = bare
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        if out.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            out.insert(0, '_');
        }
        if out.is_empty() {
            out.push_str("FOOTPRINT");
        }
        out
    }

    let builtin = crate::footprint::FootprintLibrary::new();
    let library = world.footprints().clone();
    let mut used: Vec<String> = parts.iter().map(|p| p.footprint.clone()).collect();
    used.sort();
    used.dedup();
    // What each footprint is called in the written file, so the components
    // below name exactly what the definitions above declare.
    let mut written_name: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    let mut taken: std::collections::HashSet<String> = std::collections::HashSet::new();
    for name in &used {
        if builtin.contains(name) {
            written_name.insert(name.clone(), name.clone());
            continue;
        }
        let Some(footprint) = library.get(name) else {
            written_name.insert(name.clone(), name.clone());
            continue;
        };
        let mut identifier = as_identifier(name);
        if !taken.insert(identifier.clone()) {
            let mut n = 2;
            while !taken.insert(format!("{identifier}_{n}")) {
                n += 1;
            }
            identifier = format!("{identifier}_{n}");
        }
        written_name.insert(name.clone(), identifier.clone());
        let _ = writeln!(out);
        let _ = writeln!(out, "footprint {identifier} {{");
        let (cw, ch) = (footprint.courtyard.width(), footprint.courtyard.height());
        let _ = writeln!(
            out,
            "    courtyard {}mm x {}mm",
            format_mm(cw.0 as f64 / 1e6),
            format_mm(ch.0 as f64 / 1e6)
        );
        for pad in &footprint.pads {
            let shape = match pad.shape {
                crate::components::PadShape::Circle => "circle",
                crate::components::PadShape::Rect => "rect",
                crate::components::PadShape::RoundRect { .. } => "roundrect",
                crate::components::PadShape::Oblong => "oblong",
            };
            let _ = write!(
                out,
                "    pad {} {shape} at {}mm, {}mm size {}mm x {}mm",
                pad_name_as_written(&pad.number),
                format_mm(pad.position.x.0 as f64 / 1e6),
                format_mm(pad.position.y.0 as f64 / 1e6),
                format_mm(pad.size.0 .0 as f64 / 1e6),
                format_mm(pad.size.1 .0 as f64 / 1e6)
            );
            if let Some(drill) = pad.drill {
                match pad.slot {
                    Some((w, h)) if w != h => {
                        let _ = write!(
                            out,
                            " drill {}mm x {}mm",
                            format_mm(w.0 as f64 / 1e6),
                            format_mm(h.0 as f64 / 1e6)
                        );
                    }
                    _ => {
                        let _ = write!(out, " drill {}mm", format_mm(drill.0 as f64 / 1e6));
                    }
                }
            }
            let _ = writeln!(out);
        }
        let _ = writeln!(out, "}}");
    }

    for part in &parts {
        let _ = writeln!(out);
        let _ = writeln!(
            out,
            "component {} {} {} {{",
            part.refdes,
            kind_from_refdes(&part.refdes),
            quoted(written_name.get(&part.footprint).unwrap_or(&part.footprint))
        );
        if !part.value.is_empty() {
            let _ = writeln!(out, "    value {}", quoted(&part.value));
        }
        let _ = writeln!(
            out,
            "    at {}mm, {}mm",
            format_mm(part.position.x.0 as f64 / 1e6),
            format_mm(part.position.y.0 as f64 / 1e6)
        );
        if part.rotation != 0 {
            let _ = writeln!(out, "    rotate {}", part.rotation as f64 / 1000.0);
        }
        if part.on_bottom {
            let _ = writeln!(out, "    side bottom");
        }
        let _ = writeln!(out, "}}");
    }

    // Nets, each listing the pins on it. Read off the parts rather than out of
    // the net table, because a net nothing connects to is a name with no
    // meaning in this language and would write an empty block.
    // Keyed by the raw id: `NetId` is not `Ord`, and the map is only ever a
    // lookup here so the ordering it would give is not wanted either.
    let net_names: std::collections::HashMap<u32, String> = world
        .nets()
        .map(|(id, name)| (id.0, name.to_string()))
        .collect();
    let mut pins_by_net: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for part in &parts {
        let Some(connections) = &part.connections else {
            continue;
        };
        for connection in connections.iter() {
            let Some(name) = net_names.get(&connection.net.0) else {
                continue;
            };
            pins_by_net
                .entry(name.clone())
                .or_default()
                .push(format!("{}.{}", part.refdes, connection.pin));
        }
    }
    // The second of the two places that used to drop a net for want of a
    // spelling. The trace writer was the other, and a fix to one alone left
    // the file with `trace "VBUS+"` and no `net "VBUS+"` above it - which
    // parses and then fails to sync with `MissingNet`, a worse outcome than
    // either whole answer.
    for (net, mut pins) in pins_by_net {
        pins.sort();
        pins.dedup();
        let _ = writeln!(out);
        let _ = writeln!(out, "net {} {{", net_name_as_written(&net));
        for pin in pins {
            let _ = writeln!(out, "    {pin}");
        }
        let _ = writeln!(out, "}}");
    }

    let traces = traces_as_dsl(world);
    if !traces.is_empty() {
        let _ = writeln!(out);
        out.push_str(&traces);
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use cypcb_core::Nm;

    #[test]
    fn millimetres_round_trip_exactly() {
        // The DSL is the storage format, so a value written out and read back
        // has to be the value that went in - at 1nm resolution, six decimals.
        for nm in [Nm(1), Nm(127_000), Nm(1_000_000), Nm(99_999_999)] {
            let text = format_mm(nm.to_mm());
            let parsed: f64 = text.parse().expect("a number");
            assert_eq!(
                Nm((parsed * 1_000_000.0).round() as i64),
                nm,
                "{text} did not come back as {nm:?}"
            );
        }
    }
}
