//! What it costs to load a board, which the editor pays on every keystroke.
//!
//! `cargo test -p cypcb-render --features native --test loading_a_board_is_quick -- --nocapture`
//!
//! `load_source` parses, resolves imports, syncs the board model and runs the
//! whole design rule check. The viewer calls it 300ms after the last keystroke
//! and redraws from the snapshot, so this number is the delay between typing
//! and seeing - and a lot has been added to that path since anybody measured
//! it: import resolution, interface contracts, differential pairs, the outline
//! and the part number in the snapshot.
//!
//! Measured on the build machine, 2026-08-10, before and after the silkscreen
//! rule was given a broad phase:
//!
//! ```text
//!   parts   before    after
//!     100    23.9ms    1.2ms
//!     200   104.1ms    2.3ms
//!     400   447.2ms    4.9ms
//!     800  2026.3ms   11.1ms
//! ```
//!
//! The assertion is deliberately loose. This runs on a shared build machine,
//! and a test that fails when the machine is busy teaches people to ignore it.
//! What it catches is the day loading gets an order of magnitude slower - a
//! 500-part board is 5ms of work and the ceiling here is 200.

use std::time::Instant;

use cypcb_render::PcbEngine;

/// A board with `parts` components and a net joining every second pair.
fn heavy_board(parts: usize) -> String {
    let mut out =
        String::from("version 1\n\nboard stress {\n    size 200mm x 200mm\n    layers 2\n}\n\n");
    let per_row = 20;
    for i in 0..parts {
        let x = 5 + (i % per_row) * 9;
        let y = 5 + (i / per_row) * 9;
        out.push_str(&format!(
            "component R{i} resistor \"0402\" {{\n    value \"10k\"\n    at {x}mm, {y}mm\n}}\n\n"
        ));
    }
    for i in (0..parts.saturating_sub(1)).step_by(2) {
        out.push_str(&format!(
            "net N{i} {{\n    R{i}.2\n    R{}.1\n}}\n\n",
            i + 1
        ));
    }
    out
}

fn median(mut values: Vec<u128>) -> u128 {
    values.sort_unstable();
    values[values.len() / 2]
}

/// Load the same board five times and report the median.
fn time_load(source: &str) -> u128 {
    let mut engine = PcbEngine::new();
    // One warm-up: the first load of a process pays for lazily built tables.
    engine.load_source(source);

    let runs: Vec<u128> = (0..5)
        .map(|_| {
            let started = Instant::now();
            engine.load_source(source);
            started.elapsed().as_micros()
        })
        .collect();
    median(runs)
}

#[test]
fn a_board_the_size_of_a_real_design_loads_in_a_blink() {
    for parts in [50, 200, 500] {
        let source = heavy_board(parts);
        let micros = time_load(&source);
        println!(
            "[load] {parts} parts, {} bytes -> {:.1}ms",
            source.len(),
            micros as f64 / 1000.0
        );

        // Forty times the measured number, which survives a busy machine and
        // still catches a return to the quadratic behaviour: 500 parts took
        // 731ms before the fix and takes 5ms after it.
        assert!(
            micros < 200_000,
            "{parts} parts took {:.1}ms to load",
            micros as f64 / 1000.0
        );
    }
}

#[test]
fn every_example_loads_in_a_blink() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .join("examples");

    let mut slowest = (String::new(), 0u128);
    let mut checked = 0;
    for entry in std::fs::read_dir(&dir).expect("the examples are there") {
        let path = entry.expect("a directory entry").path();
        if path.extension().is_none_or(|ext| ext != "cypcb") {
            continue;
        }
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        // The two written to fail parsing say nothing about loading time.
        if name.contains("invalid") || name.contains("unknown_keyword") {
            continue;
        }

        let source = std::fs::read_to_string(&path).expect("the example is readable");
        let micros = time_load(&source);
        checked += 1;
        if micros > slowest.1 {
            slowest = (name, micros);
        }
    }

    assert!(checked > 10, "only {checked} examples were timed");
    println!(
        "[load] slowest of {checked} examples: {} at {:.1}ms",
        slowest.0,
        slowest.1 as f64 / 1000.0
    );
    assert!(
        slowest.1 < 200_000,
        "{} took {:.1}ms to load",
        slowest.0,
        slowest.1 as f64 / 1000.0
    );
}
