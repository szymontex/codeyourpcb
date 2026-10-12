//! Every footprint a test defines is held to the courtyard rule, or says why
//! it is not.
//!
//! `cargo test -p cypcb-kicad --test every_footprint_a_test_defines_is_accounted_for`
//!
//! `every_footprint_holds_its_pads_in_its_courtyard` reads the footprints the
//! repository keeps in files. The tests define a good many more by hand - a
//! `Footprint { .. }` literal, a `footprint NAME { .. }` block in a string, a
//! KiCad footprint or board in a string - and none of those was seen by it:
//! measured on 2026-10-12, 43 sites where a pad stood outside its courtyard,
//! and nothing said whether each was meant.
//!
//! This reads the source of every test, the way a person would, and asks:
//!
//! - a `Footprint { .. }` literal ends in a call that holds it to the rule
//!   (`expect_pads_in_courtyard`, `with_courtyard_holding_its_pads`,
//!   `with_ipc_courtyard`) or states why it is wrong on purpose
//!   (`pads_outside_courtyard_because`);
//! - a footprint written as text in a string, which can be read, has its pads
//!   inside its courtyard, unless it is on [`WRONG_ON_PURPOSE`] with the
//!   reason, and the entry still names a footprint that is still wrong.
//!
//! Each kind is counted, so a walk that found nothing fails instead of passing.

use cypcb_parser::ast::Definition;
use cypcb_world::footprint::{Footprint, FootprintLibrary};
use cypcb_world::{sync_ast_to_world, BoardWorld};

/// Footprints written as text in a test whose courtyard is wrong on purpose.
///
/// `(file, footprint, reason)`. A footprint here has to exist and has to be
/// wrong, or this test fails: an exemption nobody reads is a claim nothing
/// checks.
const WRONG_ON_PURPOSE: &[(&str, &str, &str)] = &[
    (
        "crates/cypcb-kicad/tests/every_footprint_holds_its_pads_in_its_courtyard.rs",
        "HDR",
        "the control of that test: the benchmark connector as it was written before its \
         courtyard learned where its centre is, which the test says it must catch",
    ),
    (
        "crates/cypcb-parser/src/parser.rs",
        "MY_PKG",
        "a parse test that asserts the courtyard's 4mm x 2mm and pad 2 at 2mm as written; the \
         footprint is read and checked field by field and never placed",
    ),
];

/// Calls a `Footprint { .. }` literal may end in.
const HOLDS: &[&str] = &[
    ".expect_pads_in_courtyard(",
    ".with_courtyard_holding_its_pads(",
    ".with_ipc_courtyard(",
    ".pads_outside_courtyard_because(",
];

/// A source file with what is code and what is a string told apart.
struct Source {
    /// The file as it stands, with every comment, string and character
    /// literal replaced by spaces of the same length (newlines kept).
    code: String,
    /// Where the test code starts: the whole file for a test file, the
    /// `#[cfg(test)]` module for a source file.
    tests_from: usize,
    /// Each string literal in the file: its line, where it starts and its
    /// decoded text.
    strings: Vec<(usize, usize, String)>,
}

fn line_of(src: &str, offset: usize) -> usize {
    src[..offset].matches('\n').count() + 1
}

