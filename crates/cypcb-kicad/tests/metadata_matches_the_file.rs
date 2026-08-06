//! What `cypcb parse-kicad` reports has to be what the file contains.
//!
//! The command prints a component count, a net count, segments, vias and the
//! board size. Nothing had ever compared those numbers to the file they claim
//! to describe, and they are the numbers a person uses to decide whether an
//! import worked at all.
//!
//! Counted here from the raw S-expression text with a depth-aware scan, which
//! shares no code with the parser under test - a count taken with the same
//! reader would agree with it whatever both did.

use std::path::Path;

use cypcb_kicad::{parse_kicad_pcb, BENCHMARKS};

/// Count top-level lists whose head is `head`.
///
/// Depth 1 is a direct child of the outermost `(kicad_pcb ...)`, which is
/// where footprints, nets, segments and vias live. A `(net 3 "GND")` inside a
/// pad sits deeper and is not a declaration.
fn count_at_top_level(source: &str, head: &str) -> usize {
    let bytes = source.as_bytes();
    let mut depth = 0usize;
    let mut found = 0usize;
    let mut in_string = false;
    let mut i = 0usize;

    while i < bytes.len() {
        match bytes[i] {
            b'"' if !in_string => in_string = true,
            b'"' if in_string => in_string = false,
            b'(' if !in_string => {
                depth += 1;
                if depth == 2 {
                    let rest = &source[i + 1..];
                    let token: String = rest
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect();
                    if token == head {
                        found += 1;
                    }
                }
            }
            b')' if !in_string => depth = depth.saturating_sub(1),
            _ => {}
        }
        i += 1;
    }

    found
}

#[test]
fn the_reported_counts_are_the_ones_in_the_file() {
    for benchmark in BENCHMARKS {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("tests/fixtures/benchmark")
            .join(benchmark.filename);
        let source = std::fs::read_to_string(&path).expect("the fixture is in the repo");

        let parsed = parse_kicad_pcb(&path)
            .unwrap_or_else(|e| panic!("{} has to parse: {e:?}", benchmark.filename));
        let meta = parsed.metadata;

        let footprints = count_at_top_level(&source, "footprint");
        let segments = count_at_top_level(&source, "segment");
        let vias = count_at_top_level(&source, "via");
        let nets = count_at_top_level(&source, "net");

        assert_eq!(
            meta.component_count as usize, footprints,
            "{}: reported {} components against {footprints} footprints in the file",
            benchmark.filename, meta.component_count
        );
        assert_eq!(
            meta.trace_segment_count as usize, segments,
            "{}: reported {} segments against {segments} in the file",
            benchmark.filename, meta.trace_segment_count
        );
        assert_eq!(
            meta.via_count as usize, vias,
            "{}: reported {} vias against {vias} in the file",
            benchmark.filename, meta.via_count
        );

        // KiCad declares `(net 0 "")` for copper that belongs to nothing. It
        // is a placeholder rather than a net, so the count is one lower than
        // the file's declarations - and that is worth pinning, because a
        // reader who counts by hand will find the difference and wonder which
        // of the two is wrong.
        assert_eq!(
            meta.net_count as usize,
            nets - 1,
            "{}: reported {} nets against {nets} declarations, of which one is KiCad's empty net 0",
            benchmark.filename,
            meta.net_count
        );

        // The board is as big as the outline the file draws, and a zero-sized
        // board means the importer found no edge at all.
        assert!(
            meta.board_size_mm.0 > 0.0 && meta.board_size_mm.1 > 0.0,
            "{}: reported a {}x{}mm board",
            benchmark.filename,
            meta.board_size_mm.0,
            meta.board_size_mm.1
        );
    }
}
