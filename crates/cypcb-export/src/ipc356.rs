//! The IPC-D-356A netlist, the file a bare board is tested against.
//!
//! Before anything is soldered to a board, a fabricator puts it on a flying
//! probe or a bed of nails and checks two things: that every point which should
//! be connected is connected, and that no two points which should not be are.
//! The tester needs the design's own answer to compare against, and this is the
//! file that carries it. Item 6 of the KiCad parity audit; KiCad has written it
//! since 5.0 and this project wrote nothing.
//!
//! # The format, and where these columns come from
//!
//! IPC-D-356A is fixed-column text, 80 columns to a record, and the columns are
//! not negotiable: a tester reads by position, not by delimiter. The layout
//! implemented here is the published record structure - operation codes in
//! 1-3, the net name in 4-17, the component identifier in 21-32, the hole in
//! 33-38, the access code in 39-41, the location in 42-57, the feature size in
//! 58-71 and the soldermask flag in 73-74.
//!
//! Units are millimetres, declared by `P  UNITS CUST 1`, so every coordinate is
//! written in 0.001mm - micrometres - and every diameter in the same.
//!
//! # What this writes, and what it does not
//!
//! Through-hole pads are `317`, surface pads are `327`, and a via is a `317`
//! whose component identifier is the word `VIA` with the mid-net flag set,
//! which is what the format reserves for a point that is not a component's pin.
//! Blind and buried vias are `307`, because a tester cannot probe them and has
//! to know that before it decides a net is open.
//!
//! Conductor segments (`378`) and adjacency (`379`) are not written. Both are
//! optional sections that reduce a tester's work rather than change its
//! answers, and a file that claims neither is a file no tester misreads.

use crate::gerber::copper::place_pad_millideg;
use cypcb_core::Nm;
use cypcb_world::components::trace::Via;
use cypcb_world::components::{FootprintRef, NetConnections, Position, RefDes, Rotation};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::BoardWorld;
use cypcb_world::Layer;

/// A placed component, flattened out of the world before its pads are read.
///
/// The query borrows the world, and reading a net's name needs it again, so
/// what the loop needs is collected first: designator, footprint, where it is,
/// how it is turned, and which of its pins are on which net.
struct PlacedPart {
    refdes: String,
    footprint: String,
    position: cypcb_core::Point,
    rotation_millideg: i32,
    /// `(pin number, net id)` for every connected pin.
    pins: Vec<(String, String)>,
}

/// One test point, before it becomes eighty columns.
#[derive(Debug, Clone)]
struct TestPoint {
    /// `317`, `327` or `307`.
    code: &'static str,
    net: String,
    refdes: String,
    pin: String,
    /// A point that is not a component pin - a via - is a mid-net point.
    mid_net: bool,
    drill: Option<Nm>,
    /// `00` both sides, `01` primary, `02` secondary.
    access: &'static str,
    x: Nm,
    y: Nm,
    size_x: Nm,
    size_y: Nm,
    rotation_deg: u32,
    /// `0` none, `1` primary, `2` secondary, `3` both.
    soldermask: char,
}

/// Millimetres in thousandths, which is what `UNITS CUST 1` means.
fn micron(value: Nm) -> i64 {
    value.0 / 1_000
}

/// Six digits with a sign, as the location field wants them.
fn signed_six(value: i64) -> String {
    let sign = if value < 0 { '-' } else { '+' };
    format!("{sign}{:06}", value.abs().min(999_999))
}

/// Four digits, as the hole and feature-size fields want them.
///
/// A feature larger than the field can hold is clamped, and the caller is told,
/// because a silently truncated diameter is a test point in the wrong place.
fn four(value: i64, what: &str, warnings: &mut Vec<String>) -> String {
    if value > 9_999 {
        warnings.push(format!(
            "{what} is {value} micrometres and the IPC-D-356 field holds four digits; written as 9999"
        ));
        return "9999".to_string();
    }
    format!("{:04}", value.max(0))
}

/// Fourteen columns, left justified, no spaces: what the net name field is.
fn net_field(name: &str) -> String {
    let cleaned: String = name.chars().filter(|c| !c.is_whitespace()).collect();
    format!("{:<14.14}", cleaned)
}

/// The eighty-column record for one point.
fn record(point: &TestPoint, warnings: &mut Vec<String>) -> String {
    let mut line = String::with_capacity(80);
    line.push_str(point.code); // 1-3
    line.push_str(&net_field(&point.net)); // 4-17
    line.push_str("   "); // 18-20, blank
    line.push_str(&format!("{:<6.6}", point.refdes)); // 21-26
    line.push('-'); // 27
    line.push_str(&format!("{:<4.4}", point.pin)); // 28-31
    line.push(if point.mid_net { 'M' } else { ' ' }); // 32

    match point.drill {
        // 33-38: the hole, and whether it is plated. Every hole this project
        // writes is plated; an unplated one is a mounting hole with no net.
        Some(drill) => {
            line.push('D');
            line.push_str(&four(micron(drill), "a drill diameter", warnings));
            line.push('P');
        }
        None => line.push_str("      "),
    }

    line.push('A'); // 39
    line.push_str(point.access); // 40-41

    line.push('X'); // 42
    line.push_str(&signed_six(micron(point.x))); // 43-49
    line.push('Y'); // 50
    line.push_str(&signed_six(micron(point.y))); // 51-57

    line.push('X'); // 58
    line.push_str(&four(micron(point.size_x), "a pad width", warnings)); // 59-62
    line.push('Y'); // 63
    line.push_str(&four(micron(point.size_y), "a pad height", warnings)); // 64-67
    line.push('R'); // 68
    line.push_str(&format!("{:03}", point.rotation_deg % 360)); // 69-71
    line.push(' '); // 72
    line.push('S'); // 73
    line.push(point.soldermask); // 74

    line
}

