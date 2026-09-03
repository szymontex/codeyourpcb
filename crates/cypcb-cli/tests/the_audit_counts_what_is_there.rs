//! The audit counts what is there.
//!
//! `cargo test -p cypcb-cli --test the_audit_counts_what_is_there`
//!
//! V9's "Measured on our side" paragraph is the audit's own foundation: every
//! row of the parity table is read against it. Two of its numbers had gone
//! stale by 2026-09-03. It said `cypcb --help` lists ten subcommands when it
//! lists twelve - `from-dxf` and `library` are wired in `main.rs`, with help
//! text, and neither was in the sentence. And it said the rules directory
//! holds twenty rules when it holds 37.
//!
//! Neither number is hard to check; nobody was checking. So they are counted
//! here from the binary and from the tree, and the paragraph is held to them.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

fn read(path: &str) -> String {
    std::fs::read_to_string(repo_root().join(path))
        .unwrap_or_else(|error| panic!("{path}: {error}"))
}

/// The paragraph the parity table is measured against.
fn measured_paragraph() -> String {
    let tracker = read("docs/TRACKER.md");
    let start = tracker
        .find("**Measured on our side")
        .expect("V9 states what it measured");
    let rest = &tracker[start..];
    let end = rest.find("\n\n").expect("the paragraph ends");
    rest[..end].to_string()
}

/// The count and the names the audit's sentence states, read from the
/// enumeration rather than from the paragraph around it.
fn stated_subcommands(paragraph: &str) -> (usize, BTreeSet<String>) {
    let after = paragraph
        .split("subcommands: ")
        .nth(1)
        .expect("the sentence enumerates them");
    let list = after.split('.').next().expect("the sentence ends");
    let count = paragraph
        .split("lists ")
        .nth(1)
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|word| word.parse::<usize>().ok())
        .expect("the sentence states how many");

    let names = list
        .split('`')
        .skip(1)
        .step_by(2)
        .map(|name| name.trim().to_string())
        .collect();
    (count, names)
}

/// Every subcommand `cypcb --help` prints, `help` included.
fn subcommands() -> BTreeSet<String> {
    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .arg("--help")
        .output()
        .expect("the binary runs");
    let help = String::from_utf8_lossy(&output.stdout).to_string();
    let block = help
        .split("Commands:")
        .nth(1)
        .and_then(|rest| rest.split("\nOptions:").next())
        .expect("the help has a command list");

    block
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim_start();
            let indented = line.len() > trimmed.len();
            let name = trimmed.split_whitespace().next()?;
            (indented && name.chars().all(|c| c.is_ascii_lowercase() || c == '-'))
                .then(|| name.to_string())
        })
        .collect()
}

#[test]
fn the_paragraph_names_every_subcommand_the_binary_has() {
    // The list, not the paragraph. The first version of this case looked for
    // each name anywhere in the paragraph and passed while `library` was
    // missing from the sentence, because the prose beside it explains that
    // `library` was one of the two left out. A mutation found that; the
    // assertion reads the enumeration itself now, and the count it states.
    let paragraph = measured_paragraph();
    let (stated, listed) = stated_subcommands(&paragraph);
    let actual = subcommands();

    assert_eq!(
        listed, actual,
        "the audit's own list disagrees with the binary: it names {listed:?}, \
         `cypcb --help` prints {actual:?}"
    );
    assert_eq!(
        stated,
        actual.len(),
        "the audit says {stated} subcommands and there are {}",
        actual.len()
    );
}

#[test]
fn the_paragraph_states_the_number_of_rules_the_registry_holds() {
    let registered = read("crates/cypcb-drc/src/lib.rs")
        .matches("Box::new(rules::")
        .count();
    let paragraph = measured_paragraph();

    assert!(
        flat(&paragraph).contains(&format!("holds {registered} rules")),
        "the registry holds {registered} rules and the audit does not say so:\n{paragraph}"
    );
}

#[test]
fn the_paragraph_this_reads_is_the_paragraph_it_means() {
    // The control. Both cases above look for text, and a lookup that finds the
    // wrong slice - or an empty one - would pass by finding nothing to
    // contradict. So: the slice is a paragraph rather than the file, and it
    // carries the two things it is being asked about.
    let paragraph = measured_paragraph();
    let tracker = read("docs/TRACKER.md");

    assert!(
        paragraph.len() < tracker.len() / 50,
        "the slice is one paragraph, not the file: {} of {} bytes",
        paragraph.len(),
        tracker.len()
    );
    assert!(paragraph.contains("cypcb --help"), "{paragraph}");
    assert!(paragraph.contains("rules"), "{paragraph}");
    assert!(
        subcommands().len() > 5,
        "the help parse found {} commands, which is not a command list",
        subcommands().len()
    );
}

