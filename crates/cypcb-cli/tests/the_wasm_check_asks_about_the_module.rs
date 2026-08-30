//! The gate's question about `viewer/pkg` is asked about the module.
//!
//! `viewer/pkg` is committed, so the quality gate asks whether the module in
//! the tree is the one this source builds, and refuses to run the browser
//! suite against a stale one. That question was asked about all of
//! `crates/*/src` and all of `Cargo.lock` until 2026-08-31, when it sent the
//! nightly gate red over `81c6b71`: a change to `cypcb-cli` and
//! `cypcb-library`, two crates the wasm module does not link, plus the two
//! lines that change added to those same two packages' entries in the lock
//! file. A check that asks for a megabyte of rebuilt wasm to answer a change
//! that cannot reach it is a check people learn to skip.
//!
//! So both halves are held here: the crates it asks about are the module's own
//! dependency closure, and a lock file entry counts only when it belongs to a
//! package in that closure.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the workspace root is two directories above this crate")
}

fn script() -> PathBuf {
    repo_root().join("scripts/wasm-pkg-stale.sh")
}

fn run(args: &[&str]) -> String {
    let output = Command::new(script())
        .args(args)
        .output()
        .unwrap_or_else(|error| panic!("running {} failed: {error}", script().display()));
    assert!(
        output.status.success(),
        "{} {:?} exited {:?}: {}",
        script().display(),
        args,
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("the script prints paths and package names")
}

#[test]
fn the_inputs_are_the_crates_the_module_is_built_from() {
    let inputs: Vec<String> = run(&["--print-inputs"])
        .lines()
        .map(str::to_string)
        .collect();

    // Reachable from `cargo build -p cypcb-render --features wasm`: the render
    // crate itself and the board it draws.
    for reachable in ["crates/cypcb-render/src", "crates/cypcb-world/src"] {
        assert!(
            inputs.iter().any(|input| input == reachable),
            "{reachable} builds the module and is not among the inputs: {inputs:?}"
        );
    }

    // Not reachable from it. `cypcb-cli` is the binary and `cypcb-library` is
    // the parts database; the browser never loads either, and a change to
    // either used to demand a rebuilt module.
    for unreachable in ["crates/cypcb-cli/src", "crates/cypcb-library/src"] {
        assert!(
            !inputs.iter().any(|input| input == unreachable),
            "{unreachable} cannot reach the module and is among the inputs: {inputs:?}"
        );
    }

    // The script that runs the build is an input by itself: the same sources
    // built with different flags are a different module.
    assert!(
        inputs.iter().any(|input| input == "viewer/build-wasm.sh"),
        "the build script is not among the inputs: {inputs:?}"
    );
}

/// A `Cargo.lock` holding one entry per named package, enough to be parsed.
fn lock_with(entries: &[(&str, &str)]) -> String {
    let mut text = String::from("version = 4\n\n");
    for (name, version) in entries {
        text.push_str(&format!(
            "[[package]]\nname = \"{name}\"\nversion = \"{version}\"\n\n"
        ));
    }
    text
}

#[test]
fn a_lock_entry_counts_when_the_module_links_that_package() {
    let dir = std::env::temp_dir().join(format!("cypcb-lock-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a directory for two lock files");

    let before = dir.join("before.lock");
    std::fs::write(
        &before,
        lock_with(&[
            ("cypcb-world", "0.1.0-beta"),
            ("cypcb-cli", "0.1.0-beta"),
            ("lyon", "1.0.0"),
        ]),
    )
    .expect("the lock file before the change");

    // A version the module links: the committed module was built against the
    // old one, so it is stale and the rebuild is real work.
    let reachable = dir.join("reachable.lock");
    std::fs::write(
        &reachable,
        lock_with(&[
            ("cypcb-world", "0.2.0-beta"),
            ("cypcb-cli", "0.1.0-beta"),
            ("lyon", "1.0.0"),
        ]),
    )
    .expect("the lock file after a change the module links");

    // A version nothing in the module links. This is the shape of `81c6b71`.
    let unreachable = dir.join("unreachable.lock");
    std::fs::write(
        &unreachable,
        lock_with(&[
            ("cypcb-world", "0.1.0-beta"),
            ("cypcb-cli", "0.2.0-beta"),
            ("lyon", "1.0.0"),
        ]),
    )
    .expect("the lock file after a change the module cannot see");

    let changed = run(&[
        "--lock-packages",
        before.to_str().unwrap(),
        reachable.to_str().unwrap(),
    ]);
    assert_eq!(
        changed.split_whitespace().collect::<Vec<_>>(),
        ["cypcb-world"],
        "a package the module links moved and the check did not say so"
    );

    let changed = run(&[
        "--lock-packages",
        before.to_str().unwrap(),
        unreachable.to_str().unwrap(),
    ]);
    assert!(
        changed.trim().is_empty(),
        "only packages the module never links moved, and the check asked for a \
         rebuilt module anyway: {changed}"
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_gate_asks_the_script_rather_than_its_own_list() {
    let gate = std::fs::read_to_string(repo_root().join("scripts/quality-gate.sh"))
        .expect("the quality gate is a script in this repository");

    let ask = gate
        .find("./scripts/wasm-pkg-stale.sh")
        .expect("stage 7 no longer asks the script whether the committed module is current");
    // The list it used to carry. Two places answering the same question is how
    // one of them stays wrong.
    assert!(
        !gate.contains("'crates/*/src'"),
        "the gate computes its own input list again, beside the script's"
    );

    // Order is part of the answer. Half of what the script asks is whether
    // rebuilding this source changes the committed module, and it can only
    // read that from a rebuild that has already run.
    let rebuild = gate
        .find("./viewer/build-wasm.sh >/dev/null")
        .expect("stage 7 no longer rebuilds the module");
    assert!(
        rebuild < ask,
        "the gate asks whether the committed module is stale before rebuilding, \
         so the answer is the history's alone"
    );
}

#[test]
fn a_source_that_rebuilds_into_the_committed_module_is_not_stale() {
    // `315b227` rewrote a doc comment in `cypcb-parser`, which the module
    // links, and changed no byte of the module. The history said stale;
    // rebuilding reproduced the committed bytes exactly, so there was nothing
    // to commit and no run could ever have gone green. The check has to ask
    // the rebuild as well as the history, and this repository is the case: an
    // input has moved since `viewer/pkg` was last committed, and the module
    // built from it is the one that is committed.
    //
    // This case fails when the committed module really is stale, which is the
    // same thing the gate says and a true statement about the tree: rebuild
    // with `./viewer/build-wasm.sh` and commit `viewer/pkg`.
    let output = Command::new(script())
        .output()
        .unwrap_or_else(|error| panic!("running {} failed: {error}", script().display()));
    assert!(
        output.status.success(),
        "the check calls the committed module stale:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn the_module_does_not_carry_the_directory_it_was_built_in() {
    // The rebuild answer above is only worth something if a rebuild is the
    // same wherever the checkout lives. It was not: rustc records source paths
    // for panic messages, so this commit built in `/workspace/codeyourpcb` and
    // in the scheduled gate's worktree produced two different modules, and the
    // gate reads that difference as a stale artifact. Measured 2026-08-31 by
    // building both: without the remap the two files differ, with it both are
    // `b0e94102cef39dec22557fc78f717e3d`.
    let build = std::fs::read_to_string(repo_root().join("viewer/build-wasm.sh"))
        .expect("the wasm build is a script in this repository");

    assert!(
        build.contains("--remap-path-prefix="),
        "the build no longer remaps the checkout's path out of the module, so \
         the same commit built in two directories is two different modules"
    );
}

#[test]
fn a_rebuild_that_changes_the_module_is_said_before_the_commit_that_ships_it() {
    // The check asks about the committed tree, so the commit that moves an
    // input is graded by the next run - `315b227` shipped a module the gate
    // then called stale, and four runs said green before the nightly said
    // otherwise. Asking the working tree instead would red every uncommitted
    // change to a crate the module links, which is a second full gate run for
    // every one of them. The tree gets a notice instead, and the notice is
    // what was missing when `315b227` was committed.
    let verdict = |moved: &str, rebuilt: &str| {
        let output = Command::new(script())
            .args(["--verdict", moved, rebuilt])
            .output()
            .unwrap_or_else(|error| panic!("running {} failed: {error}", script().display()));
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    };

    // An input moved and rebuilding changes the committed module: a clone of
    // this branch serves an engine this source does not build.
    assert_eq!(
        verdict(
            "crates/cypcb-world/src/lib.rs",
            " M viewer/pkg/cypcb_render_bg.wasm"
        ),
        "stale"
    );
    // An input moved and rebuilding reproduces it: a doc comment, and nothing
    // to commit. This is the answer no history-only check could give.
    assert_eq!(verdict("crates/cypcb-parser/src/ast.rs", ""), "current");
    // Nothing committed has moved and the rebuild changes the module: the edit
    // is still in the tree, and `viewer/pkg` belongs in its commit.
    assert_eq!(verdict("", " M viewer/pkg/cypcb_render_bg.wasm"), "notice");
    // Neither.
    assert_eq!(verdict("", ""), "current");
}
