//! Every reader of a zone has been asked what it does with one that is not a
//! rectangle.
//!
//! `cargo test -p cypcb-world --test every_reader_of_a_zone_is_accounted_for`
//!
//! A zone with a `ZoneOutline` has `bounds` too: the box round its corners. A
//! reader that takes the box is the bounding box of an L-shaped pour, which is
//! copper where the designer cut it away - the reason the importer refused such
//! pours for as long as it did. The model keeps the two apart on purpose:
//! `BoardWorld::zones` hands out the rectangles only, `outlined_zones` and
//! `all_zones` hand out the others, and a reader that has not been taught the
//! outline does not see it.
//!
//! That is only as good as the list of readers. This reads the source of every
//! crate, the way a person would grep it, and finds each file that reads zones:
//! calls `.zones()`, takes `&Zone`, or queries the ECS for one. Each is on the
//! table below with what it does, and the table has to match the source both
//! ways - a new reader that is not on it fails, and an entry whose file no
//! longer reads zones fails too.
//!
//! The same command finds them by hand:
//! `grep -rln "\.zones()\|&Zone\b\|query::<.*Zone" crates/*/src`.

use std::collections::BTreeSet;

/// What a file does with a zone that has an outline.
enum Reads {
    /// Asks for the outline: calls `outlined_zones` or `all_zones`.
    Outline,
    /// Reads `zones()`, so does not see an outlined zone, and has not been
    /// taught to. The text says what that costs and which kinds it reads.
    Rectangles(&'static str),
    /// Reads the ECS directly, and says why that is safe: what an outlined zone
    /// is not, or never has. The first text is a line of code the file has to
    /// contain for the reason to be true of it - the filter, or the component
    /// it requires - and the test looks for it there.
    Directly(&'static str, &'static str),
}

const READERS: &[(&str, Reads)] = &[
    // The readers that act on an outline.
    ("crates/cypcb-autoroute/src/clearance_field.rs", Reads::Outline),
    ("crates/cypcb-autoroute/src/grid.rs", Reads::Outline),
    ("crates/cypcb-autoroute/src/via_optimizer.rs", Reads::Outline),
    ("crates/cypcb-drc/src/rules/mod.rs", Reads::Outline),
    ("crates/cypcb-kicad/src/board_writer.rs", Reads::Outline),
    ("crates/cypcb-render/src/lib.rs", Reads::Outline),
    ("crates/cypcb-world/src/dsl.rs", Reads::Outline),
    ("crates/cypcb-world/src/world.rs", Reads::Outline),
    ("crates/cypcb-world/src/copper.rs", Reads::Outline),
    ("crates/cypcb-export/src/gerber/copper.rs", Reads::Outline),
    ("crates/cypcb-export/src/ipc2581.rs", Reads::Outline),
    ("crates/cypcb-drc/src/rules/pour_island.rs", Reads::Outline),
    ("crates/cypcb-drc/src/rules/net_split.rs", Reads::Outline),
    ("crates/cypcb-drc/src/rules/unrouted_pin.rs", Reads::Outline),
    ("crates/cypcb-autoroute/src/orchestrator.rs", Reads::Outline),
    ("crates/cypcb-drc/src/rules/edge_clearance.rs", Reads::Outline),
    ("crates/cypcb-drc/src/rules/zone_overlap.rs", Reads::Outline),
    ("crates/cypcb-drc/src/rules/mounting_hole_clearance.rs", Reads::Outline),
    // Rectangles only. Each of these is the same as before the model could hold
    // an outline: the zone is not there.
    (
        "crates/cypcb-cli/src/commands/parse.rs",
        Reads::Rectangles("the model of a .cypcb design, which has no polygon"),
    ),
    (
        "crates/cypcb-cli/src/commands/to_kicad.rs",
        Reads::Rectangles("flex regions and named areas, which never have an outline"),
    ),
    (
        "crates/cypcb-drc/src/rules/area_off_board.rs",
        Reads::Rectangles("areas the stack points at: flex and named areas, which never have an outline"),
    ),
    (
        "crates/cypcb-drc/src/rules/area_overlap.rs",
        Reads::Rectangles("areas the stack points at: flex and named areas, which never have an outline"),
    ),
    (
        "crates/cypcb-drc/src/rules/flex_trace_angle.rs",
        Reads::Rectangles("flex regions, which never have an outline"),
    ),
    (
        "crates/cypcb-drc/src/rules/solid_pour_in_bend.rs",
        Reads::Rectangles("flex regions, which never have an outline"),
    ),
    // The ECS, read directly.
    (
        "crates/cypcb-drc/src/rules/bend_radius.rs",
        Reads::Directly(
            "is_flex()",
            "filters flex regions, and only a keepout or a pour has an outline",
        ),
    ),
    (
        "crates/cypcb-drc/src/rules/empty_area.rs",
        Reads::Directly(
            "width > 0 && height > 0",
            "asks for a zone with no width or no height; an outline encloses an area",
        ),
    ),
    (
        "crates/cypcb-drc/src/rules/flex_hole.rs",
        Reads::Directly(
            "is_flex()",
            "filters flex regions, and only a keepout or a pour has an outline",
        ),
    ),
    (
        "crates/cypcb-drc/src/rules/hatch_etchable.rs",
        Reads::Directly(
            "&Hatch",
            "needs a `Hatch`, which the importer never gives an outlined zone",
        ),
    ),
    (
        "crates/cypcb-world/src/stitch.rs",
        Reads::Directly(
            "zone: &Zone",
            "takes the zone it is handed, and what hands it one is a stitched `zones()` row",
        ),
    ),
    (
        "crates/cypcb-world/src/sync.rs",
        Reads::Directly(
            "StitchPitch",
            "stitches pours that state a `StitchPitch`, which the importer never gives an outlined zone",
        ),
    ),
];

/// The source of a file without its comments and without its test module.
fn code_of(src: &str) -> String {
    // The test module, not a `#[cfg(test)]` on a single `use` above the code.
    let cut = src.find("#[cfg(test)]\nmod ").unwrap_or(src.len());
    src[..cut]
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn is_word(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Whether `word` appears in `text` as a whole word.
fn has_word(text: &str, word: &str) -> bool {
    let b = text.as_bytes();
    text.match_indices(word).any(|(at, _)| {
        let before = at == 0 || !is_word(b[at - 1]);
        let after = at + word.len() >= b.len() || !is_word(b[at + word.len()]);
        before && after
    })
}

/// The generic argument of every `name::<..>` in the text.
fn turbofish<'a>(text: &'a str, name: &str) -> Vec<&'a str> {
    let open = format!("{name}::<");
    let mut found = Vec::new();
    for (at, _) in text.match_indices(&open) {
        let start = at + open.len();
        let mut depth = 1;
        for (i, c) in text[start..].char_indices() {
            match c {
                '<' => depth += 1,
                '>' => {
                    depth -= 1;
                    if depth == 0 {
                        found.push(&text[start..start + i]);
                        break;
                    }
                }
                _ => {}
            }
        }
    }
    found
}

/// Whether this source reads zones: `.zones()`, a `&Zone` it is given, or an
/// ECS query that names `Zone`.
fn reads_zones(src: &str) -> bool {
    let code = code_of(src);
    code.contains(".zones()")
        || code.contains("pours_of(")
        || code.contains("&Zone ")
        || code.contains("&Zone,")
        || code.contains("&Zone>")
        || code.contains("&Zone)")
        || code.contains("&mut Zone")
        || ["query", "in_build_order"].iter().any(|name| {
            turbofish(&code, name)
                .iter()
                .any(|arg| has_word(arg, "Zone"))
        })
}

/// Whether this source asks for the outline: through the world's two calls, or
/// with a query that names `ZoneOutline`.
fn asks_for_the_outline(src: &str) -> bool {
    let code = code_of(src);
    code.contains("outlined_zones(")
        || code.contains("all_zones(")
        || code.contains("pours_of(")
        || code.contains("outlined_pours(")
        || ["query", "in_build_order"].iter().any(|name| {
            turbofish(&code, name)
                .iter()
                .any(|arg| has_word(arg, "ZoneOutline"))
        })
}

fn sources() -> Vec<(String, String)> {
    let root = cypcb_fixtures::tree::repo_root();
    cypcb_fixtures::tree::files_git_tracks(&root)
        .into_iter()
        .filter(|f| f.starts_with("crates/") && f.contains("/src/") && f.ends_with(".rs"))
        .map(|f| {
            let text = std::fs::read_to_string(root.join(&f)).expect("a tracked source reads");
            (f, text)
        })
        .collect()
}

#[test]
fn every_file_that_reads_zones_is_on_the_table() {
    let found: BTreeSet<String> = sources()
        .into_iter()
        .filter(|(_, text)| reads_zones(text))
        .map(|(file, _)| file)
        .collect();
    let listed: BTreeSet<String> = READERS.iter().map(|(f, _)| f.to_string()).collect();

    let unlisted: Vec<_> = found.difference(&listed).collect();
    assert!(
        unlisted.is_empty(),
        "these files read zones and are not on the table; say what each does with a zone \
         that has an outline: {unlisted:?}"
    );
    let stale: Vec<_> = listed.difference(&found).collect();
    assert!(
        stale.is_empty(),
        "these files are on the table and no longer read zones: {stale:?}"
    );
    assert!(
        found.len() >= 20,
        "the walk found {} readers; it has stopped seeing them",
        found.len()
    );
}

#[test]
fn a_file_that_acts_on_the_outline_asks_for_it() {
    let all = sources();
    let mut checked = 0;
    for (file, how) in READERS {
        let text = &all
            .iter()
            .find(|(f, _)| f == file)
            .expect("the file is tracked")
            .1;
        match how {
            Reads::Outline => {
                checked += 1;
                assert!(
                    asks_for_the_outline(text),
                    "{file} is listed as acting on the outline and never asks for one"
                );
            }
            Reads::Rectangles(why) => assert!(
                !asks_for_the_outline(text),
                "{file} is listed as reading rectangles only ({why}) and now asks for the \
                 outline: move it to the readers that act on one"
            ),
            Reads::Directly(needle, why) => assert!(
                code_of(text).contains(needle),
                "{file} is listed as reading the ECS directly because \"{why}\", and its \
                 code has no `{needle}`: the reason is not true of the file"
            ),
        }
    }
    assert!(
        checked >= 8,
        "the walk found {checked} readers that act on the outline"
    );
}

#[test]
fn a_file_that_reads_the_ecs_directly_is_not_a_zones_reader_in_disguise() {
    // `Directly` says the file does not go through `zones()` to get a zone it
    // then reads the box of. One that does belongs under `Rectangles`.
    let all = sources();
    for (file, how) in READERS {
        if let Reads::Directly(..) = how {
            let code = code_of(&all.iter().find(|(f, _)| f == file).unwrap().1);
            assert!(
                !code.contains(".zones()") || file.ends_with("sync.rs"),
                "{file} is listed as reading the ECS directly and calls `.zones()`"
            );
        }
    }
}

/// Files that read a zone and loop over the two outer layers on purpose, and why.
const OUTER_ONLY: &[(&str, &str)] = &[(
    "crates/cypcb-world/src/stitch.rs",
    "the copper a stitching via must keep off, taken from the outer sides; inner copper there is its own item",
)];

/// The places in this code that list the two outer layers and stop there.
///
/// A list of the two outer layers followed by the inner ones is the whole stack
/// and is fine; a loop over the two alone is how a plane on an inner layer
/// reached the Gerber and never the screen or the island check.
fn outer_only_lists(code: &str) -> usize {
    let list = "[Layer::TopCopper, Layer::BottomCopper]";
    code.match_indices(list)
        .filter(|(at, _)| {
            let after: String = code[at + list.len()..]
                .chars()
                .filter(|c| !c.is_whitespace())
                .take(".into_iter().chain(".len())
                .collect();
            after != ".into_iter().chain("
        })
        .count()
}

#[test]
fn a_reader_of_a_zone_loops_over_the_layers_of_the_board() {
    let all = sources();
    let mut looked = 0;
    for (file, _) in READERS {
        let code = code_of(&all.iter().find(|(f, _)| f == file).unwrap().1);
        looked += 1;
        let allowed = OUTER_ONLY.iter().any(|(f, _)| f == file);
        if allowed {
            assert!(
                outer_only_lists(&code) > 0,
                "{file} is listed as looping over the outer layers on purpose and no longer does"
            );
        } else {
            assert_eq!(
                outer_only_lists(&code),
                0,
                "{file} reads zones and lists the two outer layers and no more: take the \
                 layers from `BoardWorld::copper_layers()`, or say on `OUTER_ONLY` why not"
            );
        }
    }
    assert!(looked >= 20, "the walk looked at {looked} readers");
}

#[test]
fn control_the_scan_tells_the_outer_pair_from_the_whole_stack() {
    assert_eq!(
        outer_only_lists("for layer in [Layer::TopCopper, Layer::BottomCopper] {"),
        1
    );
    assert_eq!(
        outer_only_lists(
            "[Layer::TopCopper, Layer::BottomCopper]\n    .into_iter()\n    .chain((0..30).map(Layer::Inner))"
        ),
        0
    );
    assert_eq!(outer_only_lists("for &layer in &layers {"), 0);
}

#[test]
fn control_the_scan_sees_each_way_of_reading_a_zone() {
    assert!(reads_zones("let z = world.zones();"));
    assert!(reads_zones("fn f(zone: &Zone, x: u8) {}"));
    assert!(reads_zones("fn f(zone: &mut Zone) {}"));
    assert!(reads_zones("in_build_order::<(Entity, &Zone)>(ecs)"));
    assert!(reads_zones("ecs.query::<&crate::components::zone::Zone>()"));
    assert!(reads_zones(
        "in_build_order::<(\n  Entity,\n  Option<&Zone>,\n)>(ecs)"
    ));
}

#[test]
fn control_the_scan_ignores_what_is_not_a_read() {
    assert!(!reads_zones("// world.zones()\nlet x = 1;"));
    assert!(!reads_zones("let z = ZoneKind::Keepout;"));
    assert!(!reads_zones(
        "in_build_order::<(Entity, &ZoneOutline)>(ecs)"
    ));
    assert!(!reads_zones(
        "fn a() {}\n#[cfg(test)]\nmod tests { fn t() { w.zones(); } }"
    ));
}

#[test]
fn control_the_outline_check_tells_the_two_calls_from_zones() {
    assert!(asks_for_the_outline("world.outlined_zones()"));
    assert!(asks_for_the_outline("world.all_zones()"));
    assert!(asks_for_the_outline("let pours = pours_of(world);"));
    assert!(asks_for_the_outline(
        "for p in copper::outlined_pours(world, layer) {}"
    ));
    assert!(asks_for_the_outline(
        "ecs.query::<(&Zone, Option<&ZoneOutline>)>()"
    ));
    assert!(!asks_for_the_outline("world.zones()"));
    assert!(!asks_for_the_outline("ecs.query::<&Zone>()"));
}

#[test]
fn control_a_reason_whose_code_is_not_in_the_file_is_not_true_of_it() {
    // The check `Directly` rows go through: the needle is in the code, and not
    // in a comment or in the test module.
    let src = "fn f() { zones.filter(|z| z.is_flex()); }";
    assert!(code_of(src).contains("is_flex()"));
    assert!(!code_of("// z.is_flex()\nfn f() {}").contains("is_flex()"));
    assert!(
        !code_of("fn f() {}\n#[cfg(test)]\nmod t { fn g() { z.is_flex(); } }")
            .contains("is_flex()")
    );
    assert!(!code_of(src).contains("&Hatch"));
}
