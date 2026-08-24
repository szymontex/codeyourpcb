//! Hover information provider.
//!
//! Provides hover information for components, nets, footprints, and pins.
//! Enhanced hover includes net connections, calculated trace width, and DRC status.

use cypcb_calc::TraceWidthCalculator;
use cypcb_parser::ast::{
    AssertDef, AssertExpression, AssertOperand, BoardDef, ComponentDef, Definition, FootprintDef,
    ImportDef, InterfaceDef, ModuleDef, NetDef, SourceFile, TraceDef, ZoneDef, ZoneKind,
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

fn hover_for_definition(doc: &DocumentState, def: &Definition, offset: usize) -> Option<HoverInfo> {
    match def {
        Definition::Component(comp) => hover_for_component(doc, comp, offset),
        Definition::Net(net) => hover_for_net(doc, net, offset),
        Definition::Footprint(fp) => hover_for_footprint_def(doc, fp, offset),
        Definition::Board(board) => hover_for_board(doc, board, offset),
        Definition::Zone(zone) => hover_for_zone(doc, zone, offset),
        Definition::Trace(trace) => hover_for_trace(doc, trace, offset),
        // v2 constructs
        Definition::Module(module) => hover_for_module(module, offset),
        // An instance is a `use` line; there is nothing to explain about it that
        // the module it names does not already say.
        Definition::ModuleInstance(_) => None,
        // A class is a list of names and a constraint block; hovering one net
        // of it should explain the net, which the net hover already does.
        Definition::NetClass(_) => None,
        // An outline is a list of coordinates; there is nothing to add.
        Definition::Outline(_) => None,
        Definition::Interface(iface) => hover_for_interface(iface, offset),
        Definition::Import(import) => hover_for_import(import, offset),
        Definition::Assert(assert_def) => hover_for_assert(assert_def, offset),
        // A pair is two net names; hovering either should explain the net,
        // which the net hover already does.
        Definition::DiffPair(_) => None,
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
        return Some(make_footprint_hover(doc, &comp.footprint.value));
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
        let has_connections = doc
            .ast
            .as_ref()
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
    doc.drc_violations
        .iter()
        .filter(|v| v.message.contains(refdes))
        .count()
}

fn make_footprint_hover(doc: &DocumentState, footprint_name: &str) -> HoverInfo {
    // The file first. This built a fresh library and looked the name up in it,
    // so it only ever found built-in parts: a footprint you wrote gave you
    // nothing while an 0402 gave you every pad, and the card said "may be a
    // custom footprint defined in this file" while the parsed file sat in the
    // document, unasked.
    if let Some(fp) = footprint_def_named(doc, footprint_name) {
        return HoverInfo {
            content: footprint_def_card(fp),
        };
    }

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
                // A slot is milled along its length, so printing the drill
                // alone - which is its narrow dimension - describes a round
                // hole less than half the size of the one the part needs.
                let drill_str = match (pad.slot, pad.drill) {
                    (Some((width, height)), _) if pad.is_slot() => {
                        format!(", slot {:.2}mm x {:.2}mm", width.to_mm(), height.to_mm())
                    }
                    (_, Some(d)) => format!(", drill {:.2}mm", d.to_mm()),
                    (_, None) => String::new(),
                };
                let width_mm: f64 = pad.size.0.to_mm();
                let height_mm: f64 = pad.size.1.to_mm();
                lines.push(format!(
                    "- {}: {} {:.2}mm x {:.2}mm{}",
                    pad.number, shape_str, width_mm, height_mm, drill_str
                ));
            }
        }

        HoverInfo {
            content: lines.join("\n"),
        }
    } else {
        // Neither the library nor the file has it, and the file has been
        // parsed - so this is a name nothing defines, which is usually a typo.
        // Saying "may be a custom footprint defined in this file" here told a
        // designer the opposite of what the document knows.
        HoverInfo {
            content: format!(
                "**Footprint: {footprint_name}** (unknown)\n\nNo footprint of that name is built in, and this file does not define one. The board will be missing this part's pads."
            ),
        }
    }
}

