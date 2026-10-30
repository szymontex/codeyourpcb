//! Copper on no net is never "the same net" as anything.
//!
//! `cargo test -p cypcb-world --test copper_on_no_net_is_never_the_same_net`
//!
//! A `Trace` or a `Via` holds `net_id: Option<NetId>`, and `None` means the
//! copper is on no net: KiCad's net 0, which a board holds for a track nobody
//! connected. It is foreign to every net and to other copper on no net. `==`
//! on two `Option<NetId>` says the opposite for a pair of `None`, and it
//! compiles, so a comparison written the old way is a bug the compiler does
//! not see. Two pieces of copper on no net would be one conductor, and a
//! pour on no net would call every trace on no net its own.
//!
//! `cypcb_world::same_net` is the one comparison: false when either side is
//! `None`. This reads the source of every crate that handles copper and
//! finds each line that compares a `.net_id` with `==` or `!=` against
//! something that is not plainly a net. Each file that has any is on the table
//! below with how many and why they are safe, and the table has to match the
//! source both ways - a new comparison fails, and a table row that no longer
//! matches fails too.
//!
//! The same command finds them by hand:
//! `grep -rnE "net_id *(==|!=)|(==|!=) *[a-z_.]*net_id" crates/*/src`.
//!
//! The second test is the other way to lose the rule: turning `None` into a
//! net with `unwrap_or_default()`, which is `NetId(0)`, the first net on the
//! board. That is how copper on no net came to join the first net.

use std::collections::BTreeMap;

/// The crates that read or write copper.
const CRATES: &[&str] = &[
    "cypcb-world",
    "cypcb-drc",
    "cypcb-router",
    "cypcb-autoroute",
    "cypcb-export",
    "cypcb-render",
    "cypcb-kicad",
    "cypcb-cli",
];

/// Files with a `.net_id` comparison that is not `same_net`, how many lines,
/// and why none of them compares two `Option<NetId>`.
const COMPARES_PLAIN_NETS: &[(&str, usize, &str)] = &[
    (
        "crates/cypcb-autoroute/src/via_optimizer.rs",
        5,
        "RouteSegment and ViaPlacement carry a NetId, not an Option, and the net routed is a \
         NetId. A drawn trace on no net enters as NO_NET, which no routed net equals",
    ),
    (
        "crates/cypcb-autoroute/src/pathfinder_v2.rs",
        1,
        "`NetCopper::net` against the net to route: both are a NetId",
    ),
];

/// A line of code with its comments cut and its test module gone.
fn code_of(src: &str) -> Vec<(usize, String)> {
    let cut = src.find("#[cfg(test)]\nmod ").unwrap_or(src.len());
    src[..cut]
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim_start().starts_with("//"))
        .map(|(i, line)| (i + 1, line.to_string()))
        .collect()
}

fn sources() -> Vec<(String, String)> {
    let root = cypcb_fixtures::tree::repo_root();
    cypcb_fixtures::tree::files_git_tracks(&root)
        .into_iter()
        .filter(|f| {
            f.ends_with(".rs")
                && f.contains("/src/")
                && CRATES
                    .iter()
                    .any(|c| f.starts_with(&format!("crates/{c}/")))
        })
        .map(|f| {
            let text = std::fs::read_to_string(root.join(&f)).expect("a tracked source reads");
            (f, text)
        })
        .collect()
}

/// Whether one side of a comparison is plainly a net and not an `Option`.
fn is_plain(side: &str) -> bool {
    let side = side.trim().trim_start_matches('&');
    side.starts_with("Some(") || side.starts_with("NetId") || side.starts_with('*')
}

/// The comparisons on the line that have a `.net_id` on a side and not a
/// plain net on the other.
fn bad_comparisons(line: &str) -> usize {
    let mut found = 0;
    for op in ["==", "!="] {
        for (at, _) in line.match_indices(op) {
            // `<=`, `>=`, `=>` and `===` are not this operator.
            let before = line[..at].chars().next_back();
            let after = line[at + 2..].chars().next();
            if matches!(before, Some('<' | '>' | '=' | '!')) || after == Some('=') {
                continue;
            }
            let left = &line[..at];
            let right = &line[at + 2..];
            let left_net = left.trim_end().ends_with(".net_id");
            let right_head: String = right
                .trim_start()
                .chars()
                .take_while(|c| c.is_alphanumeric() || matches!(c, '_' | '.' | '*' | '&'))
                .collect();
            let right_net = right_head.ends_with(".net_id");
            if (left_net && !is_plain(right.trim_start()))
                || (right_net && !is_plain(left_tail(left)))
            {
                found += 1;
            }
        }
    }
    found
}