/// The same text with every run of whitespace collapsed to one space.
///
/// The paragraph is hard-wrapped, so "a bill of materials" arrives with a
/// newline inside it and a plain `contains` says the audit never mentions it.
fn flat(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A file the export writes, and the word the audit has to use for it.
///
/// Ordered: the first marker a name carries decides its kind, so the specific
/// ones come before `.json`.
const KINDS: &[(&str, &str)] = &[
    ("_Cu.gbr", "copper"),
    ("_Mask.gbr", "mask"),
    ("_Paste.gbr", "paste"),
    ("_SilkS.gbr", "silk"),
    ("Edge_Cuts.gbr", "outline"),
    (".drl", "Excellon"),
    (".gbrjob", "job file"),
    ("-BOM.csv", "bill of materials"),
    ("-CPL.csv", "pick-and-place"),
    (".json", "assembly summary"),
];

#[test]
fn the_paragraph_names_every_file_the_export_writes() {
    // "and nothing else" is a claim about a directory, so a directory is what
    // answers it. The audit said Gerber, Excellon, a job file, a BOM and a
    // pick-and-place file; it wrote those, plus solder paste stencils and an
    // assembly summary in JSON, and said neither.
    let out = std::env::temp_dir().join("cypcb-audit-export");
    let _ = std::fs::remove_dir_all(&out);

    let status = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .arg("export")
        .arg(repo_root().join("examples/blink.cypcb"))
        .arg("-o")
        .arg(&out)
        .output()
        .expect("the binary runs");
    assert!(
        status.status.success(),
        "export failed: {}",
        String::from_utf8_lossy(&status.stderr)
    );

    let mut written = Vec::new();
    let mut stack = vec![out.clone()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("the export wrote a directory") {
            let path = entry.expect("a readable entry").path();
            if path.is_dir() {
                stack.push(path);
            } else {
                written.push(path);
            }
        }
    }

    assert!(
        written.len() > 5,
        "the export wrote {} files, which is not an export",
        written.len()
    );

    let paragraph = flat(&measured_paragraph());
    for path in &written {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        let kind = KINDS
            .iter()
            .find(|(marker, _)| name.ends_with(marker))
            .unwrap_or_else(|| panic!("`{name}` is a kind of file nothing here has a word for"));
        assert!(
            paragraph.contains(kind.1),
            "the export writes `{name}` and the audit does not say `{}`:\n{paragraph}",
            kind.1
        );
    }
}

/// Every keyword the grammar opens a construct with.
///
/// The same convention `every_definition_has_an_example` uses, because the two
/// must agree about what a keyword is: a rule name with `_definition` or
/// `_statement` trimmed, `module_instance` read as `use`, and the zone rule
/// replaced by the four words it chooses between.
fn grammar_keywords() -> BTreeSet<String> {
    let grammar = read("crates/cypcb-parser/grammar/grammar.js");

    let list = grammar
        .split("_definition: $ => choice(")
        .nth(1)
        .expect("the grammar names its top-level definitions in one place")
        .split("),")
        .next()
        .expect("that list ends");

    let kinds = grammar
        .split("field('kind', choice(")
        .nth(1)
        .expect("the zone block chooses between its four words")
        .split(')')
        .next()
        .expect("that choice ends");

    let mut found: BTreeSet<String> = kinds
        .split(',')
        .map(|word| word.trim().trim_matches('\'').to_string())
        .filter(|word| !word.is_empty())
        .collect();

    found.extend(
        list.lines()
            .filter_map(|line| line.trim().strip_prefix("$."))
            .filter_map(|rest| rest.split(',').next())
            .map(|rule| match rule.trim() {
                "module_instance" => "use".to_string(),
                other => other
                    .trim_end_matches("_definition")
                    .trim_end_matches("_statement")
                    .to_string(),
            })
            .filter(|word| word != "zone"),
    );
    found
}

#[test]
fn the_paragraph_names_every_keyword_the_grammar_opens_with() {
    // The sentence this replaced listed `via`, `stackup`, `coverlay` and
    // `stiffener` as the grammar's keywords. None of them opens a definition -
    // they are nested inside the board block - and it named none of the eleven
    // that do. Vague enough that nothing could check it, which is how it
    // stayed.
    let paragraph = flat(&measured_paragraph());
    let after = paragraph
        .split("keywords: ")
        .nth(1)
        .expect("the sentence enumerates them");
    let list = after.split('.').next().expect("the sentence ends");
    let named: BTreeSet<String> = list
        .split('`')
        .skip(1)
        .step_by(2)
        .map(|word| word.trim().to_string())
        .collect();
    let stated: usize = paragraph
        .split("opens a design with ")
        .nth(1)
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|word| word.parse().ok())
        .expect("the sentence states how many");

    let actual = grammar_keywords();
    assert_eq!(
        named, actual,
        "the audit's keyword list disagrees with the grammar"
    );
    assert_eq!(
        stated,
        actual.len(),
        "the audit says {stated} keywords and the grammar has {}",
        actual.len()
    );
    assert!(
        actual.len() > 10,
        "the grammar parse found {} keywords, which is not a language",
        actual.len()
    );
}
