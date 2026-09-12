//! A test named after what a command writes opens what the command wrote.
//!
//! `cargo test -p cypcb-cli --test a_written_file_is_opened_by_the_test_that_names_it`
//!
//! A count without a denominator passes most easily exactly when the
//! measurement disappears. A message asserted without reading the output passes
//! most easily exactly when the output stops matching the message. Both are one
//! thing: a check whose evidence is the system's own account of itself is the
//! system checking itself, and it holds whatever the system does.
//!
//! Measured on 2026-09-12, and this is why the list below is not advice.
//! Making `--no-assembly` apply to the dry-run listing and not to the write
//! left every assertion in `the_matrix_knows_what_export_writes` green and
//! fired only the one that opens the directory: `2 passed, 1 failed`, with
//! `blink-BOM.csv` sitting on disk. The command's account of itself was
//! perfectly correct.
//!
//! So this walks the tests of this crate, finds every one that drives a command
//! that writes files and never reaches what it wrote, and holds that set to two
//! named lists. A new one joins a list by name, with its reason, or it fails.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

/// How far a helper chain is followed. Three hops covers every shape in this
/// crate today: a test calls a helper, which calls a runner, which calls the
/// binary.
const MAX_DEPTH: usize = 3;

/// Below this many tests found driving a writing command, the walk has stopped
/// seeing the crate rather than found it clean - a renamed helper, a changed
/// way of invoking the binary. A check with no denominator under it passes
/// hardest when it measures nothing.
const TESTS_EXPECTED_TO_DRIVE_A_WRITING_COMMAND: usize = 20;

/// Tests whose command writes nothing to open, by right rather than by
/// omission. Each entry carries the reason in one clause: a name without a
/// reason is an excuse six months later.
const MESSAGE_ONLY_BY_RIGHT: &[(&str, &str)] = &[
    (
        "verbose_shows_what_the_router_did",
        "how loud the log is, which is the log",
    ),
    (
        "the_default_run_is_as_quiet_as_it_was",
        "how quiet the log is, which is the log",
    ),
    (
        "rust_log_names_one_crate",
        "which crate the log filter lets through",
    ),
    (
        "without_the_flag_the_command_says_nothing_about_its_own_work",
        "how loud the log is without the flag",
    ),
    (
        "one_v_turns_the_info_calls_on",
        "which log level one -v reaches",
    ),
    (
        "rust_log_is_honoured_without_the_flag_at_all",
        "that the env filter works with no flag given",
    ),
    (
        "export_says_a_library_is_a_library",
        "a refusal, which writes nothing",
    ),
    (
        "an_unknown_fab_is_refused_by_name",
        "a refusal, which writes nothing",
    ),
    (
        "a_house_export_cannot_cut_for_is_refused_with_the_reason",
        "a refusal, which writes nothing",
    ),
    (
        "the_in_house_router_refuses_the_freerouting_options_too",
        "a refusal, which writes nothing",
    ),
    ("the_help_says_what_the_flag_does", "a help text"),
    ("the_help_says_the_project_file_is_read", "a help text"),
    (
        "a_second_board_in_the_same_directory_is_reported",
        "what the command says about a directory it found occupied",
    ),
    (
        "re_exporting_the_same_board_says_nothing",
        "that the same board over itself draws no warning",
    ),
    (
        "a_fresh_directory_says_nothing",
        "that an empty directory draws no warning",
    ),
    // The design declares nothing, so the claim has no artefact half at all:
    // there is no X whose absence from the file could mean anything, and a file
    // not carrying a thing nobody asked for is true of every file ever written.
    // Not "nothing to open" - the disk owes these nothing.
    (
        "a_design_that_states_none_is_left_alone",
        "the design declares no spans, so the silence is the whole subject",
    ),
    (
        "a_stack_that_states_no_spans_is_left_alone_too",
        "the stack declares no spans, so the silence is the whole subject",
    ),
    (
        "a_board_with_no_stiffener_is_told_nothing_about_one",
        "the board declares no stiffener, so the silence is the whole subject",
    ),
];

/// Tests where nothing on disk can carry the claim. Not the same as the above:
/// the command may write plenty, and the sentence still be about something it
/// did not write.
const NOTHING_TO_OPEN: &[(&str, &str)] = &[(
    "the_ranked_line_says_how_many_contacts_the_violations_describe",
    "a default route ranks thirteen candidates and writes one, so twelve of the thirteen lines \
     describe copper that never reached a disk",
)];

