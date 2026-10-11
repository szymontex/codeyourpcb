//! The editor timings in `docs/api/lsp-server.md`, held to the boards and the
//! code they describe.
//!
//! `cargo test --release -p cypcb-render --features native --test the_editor_timings_are_measured -- --nocapture`
//!
//! The page used to give parse and DRC time as ranges by file length and part
//! count - under 10ms for a small file, up to 200ms for a large one, up to
//! 100ms of DRC for a complex board - and a 300ms debounce. None of it had a
//! source. Measured on 2026-09-27 the largest benchmark board, 905 lines and
//! 52 parts, loaded whole in under 5ms in a release build.
//!
//! So the page now carries a table of real boards, and this test keeps what in
//! it does not move with the machine: each board's lines and parts, and the
//! wait the editor takes before it loads. The milliseconds are a dated
//! measurement; this prints them again, in the table's own form, for whoever
//! wants to repeat it.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Instant;

use cypcb_fixtures::tree::{repo_root, tracked_in};
use cypcb_render::PcbEngine;

const RUNS: usize = 50;

fn read(relative: &str) -> String {
    std::fs::read_to_string(repo_root().join(relative))
        .unwrap_or_else(|e| panic!("{relative} does not read: {e}"))
}

/// The page's performance section, from its heading to the next one at the
/// same level.
fn performance_section() -> String {
    let page = read("docs/api/lsp-server.md");
    let start = page
        .find("## Performance Characteristics\n")
        .expect("the page has a performance section");
    let rest = &page[start + 3..];
    let end = rest.find("\n## ").map_or(page.len(), |at| start + 3 + at);
    page[start..end].to_owned()
}

/// A board row of the section's table: the path, the lines and the parts it
/// states.
struct Row {
    path: String,
    lines: usize,
    parts: usize,
}

fn board_rows(section: &str) -> Vec<Row> {
    section
        .lines()
        .filter(|line| line.starts_with("| `"))
        .map(|line| {
            let cells: Vec<&str> = line.split('|').map(str::trim).collect();
            let number = |at: usize| {
                cells[at]
                    .parse()
                    .unwrap_or_else(|_| panic!("cell {at} of `{line}` is not a count"))
            };
            Row {
                path: cells[1].trim_matches('`').to_owned(),
                lines: number(2),
                parts: number(3),
            }
        })
        .collect()
}

/// What `examples/lib` holds, keyed as an example imports it, the way the
/// editor hands it over with the board.
fn library_json() -> String {
    let files: BTreeMap<String, String> = tracked_in("examples/lib")
        .into_iter()
        .map(|path| {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            let text = std::fs::read_to_string(&path).expect("a library file reads");
            (format!("lib/{name}"), text)
        })
        .collect();
    serde_json::to_string(&files).expect("a map of strings serialises")
}

/// Median and max of `RUNS` calls after one warm-up, in milliseconds.
fn time(mut work: impl FnMut()) -> (f64, f64) {
    work();
    let mut runs: Vec<u128> = (0..RUNS)
        .map(|_| {
            let started = Instant::now();
            work();
            started.elapsed().as_micros()
        })
        .collect();
    runs.sort_unstable();
    (
        runs[RUNS / 2] as f64 / 1000.0,
        runs[RUNS - 1] as f64 / 1000.0,
    )
}

fn cell((median, max): (f64, f64)) -> String {
    format!("{median:.2} / {max:.2}")
}

