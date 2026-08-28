//! A thin board's thickness tolerance is not a percentage of it.
//!
//! `cargo test -p cypcb-export --test what_a_house_publishes_about_being_off`
//!
//! JLCPCB publishes two rules rather than one - "± 10%" at 1.0mm and above,
//! and "± 0.1mm" below it - because ten percent of a 0.4mm board is 0.04mm,
//! which is finer than the press can hold. A writer that knew only the
//! percentage would state a tolerance no fabricator agreed to.

use cypcb_core::Nm;
use cypcb_export::ipc2581::HouseTolerances;

/// What the capabilities page publishes, as this project reads it.
fn jlcpcb() -> HouseTolerances {
    HouseTolerances {
        thickness_percent: Some(10),
        thickness_thin: Some(Nm::from_mm(0.1)),
        hole_plus: Some(Nm::from_mm(0.13)),
        hole_minus: Some(Nm::from_mm(0.08)),
    }
}

#[test]
fn a_board_of_a_millimetre_or_more_gets_the_percentage() {
    let house = jlcpcb();
    assert_eq!(
        house.thickness(Nm::from_mm(1.6)),
        Some(Nm::from_mm(0.16)),
        "ten percent of 1.6mm"
    );
    assert_eq!(
        house.thickness(Nm::from_mm(1.0)),
        Some(Nm::from_mm(0.1)),
        "and the rule starts at exactly one millimetre"
    );
}

#[test]
fn a_thinner_board_gets_the_figure_published_for_thin_boards() {
    let house = jlcpcb();
    assert_eq!(
        house.thickness(Nm::from_mm(0.8)),
        Some(Nm::from_mm(0.1)),
        "0.1mm rather than the 0.08mm a percentage would give"
    );
    assert_eq!(
        house.thickness(Nm::from_mm(0.4)),
        Some(Nm::from_mm(0.1)),
        "and the same figure however thin it gets"
    );
}

#[test]
fn a_house_that_published_nothing_answers_nothing() {
    // The difference between zero and unknown is the difference between a
    // promise and a silence, and only one of them is honest here.
    let silent = HouseTolerances::default();
    assert_eq!(silent.thickness(Nm::from_mm(1.6)), None);
    assert_eq!(silent.thickness(Nm::from_mm(0.8)), None);

    // A house that published only the thick rule says nothing about a thin
    // board rather than applying a rule it never stated to one.
    let partial = HouseTolerances {
        thickness_percent: Some(10),
        ..HouseTolerances::default()
    };
    assert_eq!(partial.thickness(Nm::from_mm(1.6)), Some(Nm::from_mm(0.16)));
    assert_eq!(partial.thickness(Nm::from_mm(0.8)), None);
}
