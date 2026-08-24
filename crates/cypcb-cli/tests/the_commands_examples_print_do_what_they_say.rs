//! The commands the examples print, run rather than read.
//!
//! `cargo test -p cypcb-cli --test the_commands_examples_print_do_what_they_say`
//!
//! Every example carries a `cypcb ...` line in its header. Nothing ran them,
//! and three of the eight did not do what their header implied: `cutout`,
//! `v2-imports` and `v2-interfaces` each printed a command that exits 1, and
//! `v2-interfaces` said in as many words that the file "checks clean" while
//! `cypcb check` reported 22 violations against it.
//!
//! So a header that prints a command now states the outcome, and this runs
//! each one:
//!
//! - a header that says **exits 1** has to exit 1, and any other has to exit 0;
//! - a figure the header states - `reports 22 violations`, `15 unconnected
//!   pins`, `7 unrouted ones` - has to be a figure the command prints.
//!
//! A number in a comment is a claim like any other, and this is the cheapest
//! place in the project to leave one behind: the board changes, the sentence
//! does not, and nobody reads an example's header while working on a rule.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The repository root, which is where the printed commands are meant to be
/// run from - they name their boards as `examples/x.cypcb`.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

/// One example's header and the commands it prints.
struct Example {
    name: String,
    header: String,
    commands: Vec<String>,
}

/// Every example that prints a command, with the comment block above it.
fn examples() -> Vec<Example> {
    let dir = repo_root().join("examples");
    let mut found = Vec::new();

    let mut entries: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("the examples are where they have always been")
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "cypcb"))
        .collect();
    entries.sort();

    for path in entries {
        let source = std::fs::read_to_string(&path).expect("an example is readable");
        // The header is the comment block the file opens with: everything
        // before the first line that is not a comment or blank.
        let header: String = source
            .lines()
            .take_while(|line| line.starts_with("//") || line.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        let commands: Vec<String> = header
            .lines()
            .filter_map(|line| line.strip_prefix("//   cypcb "))
            .map(|rest| rest.trim().to_string())
            .collect();
        if commands.is_empty() {
            continue;
        }
        found.push(Example {
            name: path
                .file_name()
                .expect("a file has a name")
                .to_string_lossy()
                .to_string(),
            header,
            commands,
        });
    }
    found
}

/// The figures a header states about what its command prints, as the output
/// would spell them.
///
/// `reports 22 violations, 15 unconnected pins and 7 unrouted ones` becomes
/// three needles. Read by walking the words rather than by pattern, because a
/// comment is prose and the next sentence will be written differently.
fn stated_figures(header: &str) -> Vec<String> {
    let words: Vec<&str> = header.split_whitespace().collect();
    let mut needles = Vec::new();
    for pair in words.windows(2) {
        let Ok(count) = pair[0].parse::<usize>() else {
            continue;
        };
        let what = pair[1].trim_matches(|c: char| !c.is_alphanumeric());
        let needle = match what {
            "violations" => format!("{count} DRC violation(s)"),
            "unconnected" => format!("unconnected-pin: {count}"),
            "unrouted" => format!("unrouted-pin: {count}"),
            _ => continue,
        };
        needles.push(needle);
    }
    needles
}

#[test]
fn every_command_an_example_prints_ends_the_way_its_header_says() {
    let examples = examples();
    assert!(
        examples.len() >= 7,
        "seven examples printed a command when this was written and the \
         extractor has to keep finding them: {} found",
        examples.len()
    );
    let printed: usize = examples.iter().map(|e| e.commands.len()).sum();
    assert!(
        printed >= 8,
        "eight commands were printed when this was written: {printed} found"
    );

    for example in &examples {
        let expects_failure = example.header.contains("exits 1");

        for command in &example.commands {
            let words: Vec<&str> = command.split_whitespace().collect();
            let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
                .args(&words)
                .current_dir(repo_root())
                .output()
                .unwrap_or_else(|err| {
                    panic!("{}: `cypcb {command}` did not run: {err}", example.name)
                });
            let said = String::from_utf8_lossy(&output.stdout).to_string()
                + &String::from_utf8_lossy(&output.stderr);

            let code = output.status.code();
            if expects_failure {
                assert_eq!(
                    code,
                    Some(1),
                    "{}: its header says the command exits 1 and `cypcb {command}` \
                     exited {code:?}:\n{said}",
                    example.name
                );
            } else {
                assert_eq!(
                    code,
                    Some(0),
                    "{}: a command an example prints has to work, or the header \
                     has to say what it does instead. `cypcb {command}` exited \
                     {code:?}:\n{said}",
                    example.name
                );
            }

            for needle in stated_figures(&example.header) {
                assert!(
                    said.contains(&needle),
                    "{}: its header states `{needle}` and `cypcb {command}` does \
                     not print it:\n{said}",
                    example.name
                );
            }
        }
    }
}
