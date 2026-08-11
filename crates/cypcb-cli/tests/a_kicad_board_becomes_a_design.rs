//! A KiCad board comes out as text somebody can edit.
//!
//! `cargo test -p cypcb-cli --test a_kicad_board_becomes_a_design`
//!
//! `to-kicad` shipped first and made this project's designs openable by
//! everybody else. The other direction was missing entirely: a KiCad board
//! could be checked, routed, scored and exported by this tool - `check`,
//! `route` and `export` all take one - but nothing turned it into `.cypcb`
//! source. So the one thing this project exists for, a board you read and
//! change as text, was the one thing a KiCad user could not get out of it.
//!
//! The whole DSL writer was one function, `traces_as_dsl`, and it wrote traces.
//! There was no way to write a board, a component, a net or a footprint.

use std::process::Command;

fn repo_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

/// Run `from-kicad` on a fixture and hand back (success, stdout+stderr, the
/// design it wrote if any).
fn imported(fixture: &str, name: &str) -> (bool, String, String) {
    let dir = std::env::temp_dir().join("cypcb-from-kicad");
    std::fs::create_dir_all(&dir).expect("a place to work");
    let out = dir.join(format!("{name}.cypcb"));
    let _ = std::fs::remove_file(&out);

    let run = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .arg("from-kicad")
        .arg(repo_root().join(fixture))
        .arg("--output")
        .arg(&out)
        .output()
        .expect("the binary runs");

    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    let design = std::fs::read_to_string(&out).unwrap_or_default();
    (run.status.success(), said, design)
}

#[test]
fn a_board_kicad_wrote_becomes_a_design_that_parses() {
    let (ok, said, design) = imported(
        "crates/cypcb-kicad/tests/fixtures/kicad10-slotted.kicad_pcb",
        "slotted",
    );
    assert!(ok, "{said}");

    // The command re-reads what it wrote before saying it worked, so a design
    // that does not parse fails here rather than on the user's next command.
    assert!(design.starts_with("version 1"), "{design}");
    assert!(design.contains("board "), "{design}");
    assert!(
        design.contains("component J1 connector"),
        "the kind comes from the reference designator's prefix:\n{design}"
    );
}

#[test]
fn a_slot_survives_the_whole_way_round() {
    // KiCad wrote this board from a design that asked for `drill 2.4mm x 1.0mm`.
    // Coming back the other way it has to still be a slot, not the round hole
    // its narrow dimension would make.
    let (ok, said, design) = imported(
        "crates/cypcb-kicad/tests/fixtures/kicad10-slotted.kicad_pcb",
        "slot",
    );
    assert!(ok, "{said}");

    let slot = design
        .lines()
        .find(|line| line.trim_start().starts_with("pad ") && line.contains(" drill "))
        .unwrap_or_else(|| panic!("no drilled pad in:\n{design}"));
    assert!(
        slot.contains("x 1.000000mm") || slot.contains("2.400000mm x"),
        "the slot came back as a round hole:\n{slot}"
    );
}

#[test]
fn a_footprint_kicad_names_is_given_one_the_language_can_state() {
    // KiCad calls a footprint `cypcb:USB_ANCHOR` or
    // `Package_QFP:LQFP-48_7x7mm_P0.5mm`. A `footprint` definition takes a bare
    // identifier, so the library prefix has to go and the rest has to be
    // spellable - and whatever it becomes, the component below must name the
    // same thing.
    let (ok, said, design) = imported(
        "crates/cypcb-kicad/tests/fixtures/kicad10-slotted.kicad_pcb",
        "names",
    );
    assert!(ok, "{said}");

    let defined: Vec<&str> = design
        .lines()
        .filter_map(|line| line.strip_prefix("footprint "))
        .filter_map(|rest| rest.split_whitespace().next())
        .collect();
    assert!(!defined.is_empty(), "{design}");
    for name in &defined {
        assert!(
            !name.contains(':') && !name.starts_with('"'),
            "a definition takes a bare identifier, and this is {name}"
        );
        assert!(
            design.contains(&format!("\"{name}\"")),
            "{name} is defined and nothing uses it:\n{design}"
        );
    }
}