/// The expression just left of an operator: up to the nearest `(`, `,`,
/// `&&`, `||` or `{`.
fn left_tail(left: &str) -> &str {
    let cut = left.rfind(['(', ',', '{']).map_or(0, |i| i + 1);
    let cut = cut.max(left.rfind("&&").map_or(0, |i| i + 2));
    let cut = cut.max(left.rfind("||").map_or(0, |i| i + 2));
    left[cut..].trim()
}

#[test]
fn no_net_id_is_compared_with_equals_except_where_it_is_a_plain_net() {
    let mut found: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (file, text) in sources() {
        for (number, line) in code_of(&text) {
            for _ in 0..bad_comparisons(&line) {
                found.entry(file.clone()).or_default().push(number);
            }
        }
    }

    let mut wrong = Vec::new();
    for (file, lines) in &found {
        match COMPARES_PLAIN_NETS.iter().find(|(f, _, _)| f == file) {
            None => wrong.push(format!(
                "{file}: compares a `.net_id` with `==` or `!=` at line(s) {lines:?}. \
                 If both sides can be `None`, use `cypcb_world::same_net`; if the other side \
                 is a plain `NetId`, put the file on the table with a reason"
            )),
            Some((_, count, _)) if *count != lines.len() => wrong.push(format!(
                "{file}: the table says {count} comparison(s), the source has {} at {lines:?}",
                lines.len()
            )),
            Some(_) => {}
        }
    }
    for (file, count, _) in COMPARES_PLAIN_NETS {
        if !found.contains_key(*file) {
            wrong.push(format!(
                "{file}: on the table with {count} comparison(s), and the source has none"
            ));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn a_missing_net_is_never_turned_into_the_first_net() {
    let mut wrong = Vec::new();
    for (file, text) in sources() {
        // Without the line breaks and the indentation rustfmt puts between
        // `.net_id` and the call that follows it.
        let code: String = code_of(&text)
            .into_iter()
            .flat_map(|(_, line)| {
                line.chars()
                    .filter(|c| !c.is_whitespace())
                    .collect::<Vec<_>>()
            })
            .collect();
        if code.contains(".net_id.unwrap_or_default") {
            wrong.push(format!(
                "{file}: `unwrap_or_default` on a net is `NetId(0)`, the first net"
            ));
        }
        let puts_on_a_net = code.matches(".net_id.unwrap_or(").count()
            - code.matches(".net_id.unwrap_or(NO_NET)").count();
        if puts_on_a_net > 0 {
            wrong.push(format!(
                "{file}: `unwrap_or(..)` on a net puts copper on no net onto a net"
            ));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn the_comparison_this_reads_for_is_the_one_it_finds() {
    // The reader is as good as its eye. These are the lines it must flag...
    for flagged in [
        "if trace.net_id == pour_net {",
        "if a.net_id == b.net_id {",
        "if candidate.net_id != trace.net_id {",
        "x == via.net_id",
    ] {
        assert_eq!(bad_comparisons(flagged), 1, "{flagged}");
    }
    // ...and these it must let through: an `Option` against a `Some`, a plain
    // net, and everything that is not a comparison of a net.
    for passed in [
        "if trace.net_id == Some(net) {",
        "if trace.net_id != Some(positive_id) {",
        "s.net_id == *net_id",
        "s.net_id == NetId::new(1)",
        "if trace.layer != layer || same_net(trace.net_id, pour_net) {",
        "let x = trace.net_id;",
        "if a <= trace.net_id.map_or(0, |n| n.0) {",
    ] {
        assert_eq!(bad_comparisons(passed), 0, "{passed}");
    }
}