fn scan(path: &str, src: &str) -> Source {
    let b = src.as_bytes();
    let mut code = src.as_bytes().to_vec();
    let mut strings = Vec::new();
    let blank = |code: &mut Vec<u8>, from: usize, to: usize| {
        for byte in &mut code[from..to] {
            if *byte != b'\n' {
                *byte = b' ';
            }
        }
    };
    let mut i = 0;
    while i < b.len() {
        if b[i..].starts_with(b"//") {
            let end = src[i..].find('\n').map_or(b.len(), |n| i + n);
            blank(&mut code, i, end);
            i = end;
        } else if b[i..].starts_with(b"/*") {
            let end = src[i + 2..].find("*/").map_or(b.len(), |n| i + 2 + n + 2);
            blank(&mut code, i, end);
            i = end;
        } else if let Some(raw) = raw_start(b, i) {
            let (open_len, hashes) = raw;
            let closer = format!("\"{}", "#".repeat(hashes));
            let body = i + open_len;
            let end = src[body..].find(&closer).map_or(b.len(), |n| body + n);
            strings.push((line_of(src, i), i, src[body..end].to_string()));
            let stop = (end + closer.len()).min(b.len());
            blank(&mut code, i, stop);
            i = stop;
        } else if b[i] == b'"' {
            let mut j = i + 1;
            let mut text = String::new();
            while j < b.len() && b[j] != b'"' {
                if b[j] == b'\\' && j + 1 < b.len() {
                    match b[j + 1] {
                        b'n' => text.push('\n'),
                        b't' => text.push('\t'),
                        b'"' => text.push('"'),
                        b'\\' => text.push('\\'),
                        b'\n' => {
                            // A line continuation eats the next line's indentation.
                            j += 2;
                            while j < b.len() && (b[j] == b' ' || b[j] == b'\t' || b[j] == b'\n') {
                                j += 1;
                            }
                            continue;
                        }
                        other => text.push(other as char),
                    }
                    j += 2;
                } else {
                    let ch = src[j..].chars().next().unwrap();
                    text.push(ch);
                    j += ch.len_utf8();
                }
            }
            strings.push((line_of(src, i), i, text));
            let stop = (j + 1).min(b.len());
            blank(&mut code, i, stop);
            i = stop;
        } else if b[i] == b'\'' {
            // A character literal, not a lifetime: `'x'` or `'\n'`.
            let rest = &src[i + 1..];
            let len = if rest.starts_with('\\') {
                rest.find('\'').map(|n| n + 1)
            } else {
                rest.chars().next().and_then(|c| {
                    rest[c.len_utf8()..]
                        .starts_with('\'')
                        .then_some(c.len_utf8() + 1)
                })
            };
            match len {
                Some(n) => {
                    blank(&mut code, i, i + 1 + n);
                    i += 1 + n;
                }
                None => i += 1,
            }
        } else {
            i += 1;
        }
    }
    let code = String::from_utf8(code).expect("blanking keeps the bytes valid");
    let tests_from = if path.contains("/tests/") {
        0
    } else {
        code.find("#[cfg(test)]").unwrap_or(usize::MAX)
    };
    Source {
        code,
        tests_from,
        strings,
    }
}

/// `r"`, `r#"`, `br"`: the length of the opener and its number of hashes.
fn raw_start(b: &[u8], i: usize) -> Option<(usize, usize)> {
    let mut j = i;
    if b.get(j) == Some(&b'b') {
        j += 1;
    }
    if b.get(j) != Some(&b'r') {
        return None;
    }
    // Not the tail of an identifier: `for"` is not a raw string.
    if i > 0 && (b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_') {
        return None;
    }
    j += 1;
    let mut hashes = 0;
    while b.get(j) == Some(&b'#') {
        hashes += 1;
        j += 1;
    }
    (b.get(j) == Some(&b'"')).then_some((j + 1 - i, hashes))
}

