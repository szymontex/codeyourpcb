//! A command that cannot work says so in its status, not only on the screen.
//!
//! A person reads the diagnostic; a build script reads the number. A command
//! that prints "could not parse" and exits zero is a step that passes in
//! somebody's pipeline with no files behind it, and the failure shows up
//! wherever the missing output is finally needed.
//!
//! Both kinds of bad input are asked for: a file that is there and cannot be
//! read, and a file that is not there at all. The second is the one a script
//! meets most - a path typed wrong, a step that ran in the wrong directory.
//!
//! `cargo test -p cypcb-cli --test a_command_that_cannot_work_says_so_in_its_status`

use std::path::{Path, PathBuf};
use std::process::Command;

/// Subcommands taking a file, today 9.
const COMMANDS_FLOOR: usize = 9;

/// Every subcommand that reads a file, with the name it expects.
const READS_A_FILE: &[(&str, &str)] = &[
    ("parse", "broken.cypcb"),
    ("check", "broken.cypcb"),
    ("export", "broken.cypcb"),
    ("route", "broken.cypcb"),
    ("score", "broken.cypcb"),
    ("to-kicad", "broken.cypcb"),
    ("from-kicad", "broken.kicad_pcb"),
    ("parse-kicad", "broken.kicad_pcb"),
    ("from-dxf", "broken.dxf"),
];

/// Subcommands run below in a shape of their own rather than as `<command>
/// <file>`, with what stands in for the bad input.
const RUN_ANOTHER_WAY: &[(&str, &str)] = &[
    (
        "library",
        "takes a subcommand, and the directory an import reads is the thing that can be missing",
    ),
    (
        "help",
        "is the one command whose whole job is to succeed, so what is asked of it is that it \
         does - and that a help request for a subcommand nobody wrote is refused",
    ),
];

/// Subcommands this does not run, and why.
const NOT_RUN: &[(&str, &str)] = &[(
    "watch",
    "waits for the file to change and never returns on its own - it is driven by \
         `a_saved_design_is_checked_again`, which saves a board and reads what the \
         watcher printed",
)];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|crates| crates.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

fn run_in(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .current_dir(dir)
        .args(args)
        .output()
        .expect("the binary runs")
}

#[test]
fn a_command_that_cannot_work_says_so_in_its_status() {
    // Every subcommand the CLI has, so a tenth one added later is either in
    // the table above or named as not run.
    let names: Vec<String> = {
        let help = run_in(&repo_root(), &["help"]);
        String::from_utf8_lossy(&help.stdout)
            .lines()
            .skip_while(|line| !line.starts_with("Commands:"))
            .skip(1)
            .take_while(|line| line.starts_with("  "))
            .filter_map(|line| line.split_whitespace().next())
            .map(|name| name.to_string())
            .collect()
    };
    assert!(
        names.len() >= COMMANDS_FLOOR,
        "the help listed {} subcommands: {names:?}",
        names.len()
    );
    for name in &names {
        let known = READS_A_FILE.iter().any(|(command, _)| command == name)
            || RUN_ANOTHER_WAY.iter().any(|(command, _)| command == name)
            || NOT_RUN.iter().any(|(command, _)| command == name);
        assert!(
            known,
            "{name} is a subcommand this case has never run - add it to the table, or \
             say here why it cannot be run"
        );
    }

    let work = repo_root().join("target/tmp-exit-status");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).expect("a working directory");
    for (_, file) in READS_A_FILE {
        std::fs::write(work.join(file), "this is not a board\n").expect("the bad file is written");
    }

    let mut quiet_successes = Vec::new();
    let mut unnamed = Vec::new();
    let mut examined = 0;

    for (command, file) in READS_A_FILE {
        for (input, what) in [
            (*file, "a file that cannot be read"),
            ("absent.cypcb", "a file that is not there"),
        ] {
            examined += 1;
            let run = run_in(&work, &[command, input]);
            if run.status.success() {
                quiet_successes.push(format!("{command} on {what} exited 0"));
                continue;
            }
            assert!(
                !run.stderr.is_empty(),
                "{command} on {what} failed with nothing on stderr - the status is right \
                 and the person is told nothing"
            );
            // The name of the file they typed. A person running a command over
            // a directory reads a line and a column and cannot tell which
            // board they belong to without it.
            let said = String::from_utf8_lossy(&run.stderr);
            if !said.contains(input) {
                unnamed.push(format!("{command} on {what} never said {input}"));
            }
        }
    }

    // `library` takes a subcommand rather than a file, and the directory it
    // imports from is the thing that can be missing. Its excuse here used to
    // say it reads the machine's libraries; it reads an index, and an import
    // of a directory that is not there is the same question as the runs above.
    examined += 1;
    let run = run_in(&work, &["library", "import", "no-such-directory"]);
    if run.status.success() {
        quiet_successes.push("library import of a directory that is not there exited 0".into());
    } else {
        let said = String::from_utf8_lossy(&run.stderr);
        if !said.contains("no-such-directory") {
            unnamed.push("library import never said no-such-directory".into());
        }
    }

    // `help` is the one command asked to succeed. The rest of this block is the
    // other half of that: a word the CLI does not know is refused, whether it
    // arrives as a subcommand, as a request for help about one, or as a flag.
    examined += 1;
    let helped = run_in(&work, &["help"]);
    assert!(
        helped.status.success(),
        "`cypcb help` is the one command whose whole job is to succeed and it did not"
    );

    for unknown in [
        vec!["help", "nonsense"],
        vec!["nonsense"],
        vec!["--nonsense"],
    ] {
        examined += 1;
        let run = run_in(&work, &unknown);
        let said = String::from_utf8_lossy(&run.stderr);
        if run.status.success() {
            quiet_successes.push(format!("{unknown:?} exited 0"));
        } else if !said.contains("nonsense") {
            unnamed.push(format!("{unknown:?} was refused without naming the word"));
        }
    }

    println!(
        "subcommands in the help: {}; runs examined: {examined}; exiting 0 on bad input: {}; \
         failing without naming the file: {}",
        names.len(),
        quiet_successes.len(),
        unnamed.len()
    );

    let _ = std::fs::remove_dir_all(&work);

    assert!(
        quiet_successes.is_empty(),
        "a command could not do its work and reported success:\n  {}\
         \n  A build script reads the number, not the diagnostic.",
        quiet_successes.join("\n  ")
    );
    assert!(
        unnamed.is_empty(),
        "a command failed without naming the file it was given:\n  {}\
         \n  The diagnostics underneath carry a line and a column and no name.",
        unnamed.join("\n  ")
    );
}
