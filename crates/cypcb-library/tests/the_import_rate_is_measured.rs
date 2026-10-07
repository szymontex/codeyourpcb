//! The import rate `docs/api/library-format.md` gives for the desktop.
//!
//! `cargo test --release -p cypcb-library --test the_import_rate_is_measured -- --nocapture`
//!
//! The page gave "~10k components/sec" with nothing behind it. The repository
//! holds ten KiCad footprints and no library of the size that figure speaks
//! of, so this builds one: every tracked `.kicad_mod` under `tests/fixtures`,
//! copied under a new footprint name until there are `FOOTPRINTS` of them,
//! spread over `LIBRARIES` `.pretty` folders. The index is a SQLite file on
//! disk, as on the desktop, and the folder goes in through
//! `LibraryManager::import_folder`, the call the application makes.
//!
//! The rate is a measurement with a date on the page, so nothing here holds
//! it. What is held is that every generated footprint reaches the index and
//! none is refused, so the rate printed is the rate of a whole import.

use std::time::Instant;

use cypcb_fixtures::scratch_dir;
use cypcb_fixtures::tree::tracked_under;
use cypcb_library::LibraryManager;

const FOOTPRINTS: usize = 10_000;
const LIBRARIES: usize = 10;
const RUNS: usize = 5;

/// `text` with `suffix` added to its footprint name: the first word after
/// `(footprint ` or `(module `, quoted or bare.
fn renamed(text: &str, suffix: &str) -> String {
    for head in ["(footprint ", "(module "] {
        if let Some(at) = text.find(head) {
            let start = at + head.len();
            let quoted = text[start..].starts_with('"');
            let name_start = if quoted { start + 1 } else { start };
            let end = text[name_start..]
                .find(|c: char| {
                    if quoted {
                        c == '"'
                    } else {
                        c.is_whitespace() || c == ')'
                    }
                })
                .map(|len| name_start + len)
                .expect("the footprint name ends");
            return format!("{}{suffix}{}", &text[..end], &text[end..]);
        }
    }
    panic!("no footprint name in a fixture");
}

#[test]
fn a_footprint_is_renamed_quoted_or_bare() {
    assert_eq!(
        renamed("(footprint \"R_0603\" (layer", "_g1"),
        "(footprint \"R_0603_g1\" (layer"
    );
    assert_eq!(
        renamed("(module 1X04 (layer F.Cu)", "_g2"),
        "(module 1X04_g2 (layer F.Cu)"
    );
}

#[test]
fn every_generated_footprint_is_imported_and_the_rate_is_printed() {
    let fixtures: Vec<String> = tracked_under("tests/fixtures")
        .into_iter()
        .filter(|path| path.extension().is_some_and(|ext| ext == "kicad_mod"))
        .map(|path| std::fs::read_to_string(&path).expect("a fixture footprint reads"))
        .collect();
    assert!(
        fixtures.len() >= 10,
        "{} fixture footprints; the fixtures are not being read",
        fixtures.len()
    );

    let dir = scratch_dir("cypcb-import-rate");
    let root = dir.join("footprints");
    for n in 0..FOOTPRINTS {
        let library = root.join(format!("Generated_{:02}.pretty", n % LIBRARIES));
        std::fs::create_dir_all(&library).expect("a library folder");
        let text = renamed(&fixtures[n % fixtures.len()], &format!("_g{n:05}"));
        std::fs::write(library.join(format!("F{n:05}.kicad_mod")), text).expect("a footprint");
    }

    let mut taken = Vec::new();
    for run in 0..RUNS {
        let db = dir.join(format!("index-{run}.db"));
        let mut manager = LibraryManager::new(&db).expect("the index opens");
        manager.add_kicad_search_path(root.clone());
        let started = Instant::now();
        let imported = manager.import_folder(&root).expect("the folder imports");
        let elapsed = started.elapsed().as_secs_f64();
        assert_eq!(
            imported.imported.len(),
            LIBRARIES,
            "every library is imported"
        );
        let rejected: usize = imported
            .imported
            .iter()
            .map(|(_, outcome)| outcome.rejected.len())
            .sum();
        assert_eq!(rejected, 0, "no generated footprint is refused");
        assert_eq!(
            manager.component_count().expect("the index counts"),
            FOOTPRINTS
        );
        taken.push(elapsed);
    }
    taken.sort_by(f64::total_cmp);
    let rate = |seconds: f64| FOOTPRINTS as f64 / seconds;
    println!(
        "{FOOTPRINTS} footprints in {LIBRARIES} libraries, {RUNS} runs: median {:.2} s ({:.0} per second), slowest {:.2} s ({:.0} per second)",
        taken[RUNS / 2],
        rate(taken[RUNS / 2]),
        taken[RUNS - 1],
        rate(taken[RUNS - 1])
    );
}

#[test]
fn the_page_names_the_library_this_test_imports() {
    let page = std::fs::read_to_string(
        cypcb_fixtures::tree::repo_root().join("docs/api/library-format.md"),
    )
    .expect("the page reads");
    let line = page
        .lines()
        .find(|line| line.contains("the_import_rate_is_measured"))
        .expect("the page gives the command that measures the rate");
    let thousands = format!("{},{:03}", FOOTPRINTS / 1000, FOOTPRINTS % 1000);
    for said in [
        format!("{thousands} footprints in {LIBRARIES} `.pretty` folders"),
        format!("over {RUNS} runs"),
    ] {
        assert!(
            line.contains(&said),
            "the page does not say `{said}`: {line}"
        );
    }
}
