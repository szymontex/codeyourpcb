//! Writing routed copper back into the board it came from.
//!
//! A KiCad user's loop is: draw the board in KiCad, route it, open it in
//! KiCad. This project could do the middle step and neither end of it - the
//! importer read boards for benchmarks only until `check` and `export` learned
//! to, and nothing has ever written a `.kicad_pcb`.
//!
//! What is written here is deliberately narrow: the `(segment ...)` and
//! `(via ...)` forms that routing produces, inserted into a copy of the
//! original file. Everything else in that file - footprints, nets, zones,
//! setup, the things this project models loosely or not at all - is carried
//! through byte for byte, because the safest way to preserve what you do not
//! model is not to rewrite it.

use std::collections::HashMap;

use cypcb_router::types::RoutingResult;
use cypcb_world::components::Layer;
use cypcb_world::NetId;

/// Errors from writing a board back.
#[derive(Debug, thiserror::Error)]
pub enum KicadWriteError {
    /// The source did not look like a board this can append to.
    #[error("the file does not end in a closing paren, so it is not a board this can append to")]
    NotABoard,
    /// A net in the routing has no number in the file it came from.
    #[error("net {0:?} is not one of the file's nets, so a segment on it could not be written")]
    UnknownNet(NetId),
    /// A layer the router used has no KiCad name here.
    #[error("layer {0:?} has no KiCad name")]
    UnknownLayer(Layer),
}

/// The KiCad name for a copper layer.
fn layer_name(layer: Layer) -> Result<String, KicadWriteError> {
    Ok(match layer {
        Layer::TopCopper => "F.Cu".to_string(),
        Layer::BottomCopper => "B.Cu".to_string(),
        // Zero-based inside, one-based in the file, the same way
        // `cypcb-export`'s `layer_tag` does it.
        Layer::Inner(n) => format!("In{}.Cu", n + 1),
        other => return Err(KicadWriteError::UnknownLayer(other)),
    })
}

/// Millimetres, printed the way KiCad writes them.
///
/// Trailing zeros are trimmed because that is what pcbnew does, and a file
/// that differs from the one KiCad would write only in zero padding invites
/// somebody to diff the two and conclude something changed.
fn mm(nm: i64) -> String {
    let value = nm as f64 / 1_000_000.0;
    let text = format!("{value:.6}");
    let trimmed = text.trim_end_matches('0').trim_end_matches('.');
    if trimmed.is_empty() || trimmed == "-" {
        "0".to_string()
    } else {
        trimmed.to_string()
    }
}

/// The same, for a board whose file declares no nets.
///
/// **KiCad 10 writes no `(net N "name")` table.** Its pads carry `(net
/// "VBUS")` - the name - and a board it saved has nowhere for a segment to
/// find a number. Routing one used to end at `net NetId(0) is not one of the
/// file's nets`, which is true and useless: nothing the router did was wrong,
/// the file simply had no table to point into.
///
/// `declare` is that table, as `(number, name)` pairs. It is written where
/// this crate's own board writer writes one - ahead of the first footprint, so
/// every node that names a net comes after the nets exist - and `net_numbers`
/// has to agree with it. Empty for a file that already declares its nets,
/// which is every KiCad before 10 and every board this project writes itself.
pub fn append_routing_declaring(
    source: &str,
    routing: &RoutingResult,
    net_numbers: &HashMap<NetId, i64>,
    origin: (f64, f64),
    declare: &[(i64, String)],
) -> Result<String, KicadWriteError> {
    let source = if declare.is_empty() {
        source.to_string()
    } else {
        let mut table = String::new();
        for (number, name) in declare {
            table.push_str(&format!("  (net {number} \"{name}\")\n"));
        }
        // Ahead of the first footprint when there is one: a pad naming a net
        // reads better after the net is declared, and that is where this
        // crate's own writer puts the table. A board with no footprints has
        // nothing to come before, so the table goes with the copper.
        match source
            .find("  (footprint ")
            .or_else(|| source.find("(footprint "))
        {
            Some(at) => format!("{}{table}{}", &source[..at], &source[at..]),
            None => source.to_string(),
        }
    };
    let source = source.as_str();

    let cut = source.rfind(')').ok_or(KicadWriteError::NotABoard)?;

    let to_file = |x: i64, y: i64| -> (String, String) {
        (
            mm(x + (origin.0 * 1_000_000.0).round() as i64),
            mm(y + (origin.1 * 1_000_000.0).round() as i64),
        )
    };

    let mut out = String::with_capacity(source.len() + routing.routes.len() * 96);
    out.push_str(&source[..cut]);

    if !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str("\n  ; routed by cypcb\n");

    for segment in &routing.routes {
        let number = net_numbers
            .get(&segment.net_id)
            .copied()
            .ok_or(KicadWriteError::UnknownNet(segment.net_id))?;
        let (x1, y1) = to_file(segment.start.x.raw(), segment.start.y.raw());
        let (x2, y2) = to_file(segment.end.x.raw(), segment.end.y.raw());
        out.push_str(&format!(
            "  (segment (start {x1} {y1}) (end {x2} {y2}) (width {}) (layer \"{}\") (net {number}))\n",
            mm(segment.width.raw()),
            layer_name(segment.layer)?,
        ));
    }

    for via in &routing.vias {
        let number = net_numbers
            .get(&via.net_id)
            .copied()
            .ok_or(KicadWriteError::UnknownNet(via.net_id))?;
        let (x, y) = to_file(via.position.x.raw(), via.position.y.raw());
        out.push_str(&format!(
            "  (via (at {x} {y}) (size {}) (drill {}) (layers \"{}\" \"{}\") (net {number}))\n",
            // A `ViaPlacement` carries a drill and no outer copper. The
            // project already has one answer for that - `cypcb-router`
            // turns a placement into a `Via` with `drill * 2` under the
            // comment "Default annular ring" - and inventing a second one
            // here would mean the file and the model disagree about the
            // same via.
            mm(via.drill.raw() * 2),
            mm(via.drill.raw()),
            layer_name(via.start_layer)?,
            layer_name(via.end_layer)?,
        ));
    }

    out.push_str(&source[cut..]);
    Ok(out)
}
