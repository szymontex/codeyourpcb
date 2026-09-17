//! Every writer of an inner layer name counts from one.
//!
//! The enum counts from zero and the name counts from one: `Layer::Inner(0)`
//! is written `Inner1`, which `crates/cypcb-world/src/components/physical.rs`
//! states as a deliberate boundary. The viewer parses those names and
//! subtracts the one back out, and a case on that side already holds it.
//!
//! This is the other side. Four places in this workspace turn the enum into
//! the name, and a fifth written without the `+ 1` would put every inner trace
//! one layer off in the viewer while every test here still passed - the number
//! is right in the model, and wrong only where the two counts meet.
//!
//! `cargo test -p cypcb-cli --test every_writer_of_an_inner_layer_name_counts_from_one`

use std::path::{Path, PathBuf};

use cypcb_world::Layer;

/// Sites turning the enum into the name, today 4.
const WRITERS_FLOOR: usize = 4;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|crates| crates.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn every_writer_of_an_inner_layer_name_counts_from_one() {
    // The definition, called rather than read.
    assert_eq!(Layer::Inner(0).to_string(), "Inner1");
    assert_eq!(Layer::Inner(1).to_string(), "Inner2");
    assert_eq!(Layer::Inner(29).to_string(), "Inner30");

    let root = repo_root();
    let mut files = Vec::new();
    for entry in std::fs::read_dir(root.join("crates")).expect("the crates directory is there") {
        let src = entry.expect("a readable entry").path().join("src");
        if src.is_dir() {
            rust_files(&src, &mut files);
        }
    }
    files.sort();

    let mut writers = Vec::new();
    let mut counting_from_zero = Vec::new();

    for path in &files {
        let text = std::fs::read_to_string(path).expect("a readable source file");
        for (index, line) in text.lines().enumerate() {
            // The name as it is written out, in either of the two macros that
            // write it. A doc comment mentioning the name is prose, not a
            // writer, and carries neither macro.
            if !line.contains("Inner{}") && !line.contains("Inner{") {
                continue;
            }
            if !(line.contains("format!") || line.contains("write!") || line.contains("writeln!")) {
                continue;
            }
            let where_it_is = format!(
                "{}:{}",
                path.strip_prefix(&root).unwrap_or(path).display(),
                index + 1
            );
            writers.push(where_it_is.clone());
            if !line.contains("+ 1") {
                counting_from_zero.push(format!("{where_it_is}: {}", line.trim()));
            }
        }
    }

    println!(
        "sites writing an inner layer name: {} (floor {WRITERS_FLOOR}); \
         writing the enum's own number: {}",
        writers.len(),
        counting_from_zero.len()
    );

    assert!(
        writers.len() >= WRITERS_FLOOR,
        "the walk found {} of these and the workspace had {WRITERS_FLOOR} when this was \
         written - a scan that stops finding them passes for the wrong reason.\n  Found: {}",
        writers.len(),
        writers.join("\n  ")
    );
    assert!(
        counting_from_zero.is_empty(),
        "an inner layer name is written with the enum's own number:\n  {}\
         \n  `Layer::Inner(0)` is `Inner1` everywhere a person reads it, and the viewer \
         subtracts that one back out - a name written from zero moves every inner trace \
         one layer.",
        counting_from_zero.join("\n  ")
    );
}
