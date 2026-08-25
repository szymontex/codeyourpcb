//! The guide's lists of properties are the lists the parser has.
//!
//! `cargo test -p cypcb-cli --test the_guide_lists_every_property`
//!
//! Mistype a property and the parser answers with what the block does take.
//! `docs/SYNTAX.md` writes those same lists out in prose and in skeletons, and
//! the two drifted apart the way the parser's own help drifted from its reader
//! a week ago: the guide said a net's constraint block takes **width,
//! clearance, current and impedance** - four of the five it takes - and the
//! `trace` skeleton listed six of seven. Both were missing `neck`, which has
//! been in the language, both readers, four DRC rules and the router since
//! 2026-08-20.
//!
//! A list is a promise. A reader who scans it and does not find `neck`
//! concludes the language has no such thing, which is worse than saying
//! nothing at all - so the lists are read out of the binary here and required
//! to be in the document.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

fn guide() -> String {
    std::fs::read_to_string(repo_root().join("docs/SYNTAX.md"))
        .expect("the syntax guide is in the repo")
}

/// What the parser says a block takes, asked by mistyping a property in it.
fn what_the_parser_says(block: &str, source: &str) -> Vec<String> {
    let dir = std::env::temp_dir().join("cypcb-guide-lists");
    std::fs::create_dir_all(&dir).expect("a place to work");
    let file = dir.join(format!("{}.cypcb", block.replace(' ', "-")));
    std::fs::write(&file, source).expect("the board is writable");

    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .arg("check")
        .arg(&file)
        .output()
        .expect("the binary runs");
    let said = String::from_utf8_lossy(&output.stdout).to_string()
        + &String::from_utf8_lossy(&output.stderr);
    // miette draws a gutter down the left of everything it prints, so the help
    // is one line only once the whitespace is flattened.
    let flat = said.split_whitespace().collect::<Vec<_>>().join(" ");
    let marker = format!("`{block}` takes:");
    let tail = flat
        .split(&marker)
        .nth(1)
        .unwrap_or_else(|| panic!("the parser names what `{block}` takes:\n{said}"));
    // The list runs to the end of the line, and what follows is the next
    // diagnostic: its own code, or the box it is drawn in.
    let list = tail
        .split("cypcb::")
        .next()
        .unwrap_or_default()
        .split('\u{d7}')
        .next()
        .unwrap_or_default();
    list.split(',')
        .map(|word| {
            word.trim()
                .trim_matches(|c: char| !c.is_alphanumeric())
                .to_string()
        })
        .filter(|word| !word.is_empty())
        .collect()
}

/// A board with a made-up property inside a net's constraint block.
const BAD_NET: &str = r#"version 1

board lists {
    size 20mm x 20mm
    layers 2
}

net A [nonsense 1mm] {
}
"#;

/// A board with a made-up property inside a trace block.
const BAD_TRACE: &str = r#"version 1

board lists {
    size 20mm x 20mm
    layers 2
}

net A {
}

trace A {
    nonsense 1mm
}
"#;

#[test]
fn the_net_constraint_list_in_the_guide_is_the_parsers_list() {
    let takes = what_the_parser_says("net constraint", BAD_NET);
    assert!(
        takes.len() >= 5,
        "the parser named {takes:?}, which is fewer than the five this was \
         written against"
    );

    // The sentence that introduces the block, wherever the guide wrapped it.
    let flat = guide().split_whitespace().collect::<Vec<_>>().join(" ");
    let sentence = flat
        .split("A constraint block takes")
        .nth(1)
        .expect("the guide introduces the constraint block")
        .split(':')
        .next()
        .expect("the sentence ends at the example");

    for property in &takes {
        assert!(
            sentence.contains(&format!("`{property}`")),
            "the constraint block takes `{property}` and the guide's list does \
             not name it - a reader who scans that sentence concludes the \
             language has no such thing.\n  guide: {sentence}\n  parser: {takes:?}"
        );
    }
}

#[test]
fn the_trace_skeleton_in_the_guide_names_every_property() {
    let takes = what_the_parser_says("trace", BAD_TRACE);
    assert!(
        takes.len() >= 7,
        "the parser named {takes:?}, which is fewer than the seven this was \
         written against"
    );

    // The skeleton under `## Trace Definition`, which is where a reader looks
    // first: `trace <net> { ... }` with a line per property.
    let guide = guide();
    let skeleton = guide
        .split("trace <net> {")
        .nth(1)
        .expect("the guide shows the trace block")
        .split("```")
        .next()
        .expect("the skeleton ends with the fence");

    for property in &takes {
        assert!(
            skeleton.contains(property.as_str()),
            "the trace block takes `{property}` and the skeleton in the guide \
             does not show it.\n  skeleton: {skeleton}\n  parser: {takes:?}"
        );
    }
}