/// Which side a tester can reach this pad from.
fn access_of(layers: &[Layer]) -> &'static str {
    let top = layers.contains(&Layer::TopCopper);
    let bottom = layers.contains(&Layer::BottomCopper);
    match (top, bottom) {
        (true, true) => "00",
        (true, false) => "01",
        (false, true) => "02",
        // A pad on inner copper only cannot be probed at all; the format has no
        // better word for it than "not from side one".
        (false, false) => "02",
    }
}

/// Write the netlist for this board.
///
/// Returns the file and whatever had to be clamped to fit the format, so the
/// caller can put it in front of a person rather than in a file nobody reads.
pub fn export_ipc356(
    world: &mut BoardWorld,
    library: &FootprintLibrary,
    board_name: &str,
) -> (String, Vec<String>) {
    let mut warnings = Vec::new();
    let mut points: Vec<TestPoint> = Vec::new();

    // Every pad of every placed component, with the net its pin is on.
    let rows: Vec<PlacedPart> = {
        let mut query = world.ecs_mut().query::<(
            &RefDes,
            &Position,
            &Rotation,
            &FootprintRef,
            Option<&NetConnections>,
        )>();
        query
            .iter(world.ecs())
            .map(
                |(refdes, position, rotation, footprint, connections)| PlacedPart {
                    refdes: refdes.as_str().to_string(),
                    footprint: footprint.as_str().to_string(),
                    position: position.0,
                    rotation_millideg: rotation.0,
                    pins: connections
                        .map(|c| {
                            c.iter()
                                .map(|pin| (pin.pin.clone(), pin.net.id().to_string()))
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default(),
                },
            )
            .collect()
    };

    // Net ids resolved to the names a tester reads.
    let net_names: std::collections::HashMap<String, String> = world
        .nets()
        .map(|(id, name)| (id.id().to_string(), name.to_string()))
        .collect();

    for part in rows {
        let PlacedPart {
            refdes,
            footprint: footprint_name,
            position,
            rotation_millideg: rotation,
            pins,
        } = part;
        let Some(footprint) = library.get(&footprint_name) else {
            warnings.push(format!(
                "{refdes} names the footprint {footprint_name}, which the library does not have, so its pads are not in the netlist"
            ));
            continue;
        };
        for pad in &footprint.pads {
            let net = pins
                .iter()
                .find(|(pin, _)| *pin == pad.number)
                .and_then(|(_, id)| net_names.get(id))
                .cloned();
            // A pad on no net is not a test point: a tester has nothing to
            // compare it against. The format's `N/C` is for an isolated net,
            // which is a different statement than "this design says nothing".
            let Some(net) = net else {
                continue;
            };
            let centre = place_pad_millideg(position, pad.position, rotation);
            points.push(TestPoint {
                code: if pad.drill.is_some() { "317" } else { "327" },
                net,
                refdes: refdes.clone(),
                pin: pad.number.clone(),
                mid_net: false,
                drill: pad.drill,
                access: access_of(&pad.layers),
                x: centre.x,
                y: centre.y,
                size_x: pad.size.0,
                size_y: pad.size.1,
                rotation_deg: ((rotation / 1000).rem_euclid(360)) as u32,
                soldermask: if pad.drill.is_some() { '3' } else { '1' },
            });
        }
    }

    // Vias. A tester probes a through via and cannot reach a buried one, and
    // the format says which is which with a different operation code.
    let vias: Vec<Via> = {
        let mut query = world.ecs_mut().query::<&Via>();
        query.iter(world.ecs()).cloned().collect()
    };
    for via in vias {
        let through = via.start_layer == Layer::TopCopper && via.end_layer == Layer::BottomCopper;
        let Some(net) = world.net_name(via.net_id).map(str::to_string) else {
            continue;
        };
        points.push(TestPoint {
            code: if through { "317" } else { "307" },
            net,
            refdes: "VIA".to_string(),
            pin: String::new(),
            mid_net: true,
            drill: Some(via.drill),
            access: if through { "00" } else { "02" },
            x: via.position.x,
            y: via.position.y,
            size_x: via.outer_diameter,
            size_y: via.outer_diameter,
            rotation_deg: 0,
            soldermask: '3',
        });
    }

    // Sorted by net, which is what the format asks of the netlist section and
    // what makes a diff between two revisions readable.
    points.sort_by(|a, b| {
        (a.net.as_str(), a.refdes.as_str(), a.pin.as_str()).cmp(&(
            b.net.as_str(),
            b.refdes.as_str(),
            b.pin.as_str(),
        ))
    });

    let mut out = String::new();
    out.push_str(&format!("P  JOB   {board_name}\n"));
    out.push_str("P  CODE  00\n");
    out.push_str("P  UNITS CUST 1\n");
    out.push_str("P  DIM   N\n");
    out.push_str("P  VER   IPC-D-356A\n");
    out.push_str("P  IMAGE PRIMARY\n");
    out.push_str("C  Netlist written by CodeYourPCB\n");
    for point in &points {
        out.push_str(&record(point, &mut warnings));
        out.push('\n');
    }
    out.push_str("999\n");

    (out, warnings)
}
