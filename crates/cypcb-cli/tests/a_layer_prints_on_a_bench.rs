//! `--pdf` plots a copper layer to print or to attach.
//!
//! `cargo test -p cypcb-cli --test a_layer_prints_on_a_bench`
//!
//! SVG is for a screen and DXF is for a mechanical tool. PDF is the third
//! thing a plot is for: what a person attaches to a message, and what a house
//! prints and lays on the bench beside the board. The last third of item 7 of
//! the KiCad parity audit.
//!
//! A PDF is written by hand here, which means the structure is this project's
//! to get right: the cross-reference table gives the byte offset of every
//! object, a reader seeks to those offsets, and one byte out is a file that
//! opens in nothing. Every test below reads the finished file rather than the
//! code that wrote it.

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
    let dir = std::env::temp_dir().join(format!("cypcb-pdf-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// Export with pages and read one back as the bytes it is.
fn page(board: &str, out: &Path, suffix: &str) -> Vec<u8> {
    let status = cypcb()
        .arg("export")
        .arg(example(board))
        .arg("-o")
        .arg(out)
        .arg("--pdf")
        .status()
        .expect("the binary runs");
    assert!(status.success(), "the export failed");
    let name = board.trim_end_matches(".cypcb");
    std::fs::read(out.join("plot").join(format!("{name}-{suffix}.pdf")))
        .expect("the page is readable")
}

/// Where the file says its cross-reference table starts.
fn startxref(pdf: &[u8]) -> usize {
    let text = String::from_utf8_lossy(pdf);
    let at = text
        .rfind("startxref")
        .expect("the file says where to look");
    text[at + "startxref".len()..]
        .split_whitespace()
        .next()
        .expect("a number follows it")
        .parse()
        .expect("and it is a number")
}

/// The offset each object is listed at, object 1 first.
fn offsets(pdf: &[u8]) -> Vec<usize> {
    let text = String::from_utf8_lossy(pdf);
    let table = &text[startxref(pdf)..];
    table
        .lines()
        .skip(2) // `xref`, then the count
        .take_while(|line| line.ends_with(" n ") || line.ends_with(" f "))
        .skip(1) // the free head
        .map(|line| {
            line.split_whitespace()
                .next()
                .expect("an offset")
                .parse()
                .expect("a number")
        })
        .collect()
}

#[test]
fn the_file_is_a_pdf_and_says_where_its_table_is() {
    let pdf = page("usb-diff-pair.cypcb", &scratch("shape"), "F_Cu");
    let text = String::from_utf8_lossy(&pdf);
    assert!(text.starts_with("%PDF-1."), "it says what it is");
    assert!(text.trim_end().ends_with("%%EOF"), "and where it ends");
    assert!(
        pdf[startxref(&pdf)..].starts_with(b"xref"),
        "and a reader following startxref lands on the table"
    );
}

#[test]
fn every_offset_in_the_table_lands_on_the_object_it_names() {
    // The one thing a hand-written PDF has to get exactly right. An offset a
    // byte out is a file that opens in nothing, and the failure is silent
    // until somebody tries.
    let pdf = page("usb-diff-pair.cypcb", &scratch("offsets"), "F_Cu");
    let found = offsets(&pdf);
    assert_eq!(found.len(), 5, "five objects are listed: {found:?}");
    for (index, offset) in found.iter().enumerate() {
        let expected = format!("{} 0 obj", index + 1);
        let there = String::from_utf8_lossy(&pdf[*offset..*offset + expected.len()]).to_string();
        assert_eq!(
            there,
            expected,
            "offset {offset} should start object {}",
            index + 1
        );
    }
}

#[test]
fn the_content_stream_declares_the_length_it_has() {
    // A reader takes the drawing by its declared length. Too short and the
    // page is cut off; too long and it reads the rest of the file as drawing.
    let pdf = page("usb-diff-pair.cypcb", &scratch("length"), "F_Cu");
    let text = String::from_utf8_lossy(&pdf);
    let at = text.find("/Length ").expect("the stream declares one");
    let declared: usize = text[at + "/Length ".len()..]
        .split_whitespace()
        .next()
        .expect("a number")
        .parse()
        .expect("and it is a number");

    let start = text.find("stream\n").expect("the stream starts") + "stream\n".len();
    let end = text.find("endstream").expect("and ends");
    assert_eq!(
        end - start,
        declared,
        "the drawing is as long as the file says"
    );
}

#[test]
fn the_page_is_the_board_at_size_in_points() {
    // PDF user space is points - 72 to the inch - so a page printed at 100% is
    // the board at size. usb-diff-pair is 30mm by 20mm: 85.039 by 56.693.
    let pdf = page("usb-diff-pair.cypcb", &scratch("size"), "F_Cu");
    let text = String::from_utf8_lossy(&pdf);
    assert!(
        text.contains("/MediaBox [0 0 85.039 56.693]"),
        "the page is the board's own size:\n{}",
        &text[..text.len().min(400)]
    );
}

#[test]
fn a_track_is_stroked_at_the_width_it_runs_at() {
    // Copper is not a hairline, and a printed plot is measured with a rule.
    // 0.2mm is 0.567 points.
    let pdf = page("usb-diff-pair.cypcb", &scratch("width"), "F_Cu");
    let text = String::from_utf8_lossy(&pdf);
    assert_eq!(
        text.matches("0.567 w\n").count(),
        2,
        "both tracks are stroked at their own width:\n{text}"
    );
    assert!(
        text.contains("1 J\n"),
        "and they end the way copper ends at a corner"
    );
}

#[test]
fn a_measurement_prints_as_the_figure_it_is() {
    let pdf = page("board-dimensions.cypcb", &scratch("dimension"), "F_Cu");
    let text = String::from_utf8_lossy(&pdf);
    assert!(
        text.contains("(40.000mm) Tj") && text.contains("(25.000mm) Tj"),
        "the measurements are on the page:\n{text}"
    );
    assert!(
        text.contains("/BaseFont /Helvetica"),
        "set in a font every reader has"
    );
}

#[test]
fn a_board_that_does_not_ask_gets_no_pages() {
    let out = scratch("silent");
    let status = cypcb()
        .arg("export")
        .arg(example("usb-diff-pair.cypcb"))
        .arg("-o")
        .arg(&out)
        .status()
        .expect("the binary runs");
    assert!(status.success(), "the export failed");
    assert!(
        !out.join("plot").exists(),
        "the file set a house receives is unchanged unless a page is asked for"
    );
}
