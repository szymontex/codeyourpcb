//! `cypcb route` and `cypcb check` have to agree about the board route wrote.
//!
//! `cargo test -p cypcb-cli --test the_routed_board_grades_the_same_twice`
//!
//! `route` prints what the checker will say about the file it is about to
//! write - that line exists because the scorer's own number and the checker's
//! had disagreed by a factor of six, and a user should see the second one.
//! Measured on `examples/blink.cypcb` routed in-house:
//!
//! ```text
//! cypcb route --in-house  -> DRC on the routed board: 4 violations, 2 of them copper touching copper
//! cypcb check blink-routed.cypcb -> 5 DRC violation(s), copper touching copper at 0.00mm: 3
//! ```
//!
//! Two commands, one board, two answers, and the number a user acts on is
//! whichever they happened to run. `route` measures the world it just routed;
//! `check` measures the file. Anything the writer does to the board between
//! them lands here.

use std::process::Command;

/// Route the named example in-house and hand back both numbers - what `route`
/// says about the board, and what `check` says about the file it wrote - along
/// with the file itself, because the disagreement between the two has twice
/// turned out to be something the writer did rather than something either
/// checker measured.
fn routed_then_checked(example: &str) -> (usize, usize, String) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("the crate sits two levels below the repo root");
    let dir = std::env::temp_dir().join("cypcb-grade-twice");
    std::fs::create_dir_all(&dir).expect("a place to work");
    let out = dir.join(format!("{example}.routed.cypcb"));

    let routing = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .args(["route", "--in-house"])
        .arg(root.join(format!("examples/{example}.cypcb")))
        .arg("--output")
        .arg(&out)
        .output()
        .expect("the binary runs");
    assert!(
        routing.status.success(),
        "{}",
        String::from_utf8_lossy(&routing.stderr)
    );
    let said = String::from_utf8_lossy(&routing.stderr);
    let route_count = said
        .lines()
        .find_map(|line| line.strip_prefix("DRC on the routed board: "))
        .and_then(|rest| rest.split(' ').next())
        .and_then(|number| number.parse().ok())
        .unwrap_or_else(|| panic!("route did not say what the checker will say:\n{said}"));

    let checking = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .arg("check")
        .arg(&out)
        .output()
        .expect("the binary runs");
    // A pass goes to stdout, a report of faults to stderr.
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&checking.stdout),
        String::from_utf8_lossy(&checking.stderr)
    );
    let check_count = if report.contains("passed DRC") {
        0
    } else {
        report
            .lines()
            .find(|line| line.contains("DRC violation(s)"))
            .and_then(|line| line.split(" DRC violation(s)").next())
            .and_then(|number| number.trim().parse().ok())
            .unwrap_or_else(|| panic!("check did not say how many:\n{report}"))
    };

    let written = std::fs::read_to_string(&out).expect("route wrote the board it named");

    (route_count, check_count, written)
}

#[test]
fn the_number_route_prints_is_the_number_check_prints() {
    let (routed, checked, _) = routed_then_checked("blink");

    assert_eq!(
        routed, checked,
        "route says {routed} violations about the board it wrote and check says \
         {checked} about the file it wrote it to"
    );
}

#[test]
fn copper_the_source_drew_is_kept_and_not_written_again() {
    // The rule, decided here: copper already on a net is kept and left alone.
    // `route` writes its answer by appending trace blocks to a copy of the
    // source, so the source's copper is in the file before the writer adds a
    // character - and the writer used to append the whole world on top of it,
    // which put every hand-drawn trace in the file twice.
    //
    // `examples/routing-test.cypcb` is the board that shows it: it declares
    // one `trace VCC` by hand, the router finds that net already connected and
    // routes only GND. The written file used to carry VCC twice, and the two
    // commands disagreed about it - `route` said 0 violations because the
    // world it measured held one copy, `check` said 3 because the file it read
    // held two and `acute-angle` calls copper drawn over itself exactly that.
    let (routed, checked, written) = routed_then_checked("routing-test");

    assert_eq!(
        written.matches("trace VCC {").count(),
        1,
        "the hand-drawn trace belongs in the file once:\n{written}"
    );
    assert!(
        written.contains("path 9.5mm,12mm -> 9.5mm,8mm -> 16.5mm,8mm"),
        "and it is the source's own line, untouched, not a re-written copy:\n{written}"
    );

    // The positive control. A writer that appended nothing at all would pass
    // both assertions above and produce a board with no routing in it.
    assert_eq!(
        written.matches("trace GND {").count(),
        1,
        "the copper the router did lay has to be in the file:\n{written}"
    );

    assert_eq!(
        (routed, checked),
        (0, 0),
        "route says {routed} about the board and check says {checked} about the file"
    );
}
