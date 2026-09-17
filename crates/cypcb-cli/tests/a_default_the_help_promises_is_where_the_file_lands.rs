//! A default the help promises is where the file lands.
//!
//! Six flags in `cypcb`'s commands finish their help with `(default: ...)`,
//! and a default is the sentence most people rely on: it is what happens when
//! they type nothing. `--output` on `route` promised `input.routes` and wrote
//! a design instead, which is the defect this file exists to stop repeating.
//!
//! The promises are read out of the sources rather than listed here. The ones
//! this can run are run; the rest are on a keyed list with the reason, so a
//! promise that is merely inconvenient to exercise still has to be named.
//!
//! `cargo test -p cypcb-cli --test a_default_the_help_promises_is_where_the_file_lands`

use std::path::{Path, PathBuf};
use std::process::Command;

/// Help lines ending in a promised default, today 6.
const PROMISES_FLOOR: usize = 5;

/// A promise this case does not run, and why.
const NOT_RUN: &[(&str, &str)] = &[(
    "route.rs",
    "is the routing timeout, a number rather than a file",
)];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|crates| crates.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

/// Every command file whose help promises a default, once each.
fn files_promising_a_default() -> Vec<String> {
    let dir = repo_root().join("crates/cypcb-cli/src/commands");
    let mut found = Vec::new();
    for entry in std::fs::read_dir(&dir)
        .expect("the commands are there")
        .flatten()
    {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("a readable command");
        let promises = text.lines().any(|line| {
            let trimmed = line.trim();
            trimmed.starts_with("///") && trimmed.contains("(default:")
        });
        if promises {
            found.push(path.file_name().unwrap().to_string_lossy().to_string());
        }
    }
    found.sort();
    found
}

/// A drawing holding one closed rectangle, in millimetres.
///
/// The importer's own cases build richer ones; this is the smallest drawing
/// that gives `from-dxf` an outline to take, because the question here is
/// where the file lands rather than what is in it.
fn rectangle_drawing(at: &Path) {
    let mut entities = String::from("0\nLWPOLYLINE\n8\nOUTLINE\n90\n4\n70\n1\n");
    for (x, y) in [(0.0, 0.0), (40.0, 0.0), (40.0, 30.0), (0.0, 30.0)] {
        entities.push_str(&format!("10\n{x}\n20\n{y}\n"));
    }
    let text = format!(
        "0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1009\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n\
         0\nSECTION\n2\nENTITIES\n{entities}0\nENDSEC\n0\nEOF\n"
    );
    std::fs::write(at, text).expect("the drawing is written");
}

fn run_in(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .current_dir(dir)
        .args(args)
        .output()
        .expect("the binary runs")
}