#[test]
fn the_path_example_in_the_guide_is_copper_the_parser_draws() {
    // The bullet describing `path` carries its own one-line example, and a
    // bullet is not a fenced block, so nothing else here reads it. The first
    // draft of that bullet separated the points with spaces - the writer joins
    // them with `->` - and the parser answered ``trace` has no property `12``.
    let guide = guide();
    let example = guide
        .split("`path ")
        .nth(1)
        .expect("the guide shows what a path looks like")
        .split('`')
        .next()
        .expect("the example ends at the backtick")
        .trim()
        .to_string();

    let board = format!(
        r#"version 1

board paths {{
    size 30mm x 30mm
    layers 2
}}

net A {{
}}

trace A {{
    path {example}
    layer Top
    width 0.2mm
}}
"#
    );

    let dir = std::env::temp_dir().join("cypcb-guide-path");
    std::fs::create_dir_all(&dir).expect("a place to work");
    let file = dir.join("path.cypcb");
    std::fs::write(&file, &board).expect("the board is writable");

    let output = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .args([
            "parse".as_ref(),
            file.as_os_str(),
            "-o".as_ref(),
            "ast".as_ref(),
        ])
        .output()
        .expect("the binary runs");
    assert!(
        output.status.success(),
        "the guide shows `path {example}` and the parser refuses it:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// One block, where the guide documents it, and a board that mistypes a
/// property inside it.
struct Block {
    /// The name the parser uses when it says what the block takes.
    name: &'static str,
    /// The heading the guide documents it under. A section owns its
    /// subsections: the stack's fields are spread over `### A board that
    /// bends`, `### Which holes this build drills` and their neighbours, which
    /// is why `stackup` is measured against the whole board section.
    heading: &'static str,
    source: &'static str,
}

const BLOCKS: &[Block] = &[
    Block {
        name: "board",
        heading: "## Board Definition",
        source: "version 1\n\nboard b {\n    size 20mm x 20mm\n    layers 2\n    nonsense 1mm\n}\n",
    },
    Block {
        name: "stackup",
        heading: "## Board Definition",
        source: "version 1\n\nboard b {\n    size 20mm x 20mm\n    layers 2\n    stackup {\n        nonsense 1mm\n    }\n}\n",
    },
    Block {
        name: "component",
        heading: "## Component Definition",
        source: "version 1\n\nboard base {\n    size 20mm x 20mm\n    layers 2\n}\n\ncomponent R1 resistor \"0402\" {\n    value \"1k\"\n    nonsense 1mm\n}\n",
    },
    Block {
        name: "zone",
        heading: "## Zone Definition",
        source: "version 1\n\nboard base {\n    size 20mm x 20mm\n    layers 2\n}\n\nzone gnd {\n    layer Top\n    nonsense 1mm\n}\n",
    },
    Block {
        name: "footprint",
        heading: "## Custom Footprint Definition",
        source: "version 1\n\nboard base {\n    size 20mm x 20mm\n    layers 2\n}\n\nfootprint F {\n    nonsense 1mm\n}\n",
    },
    Block {
        name: "module",
        heading: "## Modules and Interfaces",
        source: "version 1\n\nboard base {\n    size 20mm x 20mm\n    layers 2\n}\n\nmodule M {\n    nonsense 1mm\n}\n",
    },
];

/// The part of the guide that documents this block, subsections included.
fn section(guide: &str, heading: &str) -> String {
    let level = heading.chars().take_while(|c| *c == '#').count();
    let start = guide
        .find(heading)
        .unwrap_or_else(|| panic!("the guide has no section `{heading}`"));
    let rest = &guide[start + heading.len()..];

    let mut end = rest.len();
    for (offset, _) in rest.match_indices('\n') {
        let line = rest[offset + 1..].lines().next().unwrap_or_default();
        let hashes = line.chars().take_while(|c| *c == '#').count();
        if hashes > 0 && hashes <= level {
            end = offset;
            break;
        }
    }
    rest[..end].to_string()
}

#[test]
fn every_block_the_parser_knows_is_documented_property_by_property() {
    let guide = guide();
    assert_eq!(
        BLOCKS.len(),
        6,
        "six blocks answer with a property list; a seventh needs a case here"
    );

    for block in BLOCKS {
        let takes = what_the_parser_says(block.name, block.source);
        assert!(
            !takes.is_empty(),
            "the parser named nothing for `{}`",
            block.name
        );
        let documented = section(&guide, block.heading);

        for property in &takes {
            // `pin.<N> = <NET>` is a form rather than a word: what the guide
            // has to name is the `pin`.
            let word = property.split('.').next().unwrap_or(property);
            assert!(
                documented.contains(word),
                "`{}` takes `{word}` and the guide's `{}` section never says \
                 so.\n  parser: {takes:?}",
                block.name,
                block.heading
            );
        }
    }
}