/// The footprint this file defines under that name, if it defines one.
fn footprint_def_named<'a>(doc: &'a DocumentState, name: &str) -> Option<&'a FootprintDef> {
    doc.ast
        .as_ref()?
        .definitions
        .iter()
        .find_map(|def| match def {
            Definition::Footprint(fp) if fp.name.value == name => Some(fp),
            _ => None,
        })
}

fn hover_for_net(doc: &DocumentState, net: &NetDef, offset: usize) -> Option<HoverInfo> {
    if offset >= net.name.span.start && offset < net.name.span.end {
        return Some(make_net_hover(doc, net));
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
        return Some(make_net_hover(doc, net));
    }

    None
}

/// The width that hits an impedance target, on each layer the stack can answer.
///
/// `None` when the design states no stack - most boards, and nothing to say
/// about them. An empty list when it states one that answers for no layer: a
/// microstrip needs a dielectric under it with both a thickness and a `dk`,
/// and a stripline needs the same above **and** below with the two matching,
/// which most stacks written by hand do not give.
///
/// Every layer rather than the top one. The card used to answer for `Top`
/// alone and say so, which is honest and half the question: a net routed on an
/// inner layer wants the stripline figure, and a net that is not routed yet
/// has no layer to ask about. A short table answers whichever one the designer
/// turns out to need.
///
/// Through `cypcb_drc::impedance_width_for`, which is what `cypcb check` uses
/// on a trace that already exists. The same arithmetic answering the same
/// question earlier - a designer asks this before drawing the copper, which is
/// what Altium's calculator is for.
fn impedance_widths(doc: &DocumentState, ohms: f64) -> Option<Vec<(String, f64)>> {
    if !(ohms.is_finite() && ohms > 0.0) {
        return None;
    }
    let stackup = doc.world.as_ref()?.stackup()?;
    let count = stackup.copper_count();
    let target_x100 = (ohms * 100.0).round().max(0.0) as u32;

    let mut answers = Vec::new();
    for index in 0..count {
        // The copper sequence runs top to bottom, and `Layer::Inner` is
        // zero-based against a one-based name: copper entry 1 is `Inner1`,
        // which is the off-by-one this project has shipped four times.
        let layer = if index == 0 {
            cypcb_world::Layer::TopCopper
        } else if index + 1 == count {
            cypcb_world::Layer::BottomCopper
        } else {
            cypcb_world::Layer::Inner((index - 1) as u8)
        };
        let Some(environment) = stackup.environment_of(index) else {
            continue;
        };
        if let Some(width) = cypcb_drc::impedance_width_for(environment, target_x100) {
            answers.push((layer.to_string(), width.to_mm()));
        }
    }
    Some(answers)
}

