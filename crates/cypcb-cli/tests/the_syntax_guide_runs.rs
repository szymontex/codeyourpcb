//! The two halves of the syntax guide the parser test leaves alone.
//!
//! `cargo test -p cypcb-cli --test the_syntax_guide_runs`
//!
//! `cypcb-parser`'s `the_syntax_guide_parses` holds every block the guide
//! presents as **correct** to the grammar, and names what it skips. Two of
//! those skips are claims of their own, and both need the binary rather than
//! the parser:
//!
//! - the blocks marked `// ERROR`, which the guide teaches as mistakes. A
//!   mistake the guide teaches and the tool accepts is worse than no section
//!   at all - the reader learns a rule that is not there;
//! - the command under **Validation**, which named `my_board.cypcb` until this
//!   test was written. The README carried the same shape of defect two commits
//!   ago: a placeholder path that answers `No such file or directory` for
//!   anybody who copies it.
//!
//! The guide's **Missing Units** section was a third: it called `at 15, 10`
//! wrong, and the grammar reads a bare number as millimetres and always has.
//! The section says so now, and names the one place the checker warns - a
//! board size, where getting it wrong resizes the whole design.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

fn guide() -> String {
    std::fs::read_to_string(repo_root().join("docs/SYNTAX.md"))
        .expect("the syntax guide is in the repo")
}

/// Every fenced block, with the tag it was opened with.
fn blocks(markdown: &str) -> Vec<(String, String)> {
    let mut found = Vec::new();
    let mut current: Option<(String, String)> = None;
    for line in markdown.lines() {
        if let Some(tag) = line.trim_start().strip_prefix("```") {
            match current.take() {
                Some(block) => found.push(block),
                None => current = Some((tag.trim().to_string(), String::new())),
            }
            continue;
        }
        if let Some((_, body)) = current.as_mut() {
            body.push_str(line);
            body.push('\n');
        }
    }
    found
}

#[test]
fn a_block_the_guide_marks_as_wrong_is_refused() {
    let blocks = blocks(&guide());
    let wrong: Vec<&(String, String)> = blocks
        .iter()
        .filter(|(_, body)| body.contains("// ERROR"))
        .collect();
    assert_eq!(
        wrong.len(),
        2,
        "two blocks were marked `// ERROR` when this was written; a third \
         needs to be a deliberate addition rather than a number to update"
    );

    let dir = std::env::temp_dir().join("cypcb-guide-wrong");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to work");

    for (index, (_, body)) in wrong.iter().enumerate() {
        let path = dir.join(format!("wrong-{index}.cypcb"));
        std::fs::write(&path, format!("version 1\n\n{body}")).expect("the block is writable");

        // `parse -o ast` stops after the grammar, so this is the parser's own
        // answer rather than a semantic complaint about a component the
        // snippet never defines.
        let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
            .args([
                "parse".as_ref(),
                path.as_os_str(),
                "-o".as_ref(),
                "ast".as_ref(),
            ])
            .output()
            .expect("the binary runs");
        assert!(
            !output.status.success(),
            "the guide teaches this as a mistake and the parser accepts it:\n{body}"
        );
    }
}

#[test]
fn the_command_the_guide_tells_the_reader_to_run_works() {
    let blocks = blocks(&guide());
    let shell: Vec<String> = blocks
        .iter()
        .filter(|(tag, _)| tag == "bash")
        .flat_map(|(_, body)| {
            body.lines()
                .filter_map(|line| line.trim().strip_prefix("cypcb "))
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(
        shell.len(),
        1,
        "the guide asks the reader to type one command: {shell:?}"
    );

    for command in &shell {
        let words: Vec<&str> = command.split_whitespace().collect();
        let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
            .args(&words)
            .current_dir(repo_root())
            .output()
            .expect("the binary runs");
        assert_eq!(
            output.status.code(),
            Some(0),
            "the guide's Validation section says to run `cypcb {command}` and it \
             does not work:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