#[test]
fn a_default_the_help_promises_is_where_the_file_lands() {
    let promising = files_promising_a_default();
    println!(
        "command files promising a default: {} (floor {PROMISES_FLOOR}): {promising:?}; \
         run here: {}",
        promising.len(),
        promising.len() - NOT_RUN.len()
    );
    assert!(
        promising.len() >= PROMISES_FLOOR,
        "the walk found {} files promising a default and there were {PROMISES_FLOOR} when \
         this was written - a scan that stops finding them passes for the wrong reason",
        promising.len()
    );
    for (excused, _) in NOT_RUN {
        assert!(
            promising.contains(&excused.to_string()),
            "{excused} is excused from a promise it no longer makes - drop the excuse"
        );
    }

    let root = repo_root();
    let work = root.join("target/tmp-defaults");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).expect("a working directory");

    // to-kicad: the input file with a .kicad_pcb suffix.
    std::fs::copy(root.join("examples/blink.cypcb"), work.join("board.cypcb"))
        .expect("the example copies");
    let run = run_in(&work, &["to-kicad", "board.cypcb"]);
    assert!(
        run.status.success(),
        "to-kicad refused the board: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(
        work.join("board.kicad_pcb").is_file(),
        "to-kicad promises the input file with a .kicad_pcb suffix and wrote {:?}",
        std::fs::read_dir(&work)
            .map(|entries| entries.flatten().map(|e| e.file_name()).collect::<Vec<_>>())
            .unwrap_or_default()
    );

    // from-kicad: the input file with a .cypcb suffix, in a directory of its
    // own so the design above cannot be mistaken for the one written here.
    let back = root.join("target/tmp-defaults-back");
    let _ = std::fs::remove_dir_all(&back);
    std::fs::create_dir_all(&back).expect("a working directory");
    std::fs::copy(
        work.join("board.kicad_pcb"),
        back.join("imported.kicad_pcb"),
    )
    .expect("the board copies");
    let run = run_in(&back, &["from-kicad", "imported.kicad_pcb"]);
    assert!(
        run.status.success(),
        "from-kicad refused the board: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(
        back.join("imported.cypcb").is_file(),
        "from-kicad promises the input file with a .cypcb suffix and wrote {:?}",
        std::fs::read_dir(&back)
            .map(|entries| entries.flatten().map(|e| e.file_name()).collect::<Vec<_>>())
            .unwrap_or_default()
    );

    // export: a folder named `output` beside the run, which is the default a
    // person meets on their first command. It is run in a directory of its
    // own, which is what the excuse this file used to carry was about.
    let exported = root.join("target/tmp-defaults-export");
    let _ = std::fs::remove_dir_all(&exported);
    std::fs::create_dir_all(&exported).expect("a working directory");
    std::fs::copy(
        root.join("examples/blink.cypcb"),
        exported.join("board.cypcb"),
    )
    .expect("the example copies");
    let run = run_in(&exported, &["export", "board.cypcb"]);
    assert!(
        run.status.success(),
        "export refused the board: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    let written = std::fs::read_dir(exported.join("output"))
        .map(|entries| entries.flatten().count())
        .unwrap_or(0);
    assert!(
        written > 0,
        "export promises ./output and the working directory holds {:?}",
        std::fs::read_dir(&exported)
            .map(|entries| entries.flatten().map(|e| e.file_name()).collect::<Vec<_>>())
            .unwrap_or_default()
    );

    // from-dxf: the drawing with a .cypcb suffix. A drawing rather than a
    // board, which is what kept this one on the excused list.
    let drawn = root.join("target/tmp-defaults-dxf");
    let _ = std::fs::remove_dir_all(&drawn);
    std::fs::create_dir_all(&drawn).expect("a working directory");
    rectangle_drawing(&drawn.join("case.dxf"));
    let run = run_in(&drawn, &["from-dxf", "case.dxf"]);
    assert!(
        run.status.success(),
        "from-dxf refused the drawing: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(
        drawn.join("case.cypcb").is_file(),
        "from-dxf promises the drawing with a .cypcb suffix and wrote {:?}",
        std::fs::read_dir(&drawn)
            .map(|entries| entries.flatten().map(|e| e.file_name()).collect::<Vec<_>>())
            .unwrap_or_default()
    );

    // library: the index named `cypcb-library.db` in the directory the command
    // is run from. The excuse this replaced said the command reads the
    // libraries installed on the machine - it reads an index, and makes one
    // where the help says when there is none.
    let indexed = root.join("target/tmp-defaults-library");
    let _ = std::fs::remove_dir_all(&indexed);
    std::fs::create_dir_all(&indexed).expect("a working directory");
    let run = run_in(&indexed, &["library", "list"]);
    assert!(
        run.status.success(),
        "library refused an empty index: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(
        indexed.join("cypcb-library.db").is_file(),
        "library promises cypcb-library.db in this directory and it holds {:?}",
        std::fs::read_dir(&indexed)
            .map(|entries| entries.flatten().map(|e| e.file_name()).collect::<Vec<_>>())
            .unwrap_or_default()
    );

    let _ = std::fs::remove_dir_all(&indexed);
    let _ = std::fs::remove_dir_all(&drawn);
    let _ = std::fs::remove_dir_all(&exported);
    let _ = std::fs::remove_dir_all(&work);
    let _ = std::fs::remove_dir_all(&back);
}
