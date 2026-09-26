//! A part is turned in one place.
//!
//! `cargo test -p cypcb-world --test one_place_turns_a_part`
//!
//! `rotate_about_origin` is the one function that turns a point by a part's
//! angle; `place_pad` and `place_box` put a pad or a box on the board through
//! it. A second copy of the trigonometry drifts: the viewer truncated where
//! the checker rounded, and the courtyard rule boxed its extent its own way.
//! This counts every `sin`/`cos` call in the crates' sources and holds each
//! file to the calls listed here, each with the reason it is not a part turn.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Every file allowed a `sin`/`cos` call, how many, and why.
const ALLOWED: &[(&str, usize, &str)] = &[
    (
        "cypcb-world/src/components/position.rs",
        1,
        "rotate_about_origin itself",
    ),
    (
        "cypcb-world/src/arc.rs",
        3,
        "points on a circle and the sagitta of a step, not a turn",
    ),
    (
        "cypcb-render/src/snapshot.rs",
        2,
        "points on an arc for the viewer",
    ),
    (
        "cypcb-export/src/gerber/silk.rs",
        2,
        "points on a silkscreen arc",
    ),
    (
        "cypcb-drc/src/rules/silk_clearance.rs",
        2,
        "points on a silkscreen arc",
    ),
    (
        "cypcb-export/src/pdf.rs",
        7,
        "arc handles, and a pad turned about its own centre in PDF points, which are never rounded to a nanometre",
    ),
    (
        "cypcb-export/src/dxf.rs",
        1,
        "a pad turned about its own centre from half-nanometre corners, which place_pad's whole-nanometre offsets cannot state",
    ),
    (
        "cypcb-autoroute/src/grid.rs",
        1,
        "cell centres turned back into a pad's own frame, and a span rounded up so the scan covers the pad",
    ),
    (
        "cypcb-world/src/teardrop.rs",
        1,
        "the angle where a teardrop meets a round land, not a part",
    ),
    (
        "cypcb-drc/src/rules/pad_entry.rs",
        2,
        "a test sweeping a trace's arm through angles",
    ),
    (
        "cypcb-autoroute/src/scoring.rs",
        1,
        "a test placing a trace beside a turned pad by hand, to check the placement",
    ),
];

fn crates_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn rust_files(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir)
        .expect("a source directory reads")
        .flatten()
    {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, found);
        } else if path.extension().is_some_and(|e| e == "rs") {
            found.push(path);
        }
    }
}

/// `sin`/`cos` calls per file under every crate's `src`.
fn trigonometry() -> BTreeMap<String, usize> {
    let root = crates_dir();
    let mut counts = BTreeMap::new();
    for krate in std::fs::read_dir(&root).expect("crates/ reads").flatten() {
        let src = krate.path().join("src");
        if !src.is_dir() {
            continue;
        }
        let mut files = Vec::new();
        rust_files(&src, &mut files);
        for file in files {
            let text = std::fs::read_to_string(&file).expect("a source file reads");
            let calls = [".sin()", ".cos()", ".sin_cos()"]
                .iter()
                .map(|call| text.matches(call).count())
                .sum::<usize>();
            if calls > 0 {
                let name = file
                    .strip_prefix(&root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                counts.insert(name, calls);
            }
        }
    }
    counts
}

#[test]
fn only_the_listed_files_call_sin_or_cos() {
    let found = trigonometry();
    let allowed: BTreeMap<String, usize> = ALLOWED
        .iter()
        .map(|(file, count, _)| (file.to_string(), *count))
        .collect();
    let unlisted: Vec<_> = found
        .iter()
        .filter(|(file, count)| allowed.get(*file) != Some(count))
        .map(|(file, count)| format!("{file}: {count} (listed: {:?})", allowed.get(file)))
        .collect();
    assert!(
        unlisted.is_empty(),
        "sin/cos outside rotate_about_origin; turn a part through place_pad, place_box or \
         rotate_about_origin, or list the file with its reason:\n{}",
        unlisted.join("\n")
    );
    let stale: Vec<_> = allowed
        .keys()
        .filter(|file| !found.contains_key(*file))
        .collect();
    assert!(
        stale.is_empty(),
        "listed files with no sin/cos left: {stale:?}"
    );
}
