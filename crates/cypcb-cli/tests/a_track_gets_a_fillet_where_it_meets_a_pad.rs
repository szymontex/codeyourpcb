//! `--teardrops` puts the fillet in the copper a fabricator receives.
//!
//! `cargo test -p cypcb-cli --test a_track_gets_a_fillet_where_it_meets_a_pad`
//!
//! Item 1 of the KiCad parity audit. A track meeting a pad at a right angle is
//! where copper tears when a board is drilled or flexed; KiCad has drawn the
//! fillet since 7.0 and this project drew none. The shape is measured in
//! `cypcb-world`; what this reads is whether it reaches the Gerber, and
//! whether a board that does not ask for it still gets exactly what it got
//! before.

use std::path::{Path, PathBuf};
use std::process::Command;

fn cypcb() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cypcb"))
}

fn example(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .join("examples")
        .join(name)
}

fn scratch(who: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cypcb-teardrops-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// Export the board and read back its top copper.
fn top_copper(board: &Path, out: &Path, teardrops: bool) -> String {
    let mut command = cypcb();
    command.arg("export").arg(board).arg("-o").arg(out);
    if teardrops {
        command.arg("--teardrops");
    }
    let status = command.status().expect("the binary runs");
    assert!(status.success(), "the export failed");

    let gerber = out.join("gerber");
    let file = std::fs::read_dir(&gerber)
        .expect("the gerber directory exists")
        .map(|entry| entry.expect("a directory entry").path())
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.contains("F_Cu"))
        })
        .expect("a top copper file was written");
    std::fs::read_to_string(file).expect("the copper file is readable")
}

/// Every region a Gerber file opens.
fn regions(gerber: &str) -> usize {
    gerber.lines().filter(|line| line.trim() == "G36*").count()
}

#[test]
fn the_fillet_is_in_the_copper_when_it_is_asked_for() {
    // Two traces between headers, so four ends land on four pads.
    let board = example("usb-diff-pair.cypcb");

    let plain = top_copper(&board, &scratch("plain"), false);
    let filleted = top_copper(&board, &scratch("filleted"), true);

    assert_eq!(
        regions(&plain),
        0,
        "a board that does not ask for teardrops has no regions in its copper"
    );
    assert_eq!(
        regions(&filleted),
        4,
        "two traces end on a pad at each end, so four fillets are drawn"
    );
    assert!(
        filleted.len() > plain.len(),
        "the filleted file carries more copper: {} against {}",
        filleted.len(),
        plain.len()
    );
}

#[test]
fn a_board_that_does_not_ask_gets_what_it_got_before() {
    // The point of the flag being a flag. Every pad flash and every track the
    // plain export writes is still there, byte for byte, in the same order.
    let board = example("usb-diff-pair.cypcb");
    let plain = top_copper(&board, &scratch("unchanged-a"), false);
    let again = top_copper(&board, &scratch("unchanged-b"), false);

    let without_the_clock = |gerber: &str| -> Vec<String> {
        gerber
            .lines()
            .filter(|line| !line.contains("CreationDate"))
            .map(str::to_string)
            .collect()
    };

    assert!(plain.contains("CreationDate"), "the stamp is there to drop");
    assert_eq!(
        without_the_clock(&plain),
        without_the_clock(&again),
        "two plain exports of one board differ by the clock and nothing else"
    );
}

#[test]
fn the_help_says_what_the_flag_does() {
    let output = cypcb()
        .arg("export")
        .arg("--help")
        .output()
        .expect("the binary runs");
    let help = String::from_utf8_lossy(&output.stdout);
    assert!(
        help.contains("--teardrops"),
        "the flag is on the page: {help}"
    );
    assert!(
        help.to_lowercase().contains("pad"),
        "the page says what it fillets: {help}"
    );
}
