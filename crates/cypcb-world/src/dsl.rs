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
                let _ = writeln!(output, "trace {} {{", net_name);

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
                let _ = writeln!(output, "trace {} {{", net_name);
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
