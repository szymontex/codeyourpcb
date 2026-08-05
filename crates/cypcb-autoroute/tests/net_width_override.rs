//! A design that states a net's width has to get it.

use cypcb_autoroute::{route_board, AutorouteConfig};
use cypcb_core::Nm;
use cypcb_router::types::RoutingStatus;
use cypcb_rules::presets::{PresetRuleSet, RulesPreset};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{sync_ast_to_world, BoardWorld};

/// Two resistors, one net told to be wide and one left alone.
const SOURCE: &str = r#"version 1

board t {
    size 40mm x 20mm
    layers 2
}

component R1 resistor "0402" {
    value "10k"
    at 10mm, 10mm
}

component R2 resistor "0402" {
    value "10k"
    at 25mm, 10mm
}

net VCC [width 0.4mm] {
    R1.1
    R2.1
}

net SIG {
    R1.2
    R2.2
}
"#;

#[test]
fn a_net_that_states_its_width_is_routed_at_it() {
    let parsed = cypcb_parser::parse(SOURCE);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);

    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    let sync = sync_ast_to_world(&parsed.value, SOURCE, &mut world, &mut library);
    assert!(sync.errors.is_empty(), "{:?}", sync.errors);

    let rules = PresetRuleSet::new(RulesPreset::from_name("jlcpcb").unwrap());
    let preset_width = {
        use cypcb_rules::RoutingRuleSet;
        rules.constraints_for_net(0).min_trace_width
    };
    assert!(
        preset_width < Nm::from_mm(0.4),
        "the test only means something if the design asks for more than the fab floor"
    );

    let result = route_board(&mut world, &library, &rules, &AutorouteConfig::default());
    assert!(
        matches!(result.status, RoutingStatus::Complete),
        "{:?}",
        result.status
    );

    let vcc = world.get_net("VCC").expect("VCC interned");
    let sig = world.get_net("SIG").expect("SIG interned");

    let widths = |net| -> Vec<Nm> {
        result
            .routes
            .iter()
            .filter(|segment| segment.net_id == net)
            .map(|segment| segment.width)
            .collect()
    };

    let vcc_widths = widths(vcc);
    let sig_widths = widths(sig);

    assert!(!vcc_widths.is_empty(), "VCC was not routed at all");
    assert!(!sig_widths.is_empty(), "SIG was not routed at all");

    assert!(
        vcc_widths.iter().all(|width| *width == Nm::from_mm(0.4)),
        "the design said 0.4mm, got {vcc_widths:?}"
    );
    assert!(
        sig_widths.iter().all(|width| *width == preset_width),
        "a net that says nothing keeps the preset {preset_width:?}, got {sig_widths:?}"
    );
}
