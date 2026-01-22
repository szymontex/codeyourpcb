//! Hover information provider.
//!
//! Provides hover information for components, nets, footprints, and pins.

use cypcb_parser::ast::{
    BoardDef, ComponentDef, Definition, FootprintDef, NetDef, TraceDef, ZoneDef, ZoneKind,
};
use cypcb_world::footprint::FootprintLibrary;

use crate::document::{DocumentState, Position};

/// Hover content with optional range.
#[derive(Debug, Clone)]
pub struct HoverInfo {
    /// Markdown content to display.
    pub content: String,
}

/// Provide hover information at the given position.
///
/// Returns hover content based on what's under the cursor:
/// - Component: footprint, value, position, rotation
/// - Net: connected pins, constraints
/// - Footprint string: size, pad count, type
/// - Pin reference: net, component info
pub fn hover_at_position(doc: &DocumentState, position: &Position) -> Option<HoverInfo> {
    let offset = doc.position_to_offset(position)?;
    let ast = doc.ast.as_ref()?;

    for def in &ast.definitions {
        let span = def.span();
        if offset >= span.start && offset < span.end {
            return hover_for_definition(doc, def, offset);
        }
    }

    None
}

fn hover_for_definition(
    doc: &DocumentState,
    def: &Definition,
    offset: usize,
) -> Option<HoverInfo> {
    match def {
        Definition::Component(comp) => hover_for_component(doc, comp, offset),
        Definition::Net(net) => hover_for_net(doc, net, offset),
        Definition::Footprint(fp) => hover_for_footprint_def(doc, fp, offset),
        Definition::Board(board) => hover_for_board(doc, board, offset),
        Definition::Zone(zone) => hover_for_zone(doc, zone, offset),
        Definition::Trace(trace) => hover_for_trace(doc, trace, offset),
    }
}

fn hover_for_component(
    _doc: &DocumentState,
    comp: &ComponentDef,
    offset: usize,
) -> Option<HoverInfo> {
    if offset >= comp.refdes.span.start && offset < comp.refdes.span.end {
        return Some(make_component_hover(comp));
    }

    if offset >= comp.footprint.span.start && offset < comp.footprint.span.end {
        return Some(make_footprint_hover(&comp.footprint.value));
    }

    if let Some(val) = &comp.value {
        if offset >= val.span.start && offset < val.span.end {
            return Some(HoverInfo {
                content: format!("**Value:** {}", val.value),
            });
        }
    }

    if let Some(pos) = &comp.position {
        if offset >= pos.span.start && offset < pos.span.end {
            return Some(HoverInfo {
                content: format!("**Position:** {}, {}", pos.x, pos.y),
            });
        }
    }

    for assign in &comp.net_assignments {
        if offset >= assign.span.start && offset < assign.span.end {
            return Some(HoverInfo {
                content: format!(
                    "**Pin {} connected to net {}**",
                    assign.pin, assign.net.value
                ),
            });
        }
    }

    if offset >= comp.span.start && offset < comp.span.end {
        return Some(make_component_hover(comp));
    }

    None
}

fn make_component_hover(comp: &ComponentDef) -> HoverInfo {
    let mut lines = vec![format!("**{}** ({:?})", comp.refdes.value, comp.kind)];

    lines.push(format!("Footprint: {}", comp.footprint.value));

    if let Some(val) = &comp.value {
        lines.push(format!("Value: {}", val.value));
    }

    if let Some(pos) = &comp.position {
        lines.push(format!("Position: {}, {}", pos.x, pos.y));
    }

    if let Some(rot) = &comp.rotation {
        lines.push(format!("Rotation: {}deg", rot.angle));
    }

    HoverInfo {
        content: lines.join("\n"),
    }
}

fn make_footprint_hover(footprint_name: &str) -> HoverInfo {
    let lib = FootprintLibrary::new();

    if let Some(fp) = lib.get(footprint_name) {
        let pad_type = if fp.pads.iter().any(|p| p.drill.is_some()) {
            "THT"
        } else {
            "SMD"
        };

        let content = format!(
            "**Footprint: {}**\n\
             Size: {:.2}mm x {:.2}mm\n\
             Pads: {}\n\
             Type: {}",
            fp.name,
            fp.bounds.width().to_mm(),
            fp.bounds.height().to_mm(),
            fp.pads.len(),
            pad_type
        );
        HoverInfo { content }
    } else {
        HoverInfo {
            content: format!("**Footprint: {}** (unknown)", footprint_name),
        }
    }
}

fn hover_for_net(
    _doc: &DocumentState,
    net: &NetDef,
    offset: usize,
) -> Option<HoverInfo> {
    if offset >= net.name.span.start && offset < net.name.span.end {
        return Some(make_net_hover(net));
    }

    for conn in &net.connections {
        if offset >= conn.span.start && offset < conn.span.end {
            return Some(HoverInfo {
                content: format!(
                    "**Pin: {}.{}**\nNet: {}",
                    conn.component.value, conn.pin, net.name.value
                ),
            });
        }
    }

    if offset >= net.span.start && offset < net.span.end {
        return Some(make_net_hover(net));
    }

    None
}