/// Tests that should open what they wrote and do not yet.
///
/// A debt, not an excuse, and the only one of the three lists that may not
/// grow: every entry names a message standing in for something in a file
/// nobody opens. The count is a ratchet - it may fall and not rise - so the
/// next person to add a test cannot quietly join this list instead of one of
/// the two above it.
///
/// Paying one is not a matter of asserting the file lacks the thing. **A bare
/// absence is satisfied by an empty file, a truncated write, and a file that
/// was never created** - it passes most easily exactly when the writer stops
/// writing, which is this check's own defect one layer further down. What pays
/// the debt is a difference: the same design down two paths with the property
/// present in one output and absent in the other, or - where there is only one
/// path - the same design exported twice, once declaring the property and once
/// not, asserting the two written files are identical. Both halves of any such
/// difference have to clear a floor in the same test, because two empty files
/// are also identical.
const OWED_AN_ARTEFACT_READING: &[(&str, &str)] = &[
    (
        "a_design_that_states_its_drill_spans_is_told_they_are_dropped",
        "the warning stands for spans absent from the board it wrote, and nothing opens it",
    ),
    (
        "a_stated_stiffener_is_named_with_its_thickness_and_material",
        "the warning names what the files cannot carry, and the files are not read",
    ),
];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

fn tests_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests")
}

fn help_for(subcommand: Option<&str>) -> String {
    let mut command = Command::new(env!("CARGO_BIN_EXE_cypcb"));
    if let Some(name) = subcommand {
        command.arg(name);
    }
    let output = command
        .arg("--help")
        .current_dir(repo_root())
        .output()
        .expect("the binary runs");
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// Every subcommand the binary lists for itself.
fn subcommands() -> Vec<String> {
    let help = help_for(None);
    let mut names = Vec::new();
    let mut inside = false;
    for line in help.lines() {
        if line.starts_with("Commands:") {
            inside = true;
            continue;
        }
        if inside {
            if line.trim().is_empty() || !line.starts_with("  ") {
                if line.ends_with(':') {
                    break;
                }
                continue;
            }
            if let Some(name) = line.split_whitespace().next() {
                if name != "help" && name.chars().all(|c| c.is_ascii_lowercase() || c == '-') {
                    names.push(name.to_string());
                }
            }
        }
    }
    names
}

/// The subcommands that write a file, read off the binary's own help rather
/// than listed here.
///
/// A command writes when its output option documents a destination with a
/// default path. `check -o json` names stdout in the same breath, and a format
/// selector is not a file. Deriving the set this way is the point: a writing
/// command added next year joins it without anybody remembering to, which is
/// the failure this whole check is about, one level up.
fn writing_subcommands() -> BTreeSet<String> {
    let mut writing = BTreeSet::new();
    for name in subcommands() {
        let help = help_for(Some(&name));
        let mut block = String::new();
        let mut collecting = false;
        for line in help.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("-o, --output")
                || trimmed.starts_with("--output")
                || trimmed.starts_with("--out-dir")
            {
                collecting = true;
                block.push_str(line);
                block.push('\n');
                continue;
            }
            if collecting {
                if trimmed.starts_with('-') && !trimmed.starts_with("--") {
                    break;
                }
                if line.trim().is_empty() && !block.trim().is_empty() && block.lines().count() > 1 {
                    break;
                }
                block.push_str(line);
                block.push('\n');
            }
        }
        let lower = block.to_lowercase();
        if lower.contains("default") && !lower.contains("stdout") {
            writing.insert(name);
        }
    }
    writing
}

/// One function of the test crate: where it lives, and its body.
struct Function {
    file: String,
    body: String,
    is_test: bool,
}