/// The `Footprint { .. }` struct literals in the test code of a source: where
/// each is and what follows its closing brace.
fn literals(source: &Source) -> Vec<(usize, String)> {
    let code = &source.code;
    let bytes = code.as_bytes();
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(at) = code[from..].find("Footprint") {
        let start = from + at;
        from = start + "Footprint".len();
        if start < source.tests_from {
            continue;
        }
        let before = code[..start].trim_end();
        let identifier_tail = start > 0 && {
            let p = bytes[start - 1];
            p.is_ascii_alphanumeric() || p == b'_'
        };
        let named = before.ends_with("struct")
            || before.ends_with("impl")
            || before.ends_with("for")
            || before.ends_with("enum")
            || before.ends_with("->")
            || before.ends_with("::");
        let after = code[from..].trim_start();
        if identifier_tail || named || !after.starts_with('{') {
            continue;
        }
        let open = code.len() - after.len();
        let mut depth = 0;
        let mut end = open;
        for (k, byte) in bytes[open..].iter().enumerate() {
            match byte {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = open + k + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        found.push((
            line_of(code, start),
            code[end..(end + 120).min(code.len())].to_string(),
        ));
    }
    found
}

fn verdict(footprint: &Footprint) -> Option<String> {
    let outside = footprint.pads_outside_courtyard();
    (!outside.is_empty()).then(|| {
        format!(
            "{}: {} of {} pads outside",
            footprint.name,
            outside.len(),
            footprint.pads.len()
        )
    })
}

/// The `footprint NAME { .. }` blocks of a string, alone and under a version
/// line, so the rest of the string - a board with a `{HOLE}` in it, a
/// `format!` template - does not decide whether they can be read. Empty when
/// the string has blocks and none of the readings reads them; `None` when it
/// has none.
fn dsl_blocks(text: &str) -> Option<String> {
    let opens = |l: &str| {
        let l = l.trim_start();
        l.starts_with("footprint ") && l.trim_end().ends_with('{')
    };
    if !text.lines().any(opens) {
        return None;
    }
    for text in [text.to_string(), text.replace("{{", "{").replace("}}", "}")] {
        let mut blocks = String::new();
        let mut rest = text.as_str();
        loop {
            let mut offset = 0;
            let mut at = None;
            for line in rest.split_inclusive('\n') {
                if opens(line) {
                    at = Some(offset);
                    break;
                }
                offset += line.len();
            }
            let Some(at) = at else { break };
            let tail = &rest[at..];
            let mut depth = 0;
            let mut end = tail.len();
            for (k, ch) in tail.char_indices() {
                match ch {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            end = k + 1;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            blocks.push_str(tail[..end].trim_start());
            blocks.push('\n');
            rest = &tail[end..];
        }
        let source = format!("version 1\n{blocks}");
        if !blocks.is_empty() && !cypcb_parser::parse(&source).has_errors() {
            return Some(source);
        }
    }
    Some(String::new())
}

fn looks_kicad(text: &str) -> bool {
    let head = text.trim_start();
    head.starts_with("(footprint") || head.starts_with("(module") || head.starts_with("(kicad_pcb")
}

/// A KiCad text with a template's holes taken out, when it is one.
fn kicad_text(text: &str) -> Option<String> {
    if !looks_kicad(text) {
        return None;
    }
    let mut out = String::new();
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        match rest[open..].find('}') {
            Some(close)
                if rest[open + 1..open + close]
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '_') =>
            {
                rest = &rest[open + close + 1..];
            }
            _ => {
                out.push('{');
                rest = &rest[open + 1..];
            }
        }
    }
    out.push_str(rest);
    Some(out)
}

/// The footprints a DSL text defines, as the library holds them after sync.
fn dsl_footprints(text: &str) -> Option<Vec<Footprint>> {
    if text.is_empty() {
        return None;
    }
    let parsed = cypcb_parser::parse(text);
    if parsed.has_errors() {
        return None;
    }
    let names: Vec<String> = parsed
        .value
        .definitions
        .iter()
        .filter_map(|d| match d {
            Definition::Footprint(f) => Some(f.name.value.clone()),
            _ => None,
        })
        .collect();
    if names.is_empty() {
        return None;
    }
    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    let _ = sync_ast_to_world(&parsed.value, text, &mut world, &mut library);
    Some(
        names
            .iter()
            .filter_map(|name| library.get(name).cloned())
            .collect(),
    )
}

/// The footprints a KiCad text defines: a footprint file or a board.
fn kicad_footprints(text: &str) -> Option<Vec<Footprint>> {
    let head = text.trim_start();
    if head.starts_with("(footprint") || head.starts_with("(module") {
        return cypcb_kicad::import_footprint_from_str(text)
            .ok()
            .map(|f| vec![f]);
    }
    if head.starts_with("(kicad_pcb") {
        let parsed = cypcb_kicad::pcb_parser::parse_kicad_pcb_str(text).ok()?;
        let builtin = FootprintLibrary::new();
        return Some(
            parsed
                .library
                .iter()
                .filter(|(name, _)| !builtin.contains(name))
                .map(|(_, f)| f.clone())
                .collect(),
        );
    }
    None
}

fn tests() -> Vec<(String, Source)> {
    let root = cypcb_fixtures::tree::repo_root();
    let mut files: Vec<String> = cypcb_fixtures::tree::files_git_tracks(&root)
        .into_iter()
        .filter(|f| f.starts_with("crates/") && f.ends_with(".rs"))
        .filter(|f| f.contains("/tests/") || f.contains("/src/"))
        .collect();
    files.sort();
    files
        .into_iter()
        .map(|f| {
            let text = std::fs::read_to_string(root.join(&f)).expect("a tracked source reads");
            let source = scan(&f, &text);
            (f, source)
        })
        .collect()
}

#[test]
fn every_literal_a_test_builds_is_held_to_the_rule_or_says_why_not() {
    let mut loose = Vec::new();
    let mut counted = 0;
    let mut files = 0;
    for (file, source) in tests() {
        if file.ends_with("every_footprint_a_test_defines_is_accounted_for.rs") {
            continue;
        }
        let found = literals(&source);
        files += usize::from(!found.is_empty());
        for (line, after) in found {
            counted += 1;
            let tail: String = after.split_whitespace().collect();
            if !HOLDS.iter().any(|call| tail.starts_with(call)) {
                loose.push(format!("{file}:{line}"));
            }
        }
    }
    eprintln!("footprint literals in tests: {counted} in {files} files");
    assert!(
        counted > 50,
        "the walk found {counted} literals; it has stopped seeing them"
    );
    assert!(
        loose.is_empty(),
        "a Footprint literal in a test that is held to no courtyard rule - end it in \
         `.expect_pads_in_courtyard()`, `.with_courtyard_holding_its_pads()` or \
         `.pads_outside_courtyard_because(\"why\")`:\n{}",
        loose.join("\n")
    );
}

#[test]
fn every_footprint_a_test_writes_as_text_holds_its_pads_or_says_why_not() {
    let mut wrong: Vec<(String, String)> = Vec::new();
    let mut counted = [("dsl", 0usize), ("kicad", 0usize)];
    let mut unreadable = 0usize;
    for (file, source) in tests() {
        if file.ends_with("every_footprint_a_test_defines_is_accounted_for.rs") {
            continue;
        }
        for (_, at, text) in &source.strings {
            if *at < source.tests_from {
                continue;
            }
            let (kind, footprints) = if let Some(blocks) = dsl_blocks(text) {
                match dsl_footprints(&blocks) {
                    Some(f) => (0, f),
                    None => {
                        unreadable += 1;
                        continue;
                    }
                }
            } else if let Some(f) = kicad_text(text).and_then(|t| kicad_footprints(&t)) {
                (1, f)
            } else {
                if looks_kicad(text) {
                    unreadable += 1;
                }
                continue;
            };
            for footprint in footprints {
                counted[kind].1 += 1;
                if let Some(said) = verdict(&footprint) {
                    wrong.push((file.clone(), said));
                }
            }
        }
    }
    eprintln!(
        "footprints written as text: {counted:?}; strings that do not read alone: {unreadable}"
    );
    for (kind, count) in counted {
        assert!(
            count > 0,
            "the walk found no {kind} footprint written as text"
        );
    }

    let problems = exemption_problems(&wrong, WRONG_ON_PURPOSE);
    assert!(
        problems.is_empty(),
        "footprints written in a test and their courtyards:\n{}",
        problems.join("\n")
    );
}

/// What is wrong with the way the wrong footprints and the exemptions line up:
/// an exemption with no reason, one whose footprint is not wrong any more or
/// not there, and a wrong footprint on no exemption.
fn exemption_problems(wrong: &[(String, String)], exempt: &[(&str, &str, &str)]) -> Vec<String> {
    let named = |said: &str, name: &str| said.starts_with(&format!("{name}:"));
    let mut problems = Vec::new();
    for (file, name, reason) in exempt {
        if reason.trim().is_empty() {
            problems.push(format!("{file}: {name}: an exemption without a reason"));
        }
        let still = wrong.iter().any(|(f, said)| f == file && named(said, name));
        if !still {
            problems.push(format!(
                "{file}: {name} is exempt because {reason:?}, but no footprint of that name \
                 there has a pad outside its courtyard any more"
            ));
        }
    }
    for (file, said) in wrong {
        if !exempt
            .iter()
            .any(|(f, name, _)| f == file && named(said, name))
        {
            problems.push(format!("{file}: {said}: on no exemption list"));
        }
    }
    problems
}

#[test]
fn control_an_exemption_is_held_to_its_reason_and_to_its_footprint() {
    let wrong = vec![("a.rs".to_string(), "HDR: 1 of 2 pads outside".to_string())];

    // Everything lines up.
    assert_eq!(
        exemption_problems(&wrong, &[("a.rs", "HDR", "why")]),
        Vec::<String>::new()
    );
    // A reason that says nothing.
    let said = exemption_problems(&wrong, &[("a.rs", "HDR", "  ")]);
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(said[0].contains("without a reason"));
    // An exemption whose footprint is right now: nothing wrong to exempt.
    let said = exemption_problems(&[], &[("a.rs", "HDR", "why")]);
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(said[0].contains("any more"));
    // The same name in another file is not the footprint.
    let said = exemption_problems(&wrong, &[("b.rs", "HDR", "why")]);
    assert_eq!(said.len(), 2, "{said:?}");
    // A wrong footprint nobody exempted.
    let said = exemption_problems(&wrong, &[]);
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(said[0].contains("on no exemption list"));
}

/// The control: the walk does see a bare literal and a wrong text footprint.
#[test]
fn control_the_walk_sees_a_loose_literal_and_a_wrong_text_footprint() {
    let path = "crates/x/tests/t.rs";
    let src = "fn f() -> Footprint {\n    Footprint {\n        name: x,\n    }\n}\n\
               fn g() -> Footprint {\n    Footprint {\n        name: y,\n    }\n    .expect_pads_in_courtyard()\n}\n\
               const S: &str = \"not { a Footprint { here\";\n// Footprint { in a comment }\n";
    let source = scan(path, src);
    let found = literals(&source);
    assert_eq!(
        found.len(),
        2,
        "two literals, none in the string or comment: {found:?}"
    );
    let ends: Vec<bool> = found
        .iter()
        .map(|(_, after)| {
            let tail: String = after.split_whitespace().collect();
            HOLDS.iter().any(|call| tail.starts_with(call))
        })
        .collect();
    assert_eq!(
        ends,
        [false, true],
        "the first is loose, the second is held"
    );

    let wrong = dsl_footprints(
        "version 1\nfootprint W {\n    courtyard 1mm x 1mm\n    pad 1 rect at 2mm, 0mm size 1mm x 1mm\n}\n",
    )
    .expect("the text reads");
    assert!(
        verdict(&wrong[0]).is_some(),
        "a pad outside a 1mm courtyard is caught"
    );
    let right = dsl_footprints(
        "version 1\nfootprint R {\n    courtyard 4mm x 2mm\n    pad 1 rect at 1mm, 0mm size 1mm x 1mm\n}\n",
    )
    .expect("the text reads");
    assert!(verdict(&right[0]).is_none(), "and one inside is not");
}
