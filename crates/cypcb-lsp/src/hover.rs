//! Hover information provider.
//!
//! Provides hover information for components, nets, footprints, and pins.
//! Enhanced hover includes net connections, calculated trace width, and DRC status.

use cypcb_parser::ast::{
    BoardDef, ComponentDef, Definition, FootprintDef, NetDef, SourceFile, TraceDef, ZoneDef, ZoneKind,
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
    doc: &DocumentState,
    comp: &ComponentDef,
    offset: usize,
) -> Option<HoverInfo> {
    if offset >= comp.refdes.span.start && offset < comp.refdes.span.end {
        return Some(make_component_hover_enhanced(doc, comp));
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
        return Some(make_component_hover_enhanced(doc, comp));
    }

    None
}

/// Enhanced component hover with net connections and DRC status.
fn make_component_hover_enhanced(doc: &DocumentState, comp: &ComponentDef) -> HoverInfo {
    let lib = FootprintLibrary::new();
    let mut lines = vec![format!("**{}** ({:?})", comp.refdes.value, comp.kind)];

    // Footprint info with size if available
    if let Some(fp) = lib.get(&comp.footprint.value) {
        lines.push(format!(
            "Footprint: {} ({:.2}mm x {:.2}mm)",
            comp.footprint.value,
            fp.bounds.width().to_mm(),
            fp.bounds.height().to_mm()
        ));
    } else {
        lines.push(format!("Footprint: {}", comp.footprint.value));
    }

    if let Some(val) = &comp.value {
        lines.push(format!("Value: {}", val.value));
    }

    if let Some(pos) = &comp.position {
        lines.push(format!("Position: {}, {}", pos.x, pos.y));
    }

    if let Some(rot) = &comp.rotation {
        lines.push(format!("Rotation: {}deg", rot.angle));
    }

    // Find net connections for this component
    let connections = find_component_net_connections(doc.ast.as_ref(), &comp.refdes.value);
    if !connections.is_empty() {
        lines.push(String::new()); // Empty line for spacing
        lines.push("**Net connections:**".to_string());
        for (pin, net_name) in connections {
            lines.push(format!("- Pin {}: {}", pin, net_name));
        }
    }

    // Include inline net assignments from component definition
    if !comp.net_assignments.is_empty() {
        let has_connections = doc.ast.as_ref()
            .map(|ast| !find_component_net_connections(Some(ast), &comp.refdes.value).is_empty())
            .unwrap_or(false);

        if !has_connections {
            lines.push(String::new());
            lines.push("**Net assignments:**".to_string());
        }

        for assign in &comp.net_assignments {
            lines.push(format!("- Pin {} = {}", assign.pin, assign.net.value));
        }
    }

    // DRC status
    let violation_count = count_component_violations(doc, &comp.refdes.value);
    if violation_count > 0 {
        lines.push(String::new());
        lines.push(format!("**DRC:** {} violation(s)", violation_count));
    } else {
        lines.push(String::new());
        lines.push("**DRC:** OK".to_string());
    }

    HoverInfo {
        content: lines.join("\n"),
    }
}

/// Find all net connections for a given component.
/// Returns a list of (pin_id, net_name) pairs.
fn find_component_net_connections(ast: Option<&SourceFile>, refdes: &str) -> Vec<(String, String)> {
    let mut connections = Vec::new();

    if let Some(ast) = ast {
        for def in &ast.definitions {
            if let Definition::Net(net) = def {
                for conn in &net.connections {
                    if conn.component.value == refdes {
                        connections.push((conn.pin.to_string(), net.name.value.clone()));
                    }
                }
            }
        }
    }

    connections
}

/// Count DRC violations related to a component.
fn count_component_violations(doc: &DocumentState, refdes: &str) -> usize {
    doc.drc_violations.iter()
        .filter(|v| v.message.contains(refdes))
        .count()
}

