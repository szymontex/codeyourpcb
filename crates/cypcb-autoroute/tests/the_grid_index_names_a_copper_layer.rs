//! Two conversions the census missed, and the claim that missed them.
//!
//! `cargo test -p cypcb-autoroute --test the_grid_index_names_a_copper_layer`
//!
//! V1's helper census was recorded as finished on 2026-08-28 - "every
//! arithmetic helper it listed now has a test that names it". It was not:
//! three of the names an earlier entry listed still had none, because the
//! sweep took the three that were in front of it and the entry generalised.
//! This is one of them.
//!
//! `index_to_layer` is the shape of the three index errors this project has
//! shipped: a grid layer is a number, a copper layer is a name, and the two
//! are off by two in the middle of the board. `Layer::Inner(0)` is the first
//! inner layer and grid index 2 is where it sits.

use cypcb_autoroute::grid::index_to_layer;
use cypcb_world::Layer;

#[test]
fn the_two_outer_layers_are_the_first_two_indices() {
    // The router searches the top layer first because bit 0 is the top face,
    // and every board has these two whatever its layer count.
    assert_eq!(index_to_layer(0), Layer::TopCopper);
    assert_eq!(index_to_layer(1), Layer::BottomCopper);
}

#[test]
fn an_inner_layer_is_two_less_than_its_index() {
    // The off-by-two that has bitten this project three times: the grid counts
    // from the top face, the language counts inner layers from one, and the
    // model counts them from zero. Index 2 is `Inner(0)`, which the language
    // calls `Inner1`.
    assert_eq!(index_to_layer(2), Layer::Inner(0));
    assert_eq!(index_to_layer(3), Layer::Inner(1));
    assert_eq!(index_to_layer(6), Layer::Inner(4));
}

#[test]
fn index_order_is_not_stack_order() {
    // Four indices, four different layers - and not in the order they sit in
    // the board. The grid numbers the two outer faces first because a route
    // starts on one of them; the physical stack puts the bottom last. A reader
    // who assumes the second is the second-from-top gets a plane wrong, which
    // is exactly the mistake this conversion exists to make impossible.
    let layers: Vec<Layer> = (0..4).map(index_to_layer).collect();
    assert_eq!(
        layers,
        vec![
            Layer::TopCopper,
            Layer::BottomCopper,
            Layer::Inner(0),
            Layer::Inner(1)
        ],
        "the grid numbers the outer faces first"
    );

    let mut distinct = layers.clone();
    distinct.dedup();
    assert_eq!(
        distinct.len(),
        layers.len(),
        "a conversion that collapsed two indices onto one layer would route \
         two planes as one"
    );
}
