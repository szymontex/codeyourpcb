//! How wide a routing result's own noise is, per benchmark board.
//!
//! PathFinder re-routes whatever passes through overused cells, so one cell
//! changing hands early puts a different set of nets in the next iteration.
//! Two settings a hundredth apart therefore produce boards tens of violations
//! apart without either setting being better. A diagnostic that reports "this
//! knob won by 12 violations" without saying how wide that board's noise is
//! reads as a finding when it is a coin landing.
//!
//! The numbers below are the spread each board shows across via prices
//! 0.22..0.28 - five settings that ask the router for nearly the same trade.
//! They lived in two test files with one of them stale by the time the other
//! was corrected, which is why they are here: a band quoted in two places is a
//! band that will disagree with itself.
//!
//! Measured 2026-08-21 by `cargo test --release -p cypcb-autoroute --test
//! via_price_sweep how_much_of_the_price_is_noise -- --ignored --nocapture`,
//! which prints each pair on a line naming the fab table it used. Each board
//! is graded on the table its own layer count asks for, so `multi_ic` - the
//! four-layer benchmark - is measured against `jlcpcb_standard_4layer`. Its
//! earlier pair of 65 / 56 was measured on the two-layer table and was wrong
//! by more than the differences this band is used to judge.

/// The measured noise band for one benchmark: violations, then shorts.
///
/// A move smaller than the band is the negotiation going differently rather
/// than a better setting. Boards this table does not know return `(0, 0)`,
/// which reads every difference as signal - the safe direction for a value
/// nobody has measured, because it invites a look rather than hiding one.
///
/// `led_blink` is the one board `how_much_of_the_price_is_noise` skips. It
/// routes to a single violation at every price in that range, so its zero is
/// an observation rather than a measurement, and it is written here as zero
/// for the same reason the sweep skips it: there is nothing to spread.
pub fn noise_band(filename: &str) -> (i64, i64) {
    match filename {
        "led_blink.kicad_pcb" => (0, 0),
        "stm32_breakout.kicad_pcb" => (59, 61),
        "multi_ic.kicad_pcb" => (34, 49),
        "shift_driver.kicad_pcb" => (17, 8),
        "qfp_fanout.kicad_pcb" => (60, 46),
        "plane_board.kicad_pcb" => (0, 0),
        _ => (0, 0),
    }
}

/// The measured noise band for one benchmark's stacked holes.
///
/// Holes the router leaves on top of each other, and the spread of that count
/// across the same five via prices. `docs/routing.md` has a table pricing a
/// stack penalty and read it against the *violation* band, which answers a
/// different question: a knob that trades violations for stacking has to be
/// read on both columns, and until 2026-08-21 only one of them had a band.
///
/// Measured 2026-08-21 by the same command as [`noise_band`], which prints it
/// as `stacked-hole band: N (lo to hi across the five prices)`.
///
/// The two boards at zero really do route without stacking a hole at any of
/// the five prices, so any movement on them is signal. `led_blink` is the
/// board that sweep skips; it routes four vias and stacks none of them, which
/// is an observation rather than a spread.
pub fn stacked_hole_band(filename: &str) -> i64 {
    match filename {
        "led_blink.kicad_pcb" => 0,
        "stm32_breakout.kicad_pcb" => 3,
        "multi_ic.kicad_pcb" => 5,
        "shift_driver.kicad_pcb" => 0,
        "qfp_fanout.kicad_pcb" => 24,
        "plane_board.kicad_pcb" => 0,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::{noise_band, stacked_hole_band};
    use cypcb_kicad::BENCHMARKS;

    /// Every benchmark has a band, and none of them got it from the fallback.
    ///
    /// The fallback exists so an unknown board reads as all-signal, not so a
    /// benchmark can quietly fall through it when somebody renames a fixture.
    /// A renamed file would keep passing every diagnostic while silently
    /// losing its band, which is exactly the failure this module was made to
    /// stop.
    #[test]
    fn every_benchmark_has_a_measured_band() {
        let missing: Vec<&str> = BENCHMARKS
            .iter()
            .map(|b| b.filename)
            .filter(|f| !KNOWN.contains(f))
            .collect();
        assert!(
            missing.is_empty(),
            "these benchmarks would fall through to the unknown-board fallback \
             and read every difference as signal: {missing:?}. Measure them \
             with `via_price_sweep how_much_of_the_price_is_noise` and add the \
             pair to `noise_band`"
        );
    }

    /// The filenames `noise_band` answers about, listed so the test above can
    /// tell an answer from the fallback. Kept beside the match it mirrors.
    const KNOWN: &[&str] = &[
        "led_blink.kicad_pcb",
        "stm32_breakout.kicad_pcb",
        "multi_ic.kicad_pcb",
        "shift_driver.kicad_pcb",
        "qfp_fanout.kicad_pcb",
        "plane_board.kicad_pcb",
    ];

    /// The list above is a mirror, so it has to be checked against the thing
    /// it mirrors rather than trusted. A name in `KNOWN` that `noise_band` has
    /// no arm for returns the fallback, and the two boards whose real band is
    /// `(0, 0)` are named here so a genuine zero is not read as a miss.
    #[test]
    fn the_known_list_matches_the_match_arms() {
        let genuinely_zero = ["led_blink.kicad_pcb", "plane_board.kicad_pcb"];
        let phantom: Vec<&&str> = KNOWN
            .iter()
            .filter(|f| !genuinely_zero.contains(*f))
            .filter(|f| noise_band(f) == (0, 0))
            .collect();
        assert!(
            phantom.is_empty(),
            "these names are in KNOWN but `noise_band` has no arm for them, so \
             they get the fallback: {phantom:?}"
        );
    }

    /// The same check for the stacking band, whose genuine zeros are a
    /// different set of boards.
    ///
    /// Three of the six really do route without stacking a hole at any price,
    /// so the two lists cannot be shared: a board that is a genuine zero here
    /// is not one above, and reusing one list would excuse a missing arm.
    #[test]
    fn the_known_list_matches_the_stacking_arms() {
        let genuinely_zero = [
            "led_blink.kicad_pcb",
            "plane_board.kicad_pcb",
            "shift_driver.kicad_pcb",
        ];
        let phantom: Vec<&&str> = KNOWN
            .iter()
            .filter(|f| !genuinely_zero.contains(*f))
            .filter(|f| stacked_hole_band(f) == 0)
            .collect();
        assert!(
            phantom.is_empty(),
            "these names are in KNOWN but `stacked_hole_band` has no arm for \
             them, so they get the fallback: {phantom:?}"
        );
    }
}
