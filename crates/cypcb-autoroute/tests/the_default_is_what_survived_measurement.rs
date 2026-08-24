//! What the router ships with, against what was measured and dropped.
//!
//! `cargo test -p cypcb-autoroute --test the_default_is_what_survived_measurement`
//!
//! `docs/routing.md` tabulates eighteen instruments that were built, measured
//! and reverted, and the phase map says two were kept. That was a sentence
//! nobody could check: the knobs are still in `AutorouteConfig` - reverting an
//! instrument here meant setting its price to zero rather than deleting it -
//! so the difference between "measured and dropped" and "shipped" is a set of
//! default values and nothing held them.
//!
//! Every number in `docs/routing.md`, every ratchet in `benchmark_validation`
//! and every figure this project has published about routing quality was
//! produced with these defaults. Turning one on is a re-baseline of all of
//! them, which is a decision rather than an edit - so it fails here first.

use cypcb_autoroute::AutorouteConfig;

#[test]
fn every_instrument_that_lost_is_priced_at_nothing() {
    let shipped = AutorouteConfig::default();

    // Each of these was built, measured on the benchmark set and reverted.
    // `docs/routing.md` carries the numbers; the price is what reverting
    // meant.
    let dropped: [(&str, f64); 5] = [
        ("via_ring_penalty", shipped.via_ring_penalty),
        ("via_stack_penalty", shipped.via_stack_penalty),
        ("via_foreign_pad_penalty", shipped.via_foreign_pad_penalty),
        ("clearance_barrier", shipped.clearance_barrier),
        ("foreign_pad_penalty", shipped.foreign_pad_penalty),
    ];

    for (name, price) in dropped {
        assert_eq!(
            price, 0.0,
            "`{name}` is priced in the shipped default, and it was measured and dropped - \
             every published routing figure was produced without it"
        );
    }

    assert!(
        !shipped.pad_zone_blocks_foreign_copper,
        "blocking a foreign net's pad inside the routing net's pad zone lost six connections \
         on stm32_breakout"
    );
}

#[test]
fn the_two_that_were_kept_are_the_two_that_are_on() {
    // The phase map's "two kept", named rather than counted: pricing the
    // copper a via's keepout actually covers, and reserving the footprint a
    // trace has already taken. The pattern the whole table points at is that
    // pricing copper that exists pays and pricing space somebody might want
    // does not, and these are the two that price copper that exists.
    let shipped = AutorouteConfig::default();

    assert_eq!(
        shipped.via_foreign_copper_penalty, 0.25,
        "the via keepout's price on foreign copper is the first of the two kept"
    );
    assert!(
        shipped.reserve_trace_footprint,
        "reserving the footprint a routed trace took is the second"
    );
}

#[test]
fn the_grid_is_derived_from_the_fab_rather_than_fixed() {
    // Not an instrument but the setting under all of them: one cell per legal
    // track position, so neighbouring cells are clearance-legal by
    // construction. Measured on stm32_breakout at 238 violations in 127.8s
    // with a half-clearance grid against 124 in 9.7s at track pitch.
    assert!(
        AutorouteConfig::default().grid_resolution_nm.is_none(),
        "a fixed grid would ignore the fab table the board is checked against"
    );
}