#[test]
fn every_benchmark_board_imports_or_says_exactly_why_not() {
    // Six boards, and two of them carry a USB-C receptacle whose pads are
    // called A1, B4, S1. The language writes `pad <number>`, so those cannot be
    // stated - and renaming them would move pins onto the wrong nets. What must
    // not happen is a bare "Missing a pad number" with nothing to act on.
    let boards = [
        "led_blink",
        "stm32_breakout",
        "multi_ic",
        "shift_driver",
        "qfp_fanout",
        "plane_board",
    ];
    let mut imported_clean = 0;
    for board in boards {
        let (ok, said, design) = imported(
            &format!("tests/fixtures/benchmark/{board}.kicad_pcb"),
            board,
        );
        assert!(
            !design.is_empty(),
            "{board}: nothing was written at all\n{said}"
        );
        if ok {
            imported_clean += 1;
        } else {
            assert!(
                said.contains("named rather than numbered"),
                "{board} failed for a reason nobody can act on:\n{said}"
            );
        }
    }
    assert!(
        imported_clean >= 4,
        "only {imported_clean} of six boards import cleanly"
    );
}

#[test]
fn the_kind_of_a_part_comes_from_its_reference() {
    use cypcb_world::dsl::kind_from_refdes;

    // KiCad records what a part is called and what it looks like, never what it
    // is. The prefix is the convention every schematic has used for decades.
    assert_eq!(kind_from_refdes("R1"), "resistor");
    assert_eq!(kind_from_refdes("C12"), "capacitor");
    assert_eq!(kind_from_refdes("U3"), "ic");
    assert_eq!(kind_from_refdes("J1"), "connector");
    assert_eq!(kind_from_refdes("Y1"), "crystal");
    assert_eq!(kind_from_refdes("LED4"), "led");
    assert_eq!(kind_from_refdes("D2"), "diode");
    assert_eq!(kind_from_refdes("Q7"), "transistor");

    // And where the convention does not reach, the language's own word for a
    // part nobody stated a kind for - rather than a guess from the footprint.
    assert_eq!(kind_from_refdes("ANT1"), "generic");
    assert_eq!(kind_from_refdes("MH2"), "generic");
    assert_eq!(kind_from_refdes(""), "generic");
}

#[test]
fn routed_copper_survives_the_whole_loop() {
    // The first version of this command lost every trace and said nothing.
    // The importer hands routed copper back as `reference_routes` - what the
    // router is measured against - rather than as entities on the board, so the
    // writer, which reads the board, found none. led_blink went in with one
    // segment and came out with none.
    //
    // The loop this checks is the whole promise in one line: a design routed
    // here, written out as a KiCad board, and read back as a design.
    let dir = std::env::temp_dir().join("cypcb-from-kicad");
    std::fs::create_dir_all(&dir).expect("a place to work");
    let routed = dir.join("loop.cypcb");
    let board = dir.join("loop.kicad_pcb");
    let back = dir.join("loop-back.cypcb");

    let run = |args: Vec<&std::ffi::OsStr>| {
        let out = Command::new(env!("CARGO_BIN_EXE_cypcb"))
            .args(&args)
            .output()
            .expect("the binary runs");
        assert!(
            out.status.success(),
            "{:?}: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
    };

    let source = repo_root().join("examples/blink.cypcb");
    run(vec![
        "route".as_ref(),
        source.as_os_str(),
        "--in-house".as_ref(),
        "--output".as_ref(),
        routed.as_os_str(),
    ]);
    run(vec![
        "to-kicad".as_ref(),
        routed.as_os_str(),
        "--output".as_ref(),
        board.as_os_str(),
    ]);
    run(vec![
        "from-kicad".as_ref(),
        board.as_os_str(),
        "--output".as_ref(),
        back.as_os_str(),
    ]);

    let paths = |path: &std::path::Path| -> usize {
        std::fs::read_to_string(path)
            .expect("the file is there")
            .lines()
            .filter(|line| line.trim_start().starts_with("path "))
            .count()
    };

    let before = paths(&routed);
    assert!(before > 0, "the router laid nothing to check");
    assert_eq!(
        before,
        paths(&back),
        "copper went missing between the router and the design it came back as"
    );
}