fn make_net_hover(net: &NetDef) -> HoverInfo {
    let mut lines = vec![format!("**Net: {}**", net.name.value)];

    if !net.connections.is_empty() {
        lines.push("Connected pins:".to_string());
        for conn in &net.connections {
            lines.push(format!("- {}.{}", conn.component.value, conn.pin));
        }
    }

    if let Some(constraints) = &net.constraints {
        if let Some(width) = &constraints.width {
            lines.push(format!("Width: {}", width));
        }
        if let Some(clearance) = &constraints.clearance {
            lines.push(format!("Clearance: {}", clearance));
        }
        if let Some(current) = &constraints.current {
            lines.push(format!("Current: {}", current));
        }
    }

    HoverInfo {
        content: lines.join("\n"),
    }
}

fn hover_for_footprint_def(
    _doc: &DocumentState,
    fp: &FootprintDef,
    offset: usize,
) -> Option<HoverInfo> {
    if offset >= fp.span.start && offset < fp.span.end {
        let mut lines = vec![format!("**Footprint Definition: {}**", fp.name.value)];

        if let Some(desc) = &fp.description {
            lines.push(format!("Description: {}", desc));
        }

        lines.push(format!("Pads: {}", fp.pads.len()));

        if let Some((w, h)) = &fp.courtyard {
            lines.push(format!("Courtyard: {} x {}", w, h));
        }

        return Some(HoverInfo {
            content: lines.join("\n"),
        });
    }

    None
}

fn hover_for_board(
    _doc: &DocumentState,
    board: &BoardDef,
    offset: usize,
) -> Option<HoverInfo> {
    if offset >= board.span.start && offset < board.span.end {
        let mut lines = vec![format!("**Board: {}**", board.name.value)];

        if let Some(size) = &board.size {
            lines.push(format!("Size: {} x {}", size.width, size.height));
        }

        if let Some(layers) = &board.layers {
            lines.push(format!("Layers: {}", layers));
        }

        return Some(HoverInfo {
            content: lines.join("\n"),
        });
    }

    None
}

fn hover_for_zone(
    _doc: &DocumentState,
    zone: &ZoneDef,
    offset: usize,
) -> Option<HoverInfo> {
    if offset >= zone.span.start && offset < zone.span.end {
        let kind_str = match zone.kind {
            ZoneKind::Keepout => "Keepout",
            ZoneKind::CopperPour => "Copper Pour",
        };

        let mut lines = vec![];

        if let Some(name) = &zone.name {
            lines.push(format!("**{}: {}**", kind_str, name.value));
        } else {
            lines.push(format!("**{}**", kind_str));
        }

        let (x1, y1, x2, y2) = &zone.bounds;
        lines.push(format!("Bounds: ({}, {}) to ({}, {})", x1, y1, x2, y2));

        if let Some(layer) = &zone.layer {
            lines.push(format!("Layer: {}", layer));
        }

        if let Some(net) = &zone.net {
            lines.push(format!("Net: {}", net.value));
        }

        return Some(HoverInfo {
            content: lines.join("\n"),
        });
    }

    None
}

fn hover_for_trace(
    _doc: &DocumentState,
    trace: &TraceDef,
    offset: usize,
) -> Option<HoverInfo> {
    if offset >= trace.span.start && offset < trace.span.end {
        let mut lines = vec![format!("**Trace: {}**", trace.net.value)];

        if let Some(from) = &trace.from {
            lines.push(format!("From: {}.{}", from.component.value, from.pin));
        }

        if let Some(to) = &trace.to {
            lines.push(format!("To: {}.{}", to.component.value, to.pin));
        }

        if !trace.waypoints.is_empty() {
            lines.push(format!("Via waypoints: {}", trace.waypoints.len()));
        }

        if let Some(layer) = &trace.layer {
            lines.push(format!("Layer: {}", layer));
        }

        if let Some(width) = &trace.width {
            lines.push(format!("Width: {}", width));
        }

        if trace.locked {
            lines.push("Locked: yes".to_string());
        }

        return Some(HoverInfo {
            content: lines.join("\n"),
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_doc(content: &str) -> DocumentState {
        let mut doc = DocumentState::new(content.to_string(), 1);
        doc.parse();
        doc
    }

    #[test]
    fn test_hover_on_component() {
        let doc = make_doc(
            r#"
component R1 resistor "0402" {
    value "330"
    at 10mm, 8mm
}
"#,
        );

        let pos = Position { line: 1, character: 10 };
        let hover = hover_at_position(&doc, &pos);
        assert!(hover.is_some());

        if let Some(info) = hover {
            assert!(info.content.contains("R1"));
            assert!(info.content.contains("0402"));
        }
    }

    #[test]
    fn test_hover_on_net() {
        let doc = make_doc(
            r#"
net VCC {
    R1.1
    C1.1
}
"#,
        );

        let pos = Position { line: 1, character: 4 };
        let hover = hover_at_position(&doc, &pos);
        assert!(hover.is_some());

        if let Some(info) = hover {
            assert!(info.content.contains("VCC"));
            assert!(info.content.contains("R1.1"));
        }
    }

    #[test]
    fn test_hover_on_whitespace() {
        let doc = make_doc("   \n\n   ");

        let pos = Position { line: 0, character: 0 };
        let hover = hover_at_position(&doc, &pos);
        assert!(hover.is_none());
    }
}
