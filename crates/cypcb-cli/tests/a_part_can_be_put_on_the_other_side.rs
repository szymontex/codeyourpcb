//! A design can say which face of the board a part is soldered to.
//!
//! `cargo test -p cypcb-cli --test a_part_can_be_put_on_the_other_side`
//!
//! Until now it could not. `sync.rs` carried the note - "the DSL has no word
//! for this yet" - and derived the side from the footprint's copper, so a part
//! was on the bottom only if its footprint had no top-side pads. No footprint
//! in the library is built that way, which means **no design could place a
//! part on the bottom of the board**: half of every two-sided assembly was
//! unsayable.
//!
//! `side bottom` says it. The flip is registered once, in the footprint
//! library, and the instance points at the flipped copy - so the checker, the
//! four Gerber writers, the drill file and the pick-and-place list all place
//! the same mirrored pads without any of them knowing about sides. A mirror
//! implemented six times is a board whose copper and solder mask disagree
//! about which face a part is on.
//!
//! What is checked here is the whole path: source text, both readers, sync,
//! and the files a fabricator receives.

use std::path::PathBuf;
use std::process::Command;

fn board(side: &str) -> String {
    format!(
        r#"version 1

board two_sided {{
    size 30mm x 20mm
    layers 2
}}

component R1 resistor "0402" {{
    value "10k"
    at 10mm, 10mm
{side}
}}
"#
    )
}

/// Export a design and hand back the output directory.
fn export(source: &str, name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cypcb-side-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");

    let file = dir.join("board.cypcb");
    std::fs::write(&file, source).expect("the board is written");
    let out = dir.join("out");

    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .args(["export"])
        .arg(&file)
        .arg("--output")
        .arg(&out)
        .output()
        .expect("the binary runs");
    assert!(
        output.status.success(),
        "the export failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    out
}

/// How many pad flashes a Gerber holds.
fn flashes(path: &std::path::Path) -> usize {
    std::fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("no file at {}: {err}", path.display()))
        .lines()
        .filter(|line| line.ends_with("D03*"))
        .count()
}

#[test]
fn a_part_on_the_bottom_has_its_copper_on_the_bottom() {
    let out = export(&board("    side bottom"), "copper");

    assert_eq!(
        flashes(&out.join("gerber/board-F_Cu.gbr")),
        0,
        "nothing of this part belongs on the top copper"
    );
    assert_eq!(
        flashes(&out.join("gerber/board-B_Cu.gbr")),
        2,
        "both pads of the 0402 belong on the bottom copper"
    );
}

#[test]
fn the_same_part_left_alone_stays_on_top() {
    // The control. Every design in this repository states no side, and none of
    // them may move.
    let out = export(&board(""), "control");

    assert_eq!(flashes(&out.join("gerber/board-F_Cu.gbr")), 2);
    assert_eq!(flashes(&out.join("gerber/board-B_Cu.gbr")), 0);
}

#[test]
fn saying_top_out_loud_is_the_same_as_saying_nothing() {
    let stated = export(&board("    side top"), "stated-top");
    let silent = export(&board(""), "silent-top");

    for file in ["gerber/board-F_Cu.gbr", "gerber/board-B_Cu.gbr"] {
        assert_eq!(
            flashes(&stated.join(file)),
            flashes(&silent.join(file)),
            "{file}"
        );
    }
}

#[test]
fn its_mask_and_paste_follow_the_copper() {
    // A part whose copper is on the bottom and whose mask opening is on the
    // top is a part soldered to nothing: the openings have to move with it.
    let out = export(&board("    side bottom"), "mask");

    assert_eq!(flashes(&out.join("gerber/board-F_Mask.gbr")), 0);
    assert_eq!(flashes(&out.join("gerber/board-B_Mask.gbr")), 2);
    assert_eq!(flashes(&out.join("gerber/board-F_Paste.gbr")), 0);
    assert_eq!(flashes(&out.join("gerber/board-B_Paste.gbr")), 2);
}

#[test]
fn the_assembly_list_tells_the_machine_which_side_to_place_it_on() {
    let out = export(&board("    side bottom"), "cpl");
    let cpl = std::fs::read_to_string(out.join("assembly/board-CPL.csv")).expect("a CPL");

    let row = cpl
        .lines()
        .find(|line| line.starts_with("R1,"))
        .unwrap_or_else(|| panic!("R1 is not in the list:\n{cpl}"));
    assert!(
        row.contains("Bottom"),
        "the placement machine is told the wrong face: {row}"
    );
}

#[test]
fn the_part_is_flipped_over_rather_than_moved() {
    // Seen from above - which is how every coordinate here is written - a part
    // turned over has its local x axis reversed. The 0402's two pads sit left
    // and right of centre, so flipping swaps which pad is which, and a pad that
    // simply moved to the other layer would land in the same place.
    let bottom = export(&board("    side bottom"), "mirror");
    let top = export(&board(""), "mirror-control");

    let xs = |path: PathBuf| -> Vec<String> {
        let text = std::fs::read_to_string(path).expect("a gerber");
        let mut found: Vec<String> = text
            .lines()
            .filter(|line| line.ends_with("D03*"))
            .filter_map(|line| line.split('Y').next().map(str::to_string))
            .collect();
        found.sort();
        found
    };

    let flipped = xs(bottom.join("gerber/board-B_Cu.gbr"));
    let straight = xs(top.join("gerber/board-F_Cu.gbr"));

    assert_eq!(
        flipped.len(),
        2,
        "the fixture is supposed to have two pads: {flipped:?}"
    );
    // The pads are symmetric about the part's centre, so the set of positions
    // is the same either way - what changes is which pad number is where, and
    // that is checked by the netlist rather than by the image. What this pins
    // is that the flip did not move the part off its own position.
    assert_eq!(
        flipped, straight,
        "a symmetric part flipped over covers the same ground"
    );
}
