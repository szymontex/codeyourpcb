//! Every constraint a net block takes, through the binary.
//!
//! `cargo test -p cypcb-cli --test every_net_constraint_reaches_the_checker`
//!
//! `net SIG [width ... clearance ... current ... impedance ... neck ...]` is
//! the design stating rules of its own, and each one has a DRC rule behind it.
//! Two of the five were reaching the checker by accident of who wrote which
//! rule: a stated **width** was read by the router and by nobody else until
//! 2026-08-24, and a **neck** stated on the net rather than on the trace was
//! read by nobody at all - `NeckDownRule` queried the `TraceNeck` component,
//! which only a `trace` block puts there.
//!
//! So this is a census rather than five tests. Each constraint gets a board
//! that states it and the same board with the statement deleted, and the list
//! of constraints is read back **from the binary's own help** - add a sixth to
//! the language and this fails until it has a fixture here.

use std::path::PathBuf;
use std::process::Command;

/// The board every case is cut from: two parts, one trace, a stack the
/// impedance rule can be asked about.
///
/// `{CONSTRAINT}` is where the statement goes, and deleting it is what makes
/// the other half of each case.
const BASE: &str = r#"version 1

board census {
    size 30mm x 30mm
    layers 2

    stackup {
        copper "F.Cu" 1oz
        core "dielectric 1" 1.5mm material "FR4" dk 4.5
        copper "B.Cu" 1oz
    }
}

component R1 resistor "0402" {
    value "10k"
    at 5mm, 10mm
}

component R2 resistor "0402" {
    value "10k"
    at 25mm, 10mm
}

net A {CONSTRAINT}{
    R1.1
    R2.1
}

trace A {
    from R1.1
    to R2.1
    layer Top
    width 0.2mm
}
"#;

/// One constraint, what it is stated as, and the words the checker owes the
/// designer when the board disobeys it.
struct Case {
    /// The word as the net block spells it.
    property: &'static str,
    /// The statement, as it goes between the brackets.
    stated: &'static str,
    /// What the report has to say with the statement in place, and must not
    /// say without it.
    reported: &'static str,
}

const CASES: &[Case] = &[
    Case {
        property: "width",
        stated: "width 0.5mm",
        // A 0.2mm trace clears JLCPCB's 0.127mm floor twice over, so this
        // figure can only come from the net.
        reported: "0.200mm actual, 0.500mm minimum",
    },
    Case {
        property: "clearance",
        stated: "clearance 5mm",
        reported: "5.00mm required",
    },
    Case {
        property: "current",
        stated: "current 5A",
        reported: "trace-current",
    },
    Case {
        property: "impedance",
        stated: "impedance 50ohm",
        reported: "asks for 50ohm",
    },
    Case {
        property: "neck",
        // Under the 0.127mm this house will etch, which is the plainest way a
        // neck can be wrong.
        stated: "neck 0.05mm for 2mm",
        reported: "neck-down",
    },
];

/// A board with this statement in it, or without one, in a directory of its
/// own: cargo runs these at the same time and a shared directory means one
/// wiping what another is reading.
fn board(who: &str, constraint: Option<&str>) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cypcb-net-census-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");

    let source = match constraint {
        Some(stated) => BASE.replace("{CONSTRAINT}", &format!("[{stated}] ")),
        None => BASE.replace("{CONSTRAINT}", ""),
    };
    let board = dir.join("board.cypcb");
    std::fs::write(&board, source).expect("the fixture is writable");
    board
}

fn check(board: &PathBuf) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .arg("check")
        .arg(board)
        .output()
        .expect("the binary runs");
    String::from_utf8_lossy(&output.stdout).to_string() + &String::from_utf8_lossy(&output.stderr)
}

#[test]
fn each_constraint_changes_what_the_checker_reports() {
    let plain = check(&board("plain", None));

    for case in CASES {
        let said = check(&board(case.property, Some(case.stated)));
        assert!(
            said.contains(case.reported),
            "`{}` stated as `{}` has to reach the checker:\n{said}",
            case.property,
            case.stated
        );
        assert!(
            !plain.contains(case.reported),
            "and the same board without the statement must not say it, or the \
             fixture proves nothing about `{}`:\n{plain}",
            case.property
        );
    }
}

#[test]
fn the_census_covers_every_constraint_the_block_takes() {
    // Read the list back from the binary rather than from a list somebody
    // maintains beside the reader: mistype inside a net block and the parser
    // answers with what the block does take.
    let dir = std::env::temp_dir().join("cypcb-net-census-help");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");
    let board = dir.join("board.cypcb");
    std::fs::write(&board, BASE.replace("{CONSTRAINT}", "[nonsense 1mm] "))
        .expect("the fixture is writable");

    // miette draws a gutter down the left of everything it prints, so the help
    // line is only one line once the whitespace is flattened.
    let said = check(&board);
    let flat = said.split_whitespace().collect::<Vec<_>>().join(" ");
    let list = flat
        .split("`net constraint` takes:")
        .nth(1)
        .expect("the parser names what the block takes");
    // The report carries more than this line - a second fault, its own code,
    // the snippet - so the list ends where the next diagnostic begins.
    let list = list
        .split("cypcb::")
        .next()
        .unwrap_or_default()
        .split('\u{d7}')
        .next()
        .unwrap_or_default()
        .trim();

    for property in list
        .split(',')
        .map(|word| word.trim().trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|word| !word.is_empty())
    {
        assert!(
            CASES.iter().any(|case| case.property == property),
            "the net block takes `{property}` and nothing here states one: a \
             constraint with no fixture is a rule nobody is holding the \
             checker to.\nThe block takes: {list}"
        );
    }
    assert_eq!(
        CASES.len(),
        5,
        "five constraints were covered when this was written; a sixth needs a \
         case above rather than a bigger number here"
    );
}