fn make_net_hover(doc: &DocumentState, net: &NetDef) -> HoverInfo {
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
        if let Some(ohms) = constraints.impedance_ohms {
            // The card said nothing about a target it had been given, which is
            // the one constraint a designer cannot work out in their head.
            lines.push(format!("- Impedance: {ohms}ohm"));
            match impedance_widths(doc, ohms) {
                Some(answers) if !answers.is_empty() => {
                    lines.push(format!(
                        "- Widths that give {ohms}ohm on this board's stack (IPC-2141, quoted \
                         at 5-7%):"
                    ));
                    for (layer, width) in answers {
                        lines.push(format!("  - {layer}: {width:.3}mm"));
                    }
                }
                Some(_) => lines.push(
                    "  The stack cannot answer on any layer: a microstrip needs a dielectric \
                     under it stating a thickness and a dk, and a stripline needs a matching \
                     one on each side."
                        .to_string(),
                ),
                // No stack stated, so there is no board to answer for.
                None => {}
            }
        }
        if let Some(current) = &constraints.current {
            lines.push(format!("- Current: {}", current));

            // Calculate recommended trace width based on IPC-2221
            let amps: f64 = current.to_amps();
            if let Some((calc_width, notes)) = calculate_trace_width(amps) {
                lines.push(format!(
                    "- IPC-2221 width: {:.2}mm (external, 10C rise)",
                    calc_width
                ));

                // When the calculator says its own answer is off the end of
                // the data, that belongs beside the number rather than
                // nowhere. An ordinary net trips none of these and the card
                // does not grow.
                if !notes.is_empty() {
                    lines.push(format!("  **Outside the standard:** {}", notes.join("; ")));
                }

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

/// Recommended trace width for a current, in mm, and what the standard says
/// about its own answer.
///
/// Delegates to `cypcb-calc`, which is the one implementation of IPC-2221 in
/// the workspace. This used to be a fourth copy of the formula and it had
/// drifted: it took 1 oz copper as 1.37 mils where every other copy uses
/// 1.378, so the hover tooltip quoted a width 0.6% wider than the router
/// would draw.
///
/// The notes were dropped for as long as this returned a bare number.
/// `min_width_for_current` takes `calculate(&params).width` and throws the
/// rest away, so a card quoting 48mm for 40A said nothing about the curves
/// being fitted to data up to about 35A. `TraceCurrentRule` says it now and a
/// hover is where the same question gets asked first.
fn calculate_trace_width(current_amps: f64) -> Option<(f64, Vec<String>)> {
    if current_amps <= 0.0 {
        return None;
    }
    // The same parameters `min_width_for_current(amps, true)` builds: an
    // external layer, 1oz copper and a 10C rise, which is what the card says.
    let params = cypcb_calc::TraceWidthParams::new(current_amps);
    let result = TraceWidthCalculator::calculate(&params);
    let notes = result
        .warnings
        .iter()
        .map(|warning| warning.to_string())
        .collect();
    Some((result.width.to_mm(), notes))
}

/// How a pad's hole reads on a hover card.
///
/// A slot is milled along its length, so naming its drill alone - which is the
/// narrow dimension, the bit that mills it - describes a round hole under half
/// the size of the one the part needs, stated with the same confidence as
/// every other number on the card.
fn hole_of(pad: &cypcb_parser::ast::PadDef) -> String {
    match (&pad.drill, &pad.drill_height) {
        (Some(width), Some(height)) if width.value != height.value => {
            format!(", slot {width} x {height}")
        }
        (Some(drill), _) => format!(", drill {drill}"),
        (None, _) => String::new(),
    }
}

fn hover_for_footprint_def(
    _doc: &DocumentState,
    fp: &FootprintDef,
    offset: usize,
) -> Option<HoverInfo> {
    if offset >= fp.span.start && offset < fp.span.end {
        return Some(HoverInfo {
            content: footprint_def_card(fp),
        });
    }

    None
}

/// The card for a footprint this file defines, wherever it is hovered from.
///
/// One rendering, because the definition and every use of its name are the
/// same footprint: hovering `USB_ANCHOR` in a component used to reach a
/// different function that knew only built-in parts and told the reader this
/// one might not exist.
fn footprint_def_card(fp: &FootprintDef) -> String {
    let mut lines = vec![format!("**Footprint Definition: {}**", fp.name.value)];

    if let Some(desc) = &fp.description {
        lines.push(format!("Description: {}", desc));
    }

    lines.push(format!("Pads: {}", fp.pads.len()));

    if let Some((w, h)) = &fp.courtyard {
        lines.push(format!("Courtyard: {} x {}", w, h));
    }

    // The pads themselves, which the card left out entirely: hovering a
    // footprint you wrote said how many pads it has and not one thing about
    // any of them. The built-in library's card has listed them all along, so a
    // designer got less about their own footprint than about an 0402.
    //
    // Held to the same eight-pad limit as the built-in card, so hovering a
    // 64-pin QFP does not bury the screen.
    if !fp.pads.is_empty() && fp.pads.len() <= 8 {
        lines.push(String::new());
        lines.push("**Pads:**".to_string());
        for pad in &fp.pads {
            lines.push(format!(
                "- {}: {} {} x {}{}",
                pad.number,
                format!("{:?}", pad.shape).to_lowercase(),
                pad.width,
                pad.height,
                hole_of(pad),
            ));
        }
    }

    lines.join("\n")
}

fn hover_for_board(_doc: &DocumentState, board: &BoardDef, offset: usize) -> Option<HoverInfo> {
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

fn hover_for_zone(_doc: &DocumentState, zone: &ZoneDef, offset: usize) -> Option<HoverInfo> {
    if offset >= zone.span.start && offset < zone.span.end {
        let kind_str = match zone.kind {
            ZoneKind::Keepout => "Keepout",
            ZoneKind::CopperPour => "Copper Pour",
            ZoneKind::Flex => "Flexible region",
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

fn hover_for_trace(_doc: &DocumentState, trace: &TraceDef, offset: usize) -> Option<HoverInfo> {
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

fn hover_for_module(module: &ModuleDef, offset: usize) -> Option<HoverInfo> {
    if offset >= module.span.start && offset < module.span.end {
        let mut lines = vec![format!("**Module: {}**", module.name.value)];

        let comp_count = module
            .definitions
            .iter()
            .filter(|d| matches!(d, Definition::Component(_)))
            .count();
        let net_count = module
            .definitions
            .iter()
            .filter(|d| matches!(d, Definition::Net(_)))
            .count();
        let assert_count = module
            .definitions
            .iter()
            .filter(|d| matches!(d, Definition::Assert(_)))
            .count();

        if comp_count > 0 {
            lines.push(format!("Components: {}", comp_count));
        }
        if net_count > 0 {
            lines.push(format!("Nets: {}", net_count));
        }
        if assert_count > 0 {
            lines.push(format!("Assertions: {}", assert_count));
        }

        if !module.pins.is_empty() {
            lines.push(String::new());
            lines.push("**Exposed pins:**".to_string());
            for pin in &module.pins {
                lines.push(format!("- {}", pin.name.value));
            }
        }

        return Some(HoverInfo {
            content: lines.join("\n"),
        });
    }
    None
}

fn hover_for_interface(iface: &InterfaceDef, offset: usize) -> Option<HoverInfo> {
    if offset >= iface.span.start && offset < iface.span.end {
        let mut lines = vec![format!("**Interface: {}**", iface.name.value)];
        lines.push(format!("Pins: {}", iface.pins.len()));

        if !iface.pins.is_empty() {
            lines.push(String::new());
            lines.push("**Pin declarations:**".to_string());
            for pin in &iface.pins {
                lines.push(format!("- {}", pin.name.value));
            }
        }

        return Some(HoverInfo {
            content: lines.join("\n"),
        });
    }
    None
}

fn hover_for_import(import: &ImportDef, offset: usize) -> Option<HoverInfo> {
    if offset >= import.span.start && offset < import.span.end {
        let mut lines = vec!["**Import**".to_string()];
        lines.push(format!("Path: {}", import.path.value));

        if import.names.is_empty() {
            lines.push("Imports: all definitions".to_string());
        } else {
            let names: Vec<_> = import.names.iter().map(|n| n.value.as_str()).collect();
            lines.push(format!("Imports: {}", names.join(", ")));
        }

        return Some(HoverInfo {
            content: lines.join("\n"),
        });
    }
    None
}

fn hover_for_assert(assert_def: &AssertDef, offset: usize) -> Option<HoverInfo> {
    if offset >= assert_def.span.start && offset < assert_def.span.end {
        let mut lines = vec!["**Assertion**".to_string()];

        match &assert_def.expression {
            AssertExpression::Comparison {
                left, op, right, ..
            } => {
                let left_str = format_operand(left);
                let right_str = format_operand(right);
                lines.push(format!("Constraint: {} {:?} {}", left_str, op, right_str));
            }
            AssertExpression::Within { left, target, .. } => {
                let left_str = format_operand(left);
                let mut target_str = format!("{}{}", target.value, target.unit);
                if let Some(tol) = &target.tolerance {
                    use cypcb_parser::ast::ToleranceKind;
                    match &tol.kind {
                        ToleranceKind::Percentage { value } => {
                            target_str = format!("{} +/- {}%", target_str, value);
                        }
                        ToleranceKind::Absolute(abs) => {
                            target_str = format!("{} +/- {}{}", target_str, abs.value, abs.unit);
                        }
                        ToleranceKind::Range(upper) => {
                            target_str = format!("{} to {}{}", target_str, upper.value, upper.unit);
                        }
                    }
                }
                lines.push(format!("Constraint: {} within {}", left_str, target_str));
            }
        }

        return Some(HoverInfo {
            content: lines.join("\n"),
        });
    }
    None
}

fn format_operand(operand: &AssertOperand) -> String {
    match operand {
        AssertOperand::QualifiedName { parts, .. } => parts.join("."),
        AssertOperand::Physical(pv) => format!("{}{}", pv.value, pv.unit),
        AssertOperand::Dimension(dim) => format!("{}", dim),
        AssertOperand::Number { value, .. } => format!("{}", value),
    }
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

        let pos = Position {
            line: 1,
            character: 10,
        };
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

        let pos = Position {
            line: 1,
            character: 4,
        };
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

        let pos = Position {
            line: 0,
            character: 0,
        };
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

        let pos = Position {
            line: 1,
            character: 10,
        };
        let hover = hover_at_position(&doc, &pos);
        assert!(hover.is_some());

        let info = hover.unwrap();
        assert!(
            info.content.contains("Net connections"),
            "Should show net connections"
        );
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

        let pos = Position {
            line: 2,
            character: 10,
        };
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

        let pos = Position {
            line: 1,
            character: 4,
        };
        let hover = hover_at_position(&doc, &pos);
        assert!(hover.is_some());

        let info = hover.unwrap();
        assert!(info.content.contains("Current: 2A"), "Should show current");
        assert!(
            info.content.contains("IPC-2221"),
            "Should show IPC-2221 calculated width"
        );
    }

    #[test]
    fn test_hover_footprint_shows_details() {
        let doc = make_doc(
            r#"
component R1 resistor "0402" {}
"#,
        );

        // Hover on the footprint string
        let pos = Position {
            line: 1,
            character: 24,
        };
        let hover = hover_at_position(&doc, &pos);
        assert!(hover.is_some());

        let info = hover.unwrap();
        assert!(
            info.content.contains("Footprint: 0402"),
            "Should show footprint name"
        );
        assert!(info.content.contains("Courtyard"), "Should show courtyard");
        assert!(info.content.contains("Pads"), "Should show pad count");
    }

    #[test]
    fn test_calculate_trace_width() {
        // Test IPC-2221 calculation
        let width = calculate_trace_width(1.0); // 1A
        assert!(width.is_some());
        let (w, _notes) = width.unwrap();
        // 1A should give roughly 0.3-0.5mm for external, 10C rise
        assert!(
            w > 0.2 && w < 1.0,
            "Width {} should be reasonable for 1A",
            w
        );
    }

    #[test]
    fn test_calculate_trace_width_zero_current() {
        let width = calculate_trace_width(0.0);
        assert!(width.is_none());
    }

    #[test]
    fn test_hover_on_module() {
        let doc = make_doc(
            r#"
module PowerSupply {
    component U1 ic "SOT-23" {
        value "LDO-3V3"
    }
    pin VIN
    pin VOUT
    pin GND
}
"#,
        );

        let pos = Position {
            line: 1,
            character: 7,
        };
        let hover = hover_at_position(&doc, &pos);
        assert!(hover.is_some(), "Should have hover for module");

        let info = hover.unwrap();
        assert!(
            info.content.contains("Module: PowerSupply"),
            "Should show module name"
        );
        assert!(
            info.content.contains("Components: 1"),
            "Should show component count"
        );
        assert!(info.content.contains("VIN"), "Should show exposed pin");
        assert!(info.content.contains("VOUT"), "Should show exposed pin");
    }

    #[test]
    fn test_hover_on_interface() {
        let doc = make_doc(
            r#"
interface I2C {
    pin SDA
    pin SCL
}
"#,
        );

        let pos = Position {
            line: 1,
            character: 10,
        };
        let hover = hover_at_position(&doc, &pos);
        assert!(hover.is_some(), "Should have hover for interface");

        let info = hover.unwrap();
        assert!(
            info.content.contains("Interface: I2C"),
            "Should show interface name"
        );
        assert!(info.content.contains("SDA"), "Should show pin");
        assert!(info.content.contains("SCL"), "Should show pin");
    }

    #[test]
    fn test_hover_on_import() {
        let doc = make_doc(r#"import I2C, SPI from "std/interfaces.cypcb""#);

        let pos = Position {
            line: 0,
            character: 7,
        };
        let hover = hover_at_position(&doc, &pos);
        assert!(hover.is_some(), "Should have hover for import");

        let info = hover.unwrap();
        assert!(info.content.contains("Import"), "Should show import");
        assert!(
            info.content.contains("std/interfaces.cypcb"),
            "Should show path"
        );
        assert!(info.content.contains("I2C"), "Should show imported name");
    }

    #[test]
    fn test_hover_on_assert() {
        let doc = make_doc(r#"assert R1.value >= 10kohm"#);

        let pos = Position {
            line: 0,
            character: 7,
        };
        let hover = hover_at_position(&doc, &pos);
        assert!(hover.is_some(), "Should have hover for assert");

        let info = hover.unwrap();
        assert!(info.content.contains("Assertion"), "Should show assertion");
        assert!(
            info.content.contains("R1.value"),
            "Should show left operand"
        );
    }

    #[test]
    fn a_current_past_the_data_the_standard_was_fitted_to_is_named_on_the_card() {
        // The hover is where a designer asks what a width would have to be,
        // and for 40A the honest answer is a number plus the fact that
        // IPC-2221's curves were fitted to measurements up to about 35A.
        let doc = make_doc(
            r#"
net VBUS [current 40A] {
    R1.1
}
"#,
        );

        let pos = Position {
            line: 1,
            character: 4,
        };
        let info = hover_at_position(&doc, &pos).expect("a net hover");

        assert!(
            info.content.contains("accuracy degrades"),
            "40A is past the data the standard was fitted to: {}",
            info.content
        );
        assert!(
            info.content.contains("multiple parallel traces"),
            "and the width it produces is a bus bar: {}",
            info.content
        );
    }

    #[test]
    fn an_ordinary_current_leaves_the_card_plain() {
        // The half that keeps the other from being noise: 1A on an external
        // layer at 1oz and a 10C rise is inside every range the calculator
        // checks, so the card carries the width and nothing else.
        let doc = make_doc(
            r#"
net VCC [current 1A] {
    R1.1
}
"#,
        );

        let pos = Position {
            line: 1,
            character: 4,
        };
        let info = hover_at_position(&doc, &pos).expect("a net hover");

        assert!(
            info.content.contains("IPC-2221 width"),
            "the card still quotes a width: {}",
            info.content
        );
        assert!(
            !info.content.contains("Outside the standard"),
            "nothing about 1A is outside it: {}",
            info.content
        );
    }

    #[test]
    fn a_net_with_a_target_is_told_the_width_that_hits_it() {
        // The question Altium's calculator answers and this project only
        // answered after the copper existed: `cypcb check` reports how far off
        // a drawn trace is. A designer asks before drawing.
        let doc = make_doc_with_world(
            r#"
board rf {
    size 40mm x 20mm
    layers 2

    stackup {
        copper 1oz
        core 0.2mm material "FR4" dk 4.5
        copper 1oz
    }
}

net RF [impedance 50ohm] {
    R1.1
}
"#,
        );

        let pos = Position {
            line: 12,
            character: 4,
        };
        let info = hover_at_position(&doc, &pos).expect("a net hover");

        assert!(
            info.content.contains("Impedance: 50ohm"),
            "the card has to name the target it was given: {}",
            info.content
        );
        assert!(
            info.content.contains("Widths that give 50ohm"),
            "and the widths that hit it on this stack: {}",
            info.content
        );
        assert!(
            info.content.contains("- Top: 0.326mm"),
            "named per layer, because a net may end up on any of them: {}",
            info.content
        );
        assert!(
            info.content.contains("IPC-2141"),
            "with the form it came from, which is quoted at 5-7%: {}",
            info.content
        );
    }

    #[test]
    fn a_stack_that_cannot_answer_says_so_rather_than_guessing() {
        // An outer layer needs a dielectric under it stating both a thickness
        // and a dk. This one states neither.
        let doc = make_doc_with_world(
            r#"
board rf {
    size 40mm x 20mm
    layers 2

    stackup {
        copper 1oz
        core 0.2mm
        copper 1oz
    }
}

net RF [impedance 50ohm] {
    R1.1
}
"#,
        );

        let pos = Position {
            line: 12,
            character: 4,
        };
        let info = hover_at_position(&doc, &pos).expect("a net hover");

        assert!(
            info.content.contains("Impedance: 50ohm"),
            "{}",
            info.content
        );
        assert!(
            info.content
                .contains("The stack cannot answer on any layer"),
            "a number invented for a stack that states no dk reads like a measurement: {}",
            info.content
        );
    }

    #[test]
    fn a_board_with_no_stack_is_asked_nothing() {
        // Most boards. There is no stack to answer for, so the card carries
        // the target and stops - the half that keeps the other two from being
        // noise on every design that never mentions a stackup.
        let doc = make_doc_with_world(
            r#"
net RF [impedance 50ohm] {
    R1.1
}
"#,
        );

        let pos = Position {
            line: 1,
            character: 4,
        };
        let info = hover_at_position(&doc, &pos).expect("a net hover");

        assert!(
            info.content.contains("Impedance: 50ohm"),
            "{}",
            info.content
        );
        assert!(
            !info.content.contains("cannot answer") && !info.content.contains("Widths that give"),
            "nothing to say about a stack that was never stated: {}",
            info.content
        );
    }

    #[test]
    fn a_four_layer_stack_answers_for_every_layer_it_can() {
        // The reason the card stopped picking one layer: a net routed inside
        // wants the stripline figure, and a net that is not routed yet has no
        // layer to ask about. Both inner layers here are centred between
        // matching dielectrics, which is the only inner case the closed forms
        // in `cypcb-calc` cover.
        let doc = make_doc_with_world(
            r#"
board rf4 {
    size 40mm x 20mm
    layers 4

    stackup {
        copper 1oz
        prepreg 0.2mm material "FR4" dk 4.5
        copper 1oz
        core 0.2mm material "FR4" dk 4.5
        copper 1oz
        prepreg 0.2mm material "FR4" dk 4.5
        copper 1oz
    }
}

net RF [impedance 50ohm] {
    R1.1
}
"#,
        );

        let pos = Position {
            line: 16,
            character: 4,
        };
        let info = hover_at_position(&doc, &pos).expect("a net hover");

        // Measured: 0.326mm on either face over one 0.2mm dielectric, 0.118mm
        // between two of them.
        assert!(
            info.content.contains("- Top: 0.326mm") && info.content.contains("- Inner1: 0.118mm"),
            "the four rows this stack produces: {}",
            info.content
        );

        for layer in ["- Top: ", "- Inner1: ", "- Inner2: ", "- Bottom: "] {
            assert!(
                info.content.contains(layer),
                "every layer the stack can answer for gets a row, and `{layer}` has none: {}",
                info.content
            );
        }

        // An inner layer sits between two planes and needs less copper for the
        // same impedance than an outer one over a single dielectric of the
        // same thickness. Said as an order rather than as two numbers.
        let width_of = |layer: &str| -> f64 {
            let at = info.content.find(layer).expect("the row is there") + layer.len();
            let rest = &info.content[at..];
            let end = rest.find("mm").expect("a width in mm");
            rest[..end].trim().parse().expect("a number")
        };
        assert!(
            width_of("- Inner1: ") < width_of("- Top: "),
            "a stripline is narrower than a microstrip on the same dielectric: {}",
            info.content
        );
    }
}
