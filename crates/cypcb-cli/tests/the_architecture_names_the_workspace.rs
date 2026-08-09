//! `docs/architecture.md` describes the crates this workspace has.
//!
//! `cargo test -p cypcb-cli --test the_architecture_names_the_workspace`
//!
//! The document once carried a `**Size**: ~N lines` line per crate and every
//! one had drifted, and it was missing `cypcb-autoroute` and `cypcb-rules`
//! entirely - the router this project has spent most of its measurement on and
//! the preset table every command resolves `--preset` through. That was fixed
//! by reading it against the tree once, by hand, which is a fix with a shelf
//! life.
//!
//! Two things are checked here, and both are the kind a person forgets: that
//! every crate has a section, and that every dependency the document claims
//! between crates is really in that crate's `Cargo.toml`. Versions of outside
//! crates are not checked - the document names them for orientation, and
//! pinning `tree-sitter` here would only be a second place to update.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

fn architecture() -> String {
    std::fs::read_to_string(repo_root().join("docs/architecture.md"))
        .expect("the architecture document is there")
}

/// Every crate directory in the workspace.
fn crates() -> BTreeSet<String> {
    std::fs::read_dir(repo_root().join("crates"))
        .expect("the crates directory is there")
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            if !path.join("Cargo.toml").is_file() {
                return None;
            }
            Some(path.file_name()?.to_string_lossy().to_string())
        })
        .collect()
}

#[test]
fn every_crate_has_a_section() {
    let doc = architecture();
    let missing: Vec<String> = crates()
        .into_iter()
        .filter(|name| !doc.contains(&format!("### {name}")))
        .collect();

    assert!(
        missing.is_empty(),
        "docs/architecture.md has no section for: {missing:?}"
    );
}

#[test]
fn no_section_describes_a_crate_that_is_not_there() {
    let doc = architecture();
    let real = crates();

    let described: Vec<String> = doc
        .lines()
        .filter_map(|line| line.strip_prefix("### "))
        .filter(|name| name.starts_with("cypcb-"))
        .map(str::to_string)
        .collect();

    let ghosts: Vec<&String> = described
        .iter()
        .filter(|name| !real.contains(*name))
        .collect();
    assert!(
        ghosts.is_empty(),
        "docs/architecture.md describes crates the workspace does not have: {ghosts:?}"
    );
}

#[test]
fn every_dependency_the_document_claims_is_real() {
    // Only dependencies between this workspace's own crates: those are facts a
    // `Cargo.toml` states plainly, and they are what a reader uses the diagram
    // for.
    let doc = architecture();
    let real = crates();
    let mut wrong: Vec<String> = Vec::new();

    let mut current: Option<String> = None;
    for line in doc.lines() {
        if let Some(name) = line.strip_prefix("### ") {
            current = real.contains(name).then(|| name.to_string());
            continue;
        }

        let Some(crate_name) = current.as_deref() else {
            continue;
        };
        let Some(claims) = line.strip_prefix("**Dependencies**: ") else {
            continue;
        };

        let manifest = std::fs::read_to_string(
            repo_root()
                .join("crates")
                .join(crate_name)
                .join("Cargo.toml"),
        )
        .expect("every crate has a manifest");

        for claimed in claims
            .split(',')
            .map(|part| part.trim().trim_matches('`').trim())
            .filter(|part| real.contains(*part))
        {
            if !manifest.contains(claimed) {
                wrong.push(format!("{crate_name} does not depend on {claimed}"));
            }
        }
    }

    assert!(
        !wrong.is_empty() || doc.contains("**Dependencies**:"),
        "no dependency lines were read, so this test proves nothing"
    );
    assert!(
        wrong.is_empty(),
        "docs/architecture.md is wrong:\n  {}",
        wrong.join("\n  ")
    );
}
