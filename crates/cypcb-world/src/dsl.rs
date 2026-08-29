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

/// A number as the language would read it back: `10`, `0.5`, `1.27`.
fn format_number(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{value:.0}")
    } else {
        let text = format!("{value:.6}");
        text.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// One operand of an assertion, or `None` when it cannot be spelled.
fn assert_operand_as_written(operand: &cypcb_parser::ast::AssertOperand) -> Option<String> {
    use cypcb_parser::ast::AssertOperand;
    match operand {
        AssertOperand::QualifiedName { parts, .. } => Some(parts.join(".")),
        AssertOperand::Physical(value) => {
            // A tolerance is part of what a value states and has no form in an
            // assertion, so a value carrying one is not written back.
            value
                .tolerance
                .is_none()
                .then(|| format!("{}{}", format_number(value.value), value.unit))
        }
        AssertOperand::Dimension(dimension) => Some(format!(
            "{}{}",
            format_number(dimension.value),
            dimension.unit
        )),
        AssertOperand::Number { value, .. } => Some(format_number(*value)),
    }
}

/// One assertion as a statement, or `None` when a half of it cannot be spelled.
fn assert_as_written(expression: &cypcb_parser::ast::AssertExpression) -> Option<String> {
    use cypcb_parser::ast::AssertExpression;
    match expression {
        AssertExpression::Comparison {
            left, op, right, ..
        } => Some(format!(
            "assert {} {op} {}",
            assert_operand_as_written(left)?,
            assert_operand_as_written(right)?
        )),
        AssertExpression::Within { left, target, .. } => target.tolerance.is_none().then(|| {
            format!(
                "assert {} within {}{}",
                assert_operand_as_written(left).unwrap_or_default(),
                format_number(target.value),
                target.unit
            )
        }),
    }
}

/// The constraint block for a net that asks for something, or nothing at all.
///
/// Written in the units the grammar reads: millimetres for a width, a
/// clearance and both halves of a neck, milliamps for a current, ohms for a
/// target impedance - which the model keeps in hundredths, so it comes back
/// out as a decimal.
fn net_constraints_as_written(asks: &crate::registry::NetConstraints) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(width) = asks.width {
        parts.push(format!("width {}mm", format_mm(width.to_mm())));
    }
    if let Some(clearance) = asks.clearance {
        parts.push(format!("clearance {}mm", format_mm(clearance.to_mm())));
    }
    if let Some(current) = asks.current_ma {
        parts.push(format!("current {current}mA"));
    }
    if let Some(impedance) = asks.impedance_ohms_x100 {
        parts.push(format!("impedance {}ohm", f64::from(impedance) / 100.0));
    }
    if let Some(neck) = asks.neck {
        parts.push(format!(
            "neck {}mm for {}mm",
            format_mm(neck.width.to_mm()),
            format_mm(neck.length.to_mm())
        ));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!(" [{}]", parts.join(" "))
    }
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
    // The neck comes with the trace. It is a separate component, and reading
    // the two in separate passes would pair them by iteration order - which is
    // not an order this project relies on anywhere else and should not start
    // relying on here.
    // The curve comes with it too, for the same reason: a trace spawned from
    // an `arc` is a dozen chords in the world, and writing those back turns
    // one sentence into twelve - and flattens the flattening on the next save.
    type WrittenTrace = (
        Trace,
        Option<crate::components::trace::TraceNeck>,
        Option<crate::components::trace::Curve>,
    );
    let trace_data: Vec<WrittenTrace> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<(
            &Trace,
            Option<&crate::components::trace::TraceNeck>,
            Option<&crate::components::trace::Curve>,
        )>();
        query
            .iter(ecs)
            .map(|(trace, neck, curve)| (trace.clone(), neck.copied(), curve.copied()))
            .collect()
    };

    // Collect all vias grouped by net name, except the ones a stitched pour
    // produced: those are what its `stitch` line means, and writing them back
    // as copper would turn one rule into a hundred holes - and stitch the
    // stitching on the next trip through.
    let via_data: Vec<Via> = {
        let ecs = world.ecs_mut();
        let mut query =
            ecs.query_filtered::<&Via, bevy_ecs::prelude::Without<crate::components::Stitched>>();
        query.iter(ecs).copied().collect()
    };

    if trace_data.is_empty() && via_data.is_empty() {
        return String::new();
    }

    // Group traces by net name, using BTreeMap for deterministic ordering
    type NeckedTrace<'a> = (
        &'a Trace,
        Option<crate::components::trace::TraceNeck>,
        Option<crate::components::trace::Curve>,
    );
    let mut net_traces: BTreeMap<String, Vec<NeckedTrace<'_>>> = BTreeMap::new();
    let mut net_vias: BTreeMap<String, Vec<&Via>> = BTreeMap::new();

    for (trace, neck, curve) in &trace_data {
        let net_name = world
            .net_name(trace.net_id)
            .unwrap_or("unknown")
            .to_string();
        net_traces
            .entry(net_name)
            .or_default()
            .push((trace, *neck, *curve));
    }

    // A net's traces are written in an order of their own rather than the
    // order the world hands them over. Bevy iterates archetypes, and a trace
    // carrying a curve sits in a different one from a plain trace - so a board
    // read from a saved file gave its blocks back in a different order and the
    // second save differed from the first. A file that changes when nothing
    // changed is a file nobody can keep in version control.
    for traces in net_traces.values_mut() {
        traces.sort_by_key(|(trace, _, _)| {
            let start = trace
                .segments
                .first()
                .map(|segment| (segment.start.x.0, segment.start.y.0))
                .unwrap_or((0, 0));
            (format!("{:?}", trace.layer), start.0, start.1)
        });
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
            for (trace, neck, curve) in traces {
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
                // A curve is written as the curve it was. The chords are what
                // the checker reads; the sentence is what a person edits, and
                // a saved file has to hold both without one becoming the other.
                if let Some(curve) = curve {
                    let start = trace
                        .segments
                        .first()
                        .map(|segment| segment.start)
                        .unwrap_or(curve.centre);
                    let _ = write!(
                        output,
                        "    arc start {}mm,{}mm centre {}mm,{}mm sweep {}",
                        format_mm(start.x.to_mm()),
                        format_mm(start.y.to_mm()),
                        format_mm(curve.centre.x.to_mm()),
                        format_mm(curve.centre.y.to_mm()),
                        format_number(curve.sweep_millideg.abs() as f64 / 1000.0)
                    );
                    // Counter-clockwise is the direction with no word on it.
                    if curve.sweep_millideg < 0 {
                        let _ = write!(output, " clockwise");
                    }
                    let _ = writeln!(output);
                }

                // A curve was written above; the chords it became are not
                // written again. `continue` here would skip the rest of the
                // block - the neck, the `locked` line and the closing brace -
                // and the first version of this did exactly that: the saved
                // file ran two `trace` blocks together and would not parse.
                for run in contiguous_runs(&trace.segments)
                    .into_iter()
                    .filter(|_| curve.is_none())
                {
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

                // The neck, so a board written down can be read back as the
                // board it was. Without this line a routed file lost the
                // declaration and, on reload, the geometry: `apply_neck` draws
                // the thin stretch from the declaration, and a file with the
                // path but not the `neck` reloads as uniform copper.
                if let Some(neck) = neck {
                    let _ = writeln!(
                        output,
                        "    neck {}mm for {}mm",
                        format_mm(neck.width.to_mm()),
                        format_mm(neck.length.to_mm())
                    );
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

/// A `zone` or `keepout` block, or a comment saying why there is neither.
///
/// Two things can stop a zone being written, and each says so in the file
/// rather than disappearing:
///
/// - a layer mask that is not exactly top, exactly bottom or every layer. The
///   language has three words for a zone's layer and a mask has thirty-two
///   bits, so an inner-layer pour - which `from-kicad` builds whenever a
///   four-layer board has one - cannot be spelled. Writing `all` instead would
///   move copper onto layers the design never put it on, and for a keepout it
///   would forbid copper where the design allowed it.
/// - a name the grammar's `identifier` refuses.
///
/// A pour whose net needs quoting used to be a third, and a pour *named* after
/// such a net was a fourth: the grammar took an identifier for a zone name, so
/// a `GND` pour came through and a `VBUS+` pour was described in a comment.
/// Both the net and the name take `net_name` now, which is the rule that knows
/// about quotes.
fn zone_as_dsl(
    zone: &crate::components::zone::Zone,
    stitch: Option<cypcb_core::Nm>,
    radius: Option<cypcb_core::Nm>,
    net_names: &std::collections::HashMap<u32, String>,
) -> String {
    use crate::components::zone::ZoneKind;

    let mut out = String::new();
    let what = match zone.kind {
        ZoneKind::Keepout => "keepout",
        ZoneKind::CopperPour => "zone",
        ZoneKind::Flex => "flex",
        ZoneKind::Region => "region",
    };

    let layer = match zone.layer_mask {
        0b01 => "top",
        0b10 => "bottom",
        0xFFFF_FFFF => "all",
        mask => {
            let _ = writeln!(
                out,
                "// one {what} on layer mask {mask:#b} is not written: the language says \
                 top, bottom or all, and this is none of the three"
            );
            return out;
        }
    };

    let net = zone
        .net
        .and_then(|net_id| net_names.get(&net_id.0))
        .map(|name| net_name_as_written(name));

    match &zone.name {
        Some(name) => {
            let _ = writeln!(out, "{what} {} {{", net_name_as_written(name));
        }
        None => {
            let _ = writeln!(out, "{what} {{");
        }
    }
    let _ = writeln!(
        out,
        "    bounds {}mm, {}mm to {}mm, {}mm",
        format_mm(zone.bounds.min.x.to_mm()),
        format_mm(zone.bounds.min.y.to_mm()),
        format_mm(zone.bounds.max.x.to_mm()),
        format_mm(zone.bounds.max.y.to_mm())
    );
    let _ = writeln!(out, "    layer {layer}");
    if let Some(net) = net {
        let _ = writeln!(out, "    net {net}");
    }
    // The pitch, not the vias. A stitched pour states a rule and the vias are
    // what the rule produces; writing them out as copper would turn a request
    // into a hundred holes the next reader cannot tell from hand-placed ones.
    if let Some(pitch) = stitch {
        let _ = writeln!(out, "    stitch {}mm", format_mm(pitch.to_mm()));
    }
    // How tightly the board is folded here, when the design says. A save that
    // dropped it would hand back a file whose bend the checker can no longer
    // measure against the stack.
    if let Some(radius) = radius {
        let _ = writeln!(out, "    radius {}mm", format_mm(radius.to_mm()));
    }
    let _ = writeln!(out, "}}");
    let _ = writeln!(out);
    out
}

/// The raw id of every net that has copper on it, trace or via.
///
/// The writer emits a `trace` block per net that carries copper, whether or
/// not any pin is on that net, so this is the set of names the file has to
/// declare on top of the ones the parts imply.
fn copper_nets(world: &mut BoardWorld) -> Vec<u32> {
    let mut ids: Vec<u32> = {
        let ecs = world.ecs_mut();
        let mut traces = ecs.query::<&Trace>();
        traces.iter(ecs).map(|trace| trace.net_id.0).collect()
    };
    let ecs = world.ecs_mut();
    let mut vias = ecs.query::<&Via>();
    ids.extend(vias.iter(ecs).map(|via| via.net_id.0));
    ids.sort_unstable();
    ids.dedup();
    ids
}

/// A `stackup` block as a `board` block writes it, indented to sit inside one.
///
/// Pulled out of `board_as_dsl` so that a test needing this stack as *source
/// text* gets the same words the writer emits. `cypcb-fixtures` holds a stack
/// whose four copper layers all answer differently - the shape three shipped
/// index errors needed and did not have - and it held it only as a
/// `Stackup` value, which a test driving the command line cannot use. The
/// alternative was a second formatter in the fixture crate, and two
/// formatters for one block is how the spelling of a layer came to disagree
/// with itself.
/// A ratio as the language writes it: `0.5`, not `0.5000000`.
fn format_ratio(value: f64) -> String {
    let mut text = format!("{value:.4}");
    while text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.push('0');
    }
    text
}

pub fn stackup_as_dsl(stackup: &crate::components::Stackup) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "    stackup {{");
    // What the fabricator does to the board, before what it presses: a reader
    // looking for the finish should not have to walk eleven layer lines first.
    if let Some(finish) = &stackup.finish {
        let _ = writeln!(out, "        finish {}", quoted(finish));
    }
    if stackup.edges_plated {
        let _ = writeln!(out, "        edges plated");
    }
    if stackup.castellated_pads {
        let _ = writeln!(out, "        pads castellated");
    }
    if let Some(connector) = stackup.edge_connector {
        let word = match connector {
            crate::components::EdgeConnector::Plain => "plain",
            crate::components::EdgeConnector::Bevelled => "bevelled",
        };
        let _ = writeln!(out, "        connector {word}");
    }
    if stackup.impedance_controlled {
        let _ = writeln!(out, "        impedance controlled");
    }
    for pair in &stackup.drill_pairs {
        let _ = writeln!(out, "        drill {} to {}", pair.start, pair.end);
    }
    for layer in &stackup.layers {
        // A layer that stated no thickness is written without one. The
        // alternative - filling in a plausible foil or prepreg - would
        // turn a gap in the design into a number the fab is quoted on.
        let mut line = format!("        {}", layer.kind.as_str());
        if let Some(name) = &layer.name {
            let _ = write!(line, " {}", quoted(name));
        }
        if let Some(thickness) = layer.thickness {
            // In the unit the design wrote, when it wrote one. A stackup that
            // says `copper 1oz` is quoting the fab table it was written
            // against, and answering `copper 0.034998mm` hands the fabricator
            // arithmetic instead of the order they asked for.
            match layer.written_as {
                Some(unit) if unit != cypcb_core::Unit::Mm => {
                    // Not `format_mm`: six decimals is 1nm resolution in
                    // millimetres and noise in any other unit - `1oz` would
                    // come back as `1.000000oz`. `f64`'s own Display prints
                    // the shortest text that reads back as the same number.
                    let _ = write!(line, " {}{}", unit.from_nm(thickness), unit);
                }
                _ => {
                    let _ = write!(line, " {}mm", format_mm(thickness.0 as f64 / 1e6));
                }
            }
        }
        if let Some(material) = &layer.material {
            let _ = write!(line, " material {}", quoted(material));
        }
        if let Some(color) = &layer.color {
            let _ = write!(line, " color {}", quoted(color));
        }
        // `f64`'s own Display prints the shortest text that reads back as
        // the same number, so 4500 thousandths comes out `4.5` and 8900
        // millionths comes out `0.0089` - and both round back to the
        // integer they left as.
        if let Some(dk) = layer.dk_x1000 {
            let _ = write!(line, " dk {}", f64::from(dk) / 1_000.0);
        }
        if let Some(df) = layer.df_x1000000 {
            let _ = write!(line, " df {}", f64::from(df) / 1_000_000.0);
        }
        // Where the layer stops, when it does not run the whole panel. Before
        // the sheets, the way the grammar reads it: a sheet belongs to the
        // slot and the coverage belongs to the layer.
        match &layer.coverage {
            Some(crate::components::LayerCoverage::Only(region)) => {
                let _ = write!(line, " covers {}", net_name_as_written(region));
            }
            Some(crate::components::LayerCoverage::Outside(region)) => {
                let _ = write!(line, " outside {}", net_name_as_written(region));
            }
            None => {}
        }
        // Every sheet after the first, on the same line: they are one slot,
        // and a fabricator reads them as one.
        for sheet in &layer.sheets {
            let _ = write!(line, " sheet");
            if let Some(thickness) = sheet.thickness {
                match sheet.written_as {
                    Some(unit) if unit != cypcb_core::Unit::Mm => {
                        let _ = write!(line, " {}{}", unit.from_nm(thickness), unit);
                    }
                    _ => {
                        let _ = write!(line, " {}mm", format_mm(thickness.0 as f64 / 1e6));
                    }
                }
            }
            if let Some(material) = &sheet.material {
                let _ = write!(line, " material {}", quoted(material));
            }
            if let Some(dk) = sheet.dk_x1000 {
                let _ = write!(line, " dk {}", f64::from(dk) / 1_000.0);
            }
            if let Some(df) = sheet.df_x1000000 {
                let _ = write!(line, " df {}", f64::from(df) / 1_000_000.0);
            }
        }
        let _ = writeln!(out, "{line}");
    }
    let _ = writeln!(out, "    }}");
    out
}

/// A layer as a `trace` block writes it.
///
/// One spelling, not two: this held its own copy of the copper names while
/// `Display` held another, and the two disagreed - this one wrote `Inner1`
/// for the first inner layer and `Display` wrote `Inner 0` for the same
/// layer. `Display` is the one spelling now, and it is this one.
fn layer_keyword(layer: crate::components::Layer) -> String {
    layer.to_string()
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
/// A component's value as the language would read it back.
///
/// `value 10kohm` is a physical value the checker can compare - `assert
/// R1.value >= 10kohm` is what `examples/v2-constraints.cypcb` demonstrates -
/// and the writer quoted every value it wrote. A quoted value is a string, a
/// string is not a resistance, and the comparison that passed before a save
/// failed after it: measured on that example, the saved board reported an
/// `assertion` violation the original did not have.
///
/// Written bare only when the whole value is a number and a unit this
/// language knows. `"LDO-3V3"` and `"10k"` are strings and stay quoted, and so
/// does anything carrying a tolerance, which has a form here that this writer
/// does not produce.
fn value_as_written(value: &str) -> String {
    let split = value
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == '+'))
        .unwrap_or(value.len());
    let (number, unit) = value.split_at(split);
    let reads_back = !number.is_empty()
        && !unit.is_empty()
        && number.parse::<f64>().is_ok()
        && unit.parse::<cypcb_core::PhysicalUnit>().is_ok();
    if reads_back {
        value.to_string()
    } else {
        quoted(value)
    }
}

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
        /// The catalogue part to buy, when the design named one.
        lcsc: Option<String>,
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
            Option<&crate::components::LcscPart>,
        )>();
        query
            .iter(ecs)
            .map(
                |(refdes, position, rotation, footprint, value, connections, side, lcsc)| Part {
                    refdes: refdes.0.clone(),
                    footprint: footprint.0.clone(),
                    value: value.map(|v| v.0.clone()).unwrap_or_default(),
                    position: position.0,
                    rotation: rotation.0,
                    on_bottom: matches!(side, Some(Side::Bottom)),
                    connections: connections.cloned(),
                    lcsc: lcsc.map(|part| part.0.clone()),
                },
            )
            .collect()
    };
    // Written in the order a person reads them, not in whatever order the ECS
    // happens to hold: a diff between two imports of the same board should be
    // empty, and entity order is not a promise the ECS makes.
    parts.sort_by(|a, b| a.refdes.cmp(&b.refdes));

    // With the pitch beside each one: a stitched pour keeps its rule through a
    // round trip, and the vias it produced are not written as copper.
    let zones: Vec<(
        crate::components::zone::Zone,
        Option<cypcb_core::Nm>,
        Option<cypcb_core::Nm>,
    )> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<(
            &crate::components::zone::Zone,
            Option<&crate::components::StitchPitch>,
            Option<&crate::components::BendRadius>,
        )>();
        query
            .iter(ecs)
            .map(|(zone, pitch, radius)| (zone.clone(), pitch.map(|p| p.0), radius.map(|r| r.0)))
            .collect()
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

    // What the board asked for where its tracks meet its pads.
    let teardrops: Option<crate::components::Teardrops> = world.teardrops();

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
        let _ = write!(out, "{}", stackup_as_dsl(stackup));
    }
    if let Some(fab) = fab {
        let _ = writeln!(out, "    fab {fab}");
    }
    // The fillets, written back as the board asked for them. A board that asked
    // with the ordinary ratios gets the bare word: writing the numbers out
    // would turn a request into a specification the source never made, and the
    // next reader would have no way to tell which of the two it was looking at.
    if let Some(teardrops) = teardrops {
        let default = crate::components::Teardrops::default();
        if teardrops == default {
            let _ = writeln!(out, "    teardrops");
        } else {
            let _ = writeln!(out, "    teardrops {{");
            let _ = writeln!(out, "        length {}", format_ratio(teardrops.length));
            let _ = writeln!(out, "        width {}", format_ratio(teardrops.width));
            let _ = writeln!(out, "    }}");
        }
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
    // A footprint definition takes the same kind of name a net does - bare
    // where it can be, quoted where it cannot - so `0805` stays `0805` and
    // `Package_QFP:LQFP-48_7x7mm_P0.5mm` stays itself. It used to be rewritten
    // into an identifier, which meant a part imported from KiCad as `0805`
    // went back out as `_0805` and a round trip renamed every footprint on the
    // board. Names that still collide are given a number rather than silently
    // merged, because two footprints wearing one name is two parts with the
    // wrong pads.

    let builtin = crate::footprint::FootprintLibrary::new();
    let library = world.footprints().clone();
    // A part on the bottom is placed against a mirrored copy of its footprint,
    // filed under a derived name - `CAP_POLARISED@bottom` - which is this
    // project's own arrangement rather than anything a design says. Written as
    // it stands, that copy came back **beside** the `side bottom` line that
    // caused it, so a reload mirrored the pads a second time: measured on
    // `examples/two-sided-power.cypcb`, a board that reported two unconnected
    // pins came back reporting a clearance fault and an unrouted pin as well.
    //
    // So the definitions are written under the names the design asked for, and
    // `side bottom` says the rest.
    let mut used: Vec<String> = parts
        .iter()
        .map(|p| crate::footprint::base_name(&p.footprint).to_string())
        .collect();
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
        // The library prefix goes: `cypcb:0402` and
        // `Package_QFP:LQFP-48_7x7mm_P0.5mm` are KiCad's way of saying which
        // library a part came from, and this language's own library is keyed
        // by the bare name - so a `0402` that went out to KiCad has to come
        // home as `0402` rather than as `cypcb:0402`.
        let mut identifier = name.rsplit(':').next().unwrap_or(name.as_str()).to_string();
        if !taken.insert(identifier.clone()) {
            let mut n = 2;
            while !taken.insert(format!("{identifier}_{n}")) {
                n += 1;
            }
            identifier = format!("{identifier}_{n}");
        }
        written_name.insert(name.clone(), identifier.clone());
        let _ = writeln!(out);
        let _ = writeln!(out, "footprint {} {{", net_name_as_written(&identifier));
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

        // The legend the fabricator prints.
        //
        // `SilkClearanceRule` measures printed designators rather than a
        // footprint's own artwork, so dropping these changed nothing the
        // checker says - it changed the board. The silkscreen gerber is drawn
        // from these shapes, so a design saved through here exported a
        // different board than the one it came from, silently.
        for shape in &footprint.silk {
            match shape {
                crate::footprint::SilkShape::Segment { start, end, width } => {
                    let _ = writeln!(
                        out,
                        "    silk line {}mm, {}mm to {}mm, {}mm width {}mm",
                        format_mm(start.x.0 as f64 / 1e6),
                        format_mm(start.y.0 as f64 / 1e6),
                        format_mm(end.x.0 as f64 / 1e6),
                        format_mm(end.y.0 as f64 / 1e6),
                        format_mm(width.0 as f64 / 1e6)
                    );
                }
                crate::footprint::SilkShape::Circle {
                    centre,
                    radius,
                    width,
                } => {
                    let _ = writeln!(
                        out,
                        "    silk circle {}mm, {}mm radius {}mm width {}mm",
                        format_mm(centre.x.0 as f64 / 1e6),
                        format_mm(centre.y.0 as f64 / 1e6),
                        format_mm(radius.0 as f64 / 1e6),
                        format_mm(width.0 as f64 / 1e6)
                    );
                }
            }
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
            quoted({
                let base = crate::footprint::base_name(&part.footprint);
                written_name.get(base).map(String::as_str).unwrap_or(base)
            })
        );
        if !part.value.is_empty() {
            let _ = writeln!(out, "    value {}", value_as_written(&part.value));
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
        // The part to buy. A footprint says what the pads look like and a
        // value says what is printed on it; neither says which part an
        // assembly house orders, and the writer dropped the one line that
        // does - so a design saved through here came back with a bill of
        // materials whose `LCSC Part #` column is empty.
        if let Some(part_number) = &part.lcsc {
            let _ = writeln!(out, "    lcsc {}", quoted(part_number));
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
    // A net that carries copper and connects to no pin is declared too, with
    // an empty block. The comment above said such a net "is a name with no
    // meaning in this language" - which is true of a net that carries nothing,
    // and false the moment a trace names it: the trace writer emits
    // `trace GND { ... }` from the copper alone, so leaving the declaration
    // out wrote a file that parses and then fails to sync with `MissingNet`.
    // The same shape as the quoting fault below, from the other direction.
    //
    // `net GND { }` is legal: `net_definition` takes an optional pin list.
    for net_id in copper_nets(world) {
        if let Some(name) = net_names.get(&net_id) {
            pins_by_net.entry(name.clone()).or_default();
        }
    }

    // The second of the two places that used to drop a net for want of a
    // spelling. The trace writer was the other, and a fix to one alone left
    // the file with `trace "VBUS+"` and no `net "VBUS+"` above it - which
    // parses and then fails to sync with `MissingNet`, a worse outcome than
    // either whole answer.
    // What a net asks for, beside its name.
    //
    // `net SIG [width 0.5mm clearance 0.3mm current 500mA]` is four figures
    // three rules read - `MinTraceWidthRule`, `TraceCurrentRule` and
    // `ImpedanceRule` - and the writer used to drop every one of them. A board
    // written out and read back came home with its nets unconstrained and
    // those three quietly checking nothing, which is the same shape of silent
    // loss as the pours and the named pads before it.
    let ids_by_name: std::collections::HashMap<&String, u32> =
        net_names.iter().map(|(id, name)| (name, *id)).collect();

    for (net, mut pins) in pins_by_net {
        pins.sort();
        pins.dedup();
        let asks = ids_by_name
            .get(&net)
            .and_then(|id| world.net_constraints(crate::NetId::new(*id)))
            .map(|asks| net_constraints_as_written(&asks))
            .unwrap_or_default();
        let _ = writeln!(out);
        let _ = writeln!(out, "net {}{} {{", net_name_as_written(&net), asks);
        for pin in pins {
            let _ = writeln!(out, "    {pin}");
        }
        let _ = writeln!(out, "}}");
    }

    // What the design asserts about itself, after the parts the assertions are
    // about.
    //
    // `assert R1.value >= 10kohm` is a rule the checker reports as an
    // `assertion` violation, and the writer dropped it: a board saved through
    // here came back with the parts and none of the claims made about them,
    // which is the same shape of loss as the differential pair - a rule rather
    // than a sentence's brevity.
    //
    // An assertion this writer cannot spell exactly is left out rather than
    // approximated: a tolerance on an operand has no written form here, and a
    // claim written down wrong is worse than one written down not at all.
    let claims: Vec<String> = world
        .assertions()
        .iter()
        .filter_map(|assertion| assert_as_written(&assertion.expression))
        .collect();
    for claim in claims {
        let _ = writeln!(out, "\n{claim}");
    }

    // The pairs that carry one signal between them, after the nets they name.
    //
    // `diffpair USB { USB_DP USB_DM }` is what `DiffPairSkewRule` measures,
    // and unlike a `netclass` it is flattened onto nothing: the world keeps
    // the pair as its own statement. The writer dropped it, so a board written
    // out came back with two ordinary nets and a rule with nothing to check -
    // a rule lost rather than a sentence's brevity, which is what a class
    // costs.
    //
    // A pair whose halves cannot be spelled bare is left out with the rest of
    // what this writer cannot say: `diffpair` takes identifiers, and a net
    // called `D+` has no written form here.
    let pairs: Vec<String> = world
        .diff_pairs()
        .iter()
        .filter(|pair| {
            is_writable_identifier(&pair.positive.value)
                && is_writable_identifier(&pair.negative.value)
        })
        .map(|pair| {
            format!(
                "\ndiffpair {} {{\n    {}\n    {}\n}}\n",
                pair.name.value, pair.positive.value, pair.negative.value
            )
        })
        .collect();
    for pair in pairs {
        out.push_str(&pair);
    }

    // Keepouts and copper pours, after the nets a pour is poured to and before
    // the copper. This block used to write a comment saying "the language has
    // no syntax for one yet", which had stopped being true: `zone_definition`
    // takes bounds, a layer and a net, and `sync_zone` reads all three. A board
    // imported from KiCad with a ground plane lost that plane on its first save
    // through the editor, under a note claiming the loss was unavoidable.
    for (zone, stitch, radius) in &zones {
        out.push_str(&zone_as_dsl(zone, *stitch, *radius, &net_names));
    }

    // The measurements, before the words: a reader meets the board's size
    // before its labels. Documentation either way - neither reaches copper.
    let dimensions: Vec<crate::components::BoardDimension> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<&crate::components::BoardDimension>();
        query.iter(ecs).copied().collect()
    };
    for dimension in &dimensions {
        let _ = writeln!(out, "dimension {{");
        let _ = writeln!(
            out,
            "    from {}mm, {}mm",
            format_mm(dimension.from.x.to_mm()),
            format_mm(dimension.from.y.to_mm())
        );
        let _ = writeln!(
            out,
            "    to {}mm, {}mm",
            format_mm(dimension.to.x.to_mm()),
            format_mm(dimension.to.y.to_mm())
        );
        let _ = writeln!(out, "    offset {}mm", format_mm(dimension.offset.to_mm()));
        let _ = writeln!(out, "}}");
        let _ = writeln!(out);
    }

    // The design's own words. Written after the zones for the same reason the
    // zones come after the parts: a reader meets the board, then what is on it,
    // then what is written on top.
    let texts: Vec<crate::components::BoardText> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<&crate::components::BoardText>();
        query.iter(ecs).cloned().collect()
    };
    for text in &texts {
        let layer = if text.layer == crate::Layer::BottomSilk {
            "bottom"
        } else {
            "top"
        };
        let _ = writeln!(out, "text {} {{", quoted(&text.content));
        let _ = writeln!(
            out,
            "    at {}mm, {}mm",
            format_mm(text.position.x.to_mm()),
            format_mm(text.position.y.to_mm())
        );
        let _ = writeln!(out, "    layer {layer}");
        let _ = writeln!(out, "    height {}mm", format_mm(text.height.to_mm()));
        let _ = writeln!(out, "}}");
        let _ = writeln!(out);
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
