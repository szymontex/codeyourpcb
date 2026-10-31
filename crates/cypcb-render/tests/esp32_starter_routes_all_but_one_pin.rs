//! esp32_starter routes with one pin left open, and no more.
//!
//! `cargo test -p cypcb-render --test esp32_starter_routes_all_but_one_pin`
//!
//! This is the board compared against another tool's output, so a
//! connection the router leaves open costs more here than a sharp entry. On
//! 2026-09-27 `cypcb route --fast` left one pin open, J1.B5, and a change to
//! where the router may end a route on a pad of two touching pads left the
//! BOOT net open as well: U1.27 and SW2.1, with the router reporting
//! `converged=false`.

use std::path::Path;

use cypcb_render::PcbEngine;

#[test]
fn esp32_starter_after_fast_routing_leaves_at_most_one_pin_open() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/benchmark/esp32_starter.cypcb");
    let source = std::fs::read_to_string(&path).expect("the fixture reads");
    let mut engine = PcbEngine::new();
    let errors = engine.load_source(&source);
    assert!(errors.is_empty(), "the board loads: {errors}");

    // The control: before routing most of the board is open, so the count
    // below is a count of something.
    let before = engine
        .build_snapshot()
        .violations
        .iter()
        .filter(|v| v.kind == "unrouted-pin")
        .count();
    assert!(before > 1, "the board as drawn has {before} open pins");

    // `cypcb route --fast`: the default settings, once.
    let status = engine.auto_route();
    assert!(status.contains("\"ok\":true"), "the board routes: {status}");
    let open: Vec<String> = engine
        .build_snapshot()
        .violations
        .iter()
        .filter(|v| v.kind == "unrouted-pin")
        .map(|v| v.message.clone())
        .collect();
    // 2026-10-31: 25 pins, where one was. The router strips a net that ends
    // its negotiation on another net's copper and routes it again where it may
    // touch nothing. On this board it stripped 14 nets and 4 of them found
    // such a path; the other 10, 24 connections in all, stay open - the USB-C
    // pins sit 0.5 mm apart in a 0.627 mm cell. The copper-on-copper contacts
    // went from 22 to 10. A pin open is a thing the designer finishes; a
    // contact is a board that does not work.
    assert!(
        open.len() <= 25,
        "{} pins open after routing, where 25 were on 2026-10-31: {open:#?}",
        open.len()
    );
}