fn make_footprint_hover(footprint_name: &str) -> HoverInfo {
    let lib = FootprintLibrary::new();

    if let Some(fp) = lib.get(footprint_name) {
        let pad_type = if fp.pads.iter().any(|p| p.drill.is_some()) {
            "THT"
        } else {
            "SMD"
        };

        let mut lines = vec![format!("**Footprint: {}**", fp.name)];
        lines.push(format!("Type: {}", pad_type));
        lines.push(format!("Pads: {}", fp.pads.len()));
        lines.push(String::new());

        lines.push("**Dimensions:**".to_string());
        lines.push(format!(
            "- Body: {:.2}mm x {:.2}mm",
            fp.bounds.width().to_mm(),
            fp.bounds.height().to_mm()
        ));
        lines.push(format!(
            "- Courtyard: {:.2}mm x {:.2}mm",
            fp.courtyard.width().to_mm(),
            fp.courtyard.height().to_mm()
        ));

        // Show pad details for small footprints
        if fp.pads.len() <= 8 {
            lines.push(String::new());
            lines.push("**Pads:**".to_string());
            for pad in &fp.pads {
                let shape_str = format!("{:?}", pad.shape).to_lowercase();
                let drill_str = if let Some(d) = pad.drill {
                    let d_mm: f64 = d.to_mm();
                    format!(", drill {:.2}mm", d_mm)
                } else {
                    String::new()
                };
                let width_mm: f64 = pad.size.0.to_mm();
                let height_mm: f64 = pad.size.1.to_mm();
                lines.push(format!(
                    "- {}: {} {:.2}mm x {:.2}mm{}",
                    pad.number,
                    shape_str,
                    width_mm,
                    height_mm,
                    drill_str
                ));
            }
        }

        HoverInfo { content: lines.join("\n") }
    } else {
        HoverInfo {
            content: format!("**Footprint: {}** (unknown)\n\nNot in built-in library. May be a custom footprint defined in this file.", footprint_name),
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

    // Connection count
    if !net.connections.is_empty() {
        lines.push(format!("Connections: {} pins", net.connections.len()));
        lines.push(String::new());
        lines.push("**Connected pins:**".to_string());
        for conn in &net.connections {
            lines.push(format!("- {}.{}", conn.component.value, conn.pin));
        }
    }

    if let Some(constraints) = &net.constraints {
        lines.push(String::new());
        lines.push("**Constraints:**".to_string());

        if let Some(width) = &constraints.width {
            lines.push(format!("- Trace width: {}", width));
        }
        if let Some(clearance) = &constraints.clearance {
            lines.push(format!("- Clearance: {}", clearance));
        }
        if let Some(current) = &constraints.current {
            lines.push(format!("- Current: {}", current));

            // Calculate recommended trace width based on IPC-2221
            let amps: f64 = current.to_amps();
            if let Some(calc_width) = calculate_trace_width(amps) {
                lines.push(format!("- IPC-2221 width: {:.2}mm (external, 10C rise)", calc_width));

                // Warning if specified width is less than calculated
                if let Some(specified) = &constraints.width {
                    let specified_nm: cypcb_core::Nm = specified.to_nm();
                    let specified_mm: f64 = specified_nm.to_mm();
                    if specified_mm < calc_width {
                        lines.push(format!(
                            "  **Warning:** Specified width ({:.2}mm) < recommended ({:.2}mm)",
                            specified_mm, calc_width
                        ));
                    }
                }
            }
        }
    }

    HoverInfo {
        content: lines.join("\n"),
    }
}

/// Calculate recommended trace width using IPC-2221 formula.
/// Returns width in mm for external layer with 10C temperature rise.
fn calculate_trace_width(current_amps: f64) -> Option<f64> {
    // IPC-2221 formula: I = k * dT^0.44 * A^0.725
    // Solving for A (cross-section): A = (I / (k * dT^0.44))^(1/0.725)
    // For external layer, k = 0.048
    // Default temp rise = 10C

    if current_amps <= 0.0 {
        return None;
    }

    let k: f64 = 0.048; // External layer constant
    let temp_rise: f64 = 10.0; // Default 10C rise

    // Cross-section in mil^2
    let area_mil2 = (current_amps / (k * temp_rise.powf(0.44))).powf(1.0 / 0.725);

    // Convert to width assuming 1oz copper (1.37 mil = 0.035mm thickness)
    let copper_thickness_mil: f64 = 1.37;
    let width_mil = area_mil2 / copper_thickness_mil;

    // Convert mil to mm
    let width_mm = width_mil * 0.0254;

    Some(width_mm)
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
        let mut doc = DocumentState::new("test://file".into(), content.to_string(), 1);
        doc.parse();
        doc
    }

    fn make_doc_with_world(content: &str) -> DocumentState {
        let mut doc = DocumentState::new("test://file".into(), content.to_string(), 1);
        doc.parse();
        doc.build_world();
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

    #[test]
    fn test_hover_component_shows_net_connections() {
        let doc = make_doc(
            r#"
component R1 resistor "0402" {
    at 10mm, 10mm
}

net VCC { R1.1 }
net GND { R1.2 }
"#,
        );

        let pos = Position { line: 1, character: 10 };
        let hover = hover_at_position(&doc, &pos);
        assert!(hover.is_some());

        let info = hover.unwrap();
        assert!(info.content.contains("Net connections"), "Should show net connections");
        assert!(info.content.contains("VCC"), "Should show VCC connection");
        assert!(info.content.contains("GND"), "Should show GND connection");
    }

    #[test]
    fn test_hover_component_shows_drc_status() {
        let doc = make_doc_with_world(
            r#"
board test { size 50mm x 30mm }
component R1 resistor "0402" {
    at 10mm, 10mm
}
"#,
        );

        let pos = Position { line: 2, character: 10 };
        let hover = hover_at_position(&doc, &pos);
        assert!(hover.is_some());

        let info = hover.unwrap();
        // Should show DRC status (either OK or violations)
        assert!(info.content.contains("DRC:"), "Should show DRC status");
    }

    #[test]
    fn test_hover_net_with_current_shows_calculated_width() {
        let doc = make_doc(
            r#"
net VCC [current 2A] {
    R1.1
}
"#,
        );

        let pos = Position { line: 1, character: 4 };
        let hover = hover_at_position(&doc, &pos);
        assert!(hover.is_some());

        let info = hover.unwrap();
        assert!(info.content.contains("Current: 2A"), "Should show current");
        assert!(info.content.contains("IPC-2221"), "Should show IPC-2221 calculated width");
    }

    #[test]
    fn test_hover_footprint_shows_details() {
        let doc = make_doc(
            r#"
component R1 resistor "0402" {}
"#,
        );

        // Hover on the footprint string
        let pos = Position { line: 1, character: 24 };
        let hover = hover_at_position(&doc, &pos);
        assert!(hover.is_some());

        let info = hover.unwrap();
        assert!(info.content.contains("Footprint: 0402"), "Should show footprint name");
        assert!(info.content.contains("Courtyard"), "Should show courtyard");
        assert!(info.content.contains("Pads"), "Should show pad count");
    }

    #[test]
    fn test_calculate_trace_width() {
        // Test IPC-2221 calculation
        let width = calculate_trace_width(1.0); // 1A
        assert!(width.is_some());
        let w = width.unwrap();
        // 1A should give roughly 0.3-0.5mm for external, 10C rise
        assert!(w > 0.2 && w < 1.0, "Width {} should be reasonable for 1A", w);
    }

    #[test]
    fn test_calculate_trace_width_zero_current() {
        let width = calculate_trace_width(0.0);
        assert!(width.is_none());
    }
}
