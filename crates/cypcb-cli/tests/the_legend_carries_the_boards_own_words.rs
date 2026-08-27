//! The words a board states reach the legend a fabricator prints.
//!
//! `cargo test -p cypcb-cli --test the_legend_carries_the_boards_own_words`
//!
//! A `text` block is only worth having if it ends up in the Gerber. It is
//! drawn from the same stroke font as every designator and clipped by the same
//! rule, because ink over solderable copper starves the joint under it
//! whoever put the letters there.

use std::path::PathBuf;
use std::process::Command;

fn cypcb() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cypcb"))
}

fn scratch(who: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cypcb-boardtext-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

const WITH_TEXT: &str = "version 1\n\nboard legend {\n    size 20mm x 10mm\n    layers 2\n}\n\ntext \"REV B\" {\n    at 10mm, 5mm\n    layer top\n    height 1.5mm\n}\n";

const WITHOUT: &str = "version 1\n\nboard legend {\n    size 20mm x 10mm\n    layers 2\n}\n";

/// Export this source and read back the top legend.
fn top_silk(source: &str, who: &str) -> String {
    let board = std::env::temp_dir().join(format!("cypcb-boardtext-{who}.cypcb"));
    std::fs::write(&board, source).expect("the board is writable");
    let out = scratch(who);

    let status = cypcb()
        .arg("export")
        .arg(&board)
        .arg("-o")
        .arg(&out)
        .status()
        .expect("the binary runs");
    assert!(status.success(), "the export failed");

    let gerber = out.join("gerber");
    let file = std::fs::read_dir(&gerber)
        .expect("the gerber directory exists")
        .map(|entry| entry.expect("a directory entry").path())
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.contains("F_SilkS"))
        })
        .expect("a top legend was written");
    std::fs::read_to_string(file).expect("the legend is readable")
}

/// Every draw command in a Gerber file.
fn strokes(gerber: &str) -> usize {
    gerber.lines().filter(|line| line.ends_with("D01*")).count()
}

#[test]
fn the_words_are_in_the_legend() {
    let with_text = top_silk(WITH_TEXT, "with");
    let without = top_silk(WITHOUT, "without");

    assert_eq!(
        strokes(&without),
        0,
        "a board with nothing on its legend draws nothing:\n{without}"
    );
    assert!(
        strokes(&with_text) >= 10,
        "and `REV B` is five glyphs of strokes: {} drawn",
        strokes(&with_text)
    );
}

#[test]
fn the_words_are_where_the_board_put_them() {
    // 10mm, 5mm in a 2:6 Gerber is X10000000Y5000000 - and the letters are
    // centred on it, so every stroke lands within a few millimetres of that.
    let gerber = top_silk(WITH_TEXT, "placed");
    let near_middle = gerber
        .lines()
        .filter(|line| line.starts_with('X') && line.ends_with("D01*"))
        .filter(|line| {
            let x: i64 = line[1..line.find('Y').unwrap_or(1)].parse().unwrap_or(0);
            (x - 10_000_000).abs() < 5_000_000
        })
        .count();
    assert!(
        near_middle > 5,
        "the letters sit where the text was put:\n{gerber}"
    );
}