#[test]
fn every_board_in_the_table_has_the_lines_and_parts_it_states() {
    let rows = board_rows(&performance_section());
    assert!(
        rows.len() >= 9,
        "the table lists {} boards; the section is not being read",
        rows.len()
    );
    // What the old ranges spoke of: a file past 500 lines and a board past 50
    // parts. The table has to reach both or it measures only the easy end.
    assert!(
        rows.iter().any(|row| row.lines > 500),
        "no board in the table is past 500 lines"
    );
    assert!(
        rows.iter().any(|row| row.parts > 50),
        "no board in the table is past 50 parts"
    );

    let library = library_json();
    let mut wrong = Vec::new();
    for row in &rows {
        let source = read(&row.path);
        let kicad = Path::new(&row.path)
            .extension()
            .is_some_and(|ext| ext == "kicad_pcb");
        let load = |engine: &mut PcbEngine| {
            if kicad {
                engine.load_kicad(&source)
            } else {
                engine.load_source_with_imports(&source, &library)
            }
        };

        let mut engine = PcbEngine::new();
        let said = load(&mut engine);
        assert!(said.is_empty(), "{} does not load: {said}", row.path);
        let snapshot: serde_json::Value =
            serde_json::from_str(&engine.get_snapshot()).expect("the snapshot is JSON");
        let parts = snapshot["components"].as_array().map_or(0, Vec::len);
        let lines = source.lines().count();
        if (lines, parts) != (row.lines, row.parts) {
            wrong.push(format!(
                "{}: the table says {} lines and {} parts, the board has {lines} and {parts}",
                row.path, row.lines, row.parts
            ));
        }

        let parse = time(|| {
            if kicad {
                let _ = cypcb_kicad::pcb_parser::parse_kicad_pcb_str(&source);
            } else {
                let _ = cypcb_parser::parse(&source);
            }
        });
        let drc = time(|| {
            engine.run_drc_incremental();
        });
        let mut fresh = PcbEngine::new();
        let whole = time(|| {
            load(&mut fresh);
        });
        println!(
            "| `{}` | {lines} | {parts} | {} | {} | {} |",
            row.path,
            cell(parse),
            cell(drc),
            cell(whole)
        );
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// The WASM engine reads alone since `parse_only`, so the table has a column
/// for it and no row leaves it empty. The browser spec measures the cells and
/// holds that the call answers; this holds that the page carries one for every
/// board, and no longer says the call does not exist.
#[test]
fn every_board_states_a_wasm_parse_time() {
    let section = performance_section();
    assert!(
        section.contains("| WASM Parse |"),
        "the table has no WASM parse column"
    );
    assert!(
        !section.contains("no call that parses alone"),
        "the page still says the WASM engine cannot read alone"
    );
    let mut empty = Vec::new();
    for line in section.lines().filter(|line| line.starts_with("| `")) {
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        let stated = cells.get(7).is_some_and(|cell| {
            let mut halves = cell.split(" / ");
            let number = |half: Option<&str>| {
                half.is_some_and(|h| h.parse::<f64>().is_ok_and(|v| v >= 0.0 && h.contains('.')))
            };
            number(halves.next()) && number(halves.next()) && halves.next().is_none()
        });
        // A cell of zeros is the placeholder, not a reading.
        let placeholder = cells.get(7).is_some_and(|cell| *cell == "0.00 / 0.00");
        if !stated || placeholder {
            empty.push(cells[1].to_owned());
        }
    }
    assert!(empty.is_empty(), "no WASM parse time for {empty:?}");
}

/// Every figure in milliseconds on `text`'s lines outside a table.
fn milliseconds_in_prose(text: &str) -> Vec<(usize, u64)> {
    let mut found = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if line.trim_start().starts_with('|') {
            continue;
        }
        let bytes = line.as_bytes();
        let mut at = 0;
        while let Some(offset) = line[at..].find("ms") {
            let unit = at + offset;
            at = unit + 2;
            let mut start = unit;
            while start > 0 && bytes[start - 1].is_ascii_digit() {
                start -= 1;
            }
            let followed = bytes
                .get(unit + 2)
                .is_some_and(|b| b.is_ascii_alphanumeric());
            if start < unit && !followed {
                found.push((index + 1, line[start..unit].parse().expect("digits parse")));
            }
        }
    }
    found
}

#[test]
fn the_wait_the_page_gives_is_the_one_the_editor_takes() {
    let editor = read("viewer/src/main.ts");
    let declared = editor
        .lines()
        .find_map(|line| line.trim().strip_prefix("const EDITOR_SYNC_DEBOUNCE_MS = "))
        .and_then(|rest| rest.trim_end_matches(';').parse::<u64>().ok())
        .expect("viewer/src/main.ts declares EDITOR_SYNC_DEBOUNCE_MS as a number");
    assert!(
        editor.contains("}, EDITOR_SYNC_DEBOUNCE_MS);"),
        "the editor's sync timer has to wait EDITOR_SYNC_DEBOUNCE_MS, or the constant describes nothing"
    );

    // Every millisecond figure the page states outside a table is this wait.
    // The measured ones sit in the tables, where they carry their date.
    let page = read("docs/api/lsp-server.md");
    let stated = milliseconds_in_prose(&page);
    assert!(
        stated.len() >= 3,
        "only {} figures in ms were found on the page",
        stated.len()
    );
    let other: Vec<String> = stated
        .iter()
        .filter(|(_, ms)| *ms != declared)
        .map(|(line, ms)| format!("docs/api/lsp-server.md:{line}: {ms}ms"))
        .collect();
    assert!(
        other.is_empty(),
        "the editor waits {declared}ms; these say otherwise:\n{}",
        other.join("\n")
    );
}

#[test]
fn a_figure_in_ms_is_found_in_prose_and_left_alone_in_a_table() {
    let text = "waits 300ms here\n| 12ms | 4ms |\nitems and 5 ms and msg\nand (250ms).\n";
    assert_eq!(milliseconds_in_prose(text), [(1, 300), (4, 250)]);
}
