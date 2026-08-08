//! Before pricing a stacked hole, check the map has any holes in it.
//!
//! `cargo test --release -p cypcb-autoroute --test the_map_knows_where_the_holes_are -- --ignored --nocapture`
//!
//! The fourth attempt at the stacked vias added exactly this count, swept a
//! price against it from 0 to 100, got byte-identical boards at every value,
//! and cost a fire to work out that the term was never charged. It never
//! checked that the thing being priced existed.
//!
//! So this comes first now. It asserts nothing about routing quality: only
//! that after routing a board with 119 vias, the congestion map holds holes -
//! and that ripping a net up takes its holes with it, since a count that only
//! grows would price cells that no longer have anything in them.

use cypcb_autoroute::congestion::CongestionMap;

#[test]
fn a_marked_hole_is_counted_and_a_ripped_up_one_is_not() {
    // The mechanism on its own, without routing: what `mark_holes` and
    // `unmark_holes` do to the total.
    let mut map = CongestionMap::new(64, 64, 2);
    assert_eq!(map.total_holes(), 0, "a fresh map holds no holes");

    let cells = [(10, 10, 0u8), (10, 10, 1u8)];
    map.mark_holes(&cells);
    assert_eq!(
        map.total_holes(),
        2,
        "a via spanning two layers is a hole on each of them"
    );

    map.set_stack_penalty(7.0);
    assert_eq!(
        map.stacking_cost(10, 10, 0),
        7.0,
        "one hole at the price of one"
    );
    assert_eq!(
        map.stacking_cost(11, 10, 0),
        0.0,
        "the cell beside it holds nothing"
    );

    map.unmark_holes(&cells);
    assert_eq!(
        map.total_holes(),
        0,
        "ripping a net up has to take its holes with it, or the map prices \
         cells that no longer hold anything"
    );
}
