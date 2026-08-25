//! IPC-2221's current form, held to two geometries worked by hand.
//!
//! `cargo test -p cypcb-calc --test the_current_form_is_worked_by_hand`
//!
//! `TraceWidthCalculator` is the number behind every `trace-current`
//! violation: how wide a trace has to be to carry a current at a stated
//! temperature rise. Its own tests state bands - `8.0 < mils < 13.0` for 1A,
//! `5.0 < mm < 10.0` for 10A - which is the shape of test the impedance forms
//! had until 2026-08-25, and a constant a few percent wrong passes every one
//! of them.
//!
//! The form is `I = k * dT^0.44 * A^0.725`, solved for area, with `k` = 0.048
//! for an outer layer and 0.024 for an inner one. Two cases here are computed
//! from it and written out beside the assertion, and the halving of `k` is
//! what the second holds: an inner layer at 1A needs the copper an outer layer
//! needs at 2A, which is the same cross-section by another route.

use cypcb_calc::{TraceWidthCalculator, TraceWidthParams};

/// Millimetres, to the micron: the width a fabricator would be given.
fn width_mm(params: &TraceWidthParams) -> f64 {
    (TraceWidthCalculator::calculate(params).width.to_mm() * 1_000.0).round() / 1_000.0
}

#[test]
fn one_amp_on_an_outer_layer_is_this_wide() {
    // 1A, 1oz copper, external, 10C rise:
    //
    //   dT^0.44          = 10^0.44                     = 2.7542
    //   k x dT^0.44      = 0.048 x 2.7542              = 0.13220
    //   A = (I / that)^(1/0.725) = (7.5642)^1.3793     = 16.296 mil^2
    //   width = A / 1.378 mil                          = 11.826 mil
    //                                                  = 0.300mm
    //
    // The same figure reaches a person through the checker:
    // `IPC-2221 wants 0.300mm on an outer layer at 1.0oz copper and a 10C
    // rise`.
    assert_eq!(width_mm(&TraceWidthParams::new(1.0)), 0.300);
}

#[test]
fn two_amps_needs_the_cross_section_the_form_asks_for() {
    //   A = (2 / 0.13220)^1.3793 = (15.128)^1.3793     = 42.393 mil^2
    //   width = 42.393 / 1.378                         = 30.764 mil
    //                                                  = 0.781mm
    assert_eq!(width_mm(&TraceWidthParams::new(2.0)), 0.781);
}

#[test]
fn an_inner_layer_at_one_amp_needs_what_an_outer_needs_at_two() {
    // `k` halves inside the board, because the copper there is wrapped in
    // laminate and has nowhere to put its heat. Halving `k` moves the same
    // current onto the cross-section the outer layer needs at twice it - the
    // two numbers meet at 42.393 mil^2, by two different routes through the
    // form.
    let inner = TraceWidthParams::new(1.0).internal();
    assert_eq!(width_mm(&inner), 0.781);
    assert_eq!(width_mm(&inner), width_mm(&TraceWidthParams::new(2.0)));
}

#[test]
fn a_hotter_trace_may_be_narrower() {
    // The other variable in the form, and the direction it moves: a design
    // that will tolerate 20C of rise can carry the same current on less
    // copper.
    let cool = width_mm(&TraceWidthParams::new(1.0));
    let warm = width_mm(&TraceWidthParams::new(1.0).with_temp_rise(20.0));
    assert!(
        warm < cool,
        "20C of rise carries 1A on {warm}mm where 10C needs {cool}mm"
    );
}
