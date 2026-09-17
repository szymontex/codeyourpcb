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
const NOT_RUN: &[(&str, &str)] = &[
    (
        "from_dxf.rs",
        "needs a drawing rather than a board, and the DXF fixtures live with \
         the importer's own cases",
    ),
    (
        "library.rs",
        "indexes the footprint libraries installed on the machine, which a \
         check cannot assume are there",
    ),
    (
        "route.rs",
        "is the routing timeout, a number rather than a file",
    ),
];

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

    let _ = std::fs::remove_dir_all(&exported);
    let _ = std::fs::remove_dir_all(&work);
    let _ = std::fs::remove_dir_all(&back);
}