/// Every function in every test source, with the `#[test]` ones marked.
fn functions() -> Vec<(String, Function)> {
    let mut found = Vec::new();
    let mut stack = vec![tests_dir()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let file = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("?")
                .to_string();
            let text = std::fs::read_to_string(&path).expect("a test source is readable");
            let lines: Vec<&str> = text.lines().collect();
            for (index, line) in lines.iter().enumerate() {
                let trimmed = line.trim_start();
                let Some(rest) = trimmed.strip_prefix("fn ") else {
                    continue;
                };
                let Some(name) = rest.split('(').next() else {
                    continue;
                };
                if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                    continue;
                }
                let is_test = lines[index.saturating_sub(4)..index]
                    .iter()
                    .any(|above| above.trim() == "#[test]");
                let indent = line.len() - trimmed.len();
                let closing = " ".repeat(indent) + "}";
                let mut body = String::new();
                for candidate in &lines[index..] {
                    body.push_str(candidate);
                    body.push('\n');
                    if *candidate == closing {
                        break;
                    }
                }
                found.push((
                    name.to_string(),
                    Function {
                        file: file.clone(),
                        body,
                        is_test,
                    },
                ));
            }
        }
    }
    found
}

/// The names a body calls, as far as `name(` can tell.
fn calls(body: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let bytes: Vec<char> = body.chars().collect();
    let mut word = String::new();
    for (index, character) in bytes.iter().enumerate() {
        if character.is_alphanumeric() || *character == '_' {
            word.push(*character);
            continue;
        }
        if *character == '(' && !word.is_empty() && index > 0 {
            names.insert(word.clone());
        }
        word.clear();
    }
    names
}

/// A test's body plus the bodies of everything it calls, three hops deep.
///
/// Same file first, then the crate. A name defined twice with different
/// bodies resolves against the test: for a gate holding a list of names, the
/// only safe direction of error is to accuse, because an accusation reaches
/// the author and a pardon disappears. Macros and calls through a function
/// pointer are out of reach of this map, and a helper reached that way would
/// be accused.
fn expanded(start: &Function, by_name: &BTreeMap<String, Vec<(String, String)>>) -> String {
    let mut text = start.body.clone();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut frontier: Vec<String> = calls(&start.body).into_iter().collect();
    for _ in 0..MAX_DEPTH {
        let mut next = Vec::new();
        for name in frontier.drain(..) {
            if !seen.insert(name.clone()) {
                continue;
            }
            let Some(definitions) = by_name.get(&name) else {
                continue;
            };
            let local: Vec<&(String, String)> = definitions
                .iter()
                .filter(|(file, _)| *file == start.file)
                .collect();
            let chosen: Vec<&(String, String)> = if local.is_empty() {
                definitions.iter().collect()
            } else {
                local
            };
            // A name defined once resolves. A name defined in several files
            // resolves only if every candidate agrees about reading an
            // artefact; where they disagree, none of them is added and the
            // test gets no credit from that name. The judgement is per name
            // rather than per test, because one ambiguous helper elsewhere
            // must not throw away what the test plainly does itself.
            let readers = chosen
                .iter()
                .filter(|(_, body)| reads_artefact(body))
                .count();
            if chosen.len() > 1 && readers != 0 && readers != chosen.len() {
                continue;
            }
            for (_, body) in &chosen {
                text.push_str(body);
                next.extend(calls(body));
            }
        }
        frontier = next;
    }
    text
}

/// Whether a body reaches a file at all.
///
/// The limit, said here so nobody re-derives it: this cannot tell the artefact
/// from any other file. A test that opens a document instead of the output it
/// wrote reads as reaching it. Tightening that means knowing which path the run
/// under test was given, which this walk does not track - so the check finds
/// tests that read nothing, and not tests that read the wrong thing.
fn reads_artefact(text: &str) -> bool {
    ARTEFACT_READS.iter().any(|needle| text.contains(needle))
        || ARTEFACT_READERS.iter().any(|needle| text.contains(needle))
}

const ARTEFACT_READS: &[&str] = &[
    "read_to_string",
    "fs::read",
    "File::open",
    "OpenOptions",
    "read_dir",
    "metadata(",
    ".exists()",
];

/// The subcommands that read a board back. Running one of these over a path the
/// run under test wrote is reaching the artefact: the importer reading what the
/// exporter wrote is two subsystems meeting, not one piece of code confirming
/// itself.
const ARTEFACT_READERS: &[&str] = &["\"check\"", "\"score\"", "\"parse\"", "\"parse-kicad\""];

