//! The folder a flag's help promises is the folder the export writes to.
//!
//! `--svg`, `--dxf` and `--pdf` each say their files land in `plot/`, and
//! `--ipc356` says its netlist lands in `netlist/` beside the Gerbers. Those
//! sentences are what a person reads before they write a script that collects
//! the output, and nothing held them to it: the folder is chosen in
//! `export.rs` and the promise is made in a doc comment a few hundred lines
//! above.
//!
//! The flags are read out of the source rather than listed here, so a fifth
//! flag promising a folder is held to its promise on the day it is written.
//!
//! `cargo test -p cypcb-cli --test the_help_promises_the_folder_the_export_writes_to`

use std::path::{Path, PathBuf};
use std::process::Command;

/// Flags whose help names a folder, today 4.
const PROMISES_FLOOR: usize = 4;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|crates| crates.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

/// Every flag whose doc comment names a folder, with the folder it names.
fn promises() -> Vec<(String, String)> {
    let source =
        std::fs::read_to_string(repo_root().join("crates/cypcb-cli/src/commands/export.rs"))
            .expect("the export command is there");
    let lines: Vec<&str> = source.lines().collect();

    let mut found = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        let Some(prose) = trimmed.strip_prefix("///") else {
            continue;
        };
        // A folder is named as `plot/` or `netlist/` - a word in backticks
        // ending in a slash.
        let Some(folder) = prose.split('`').find(|piece| {
            piece.ends_with('/') && piece.len() > 1 && !piece.contains(' ') && !piece.contains('.')
        }) else {
            continue;
        };
        // The field this comment belongs to, which is the flag it describes.
        // The fields here are private, so `pub` is optional rather than the
        // marker - the first line that is neither prose nor an attribute is
        // the declaration.
        let Some(field) = lines[index + 1..]
            .iter()
            .take(12)
            .map(|next| next.trim())
            .find(|next| !next.starts_with("///") && !next.starts_with("#["))
            .map(|declaration| declaration.strip_prefix("pub ").unwrap_or(declaration))
            .and_then(|declaration| declaration.split(':').next())
            .filter(|name| {
                !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_')
            })
        else {
            continue;
        };
        let flag = format!("--{}", field.replace('_', "-"));
        let folder = folder.trim_end_matches('/').to_string();
        if !found.iter().any(|(f, d)| f == &flag && d == &folder) {
            found.push((flag, folder));
        }
    }
    found
}

#[test]
fn the_help_promises_the_folder_the_export_writes_to() {
    let promises = promises();
    println!(
        "export flags whose help names a folder: {} (floor {PROMISES_FLOOR}): {promises:?}",
        promises.len()
    );
    assert!(
        promises.len() >= PROMISES_FLOOR,
        "the walk found {} of these and there were {PROMISES_FLOOR} when this was written - \
         a scan that stops finding them passes for the wrong reason",
        promises.len()
    );

    let root = repo_root();
    let mut broken = Vec::new();

    for (flag, folder) in &promises {
        let out = root.join(format!("target/tmp-export{}", flag.replace('-', "_")));
        let _ = std::fs::remove_dir_all(&out);

        let run = Command::new(env!("CARGO_BIN_EXE_cypcb"))
            .current_dir(&root)
            .args([
                "export",
                "examples/blink.cypcb",
                "--output",
                out.to_str().expect("a path this test made"),
                flag,
            ])
            .output()
            .expect("the binary runs");
        assert!(
            run.status.success(),
            "{flag} did not export: {}",
            String::from_utf8_lossy(&run.stderr)
        );

        let written = std::fs::read_dir(out.join(folder))
            .map(|entries| entries.flatten().count())
            .unwrap_or(0);
        if written == 0 {
            broken.push(format!("{flag} promises {folder}/ and wrote nothing there"));
        }

        let _ = std::fs::remove_dir_all(&out);
    }

    assert!(
        broken.is_empty(),
        "a flag's help names a folder its files do not land in:\n  {}\
         \n  The sentence is what a person reads before they write the script that \
         collects the output.",
        broken.join("\n  ")
    );
}

/// The plot flags promise one file per copper layer, so a four-layer board
/// gets four.
///
/// `examples/blink.cypcb` has two copper layers, and a plotter that only ever
/// drew the outer pair would satisfy every check written against it. The
/// count comes from the board rather than from this file: the example states
/// `layers 4`, and the exporter has to agree with what the design declares.
#[test]
fn one_file_per_copper_layer_is_one_per_layer_the_board_declares() {
    let root = repo_root();
    let source = std::fs::read_to_string(root.join("examples/four-layer.cypcb"))
        .expect("the four-layer example is there");
    let declared: usize = source
        .lines()
        .find_map(|line| line.trim().strip_prefix("layers "))
        .and_then(|count| count.trim().parse().ok())
        .expect("the example declares a layer count");
    assert!(
        declared > 2,
        "a board with {declared} copper layers cannot show that the inner ones are drawn"
    );

    for flag in ["--svg", "--dxf", "--pdf"] {
        let out = root.join(format!("target/tmp-layers{}", flag.replace('-', "_")));
        let _ = std::fs::remove_dir_all(&out);

        let run = Command::new(env!("CARGO_BIN_EXE_cypcb"))
            .current_dir(&root)
            .args([
                "export",
                "examples/four-layer.cypcb",
                "--output",
                out.to_str().expect("a path this test made"),
                flag,
            ])
            .output()
            .expect("the binary runs");
        assert!(
            run.status.success(),
            "{flag} did not export: {}",
            String::from_utf8_lossy(&run.stderr)
        );

        let mut drawn: Vec<String> = std::fs::read_dir(out.join("plot"))
            .expect("the plot folder is there")
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .collect();
        drawn.sort();

        assert_eq!(
            drawn.len(),
            declared,
            "{flag} promises one file per copper layer, the board declares {declared}, \
             and it wrote {drawn:?}"
        );
        for layer in ["F_Cu", "In1_Cu", "In2_Cu", "B_Cu"] {
            assert!(
                drawn.iter().any(|name| name.contains(layer)),
                "{flag} drew {drawn:?}, with nothing for {layer}"
            );
        }

        let _ = std::fs::remove_dir_all(&out);
    }
}
