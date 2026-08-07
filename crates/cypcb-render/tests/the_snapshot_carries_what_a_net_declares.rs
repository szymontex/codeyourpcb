//! What a design says about a net has to reach the screen.
//!
//! `cargo test -p cypcb-render --features native --test the_snapshot_carries_what_a_net_declares`
//!
//! A net's width, clearance and current are the three things a `.cypcb` file
//! can state about copper it has not drawn yet. The viewer uses them to pick
//! the width of a trace the user is about to draw, and to show what the design
//! demands. They were read from a side map built by walking the AST, which had
//! two holes: only the native build looked at it, and it saw only constraints
//! written on the `net` block itself - a net that gets its width from a
//! `netclass` had none.
//!
//! The world knows both, because `sync_ast_to_world` merges them. These tests
//! ask the snapshot.

use cypcb_render::PcbEngine;

/// Build an engine holding the board a source string describes.
fn engine_for(source: &str) -> PcbEngine {
    let mut engine = PcbEngine::new();
    let errors = engine.load_source(source);
    assert!(
        errors.is_empty(),
        "the board must load: {errors:?} for source:\n{source}"
    );
    engine
}

const STATED_ON_THE_NET: &str = r#"
board constrained {
    size 20mm x 20mm
    layers 2
}

component R1 resistor "0805" {
    value "10k"
    at 5mm, 10mm
}

component R2 resistor "0805" {
    value "10k"
    at 15mm, 10mm
}

net VCC [width 0.5mm clearance 0.25mm current 3A] {
    R1.1
    R2.1
}
"#;

/// The same board, with the constraints coming from a class instead.
const STATED_ON_A_NETCLASS: &str = r#"
board constrained {
    size 20mm x 20mm
    layers 2
}

component R1 resistor "0805" {
    value "10k"
    at 5mm, 10mm
}

component R2 resistor "0805" {
    value "10k"
    at 15mm, 10mm
}

netclass Power [width 0.5mm clearance 0.25mm current 3A] {
    VCC
}

net VCC {
    R1.1
    R2.1
}
"#;

#[test]
fn a_net_that_states_its_own_constraints_carries_them_into_the_snapshot() {
    let mut engine = engine_for(STATED_ON_THE_NET);
    let snapshot = engine.build_snapshot();

    let vcc = snapshot
        .nets
        .iter()
        .find(|net| net.name == "VCC")
        .expect("VCC is in the snapshot");

    assert_eq!(vcc.width_nm, Some(500_000), "width");
    assert_eq!(vcc.clearance_nm, Some(250_000), "clearance");
    assert_eq!(vcc.current_ma, Some(3000.0), "current");
}

#[test]
fn a_net_that_gets_its_constraints_from_a_class_carries_them_too() {
    // The case the AST walk could not see: nothing is written on the `net`
    // block, and the class fills it in during sync. A viewer reading the walk
    // drew this net at the default width while the router used 0.5mm.
    let mut engine = engine_for(STATED_ON_A_NETCLASS);
    let snapshot = engine.build_snapshot();

    let vcc = snapshot
        .nets
        .iter()
        .find(|net| net.name == "VCC")
        .expect("VCC is in the snapshot");

    assert_eq!(vcc.width_nm, Some(500_000), "width from the class");
    assert_eq!(vcc.clearance_nm, Some(250_000), "clearance from the class");
    assert_eq!(vcc.current_ma, Some(3000.0), "current from the class");
}

#[test]
fn a_net_with_nothing_stated_says_nothing_rather_than_guessing() {
    // The absence has to survive too: `None` means "use the default", and a
    // snapshot that invented a width would have the viewer draw a number the
    // design never asked for.
    let mut engine = engine_for(
        r#"
board plain {
    size 20mm x 20mm
    layers 2
}

component R1 resistor "0805" {
    value "10k"
    at 5mm, 10mm
}

component R2 resistor "0805" {
    value "10k"
    at 15mm, 10mm
}

net SIG {
    R1.1
    R2.1
}
"#,
    );
    let snapshot = engine.build_snapshot();

    let sig = snapshot
        .nets
        .iter()
        .find(|net| net.name == "SIG")
        .expect("SIG is in the snapshot");

    assert_eq!(sig.width_nm, None);
    assert_eq!(sig.clearance_nm, None);
    assert_eq!(sig.current_ma, None);
}