#[test]
fn a_test_that_drives_a_writing_command_opens_what_it_wrote() {
    let writing = writing_subcommands();
    assert!(
        writing.contains("export"),
        "the writing set is read off the binary's own help, and `export` writes files: {writing:?}"
    );

    let all = functions();
    let mut by_name: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    for (name, function) in &all {
        by_name
            .entry(name.clone())
            .or_default()
            .push((function.file.clone(), function.body.clone()));
    }

    let mut drives = 0usize;
    let mut reaches = 0usize;
    let mut owed = 0usize;
    let mut offenders: Vec<(String, String, String)> = Vec::new();
    for (name, function) in &all {
        if !function.is_test {
            continue;
        }
        let text = expanded(function, &by_name);
        if !text.contains("CARGO_BIN_EXE_cypcb") && !text.contains("cypcb_binary") {
            continue;
        }
        if text.contains("--dry-run") {
            continue; // a dry run writes nothing by contract
        }
        let Some(subcommand) = writing
            .iter()
            .find(|sub| text.contains(&format!("\"{sub}\"")))
        else {
            continue;
        };
        drives += 1;
        if reads_artefact(&text) {
            reaches += 1;
            continue;
        }
        if MESSAGE_ONLY_BY_RIGHT.iter().any(|(named, _)| named == name)
            || NOTHING_TO_OPEN.iter().any(|(named, _)| named == name)
        {
            continue;
        }
        if OWED_AN_ARTEFACT_READING
            .iter()
            .any(|(named, _)| named == name)
        {
            owed += 1;
            continue;
        }
        offenders.push((function.file.clone(), name.clone(), subcommand.clone()));
    }

    eprintln!(
        "tests driving a writing command: {drives}; reaching the artefact: {reaches}; \
         message-only by right: {}; nothing to open: {}; owed a reading: {owed} of {}",
        MESSAGE_ONLY_BY_RIGHT.len(),
        NOTHING_TO_OPEN.len(),
        OWED_AN_ARTEFACT_READING.len()
    );

    assert!(
        drives >= TESTS_EXPECTED_TO_DRIVE_A_WRITING_COMMAND,
        "this check found {drives} tests driving a command that writes, and expected at least \
         {TESTS_EXPECTED_TO_DRIVE_A_WRITING_COMMAND}. Either the crate shrank, or the way tests \
         invoke the binary changed and this check is now looking at nothing - which is the \
         failure it exists to prevent, one level up."
    );

    // The debt may be paid and may not be taken on. A test that stops needing
    // its entry has to lose it, or the list pardons a name owed nothing.
    assert_eq!(
        owed,
        OWED_AN_ARTEFACT_READING.len(),
        "the debt list names {} tests and {owed} of them still fail to read what they wrote. \
         An entry that no longer applies is a pardon nobody asked for: delete it.",
        OWED_AN_ARTEFACT_READING.len()
    );

    let named: BTreeSet<&str> = all
        .iter()
        .filter(|(_, function)| function.is_test)
        .map(|(name, _)| name.as_str())
        .collect();
    for (entry, _) in MESSAGE_ONLY_BY_RIGHT
        .iter()
        .chain(NOTHING_TO_OPEN.iter())
        .chain(OWED_AN_ARTEFACT_READING.iter())
    {
        assert!(
            named.contains(entry),
            "a list names `{entry}`, which no test in this crate defines. A list that excuses a \
             test that no longer exists excuses nothing and hides the next one. Delete the entry, \
             or fix the name it was renamed to."
        );
    }

    assert!(
        offenders.is_empty(),
        "{}",
        offenders
            .iter()
            .map(|(file, name, subcommand)| format!(
                "\n{file}::{name} drives `cypcb {subcommand}`, which writes, and never reaches \
                 what it wrote.\n\
                 \n  Three doors, and the third is as respectable as the first:\n\
                 \n  1. Read the artefact. Open the file or directory the run produced and assert \
                 on its content, or run the binary again over that path - reading what this \
                 project wrote with this project's reader is two subsystems, not one, and it \
                 counts.\n\
                 \n  2. The run writes nothing by right - a help text, a refusal, a format \
                 selector. Add the test to MESSAGE_ONLY_BY_RIGHT with the reason in one clause.\n\
                 \n  3. Nothing on disk can carry the claim. Add it to NOTHING_TO_OPEN with the \
                 reason.\n\
                 \n  There is no fourth door. OWED_AN_ARTEFACT_READING is a debt taken on \
                 once and it does not accept new names.\n"
            ))
            .collect::<String>()
    );
}
