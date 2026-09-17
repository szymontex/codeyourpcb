//! An inner layer is the same bit everywhere it is turned into one.
//!
//! `Layer::to_copper_mask` is the definition: top is bit 0, bottom is bit 1,
//! and `Inner(n)` is bit `2 + n`. Four more places write that arithmetic out
//! by hand - the DRC's own `layer_bit`, the KiCad writer, the renderer and the
//! viewer - and `crates/cypcb-drc/src/rules/mod.rs` says why that matters in
//! its own prose: two copies of a layer numbering is how an off-by-one gets
//! fixed in one rule and left standing in the other.
//!
//! The offset is what this holds. A site that shifts by a different literal
//! puts a pad on a layer nobody asked for, and nothing else in the workspace
//! would notice.
//!
//! `cargo test -p cypcb-cli --test every_layer_bit_is_the_same_bit`

use std::path::{Path, PathBuf};

use cypcb_world::Layer;

/// Sites that turn a layer into a bit with a literal offset, today 5.
const SITES_FLOOR: usize = 5;

/// The bit the first inner layer takes, which every site must agree on.
const INNER_OFFSET: u32 = 2;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|crates| crates.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

fn source_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            source_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs" || e == "ts") {
            out.push(path);
        }
    }
}

/// The literal in a shift written as `1 << (2 + x)` or `1 << (x + 2)`.
///
/// `1 << layer` carries no literal and is not this pattern: it indexes by a
/// copper index that is already the bit number.
fn literal_offset(line: &str) -> Option<u32> {
    let after = line.split("1 << (").nth(1)?;
    let inside = after.split(')').next()?;
    let (left, right) = inside.split_once('+')?;
    for side in [left, right] {
        if let Ok(value) = side.trim().trim_start_matches('*').parse::<u32>() {
            return Some(value);
        }
    }
    None
}

#[test]
fn every_layer_bit_is_the_same_bit() {
    // The definition, called rather than read: the scan below compares
    // against this, so a scan of nothing would still have to face it.
    assert_eq!(Layer::TopCopper.to_copper_mask(), 1);
    assert_eq!(Layer::BottomCopper.to_copper_mask(), 2);
    assert_eq!(Layer::Inner(0).to_copper_mask(), 1 << INNER_OFFSET);
    assert_eq!(Layer::Inner(1).to_copper_mask(), 1 << (INNER_OFFSET + 1));

    let root = repo_root();
    let mut files = Vec::new();
    for entry in std::fs::read_dir(root.join("crates")).expect("the crates directory is there") {
        let src = entry.expect("a readable entry").path().join("src");
        if src.is_dir() {
            source_files(&src, &mut files);
        }
    }
    source_files(&root.join("viewer/src"), &mut files);
    files.sort();

    let mut sites = Vec::new();
    let mut disagreeing = Vec::new();

    for path in &files {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        for (index, line) in text.lines().enumerate() {
            let Some(offset) = literal_offset(line) else {
                continue;
            };
            let where_it_is = format!(
                "{}:{}",
                path.strip_prefix(&root).unwrap_or(path).display(),
                index + 1
            );
            sites.push(where_it_is.clone());
            if offset != INNER_OFFSET {
                disagreeing.push(format!("{where_it_is} shifts by {offset}: {}", line.trim()));
            }
        }
    }

    println!(
        "sites turning a layer into a bit with a literal offset: {} (floor {SITES_FLOOR}); \
         shifting by something other than {INNER_OFFSET}: {}",
        sites.len(),
        disagreeing.len()
    );

    assert!(
        sites.len() >= SITES_FLOOR,
        "the walk found {} of these and the workspace had {SITES_FLOOR} when this was \
         written - a scan that stops finding them passes for the wrong reason.\n  Found: {}",
        sites.len(),
        sites.join("\n  ")
    );
    assert!(
        disagreeing.is_empty(),
        "a layer is turned into a different bit here than `to_copper_mask` gives it:\n  {}\
         \n  The mask travels between crates and into the viewer, so one site \
         disagreeing puts copper on a layer nobody asked for.",
        disagreeing.join("\n  ")
    );
}
