//! The help says which boards a command reads.
//!
//! `cargo test -p cypcb-cli --test the_help_says_which_boards_it_reads`
//!
//! Six of the nine subcommands take a `.kicad_pcb` file as readily as a
//! `.cypcb` one - `parse`, `check`, `route`, `export`, `score` and `watch`
//! each open with `board_source::is_kicad` - and every one of their help lines
//! said ".cypcb file". `score`'s said "a routed .cypcb file" in the same
//! sentence as "routes the board", so it was wrong about the format and about
//! what it does with it. The argument's own doc under `score` had said
//! "a `.cypcb` design or a `.kicad_pcb` file" the whole time, which is how a
//! reader could find out.
//!
//! The pairing is what this holds: a command that reads both formats says so,
//! and a command that says so reads both. The second half matters as much -
//! a help line promising KiCad support that the code does not have sends a
//! person to a file that will be refused.

use std::fs;
use std::path::PathBuf;

/// The phrase a dual-format command's help line carries.
const BOTH_FORMATS: &str = ".cypcb or .kicad_pcb";

fn crate_src() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// `ParseKicadCommand` -> `parse_kicad`.
fn module_of(type_name: &str) -> String {
    let stem = type_name.strip_suffix("Command").unwrap_or(type_name);
    let mut out = String::new();
    for (index, ch) in stem.char_indices() {
        if ch.is_uppercase() && index > 0 {
            out.push('_');
        }
        out.extend(ch.to_lowercase());
    }
    out
}

/// Each subcommand, as (type name, the doc comment clap prints).
fn subcommands() -> Vec<(String, String)> {
    let source = fs::read_to_string(crate_src().join("main.rs")).expect("main.rs is readable");
    let lines: Vec<&str> = source.lines().collect();
    let mut found = Vec::new();

    for (index, line) in lines.iter().enumerate() {
        let Some(open) = line.find("(commands::") else {
            continue;
        };
        let rest = &line[open + "(commands::".len()..];
        let Some(type_name) = rest.trim_end().strip_suffix("),") else {
            continue;
        };

        // The `///` lines directly above are what clap prints for it.
        let mut doc = Vec::new();
        for above in lines[..index].iter().rev() {
            let trimmed = above.trim();
            match trimmed.strip_prefix("///") {
                Some(text) => doc.push(text.trim().to_string()),
                None => break,
            }
        }
        doc.reverse();
        found.push((type_name.to_string(), doc.join(" ")));
    }

    found
}

#[test]
fn a_command_that_reads_both_formats_says_so() {
    let commands = subcommands();

    // A reader that found nothing would make the loop below pass while proving
    // nothing. Nine subcommands ship today; a floor, not a census.
    assert!(
        commands.len() >= 9,
        "only {} subcommands were read out of main.rs, so the reader is broken rather than the crate: {commands:?}",
        commands.len()
    );

    for (type_name, doc) in &commands {
        let module = crate_src()
            .join("commands")
            .join(format!("{}.rs", module_of(type_name)));
        let body = fs::read_to_string(&module)
            .unwrap_or_else(|e| panic!("{} is readable ({e})", module.display()));

        let reads_kicad = body.contains("is_kicad(");
        let says_kicad = doc.contains(BOTH_FORMATS);

        assert_eq!(
            reads_kicad,
            says_kicad,
            "{type_name} {} a KiCad board and its help line {} say so: {doc:?}",
            if reads_kicad {
                "reads"
            } else {
                "does not read"
            },
            if says_kicad { "does" } else { "does not" },
        );
    }
}

#[test]
fn the_help_output_carries_what_the_source_says() {
    // The source is not the surface. This asks the binary.
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .arg("--help")
        .output()
        .expect("the binary runs");
    let help = String::from_utf8_lossy(&output.stdout).to_string();

    for command in ["parse", "check", "route", "export", "score", "watch"] {
        let line = help
            .lines()
            .find(|line| line.trim_start().starts_with(command))
            .unwrap_or_else(|| panic!("no line for `{command}` in:\n{help}"));
        assert!(
            line.contains(BOTH_FORMATS),
            "`{command}` reads a KiCad board and its line does not say so: {line}"
        );
    }
}
