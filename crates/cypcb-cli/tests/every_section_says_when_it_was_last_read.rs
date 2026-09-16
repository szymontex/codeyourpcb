//! Every section of the routing canon says when somebody last read it.
//!
//! `cargo test -p cypcb-cli --test every_section_says_when_it_was_last_read`
//!
//! The file used to carry one paragraph at the end listing what had been
//! verified and when. It went stale the way that shape always does: it is the
//! one place that has to know about every other, and the author of a new
//! section has no reason to look at it. By the time it was replaced it claimed
//! 2026-09-11 for a set of rules that included two statements false since
//! `04fbdc7`, and named the registry's size as 38 where the command three
//! hundred lines above it expected 39.
//!
//! So the obligation moved to where the content is written. Each `###` section
//! carries its own line, a new section without one fails on the day it is
//! written rather than six months later, and the run prints the oldest date it
//! found together with the section holding it - the document publishing its own
//! weakest point instead of asserting collectively that it is sound.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Below this the walk has stopped seeing the file rather than found it clean.
/// The canon carries thirty-one sections today and the number only grows.
const SECTIONS_EXPECTED: usize = 25;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .to_path_buf()
}

/// Today, from the system rather than from a constant that would need editing.
fn today() -> String {
    let output = Command::new("date")
        .arg("+%F")
        .output()
        .expect("the date command runs");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn is_iso_date(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(i, b)| i == 4 || i == 7 || b.is_ascii_digit())
}

#[test]
fn every_section_says_when_it_was_last_read() {
    let canon = std::fs::read_to_string(repo_root().join("docs/ROUTING-CANON.md"))
        .expect("the canon is there");
    let lines: Vec<&str> = canon.lines().collect();

    let mut sections: Vec<(String, Vec<String>)> = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if !line.starts_with("### ") {
            continue;
        }
        let name = line[4..].replace('`', "");
        // Its own line, before the next heading of any depth.
        let mut stamps = Vec::new();
        for following in &lines[index + 1..] {
            if following.starts_with("## ") || following.starts_with("### ") {
                break;
            }
            if let Some(rest) = following.strip_prefix("*Verified: ") {
                if let Some(value) = rest.strip_suffix('*') {
                    stamps.push(value.to_string());
                }
            }
        }
        sections.push((name, stamps));
    }

    assert!(
        sections.len() >= SECTIONS_EXPECTED,
        "this check found {} sections and expected at least {SECTIONS_EXPECTED}. Either the \
         canon lost most of itself, or the heading it walks changed shape and this check is \
         now reading nothing - which is the failure it exists to prevent.",
        sections.len()
    );

    let missing: Vec<&String> = sections
        .iter()
        .filter(|(_, stamps)| stamps.len() != 1)
        .map(|(name, _)| name)
        .collect();
    assert!(
        missing.is_empty(),
        "every section of the canon carries exactly one `*Verified: <date or never>*` line \
         directly under its heading, and these do not: {missing:?}\n\
         \n  The line records the day somebody read that section's claims against the working \
         tree - opening the file, not grepping for a name. A section nobody has re-read since \
         it was written says `never`, which is honest and sorts oldest, so it surfaces in the \
         line this check prints. A commit date is not an answer to this question: a commit says \
         somebody typed, this line says somebody checked."
    );

    let now = today();
    let mut oldest: Option<(&str, &str)> = None;
    let mut never = 0usize;
    for (name, stamps) in &sections {
        let stamp = stamps[0].as_str();
        if stamp == "never" {
            never += 1;
            continue;
        }
        assert!(
            is_iso_date(stamp),
            "`{name}` states `{stamp}`, which is neither a YYYY-MM-DD date nor `never`"
        );
        assert!(
            stamp <= now.as_str(),
            "`{name}` says it was read on {stamp}, which is after today ({now}). A date \
             written ahead of the reading is the one kind of entry here that is worse than \
             `never`."
        );
        if oldest.is_none_or(|(_, seen)| stamp < seen) {
            oldest = Some((name.as_str(), stamp));
        }
    }

    let (section, date) = oldest.expect("some section carries a date");
    eprintln!(
        "{} sections: {} carry a date, oldest {date} in \"{section}\"; {never} say never.",
        sections.len(),
        sections.len() - never
    );
}

/// How many line-number references the canon's prose still carries.
///
/// A ceiling, and it may only fall. The file states the rule - a line number is
/// not a reference - and states this number in the same breath, because the
/// rule was false in forty eight places on the day it was written and a rule
/// with no count under it is the defect it describes.
const LINE_NUMBER_REFERENCES: usize = 42;

#[test]
fn line_numbers_in_this_file_only_fall() {
    let canon = std::fs::read_to_string(repo_root().join("docs/ROUTING-CANON.md"))
        .expect("the canon is there");

    // Command blocks are exempt and the count says so: `grep -n` prints a line
    // number, and a number that came out of a run quoted beside it is the form
    // the rule asks for.
    let mut in_block = false;
    let mut found: Vec<String> = Vec::new();
    for line in canon.lines() {
        if line.starts_with("```") {
            in_block = !in_block;
            continue;
        }
        if in_block {
            continue;
        }
        let bytes: Vec<char> = line.chars().collect();
        let mut index = 0;
        while index < bytes.len() {
            if bytes[index] == ':'
                && index >= 3
                && bytes[index - 3..index].iter().collect::<String>() == ".rs"
                && bytes.get(index + 1).is_some_and(char::is_ascii_digit)
            {
                let start = line[..index].rfind(['`', ' ']).map_or(0, |i| i + 1);
                found.push(line[start..].chars().take(48).collect());
            }
            index += 1;
        }
    }

    eprintln!(
        "line-number references in the canon's prose: {} of a ceiling of {LINE_NUMBER_REFERENCES}",
        found.len()
    );
    assert!(
        found.len() <= LINE_NUMBER_REFERENCES,
        "the canon carries {} line-number references and the ceiling is \
         {LINE_NUMBER_REFERENCES}. A line number is found by nothing and breaks silently on \
         every insertion above it: name the symbol instead, or print the number from a command \
         quoted beside it. The ceiling falls as they go, and it does not rise. All of them, \
         in file order, so the new one can be found by comparing against the last run: {found:?}",
        found.len()
    );
}

/// The one name in the canon that is not a registry entry.
///
/// `DrcRule` is the trait every entry implements, so it appears in prose about
/// the shape of a rule rather than about a rule. Any other exception has to be
/// argued for here, in the open, rather than added to a regex.
const NOT_A_REGISTRY_ENTRY: &[&str] = &["DrcRule"];

/// How many entries the registry must hold before this check believes it read
/// the registry at all. It holds thirty-nine today.
const REGISTRY_FLOOR: usize = 30;

/// Every rule name in `Box::new(rules::...)`, which is the registry itself.
fn registry() -> BTreeSet<String> {
    let source = std::fs::read_to_string(repo_root().join("crates/cypcb-drc/src/lib.rs"))
        .expect("the registry is there");
    let mut names = BTreeSet::new();
    for line in source.lines() {
        let Some(rest) = line.trim().strip_prefix("Box::new(rules::") else {
            continue;
        };
        if let Some(name) = rest.split(')').next() {
            names.insert(name.to_string());
        }
    }
    names
}

/// Every `SomethingRule` the canon's prose names, and where.
fn names_in_canon(canon: &str) -> BTreeMap<String, BTreeSet<String>> {
    let mut found: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut section = String::from("(before the first heading)");
    for line in canon.lines() {
        if let Some(rest) = line.strip_prefix("### ") {
            section = rest.replace('`', "");
        } else if let Some(rest) = line.strip_prefix("## ") {
            section = rest.replace('`', "");
        }
        let chars: Vec<char> = line.chars().collect();
        let mut start = 0;
        while start < chars.len() {
            if !chars[start].is_ascii_uppercase() {
                start += 1;
                continue;
            }
            let mut end = start;
            while end < chars.len() && (chars[end].is_ascii_alphanumeric()) {
                end += 1;
            }
            let word: String = chars[start..end].iter().collect();
            if word.len() > 4 && word.ends_with("Rule") {
                found.entry(word).or_default().insert(section.clone());
            }
            start = end.max(start + 1);
        }
    }
    found
}

#[test]
fn every_rule_the_canon_names_is_one_the_registry_runs() {
    let canon = std::fs::read_to_string(repo_root().join("docs/ROUTING-CANON.md"))
        .expect("the canon is there");
    let registry = registry();
    assert!(
        registry.len() >= REGISTRY_FLOOR,
        "this check read {} registry entries and expected at least {REGISTRY_FLOOR}. Either          the registry shrank, or `Box::new(rules::` stopped being how an entry is written and          this check is now comparing the canon against nothing.",
        registry.len()
    );

    let named = names_in_canon(&canon);
    let unknown: Vec<(&String, &BTreeSet<String>)> = named
        .iter()
        .filter(|(name, _)| !registry.contains(name.as_str()))
        .filter(|(name, _)| !NOT_A_REGISTRY_ENTRY.contains(&name.as_str()))
        .collect();

    eprintln!(
        "rule names in the canon: {} against a registry of {}; {} allowed as not an entry",
        named.len(),
        registry.len(),
        NOT_A_REGISTRY_ENTRY.len()
    );

    assert!(
        unknown.is_empty(),
        "the canon names a rule the registry does not run: {unknown:?}\n\
         \n  A rule renamed in the code leaves its old name in this document, where nothing \
         breaks and nobody looks. If the name is right and the rule is genuinely not a \
         registry entry - a trait, a helper, a rule that was never registered - say so in \
         NOT_A_REGISTRY_ENTRY with the reason, so the exception is argued rather than \
         pattern-matched."
    );
}

/// The whole vocabulary a rule's condition may run over.
///
/// Closed on purpose. A sixth value bought for one rule costs a decision on
/// every rule after it, and the point of the tag is that the count on the page
/// can be compared with the count in the file - which needs the categories to
/// stay the same length.
const SUBJECTS: &[&str] = &[
    "copper",
    "output row",
    "the tool",
    "the canon",
    "a component",
];

/// The line in R-16 that states the tally, in the form the check reads back.
const TALLY: &str = "**copper 12, output row 3, the tool 2, the canon 1, a component 1.**";

#[test]
fn every_rule_says_what_its_condition_runs_over() {
    let canon = std::fs::read_to_string(repo_root().join("docs/ROUTING-CANON.md"))
        .expect("the canon is there");
    let lines: Vec<&str> = canon.lines().collect();

    let mut rules = 0usize;
    let mut tags: BTreeMap<String, usize> = BTreeMap::new();
    let mut untagged: Vec<String> = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let Some(rest) = line.strip_prefix("### ") else {
            continue;
        };
        if !rest.starts_with("R-") {
            continue;
        }
        rules += 1;
        let name = rest.replace('`', "");
        let applies = lines[index + 1..]
            .iter()
            .take_while(|l| !l.starts_with("### ") && !l.starts_with("## "))
            .find(|l| l.starts_with("*Applies when:*"));
        let tag = applies
            .and_then(|l| l.strip_prefix("*Applies when:* ["))
            .and_then(|rest| rest.split(']').next());
        match tag {
            Some(tag) if SUBJECTS.contains(&tag) => *tags.entry(tag.to_string()).or_default() += 1,
            _ => untagged.push(name),
        }
    }

    let counted: usize = tags.values().sum();
    eprintln!(
        "rule sections: {rules}; tagged {counted}; {}",
        SUBJECTS
            .iter()
            .map(|s| format!("{s} {}", tags.get(*s).copied().unwrap_or(0)))
            .collect::<Vec<_>>()
            .join(", ")
    );

    assert!(
        rules >= 19,
        "this check found {rules} rule sections and the canon has nineteen. Either rules were          deleted, or the heading it walks changed shape and the tally below is over nothing."
    );
    assert!(
        untagged.is_empty(),
        "every rule opens its `*Applies when:*` with the subject its condition runs over, in          square brackets, from {SUBJECTS:?} - and these do not: {untagged:?}\n\
         \n  The tag names what the **condition** runs over, not what the rule reads on the \
         way there: R-08 is `copper` though it reads a part's position, and R-07 is `copper` \
         though its subject is holes. If none of the five fits, that is worth an argument in \
         the section rather than a sixth word here - the vocabulary is closed so that the \
         count on the page and the count in the file stay comparable."
    );
    assert_eq!(
        counted, rules,
        "the tally covers every rule section: {counted} tagged against {rules} sections"
    );

    // The number the prose states, held to the number the walk finds. This is
    // the half that makes a claim about the other eighteen sections checkable
    // by the person writing the nineteenth.
    let stated = format!(
        "**copper {}, output row {}, the tool {}, the canon {}, a component {}.**",
        tags.get("copper").copied().unwrap_or(0),
        tags.get("output row").copied().unwrap_or(0),
        tags.get("the tool").copied().unwrap_or(0),
        tags.get("the canon").copied().unwrap_or(0),
        tags.get("a component").copied().unwrap_or(0),
    );
    assert!(
        canon.contains(&stated),
        "R-16 states the tally and the walk disagrees with it.\n  the file says: {stated}\n           and the line there reads: {TALLY}\n\
         \n  Whichever moved, the other follows in the same commit: a count in prose beside a \
         count in a walk is two places for one fact, and this document has seven recorded \
         cases of that going wrong."
    );
}

/// The three forms below all read prose rather than code, so they share one
/// reading of the file.
///
/// Two exclusions, and measurement forced both. **Whitespace is flattened
/// first**, because the canon is hard-wrapped and one match hid across a line
/// break. **Text inside double quotes is dropped**, because this file records
/// its own corrections by quoting the sentence it replaced - a document that
/// documents its repairs would otherwise be punished for it.
fn canon_paragraphs(canon: &str) -> Vec<String> {
    let mut paragraphs = Vec::new();
    let mut buffer: Vec<&str> = Vec::new();
    for line in canon.lines() {
        if line.trim().is_empty() {
            if !buffer.is_empty() {
                paragraphs.push(buffer.join(" "));
                buffer.clear();
            }
        } else {
            buffer.push(line);
        }
    }
    if !buffer.is_empty() {
        paragraphs.push(buffer.join(" "));
    }
    paragraphs
        .into_iter()
        .map(|p| {
            let mut out = String::with_capacity(p.len());
            let mut quoted = false;
            for c in p.chars() {
                if c == '"' {
                    quoted = !quoted;
                    out.push(' ');
                } else if quoted {
                    out.push(' ');
                } else {
                    out.push(c);
                }
            }
            out.split_whitespace().collect::<Vec<_>>().join(" ")
        })
        .collect()
}

/// Every byte offset in `haystack` where `needle` starts on a word boundary.
fn word_starts(haystack: &str, needle: &str) -> Vec<usize> {
    let mut out = Vec::new();
    let mut from = 0usize;
    while let Some(offset) = haystack[from..].find(needle) {
        let at = from + offset;
        if at == 0 || !haystack.as_bytes()[at - 1].is_ascii_alphanumeric() {
            out.push(at);
        }
        from = at + 1;
    }
    out
}

/// The largest char boundary at or below `at`, so a window cut by length never
/// splits a multi-byte character.
fn boundary_at_or_below(text: &str, mut at: usize) -> usize {
    at = at.min(text.len());
    while at > 0 && !text.is_char_boundary(at) {
        at -= 1;
    }
    at
}

/// Word offsets and the words themselves, so a window can be measured from the
/// end of a word rather than from a search that would find the wrong instance.
fn words_with_offsets(text: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut start = None;
    for (index, c) in text.char_indices() {
        if c.is_whitespace() {
            if let Some(from) = start.take() {
                out.push((from, &text[from..index]));
            }
        } else if start.is_none() {
            start = Some(index);
        }
    }
    if let Some(from) = start {
        out.push((from, &text[from..]));
    }
    out
}

/// `contains`, but on whole words. Without this, `the measured one takes work`
/// carries the substring `measured on` and a paragraph about a preposition is
/// asked for a date - which is exactly what it did on the first run.
fn contains_phrase(haystack: &str, phrase: &str) -> bool {
    word_starts(haystack, phrase).into_iter().any(|at| {
        let end = at + phrase.len();
        end == haystack.len() || !haystack.as_bytes()[end].is_ascii_alphanumeric()
    })
}

fn bare(word: &str) -> &str {
    word.trim_matches(|c: char| !c.is_ascii_alphanumeric())
}

/// The parts of this document a sentence can quantify over. A superlative about
/// a *pad* or a *fixture* is a claim about the board; a superlative about a
/// *rule* is a claim about the eighteen sections the writer is not looking at.
const DOCUMENT_PARTS: &[&str] = &[
    "rule",
    "rules",
    "section",
    "sections",
    "paragraph",
    "paragraphs",
    "claim",
    "claims",
];

/// A word that says the rule belongs to somebody else. `The only
/// rectangular-specific published rule found` is a claim about a search of
/// vendor pages, not about the eighteen sections beside it, and the two are
/// opposite in kind: one is checkable only by reading this file, the other only
/// by reading the web. One word, because a second one should be argued for here
/// rather than appended - the list is the exception, and an exception nobody
/// has to justify stops being one.
const SOMEBODY_ELSES: &[&str] = &["published"];

/// `the first eleven rules` is a range, not a superlative: a count immediately
/// after the word turns "first" into an ordinal over an enumeration.
const COUNTS: &[&str] = &[
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "ten",
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen",
    "twenty",
];

fn is_count(word: &str) -> bool {
    let word = bare(word);
    !word.is_empty() && (word.chars().all(|c| c.is_ascii_digit()) || COUNTS.contains(&word))
}

/// Every name a **check** in this tree answers to: the function under each
/// `#[test]`, and the stem of every integration test file - a file is a check a
/// reader can run by name even when the function inside it is called something
/// else.
///
/// Not every function: `fill_zone` and `by_kind` are real code and neither one
/// re-runs anything. The first draft of this narrowing took every `fn` and
/// **widened** the door rather than closing it - two paragraphs that had been
/// failing started passing on the name of a helper.
fn names_in_this_tree() -> BTreeSet<String> {
    fn walk(dir: &Path, names: &mut BTreeSet<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.file_name().is_some_and(|n| n == "target") {
                    continue;
                }
                walk(&path, names);
                continue;
            }
            if path.extension().is_none_or(|e| e != "rs") {
                continue;
            }
            if path.parent().is_some_and(|p| p.ends_with("tests")) {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    names.insert(stem.to_string());
                }
            }
            let Ok(source) = std::fs::read_to_string(&path) else {
                continue;
            };
            for (at, _) in source.match_indices("#[test]") {
                let Some(fn_at) = source[at..].find("fn ") else {
                    continue;
                };
                let name: String = source[at + fn_at + 3..]
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .collect();
                if !name.is_empty() {
                    names.insert(name);
                }
            }
        }
    }
    let mut names = BTreeSet::new();
    walk(&repo_root().join("crates"), &mut names);
    names
}

/// A backticked `snake_case_name_with_underscores` **that this tree answers to**.
///
/// The shape alone was the whole test until 2026-09-16, and the shape is also
/// how this project names a constraint: of the 55 tokens in the canon that look
/// like a check, 35 are a function or a test file and **20 are field names** -
/// `max_stub_length`, `min_annular_ring`, `pour_thermal_gap`,
/// `stop_at_own_copper`. A field name re-runs nothing. One paragraph was
/// carrying a measured figure - "27 of 33 rows" - through this door on
/// `stop_at_own_copper`, and the same paragraph was carrying a superlative
/// through `cites_an_instrument` on the same token: **one hole, two checks**.
fn names_a_check(paragraph: &str, known: &BTreeSet<String>) -> bool {
    paragraph
        .split('`')
        .skip(1)
        .step_by(2)
        .any(|token| known.contains(token))
}

/// The content escape hatch: a paragraph may make a claim about the other
/// sections **if it points at the instrument that holds the claim** - a check
/// by name, or R-16's tally. Not a syntactic hatch: "used to" and "until 2026"
/// were both proposed and both refused, because anybody can type them and
/// neither one puts a number under the sentence.
fn cites_an_instrument(paragraph: &str, known: &BTreeSet<String>) -> bool {
    names_a_check(paragraph, known) || paragraph.to_ascii_lowercase().contains("tally")
}

#[test]
fn a_superlative_about_the_other_rules_cites_the_check_that_holds_it() {
    let canon = std::fs::read_to_string(repo_root().join("docs/ROUTING-CANON.md"))
        .expect("the canon is there");

    const SUPERLATIVES: &[&str] = &[
        "the only ",
        "no other ",
        "every other ",
        "the sole ",
        "the first ",
        "the last ",
    ];

    let known = names_in_this_tree();
    let mut claiming = 0usize;
    let mut hatched = 0usize;
    let mut blocked: Vec<String> = Vec::new();
    for paragraph in canon_paragraphs(&canon) {
        let lowered = paragraph.to_ascii_lowercase();
        let mut hits: Vec<String> = Vec::new();
        for phrase in SUPERLATIVES {
            for at in word_starts(&lowered, phrase) {
                let after = &lowered[at + phrase.len()..];
                let words: Vec<&str> = after.split_whitespace().take(3).collect();
                let Some(first) = words.first() else { continue };
                if is_count(first) {
                    continue;
                }
                if words.iter().any(|w| SOMEBODY_ELSES.contains(&bare(w))) {
                    continue;
                }
                if words.iter().any(|w| DOCUMENT_PARTS.contains(&bare(w))) {
                    let end = boundary_at_or_below(&lowered, at + phrase.len() + 40);
                    hits.push(lowered[at..end].to_string());
                }
            }
        }
        if hits.is_empty() {
            continue;
        }
        claiming += 1;
        if cites_an_instrument(&paragraph, &known) {
            hatched += 1;
        } else {
            blocked.push(format!(
                "{hits:?} in: {}",
                &paragraph[..paragraph.len().min(120)]
            ));
        }
    }

    eprintln!(
        "paragraphs claiming something about the other rules: {claiming}; \
         citing the check that holds it: {hatched}; blocked: {}",
        blocked.len()
    );

    assert!(
        blocked.is_empty(),
        "a sentence here claims something about the other rules and names nothing that checks \
         it: {blocked:#?}\n\
         \n  This is the one claim a writer cannot verify while writing, because it is a claim \
         about the eighteen sections they are not looking at - and both sentences in this file \
         that were false the day they were typed had this shape. Two cures, and neither is a \
         rewording: name the check or the tally that holds the claim, or narrow the sentence to \
         what you read."
    );
}

#[test]
fn nothing_here_quantifies_over_the_document_itself() {
    let canon = std::fs::read_to_string(repo_root().join("docs/ROUTING-CANON.md"))
        .expect("the canon is there");

    const QUANTIFIERS: &[&str] = &["every ", "each ", "all ", "no ", "none of the ", "nothing "];
    const THE_DOCUMENT: &[&str] = &[
        "this file",
        "this canon",
        "this document",
        "the file",
        "the canon",
        "the document",
    ];

    let mut scanned = 0usize;
    let mut offenders: Vec<String> = Vec::new();
    for paragraph in canon_paragraphs(&canon) {
        scanned += 1;
        let lowered = paragraph.to_ascii_lowercase();
        for quantifier in QUANTIFIERS {
            for at in word_starts(&lowered, quantifier) {
                let base = at + quantifier.len();
                let after = &lowered[base..];
                for (offset, word) in words_with_offsets(after).into_iter().take(2) {
                    if !DOCUMENT_PARTS.contains(&bare(word)) {
                        continue;
                    }
                    let cursor = base + offset + word.len();
                    let window: String = lowered[cursor..].chars().take(60).collect();
                    if THE_DOCUMENT.iter().any(|d| window.contains(d)) {
                        let end = boundary_at_or_below(&lowered, cursor + 60);
                        offenders.push(lowered[at..end].to_string());
                    }
                }
            }
        }
    }

    eprintln!(
        "paragraphs read: {scanned}; quantifying over the document itself: {}",
        offenders.len()
    );

    assert!(
        offenders.is_empty(),
        "a sentence here says something about every rule, section or claim in this document: \
         {offenders:#?}\n\
         \n  Nothing in this repository can check that, and the two false sentences this file \
         has carried were both of this shape. If the claim is worth making it is worth counting: \
         R-16 carries a tally the walk holds it to, and a count on the page is a counter-example \
         to the sentence beside it."
    );
}

/// The canon's prose with fenced blocks blanked out. A verification block is a
/// command, and a comment inside a command is not a claim the prose makes:
/// `# R-13: coverage has to be measured against the filled geometry` sits in
/// one. Counted both ways on 2026-09-16 - with the blocks 16 paragraphs state a
/// measured figure and six carry no provenance, without them 15 and five. That
/// shell comment is the whole difference.
fn canon_prose_only(canon: &str) -> String {
    let mut out = String::with_capacity(canon.len());
    let mut fenced = false;
    for line in canon.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
            out.push('\n');
            continue;
        }
        if fenced {
            out.push('\n');
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// **The dictionary is a product, not a list of sentences somebody met.** The
/// narrow form of this check read `measured on`, `counted on` and `this scan
/// sees`, and `bfeb92c` - the commit that introduced it - rewrote `the gap is
/// measured on the placed field` to `measured across` in the same diff. The
/// paragraph did not gain a date; it left the denominator. The check was born
/// green partly by its own hand, and a ceiling with nothing under it reports
/// that as progress.
const MEASURING_VERBS: &[&str] = &["measured", "counted", "re-counted"];
const MEASURING_PREPOSITIONS: &[&str] = &["on", "at", "across", "over", "against", "by", "in"];
/// Phrases that state a measurement without taking a preposition at all.
const MEASURING_PHRASES: &[&str] = &["this scan sees"];

/// **The denominator, and the reason this file holds one at all.** Measured
/// over twenty revisions of the canon, from `02b7f4a` to `67b4f87` - 279 lines
/// and 26 paragraphs added across them - the product dictionary answered 15
/// every single time. The narrow dictionary answered 8, then 7 from `bfeb92c`
/// on, and that fall was a paragraph escaping rather than a sentence going
/// away. Like `LINE_NUMBER_REFERENCES`, this number may only be lowered on
/// purpose, in a commit that says which paragraph went and why.
const STATING_FLOOR: usize = 15;

/// Paragraphs that read as measurements to the dictionary and are not.
///
/// The key is a literal run of text **from the flattened paragraph**, and that
/// distinction is the trap: `grep -cF` for each of these four against
/// `docs/ROUTING-CANON.md` returns 0, 0, 0, 0, because the canon is hard-wrapped
/// and every one of them breaks across a line. Match them against the raw file
/// and all four go stale on day one; the repair somebody reaches for then is a
/// shorter, looser key, which is the whole point of the list thrown away.
///
/// A key that no longer appears is an error, not a skip. Without that half the
/// entry outlives the paragraph it excused and the list quietly rots into a
/// blanket permission.
const NOT_A_MEASUREMENT: &[(&str, &str)] = &[
    (
        "Coverage has to be measured against the filled geometry",
        "says what the rule measures over and points at the code; states no figure of its own",
    ),
    (
        "a board that declares nothing is measured against 1 GHz",
        "1 GHz is the stand-in threshold the same paragraph defines, not a reading",
    ),
    (
        "the gap is measured across the placed field",
        "7.5 mm and 1 GHz are declared thresholds; the sentence explains where the gap comes from",
    ),
    (
        "its size is counted by the command in the verification block",
        "hands its provenance to the verification block and names the commit",
    ),
    (
        "clearance rows measured at 0.00 mm",
        // The word this row names is deliberately not written here: the
        // table's own figures are re-measured by a search over this tree,
        // and a term typed into this file is found by the search it checks.
        "the unit of the zero-distance row in R-12's table of what the score reads, not a reading",
    ),
];

#[test]
fn a_measured_figure_says_when_it_was_measured() {
    let canon = std::fs::read_to_string(repo_root().join("docs/ROUTING-CANON.md"))
        .expect("the canon is there");

    let measuring: Vec<String> = MEASURING_VERBS
        .iter()
        .flat_map(|verb| {
            MEASURING_PREPOSITIONS
                .iter()
                .map(move |preposition| format!("{verb} {preposition}"))
        })
        .chain(MEASURING_PHRASES.iter().map(|p| p.to_string()))
        .collect();

    let paragraphs = canon_paragraphs(&canon_prose_only(&canon));
    let known = names_in_this_tree();

    let mut stating = 0usize;
    let mut unsourced: Vec<String> = Vec::new();
    for paragraph in &paragraphs {
        let lowered = paragraph.to_ascii_lowercase();
        let states_a_measurement = measuring.iter().any(|m| contains_phrase(&lowered, m))
            && paragraph.chars().any(|c| c.is_ascii_digit());
        if !states_a_measurement {
            continue;
        }
        stating += 1;
        let dated = paragraph
            .split(|c: char| !(c.is_ascii_digit() || c == '-'))
            .any(is_iso_date);
        if !dated && !names_a_check(paragraph, &known) {
            unsourced.push(paragraph.clone());
        }
    }

    let stale: Vec<&str> = NOT_A_MEASUREMENT
        .iter()
        .map(|(key, _)| *key)
        .filter(|key| !paragraphs.iter().any(|p| p.contains(key)))
        .collect();

    let remaining: Vec<String> = unsourced
        .iter()
        .filter(|p| !NOT_A_MEASUREMENT.iter().any(|(key, _)| p.contains(key)))
        .map(|p| p[..p.len().min(140)].to_string())
        .collect();

    eprintln!(
        "paragraphs stating a measured figure: {stating} (floor {STATING_FLOOR}); \
         without a date or a check name: {}; excused: {}; still answering nothing: {}",
        unsourced.len(),
        NOT_A_MEASUREMENT.len() - stale.len(),
        remaining.len()
    );

    assert!(
        stale.is_empty(),
        "an entry here excuses a paragraph that is no longer in the canon: {stale:#?}\n\
         \n  The paragraph was rewritten or removed and its excuse outlived it. Re-read the \
         sentence as it stands now and either drop the entry or key it to the new text - an \
         excuse nobody re-reads is a permission that widens on its own."
    );

    assert!(
        remaining.is_empty(),
        "a figure here is called measured and nothing says when or by what: {remaining:#?}\n\
         \n  A measurement without a date is a claim about the working tree on a day nobody \
         recorded, and this file has already been caught seven times holding a figure that was \
         true when it was written. Either carry the ISO date it was read on, or name the check \
         that re-reads it."
    );

    assert!(
        stating >= STATING_FLOOR,
        "the scan for measured figures now sees {stating} paragraphs, below the floor of \
         {STATING_FLOOR}\n\
         \n  Paragraphs do not usually leave this scan because the canon got shorter - they \
         leave because a word was changed and the sentence stopped matching, which is exactly \
         how this check was born green. If the text really did go away, lower the floor in the \
         same commit and say which paragraph went."
    );
}

/// The directories a name cited in the canon may resolve into. `docs/` is not
/// among them on purpose: a name that resolves only in prose resolves to the
/// sentence that cited it, and the check would be reading its own input.
const SEARCHED: &[&str] = &["crates", "viewer/src", "viewer/e2e", "scripts"];

/// Directories that hold no source anybody wrote.
const NOT_SOURCE: &[&str] = &["target", "node_modules", "pkg", "dist", ".git"];

/// The prefixes that make a backticked span a path rather than a phrase.
const PATH_ROOTS: &[&str] = &[
    "crates/",
    "viewer/",
    "docs/",
    "tests/",
    "scripts/",
    "examples/",
];

/// Every extension this repository actually holds - 23 of them on 2026-09-16,
/// read off the tree rather than listed here, because a list of extensions is a
/// second place to keep the same fact.
fn extensions_in_this_tree() -> BTreeSet<String> {
    fn walk(dir: &Path, found: &mut BTreeSet<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let skip = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| NOT_SOURCE.contains(&n));
                if !skip {
                    walk(&path, found);
                }
                continue;
            }
            if let Some(extension) = path.extension().and_then(|e| e.to_str()) {
                found.insert(extension.to_ascii_lowercase());
            }
        }
    }
    let mut found = BTreeSet::new();
    walk(&repo_root(), &mut found);
    found
}

/// A backticked span that reads as a file **of a kind this repository holds**.
///
/// The extension is what separates a citation from everything else shaped like
/// one: `0.5` is a number, `0.1.0-beta` is a version, `params.density` is a
/// field and `sigcon.com/Pubs/news/3_7.htm` is a source on the open web. The
/// first draft asked only for a dot and letters after it and caught all four.
fn looks_like_a_file(token: &str, extensions: &BTreeSet<String>) -> bool {
    let Some((stem, extension)) = token.rsplit_once('.') else {
        return false;
    };
    let extension = extension.split(':').next().unwrap_or(extension);
    !stem.is_empty() && extensions.contains(extension)
}

/// Four names the canon asserts are **not** in the rules - the section on what
/// nothing measures says nothing keys on a bit rate or an edge rate. They are
/// the positive control this check would otherwise lack: a walk that had
/// stopped reading the tree would report every cited name resolved and these
/// four absent, and the two halves cannot both be satisfied by reading nothing.
/// They are looked for in the two rule crates rather than in the whole tree,
/// because this file names them itself.
const ASSERTED_ABSENT: &[&str] = &["bit_rate", "bitrate", "data_rate", "rise_time"];

/// Below these the walk has lost the file rather than found it clean. The canon
/// cites 67 paths and 48 long names today.
const PATHS_FLOOR: usize = 50;
const NAMES_FLOOR: usize = 40;

/// Every file under `dir` that is not build output: its stem into `stems`, and
/// its text onto `text` when it is source and is not `skip`.
///
/// `skip` is this file. Leaving it in cost the filename pass its whole reason to
/// exist: the comment justifying that pass named `sharp_entry_anatomy`, the one
/// test whose name appears in no other file, and the check then found the name
/// in its own justification. The stem still goes in - what is excluded is the
/// text, not the file.
fn read_source_tree(dir: &Path, stems: &mut BTreeSet<String>, text: &mut String, skip: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if NOT_SOURCE.contains(&name.as_str()) {
            continue;
        }
        if path.is_dir() {
            read_source_tree(&path, stems, text, skip);
            continue;
        }
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            stems.insert(stem.to_string());
        }
        let source = path != skip
            && matches!(
                path.extension().and_then(|e| e.to_str()),
                Some("rs" | "ts" | "tsx" | "js" | "sh" | "toml")
            );
        if source {
            if let Ok(body) = std::fs::read_to_string(&path) {
                text.push_str(&body);
                text.push('\n');
            }
        }
    }
}

/// The text between backticks, which is how this document cites anything.
fn backticked(canon: &str) -> Vec<&str> {
    canon.split('`').skip(1).step_by(2).collect()
}

fn is_long_name(token: &str) -> bool {
    token.matches('_').count() >= 2
        && token.starts_with(|c: char| c.is_ascii_lowercase())
        && token
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

#[test]
fn every_path_and_name_the_canon_cites_is_in_the_tree() {
    let root = repo_root();
    let canon =
        std::fs::read_to_string(root.join("docs/ROUTING-CANON.md")).expect("the canon is there");

    let itself = root.join(file!());
    let mut stems = BTreeSet::new();
    let mut text = String::new();
    for dir in SEARCHED {
        read_source_tree(&root.join(dir), &mut stems, &mut text, &itself);
    }

    // The checks this file defines are cited by name in the canon and exist
    // nowhere else. They resolve to a function declared here - not to a mention
    // of one, which is the difference between the two.
    let own = std::fs::read_to_string(&itself).expect("this test file is readable");
    let declared_here: BTreeSet<&str> = own
        .lines()
        .filter_map(|line| line.trim().strip_prefix("fn "))
        .filter_map(|rest| rest.split('(').next())
        .collect();

    // The control is read from the two crates a rule would be written in, not
    // from the whole tree. The first run of this check reported all four names
    // present because it had read the constant on the line above them, which is
    // the same defect as a document citing itself.
    let mut rule_stems = BTreeSet::new();
    let mut rule_text = String::new();
    for dir in ["crates/cypcb-drc/src", "crates/cypcb-rules/src"] {
        read_source_tree(&root.join(dir), &mut rule_stems, &mut rule_text, &itself);
    }

    let mut paths = 0usize;
    let mut with_a_line = 0usize;
    let mut globs = 0usize;
    let mut unrooted: Vec<String> = Vec::new();
    let extensions = extensions_in_this_tree();
    let mut missing_paths: Vec<String> = Vec::new();
    let mut names: BTreeSet<&str> = BTreeSet::new();
    for token in backticked(&canon) {
        if token.contains(char::is_whitespace) {
            continue;
        }
        if is_long_name(token) {
            names.insert(token);
            continue;
        }
        let rooted = PATH_ROOTS.iter().any(|r| token.starts_with(r));
        if !rooted {
            if looks_like_a_file(token, &extensions) {
                unrooted.push(token.to_string());
            }
            continue;
        }
        if token.contains('*') {
            // `crates/*/src` is prose about a search, not a path to a file.
            globs += 1;
            continue;
        }
        // A `:line` or `:from-to` suffix is held by the line-number ceiling
        // above; what this check asks is whether the file is still there.
        let file = token.split(':').next().unwrap_or(token);
        if file.len() != token.len() {
            with_a_line += 1;
        }
        paths += 1;
        if !root.join(file).exists() {
            missing_paths.push(token.to_string());
        }
    }

    // Three passes, because they answer three different questions and each one
    // carries names the other two miss. `sharp_entry_anatomy` is a test file
    // whose name appears inside no other file, so contents alone would call it
    // missing; five of the checks over this document are declared in this file
    // and named in no other, so a walk that skips this file to stay honest
    // would call them missing too.
    let mut unresolved: Vec<&str> = Vec::new();
    let mut by_filename = 0usize;
    let mut by_declaration = 0usize;
    for name in &names {
        if stems.contains(*name) {
            by_filename += 1;
        } else if declared_here.contains(*name) {
            by_declaration += 1;
        } else if !text.contains(*name) {
            unresolved.push(name);
        }
    }

    let present_but_asserted_absent: Vec<&str> = ASSERTED_ABSENT
        .iter()
        .copied()
        .filter(|n| rule_stems.contains(*n) || rule_text.contains(n))
        .collect();

    eprintln!(
        "paths cited: {paths} ({with_a_line} with a line number, {globs} globs skipped), \
         missing {}; long names cited: {} (by filename {by_filename}, declared here \
         {by_declaration}), unresolved {}; \
         names the canon says are absent: {} of {} found in the tree; \
         file names cited without a directory: {}",
        missing_paths.len(),
        names.len(),
        unresolved.len(),
        present_but_asserted_absent.len(),
        ASSERTED_ABSENT.len(),
        unrooted.len()
    );

    assert!(
        unrooted.is_empty(),
        "the canon cites a file by name with no directory in front of it: {unrooted:#?}\n\
         \n  A bare filename is out of reach of this check, which only reads a span that \
         starts at a directory, and `lib.rs` names 17 files in this tree. Three line numbers \
         drifted for weeks behind citations written that way. Spell the path out."
    );

    assert!(
        paths >= PATHS_FLOOR && names.len() >= NAMES_FLOOR,
        "this check read {paths} paths and {} names, below the floors of {PATHS_FLOOR} and \
         {NAMES_FLOOR}. Either the canon shrank by a third, or backticks stopped being how it \
         cites things and the two clean reports below are over nothing.",
        names.len()
    );
    assert!(
        missing_paths.is_empty(),
        "the canon points at a file that is not there: {missing_paths:#?}\n\
         \n  A file moved in the code leaves its old path in this document, where nothing \
         breaks and nobody looks - the same failure the rule-name check stops for symbols. The \
         line number after the colon is not what this asks about; the file is."
    );
    assert!(
        unresolved.is_empty(),
        "the canon names something that is in no file and in no filename: {unresolved:#?}\n\
         \n  Both passes ran: the name is not a file's own name under {SEARCHED:?}, and it is \
         not in the text of any source there. If it was renamed, the sentence citing it now \
         describes something that does not exist."
    );
    assert!(
        present_but_asserted_absent.is_empty(),
        "the canon says this workspace has no such thing and the workspace now does: \
         {present_but_asserted_absent:#?}\n\
         \n  These four are the control on the walk above, and they are also a claim in the \
         section on what nothing measures. If one of them has been implemented, that section is \
         wrong and the rule it excuses is now checkable."
    );
}

/// The section whose subject is this rule. A paragraph explaining what a claim
/// about the published world owes its reader states the pattern it describes,
/// so it matches it - the way a section about superlatives would have to use
/// one. One name, argued here, rather than a shape in the matcher: the rule can
/// be stated in one place and that place is known.
const THE_SECTION_THAT_STATES_THE_RULE: &str = "A claim that the world publishes nothing [S]";

/// The three parts of a claim that the world publishes nothing. All three have
/// to be in one sentence: a negation alone is half this document, and a noun
/// for somebody else's page without one is every source line in it.
const ABSENCE_NEGATIONS: &[&str] = &["no", "none", "nothing", "neither", "nobody"];
const SOMEBODY_ELSES_PUBLICATION: &[&str] = &[
    "source",
    "sources",
    "standard",
    "standards",
    "published",
    "publishes",
    "clause",
    "literature",
    "page",
    "pages",
    "vendor",
    "guide",
    "guidance",
];
const VERBS_OF_STATING: &[&str] = &[
    "states",
    "gives",
    "publishes",
    "specifies",
    "bounds",
    "covers",
    "found",
    "ranks",
    "supplies",
    "provides",
    "fixes",
];

/// Below these the walk has lost the file. It reads 944 sentences and matches
/// 15 of them today; a matcher that had stopped matching would report every
/// claim dated and be believed.
const SENTENCES_FLOOR: usize = 700;
const ABSENCE_CLAIMS_FLOOR: usize = 10;

/// The prose of the canon, paragraph by paragraph, with the section each one
/// sits under. Fenced code, indented commands and table rows are not prose and
/// carry no claims; a heading is a title rather than a claim.
fn prose_paragraphs(canon: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut section = String::from("(before the first heading)");
    let mut buffer: Vec<&str> = Vec::new();
    let mut fenced = false;
    let flush = |buffer: &mut Vec<&str>, section: &str, out: &mut Vec<(String, String)>| {
        if !buffer.is_empty() {
            out.push((section.to_string(), buffer.join(" ")));
            buffer.clear();
        }
    };
    for line in canon.lines() {
        if line.starts_with("```") {
            fenced = !fenced;
            flush(&mut buffer, &section, &mut out);
            continue;
        }
        if fenced || line.starts_with("    ") || line.trim_start().starts_with('|') {
            flush(&mut buffer, &section, &mut out);
            continue;
        }
        if let Some(rest) = line
            .strip_prefix("### ")
            .or_else(|| line.strip_prefix("## "))
        {
            flush(&mut buffer, &section, &mut out);
            section = rest.replace('`', "").trim().to_string();
            continue;
        }
        if line.trim().is_empty() {
            flush(&mut buffer, &section, &mut out);
        } else {
            buffer.push(line);
        }
    }
    flush(&mut buffer, &section, &mut out);
    out
}

/// A paragraph split where a full stop is followed by something that starts a
/// sentence in this document - a capital, a bold marker, a backtick or a tag.
fn sentences(paragraph: &str) -> Vec<&str> {
    let flat = paragraph;
    let bytes = flat.as_bytes();
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut index = 0usize;
    while index + 2 < bytes.len() {
        let ends = matches!(bytes[index], b'.' | b'!' | b'?') && bytes[index + 1] == b' ';
        if ends {
            let next = bytes[index + 2];
            if next.is_ascii_uppercase() || matches!(next, b'*' | b'`' | b'[') {
                out.push(flat[start..=index].trim());
                start = index + 2;
            }
        }
        index += 1;
    }
    if start < flat.len() {
        out.push(flat[start..].trim());
    }
    out
}

fn any_word(haystack: &str, words: &[&str]) -> bool {
    words.iter().any(|w| contains_phrase(haystack, w))
}

fn carries_a_date(text: &str) -> bool {
    text.split(|c: char| !(c.is_ascii_digit() || c == '-'))
        .any(is_iso_date)
}

#[test]
fn a_claim_that_nothing_is_published_says_when_somebody_looked() {
    let canon = std::fs::read_to_string(repo_root().join("docs/ROUTING-CANON.md"))
        .expect("the canon is there");

    let mut read = 0usize;
    let mut claims = 0usize;
    let mut undated: Vec<String> = Vec::new();
    for (section, paragraph) in prose_paragraphs(&canon) {
        let flat = paragraph.split_whitespace().collect::<Vec<_>>().join(" ");
        for sentence in sentences(&flat) {
            read += 1;
            if section == THE_SECTION_THAT_STATES_THE_RULE {
                continue;
            }
            let lowered = sentence.to_ascii_lowercase();
            let is_a_claim = any_word(&lowered, ABSENCE_NEGATIONS)
                && any_word(&lowered, SOMEBODY_ELSES_PUBLICATION)
                && any_word(&lowered, VERBS_OF_STATING);
            if !is_a_claim {
                continue;
            }
            claims += 1;
            if !carries_a_date(sentence) && !carries_a_date(&flat) {
                undated.push(format!(
                    "{section}: {}",
                    &sentence[..sentence.len().min(140)]
                ));
            }
        }
    }

    eprintln!(
        "sentences read: {read}; claiming the world publishes nothing: {claims}; \
         without a date in the sentence or its paragraph: {}",
        undated.len()
    );

    assert!(
        read >= SENTENCES_FLOOR && claims >= ABSENCE_CLAIMS_FLOOR,
        "this walk read {read} sentences and matched {claims} claims, below the floors of \
         {SENTENCES_FLOOR} and {ABSENCE_CLAIMS_FLOOR}. A matcher that has stopped matching \
         reports every claim dated, which is the same clean answer as a document with nothing \
         wrong in it."
    );
    assert!(
        undated.is_empty(),
        "a sentence here says the world publishes nothing and does not say when somebody \
         looked: {undated:#?}\n\
         \n  That is evidence about a search rather than about copper or about this \
         repository, and nothing in this checkout can settle it: no walk can ask the world \
         whether a page exists. The date is the whole of the evidence, in the sentence or in \
         its paragraph. If no search was made, the claim is not a finding and the clause comes \
         out."
    );
}

/// Below this the extraction has stopped finding hashes rather than found them
/// all good. The canon cites fourteen today.
const HASHES_FLOOR: usize = 10;

/// A backticked token that is an abbreviated object name: hex, long enough to
/// be one, and not a plain number - `0254` is a clearance in hundredths of a
/// millimetre and appears in this document more often than any commit does.
fn looks_like_a_commit(token: &str) -> bool {
    (7..=40).contains(&token.len())
        && token
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
        && token.chars().any(|c| !c.is_ascii_digit())
}

#[test]
fn every_commit_the_canon_cites_is_a_commit_in_this_repository() {
    let root = repo_root();
    let canon =
        std::fs::read_to_string(root.join("docs/ROUTING-CANON.md")).expect("the canon is there");

    let cited: BTreeSet<&str> = backticked(&canon)
        .into_iter()
        .filter(|t| looks_like_a_commit(t))
        .collect();

    let mut unresolved: Vec<&str> = Vec::new();
    for hash in &cited {
        let output = Command::new("git")
            .arg("-C")
            .arg(&root)
            .arg("rev-parse")
            .arg("--verify")
            .arg("--quiet")
            .arg(format!("{hash}^{{commit}}"))
            .output()
            .expect("git runs");
        if !output.status.success() {
            unresolved.push(hash);
        }
    }

    eprintln!(
        "commits cited by the canon: {}; not a commit in this repository: {}",
        cited.len(),
        unresolved.len()
    );

    assert!(
        cited.len() >= HASHES_FLOOR,
        "this check found {} cited commits and expected at least {HASHES_FLOOR}. Either the \
         references were taken out, or backticks stopped being how they are written and the \
         clean report below is over nothing.",
        cited.len()
    );
    assert!(
        unresolved.is_empty(),
        "the canon cites something that is not a commit here: {unresolved:#?}\n\
         \n  A hash is the reference this document uses for its own history, and it is the one \
         kind that breaks in silence: a rebase, a squash or a cherry-pick leaves the sentence \
         reading perfectly well and pointing at nothing. If the history moved, the sentence \
         follows it; if the event was never recorded, the sentence says what happened without \
         a figure nobody can reproduce."
    );
}

/// Below this the extraction has stopped reading the sentence rather than found
/// its list clean. One sentence carries six names today.
const NAMES_SEARCHED_FOR_FLOOR: usize = 5;

#[test]
fn a_recorded_search_of_the_tree_still_returns_what_it_says() {
    let root = repo_root();
    let canon =
        std::fs::read_to_string(root.join("docs/ROUTING-CANON.md")).expect("the canon is there");

    // A sentence of the form: no hits for "a", "b" or "c", with the scope it
    // searched in backticks earlier in the same paragraph.
    let mut searched: Vec<(String, String)> = Vec::new();
    for (_, paragraph) in prose_paragraphs(&canon) {
        let flat = paragraph.split_whitespace().collect::<Vec<_>>().join(" ");
        let Some(at) = flat.find("no hits for") else {
            continue;
        };
        let scope = flat[..at]
            .split('`')
            .rev()
            .nth(1)
            .unwrap_or("crates")
            .to_string();
        // Only the sentence that records the search. The paragraph around it
        // may quote one of the same names again to say what changed since -
        // reading on to the end of the paragraph put `antipad` back into the
        // list the moment the sentence was corrected for it.
        let rest = &flat[at..];
        let end = rest.find("\". ").map(|i| i + 1).unwrap_or(rest.len());
        for name in rest[..end].split('"').skip(1).step_by(2) {
            searched.push((scope.clone(), name.to_string()));
        }
    }

    assert!(
        searched.len() >= NAMES_SEARCHED_FOR_FLOOR,
        "this check read {} names out of the canon's recorded searches and expected at least \
         {NAMES_SEARCHED_FOR_FLOOR}. Either the sentence was rewritten into another shape, or \
         the extraction is reading nothing and every search below passed by not happening.",
        searched.len()
    );

    let mut answering: Vec<String> = Vec::new();
    for (scope, name) in &searched {
        let output = Command::new("sh")
            .arg("-c")
            .arg(format!(
                "grep -rlF --include=*.rs -- \"$1\" {scope} 2>/dev/null",
            ))
            .arg("sh")
            .arg(name)
            .current_dir(&root)
            .output()
            .expect("grep runs");
        let hits = String::from_utf8_lossy(&output.stdout);
        let hits: Vec<&str> = hits.lines().collect();
        if !hits.is_empty() {
            answering.push(format!("{name:?} in {scope}: {hits:?}"));
        }
    }

    eprintln!(
        "names the canon records as absent from the tree: {}; answering today: {}",
        searched.len(),
        answering.len()
    );

    assert!(
        answering.is_empty(),
        "the canon records a search of this tree that no longer returns what it says: \
         {answering:#?}\n\
         \n  A recorded grep result is a claim about the tree on the day it ran, and it is the \
         only kind this document can re-run rather than re-read. The word arriving in the tree \
         does not by itself make the section wrong - a comment naming a property is not a \
         measurement of it - but the sentence stating the count is, and it says what it found."
    );
}

/// The three tables R-17 quotes, and what each row is computed from.
///
/// A pair of figures copied out of a preset is the cheapest kind of rot: the
/// preset moves in another crate, nothing here fails, and the table goes on
/// describing a grid the router no longer builds. So the row is not compared
/// against a stored expectation - it is **built** from the preset and the
/// arithmetic the section states, and the canon has to contain the result.
const SNAP_TABLE: &[(&str, cypcb_rules::presets::RulesPreset)] = &[
    (
        "JLCPCB standard, 2 layer",
        cypcb_rules::presets::RulesPreset::JlcpcbStandard2Layer,
    ),
    (
        "JLCPCB standard, 4 layer",
        cypcb_rules::presets::RulesPreset::JlcpcbStandard4Layer,
    ),
    (
        "JLCPCB advanced, 4 layer",
        cypcb_rules::presets::RulesPreset::JlcpcbAdvanced4Layer,
    ),
];

#[test]
fn the_snap_table_is_recomputed_from_the_presets_it_quotes() {
    let canon = std::fs::read_to_string(repo_root().join("docs/ROUTING-CANON.md"))
        .expect("the canon is there");

    let mut built = Vec::new();
    let mut missing = Vec::new();
    for (label, preset) in SNAP_TABLE {
        let constraints = preset.constraints();
        let width = constraints.min_trace_width.to_mm();
        let clearance = constraints.min_clearance.to_mm();
        let pitch = width + clearance;
        // The section states the worst case as `resolution * sqrt(2)`, because
        // the snap truncates on each axis rather than rounding.
        let snap = pitch * std::f64::consts::SQRT_2;
        let row =
            format!("| {label} ({width:.3} + {clearance:.3}) | {pitch:.3} mm | {snap:.3} mm |");
        built.push(row.clone());
        if !canon.contains(&row) {
            missing.push(row);
        }
    }

    eprintln!(
        "snap rows rebuilt from the presets: {}; not in the canon: {}",
        built.len(),
        missing.len()
    );

    assert_eq!(
        built.len(),
        SNAP_TABLE.len(),
        "one row per preset, and the table has {} of them",
        SNAP_TABLE.len()
    );
    assert!(
        missing.is_empty(),
        "R-17's table quotes a grid these presets no longer describe. Rebuilt from the presets \
         and the section's own arithmetic, these rows are not in the file: {missing:#?}\n\
         \n  Every figure in them comes from `min_trace_width` and `min_clearance` in \
         `cypcb-rules`, which is another crate: it can move without anything here failing, and \
         then this table describes a grid the router does not build. Copy the rows above into \
         the section, or say why the arithmetic changed."
    );
}

/// A number this document copied out of the code, and the line it came from.
///
/// The snap table above could be rebuilt, because R-17 states the arithmetic
/// beside it. These cannot: each is one number in one sentence with no formula
/// to recompute, so the pair is the instrument. **Both halves are load-bearing.**
/// If the constant moves, `in_code` stops matching and the row fails; if the
/// sentence is reworded, `in_canon` stops matching and whoever reworded it has
/// to come back and say which constant the new words are about.
struct Copied {
    what: &'static str,
    file: &'static str,
    in_code: &'static str,
    in_canon: &'static str,
    /// Digits that must appear in both needles. `None` where the two are in
    /// different units - the tree counts nanometres and the prose reads
    /// microns or millimetres - and there the pairing is held by the two
    /// needles alone.
    shared: Option<&'static str>,
}

const COPIED_FIGURES: &[Copied] = &[
    Copied {
        what: "the entry angle a land refuses below, in the unit it is kept in",
        file: "crates/cypcb-drc/src/rules/pad_entry.rs",
        in_code: "pub const ENTRY_ANGLE_MIN_MDEG: u32 = 45_000;",
        in_canon: "`ENTRY_ANGLE_MIN_MDEG` is 45_000 millidegrees",
        shared: Some("45_000"),
    },
    Copied {
        what: "the relief a pour cuts, which the shipped presets happen to match",
        file: "crates/cypcb-core/src/pour.rs",
        in_code: "thermal_gap: Nm::from_mm(0.254)",
        in_canon: "publish 0.254 mm for gap and for spoke width",
        shared: Some("0.254"),
    },
    Copied {
        what: "the via a stitching field is made of",
        file: "crates/cypcb-world/src/stitch.rs",
        in_code: "diameter: Nm::from_mm(0.6)",
        in_canon: "0.3 mm hole in a 0.6 mm pad",
        shared: Some("0.6"),
    },
    Copied {
        what: "the floor under a routing grid",
        file: "crates/cypcb-autoroute/src/lib.rs",
        in_code: "pitch.max(10_000)",
        in_canon: "floored at 10 um",
        shared: None,
    },
    Copied {
        what: "the board size that coarsens the grid",
        file: "crates/cypcb-autoroute/src/lib.rs",
        in_code: "let threshold_nm: i64 = 80_000_000;",
        in_canon: "wider or taller than 80 mm is coarsened by 2",
        shared: None,
    },
    Copied {
        what: "the board size that coarsens it again",
        file: "crates/cypcb-autoroute/src/lib.rs",
        in_code: "if max_dim > 200_000_000 { 3 } else { 2 }",
        in_canon: "above 200 mm by 3",
        shared: None,
    },
    Copied {
        what: "how far a caller may scale the grid",
        file: "crates/cypcb-autoroute/src/lib.rs",
        in_code: "self.params.density.clamp(0.5, 2.0)",
        in_canon: "clamped to 0.5 to 2.0",
        shared: Some("0.5"),
    },
    Copied {
        what: "the chord tolerance an arc is flattened to",
        file: "crates/cypcb-world/src/arc.rs",
        in_code: "pub const DEFAULT_TOLERANCE: Nm = Nm(10_000);",
        in_canon: "a default tolerance of 10 microns",
        shared: None,
    },
    Copied {
        what: "what the score charges for a violation row",
        file: "crates/cypcb-autoroute/src/scoring.rs",
        in_code: "* drc_violations as f64 * 1000.0",
        in_canon: "the 1000 on a violation row",
        shared: Some("1000"),
    },
    Copied {
        what: "what the score charges for a crossing",
        file: "crates/cypcb-autoroute/src/scoring.rs",
        in_code: "* crossings as f64 * 500.0",
        in_canon: "the 500 on a crossing",
        shared: Some("500"),
    },
    Copied {
        what: "the middle rung of the IPC clearance ladder",
        file: "crates/cypcb-rules/src/presets/ipc.rs",
        in_code: "min_clearance: Nm::from_mm(0.15)",
        in_canon: "0.2 / 0.15 / 0.1 mm figures",
        shared: Some("0.15"),
    },
];

/// Whitespace flattened, because both the canon and the source are hard-wrapped
/// and a needle that crosses a wrap is a needle nothing finds.
fn flattened(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn a_number_copied_out_of_the_code_still_matches_the_line_it_came_from() {
    let root = repo_root();
    let canon = flattened(
        &std::fs::read_to_string(root.join("docs/ROUTING-CANON.md")).expect("the canon is there"),
    );

    let mut broken: Vec<String> = Vec::new();
    let mut paired = 0usize;
    for row in COPIED_FIGURES {
        let source = flattened(
            &std::fs::read_to_string(root.join(row.file))
                .unwrap_or_else(|_| panic!("{} is there", row.file)),
        );
        let in_code = flattened(row.in_code);
        let in_canon = flattened(row.in_canon);
        if !source.contains(&in_code) {
            broken.push(format!(
                "{}: {} is no longer in {}",
                row.what, row.in_code, row.file
            ));
            continue;
        }
        if !canon.contains(&in_canon) {
            broken.push(format!(
                "{}: the canon no longer says {:?}",
                row.what, row.in_canon
            ));
            continue;
        }
        if let Some(digits) = row.shared {
            if !(in_code.contains(digits) && in_canon.contains(digits)) {
                broken.push(format!(
                    "{}: {digits} is not in both halves of this pair, so the row pairs two \
                     different numbers",
                    row.what
                ));
                continue;
            }
        }
        paired += 1;
    }

    eprintln!(
        "figures the canon copies out of the code: {}; still matching the line they came from: {paired}",
        COPIED_FIGURES.len()
    );

    assert!(
        broken.is_empty(),
        "a number in this document no longer matches the code it was copied from: {broken:#?}\n\
         \n  These are the figures with no formula beside them, so nothing can recompute them - \
         the pair of needles is the instrument. A constant that moved leaves the prose reading \
         perfectly well and describing a tool that no longer behaves that way; a sentence \
         reworded without the constant in front of it leaves this row pairing nothing."
    );
}

/// The header of the table this check re-runs. Found by its header rather than
/// by the shape of a row: this document has three other tables, and a match on
/// row shape alone either takes them too or misses this one.
const RECORDED_TABLE_HEADER: &str =
    "| the phrase a reader would search | files | the name the code uses | files |";

/// Below this the reader has stopped recognising the rows rather than found the
/// table shortened. Without it the check passes hardest exactly when it has
/// stopped finding anything: rename the header and it reads zero rows, makes
/// zero comparisons and reports green.
const RECORDED_TABLE_ROWS_FLOOR: usize = 6;

/// The numbers a sentence can spell instead of writing.
const SPELLED: &[(&str, usize)] = &[
    ("no", 0),
    ("none", 0),
    ("one", 1),
    ("two", 2),
    ("three", 3),
    ("four", 4),
    ("five", 5),
    ("six", 6),
    ("seven", 7),
    ("eight", 8),
];

/// A table of counts, re-run rather than read.
///
/// **Nothing this check looks for is written here.** Every string it searches
/// for comes out of the document at run time, and the reason is the search
/// itself: a term spelled in this file would be found by the command it is
/// checking, and the figure it moved would be the one it exists to hold. Three
/// checks in this repository have had that defect, and all three had the same
/// shape - the comment explaining the check named the thing the check looked
/// for. So the comments here describe the shape of a row and never its content.
///
/// **What is asserted and what is only reported are different questions.** The
/// claim the table makes is which side of a pair is empty: a zero that stopped
/// being a zero is a claim coming apart, and 51 that became 52 is a tree
/// growing. Holding twelve magnitudes as equalities would fail on work that has
/// nothing to do with this section, and a check that fails for the wrong reason
/// three times is a check somebody weakens. So the zeros are a gate, the
/// magnitudes are a drift line, and the sentence above the table is a gate
/// because it is the number a reader acts on.
///
/// The scope comes from the command the paragraph prints. It is not executed -
/// the terms are passed as arguments, the way the neighbouring check does it -
/// because the document says what to search for and where, not what to run.
#[test]
fn the_table_of_searches_is_re_run_rather_than_read() {
    let root = repo_root();
    let canon =
        std::fs::read_to_string(root.join("docs/ROUTING-CANON.md")).expect("the canon is there");
    let lines: Vec<&str> = canon.lines().collect();

    let header_at = lines
        .iter()
        .position(|line| line.trim() == RECORDED_TABLE_HEADER)
        .expect("the table this check re-runs is found by its header row");

    let command = lines[..header_at]
        .iter()
        .rev()
        .find_map(|line| {
            line.split('`')
                .skip(1)
                .step_by(2)
                .find(|span| span.starts_with("grep -ril") && span.contains("--include="))
        })
        .expect(
            "the paragraph above the table prints the command it ran, in backticks, and this \
             check takes its scope from there. A table of counts with no command beside it is \
             unreproducible by construction, and a scope kept here instead would be the copy \
             that goes stale while the section moves.",
        );
    let include = command
        .split_whitespace()
        .find(|token| token.starts_with("--include="))
        .expect("the command names what it searched");
    let scope = command
        .split_whitespace()
        .last()
        .expect("the command names where it searched");

    struct Row {
        phrase: String,
        phrase_says: usize,
        name: String,
        name_says: usize,
    }

    let mut rows: Vec<Row> = Vec::new();
    let mut malformed: Vec<String> = Vec::new();
    for line in lines[header_at + 1..]
        .iter()
        .map(|line| line.trim())
        .skip_while(|line| {
            line.starts_with("|-") || line.starts_with("|:") || line.starts_with("|-")
        })
        .take_while(|line| line.starts_with('|'))
    {
        let cells: Vec<&str> = line
            .trim_matches('|')
            .split('|')
            .map(|cell| cell.trim())
            .collect();
        let parsed = if cells.len() == 4 {
            match (cells[1].parse::<usize>(), cells[3].parse::<usize>()) {
                (Ok(phrase_says), Ok(name_says))
                    if cells[0].starts_with('"')
                        && cells[0].ends_with('"')
                        && cells[2].starts_with('`')
                        && cells[2].ends_with('`') =>
                {
                    Some(Row {
                        phrase: cells[0].trim_matches('"').to_string(),
                        phrase_says,
                        name: cells[2].trim_matches('`').to_string(),
                        name_says,
                    })
                }
                _ => None,
            }
        } else {
            None
        };
        // A row in this table that does not parse is an error and not a skip.
        // Skipping it would let a typo remove a row from the measurement in
        // silence, which is the same defect as a search that misses its term.
        match parsed {
            Some(row) => rows.push(row),
            None => malformed.push(line.to_string()),
        }
    }

    assert!(
        malformed.is_empty(),
        "a row of this table is not in the shape the rest of it uses: {malformed:#?}\n\
         \n  Four cells: a quoted term, a count, a name in backticks, a count. A row that does \
         not parse is not measured, and a row nobody measures is a figure nobody checks."
    );
    assert!(
        rows.len() >= RECORDED_TABLE_ROWS_FLOOR,
        "this check read {} rows of the table and expected at least {RECORDED_TABLE_ROWS_FLOOR}. \
         Either the table was rewritten into another shape, or the reader is finding nothing and \
         every figure below agreed by not being looked at.",
        rows.len()
    );

    let files_naming = |term: &str| -> Vec<String> {
        let output = Command::new("sh")
            .arg("-c")
            .arg(format!(
                "grep -rilF {include} -- \"$1\" {scope} 2>/dev/null"
            ))
            .arg("sh")
            .arg(term)
            .current_dir(&root)
            .output()
            .expect("grep runs");
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(|line| line.to_string())
            .collect()
    };

    let mut broken: Vec<String> = Vec::new();
    let mut drift: Vec<String> = Vec::new();
    let mut found_itself: Vec<String> = Vec::new();
    let mut measured_silent = 0usize;

    for row in &rows {
        for (what, term, says) in [
            ("the phrase of", &row.phrase, row.phrase_says),
            ("the name", &row.name, row.name_says),
        ] {
            let hits = files_naming(term);
            // This file is inside the scope, so a term written into it would be
            // counted by the search it belongs to.
            if hits.iter().any(|hit| hit.ends_with(file!())) {
                found_itself.push(format!("{what} the row for {:?}", row.name));
            }
            let now = hits.len();
            match (says, now) {
                (0, 0) => {}
                (0, _) => broken.push(format!(
                    "{what} the row for {:?}: the table records nothing in the tree, the search \
                     answers {now} file(s)",
                    row.name
                )),
                (_, 0) => broken.push(format!(
                    "{what} the row for {:?}: the table records {says} file(s), the search \
                     answers nothing",
                    row.name
                )),
                _ if says != now => drift.push(format!(
                    "{what} the row for {:?}: {says} in the canon, {now} today - non-zero either \
                     way, so the claim holds and the figure has moved",
                    row.name
                )),
                _ => {}
            }
        }
        if row.phrase_says == 0 {
            measured_silent += 1;
        }
    }

    // Re-count from the tree rather than from the table, so the sentence is
    // checked against what was measured and not against what was written.
    let silent_now = rows
        .iter()
        .filter(|row| files_naming(&row.phrase).is_empty())
        .count();

    let flat = canon.split_whitespace().collect::<Vec<_>>().join(" ");
    let marker = "phrases return nothing";
    let at = flat
        .find(marker)
        .expect("the sentence under the table says how many of the terms answered nothing");
    let before: Vec<String> = flat[..at]
        .split_whitespace()
        .rev()
        .take(4)
        .map(|word| word.trim_matches('*').to_ascii_lowercase())
        .collect();
    let spelled = |word: &str| SPELLED.iter().find(|(w, _)| *w == word).map(|(_, n)| *n);
    let sentence_silent = spelled(&before[3]);
    let sentence_total = spelled(&before[0]);

    eprintln!(
        "recorded-search table rows read: {}; figures re-measured: {}; claims broken: {}; \
         drifted: {}; silent in the tree: {silent_now}, in the table: {measured_silent}, in the \
         sentence: {sentence_silent:?} of {sentence_total:?}",
        rows.len(),
        rows.len() * 2,
        broken.len(),
        drift.len(),
    );
    for line in &drift {
        eprintln!("  drift: {line}");
    }

    assert!(
        found_itself.is_empty(),
        "this check found its own source file in the results of a search it is checking: \
         {found_itself:#?}\n\
         \n  The term has been written into this file, so the count it holds now includes the \
         holder. Read the term from the document instead of spelling it here."
    );
    assert!(
        broken.is_empty(),
        "the table records which side of a pair the tree is silent on, and that has changed: \
         {broken:#?}\n\
         \n  A zero here is the claim: this document's own word for a quantity finds nothing \
         while the code's name for it finds something. A zero that stopped being zero, or a \
         figure that became one, is that claim coming apart rather than a number drifting."
    );
    assert_eq!(
        sentence_silent,
        Some(silent_now),
        "the sentence under the table and the tree do not agree on how many terms answer \
         nothing: the searches say {silent_now}. A document that says one number in prose and \
         another in a row is wrong in the half a reader believes, and the prose is the half they \
         read."
    );
    assert_eq!(
        sentence_total,
        Some(rows.len()),
        "the sentence under the table names a number of terms the table does not have: it has \
         {} rows.",
        rows.len()
    );
}

/// Language variants rather than names from this tree. `Some(skew)` is cited
/// beside `crates/cypcb-drc/src/rules/diff_pair.rs:106`, and the sentence is
/// true: line 106 opens the call and the `Some` is on line 116, inside it. A
/// check reading the cited line literally would fail a sentence that is right,
/// so these four are not identifiers for this purpose.
const NOT_A_NAME_IN_THIS_TREE: &[&str] = &["Some", "None", "Ok", "Err"];

/// **The pairs this check re-reads, and why there is a floor under them.** The
/// canon cites 42 places as `path:line` and `LINE_NUMBER_REFERENCES` counts
/// them; until this check nothing read a single one. Five had drifted by exactly
/// five lines while the sections holding them carried `*Verified:*` stamps of
/// 2026-09-13 and 2026-09-14 - **a date says somebody looked, not that the
/// number is true.** Three of those five were written `lib.rs:160` rather than
/// with a path, so this check could not see them at all until the citations were
/// spelled out; a short citation is not only harder to follow, it is out of
/// reach of the thing that would have caught it. 20 of the 42 name an identifier
/// next to the citation and are re-read here; the rest cite a place without
/// naming what is at it.
const LINE_NUMBER_PAIRS_FLOOR: usize = 20;

/// The gap between the name and the citation that opens it: `name` (`path:line`).
/// Kept short and parenthesis-free on purpose - a longer reach starts pairing a
/// name with the next citation in the sentence rather than its own. A looser
/// version of this rule produced 25 pairs and three of them were artefacts.
const NAME_TO_CITATION_GAP: usize = 70;

/// A `path:line` citation, split into the file and the first line it names.
/// `oshpark.rs:29-30` cites a pair of lines and the first one is read.
fn path_and_line(token: &str) -> Option<(&str, usize)> {
    let (path, rest) = token.rsplit_once(':')?;
    if !path.contains('/') || !path.ends_with(".rs") {
        return None;
    }
    let first = rest.split('-').next()?;
    let line: usize = first.parse().ok()?;
    if line == 0 {
        return None;
    }
    Some((path, line))
}

/// A name this tree could hold: letters, digits and underscores, at least one
/// letter, and no dot - `0.1524` and `R-08` are not names.
fn looks_like_a_name(token: &str) -> bool {
    !token.is_empty()
        && token.chars().any(|c| c.is_ascii_alphabetic())
        && token.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !NOT_A_NAME_IN_THIS_TREE.contains(&token)
}

#[test]
fn a_line_number_beside_a_name_still_points_at_that_name() {
    let root = repo_root();
    let canon =
        std::fs::read_to_string(root.join("docs/ROUTING-CANON.md")).expect("the canon is there");

    let mut pairs: Vec<(String, String, usize)> = Vec::new();
    for paragraph in canon_paragraphs(&canon) {
        let parts: Vec<&str> = paragraph.split('`').collect();
        // Odd indices are backticked; between two of them sits one plain run.
        for index in (1..parts.len()).step_by(2) {
            if !looks_like_a_name(parts[index]) {
                continue;
            }
            let Some(gap) = parts.get(index + 1) else {
                continue;
            };
            if gap.len() > NAME_TO_CITATION_GAP
                || !gap.trim_end().ends_with('(')
                || gap.matches('(').count() != 1
                || gap.contains(')')
            {
                continue;
            }
            let Some(citation) = parts.get(index + 2) else {
                continue;
            };
            // The citation has to close its own parenthesis. R-11 names three
            // presets and then cites three lines - "`IpcClass1`, `IpcClass2` or
            // `IpcClass3` (`...mod.rs:58`, `:60`, `:62`)" - where the nearest
            // name before the bracket belongs to the last of them and the first
            // citation to the first. A list of names followed by a list of
            // places is not a pair, and reading it as one produced two hits
            // that were both the check being wrong.
            if !parts
                .get(index + 3)
                .is_some_and(|after| after.starts_with(')'))
            {
                continue;
            }
            if let Some((path, line)) = path_and_line(citation) {
                pairs.push((path.to_string(), parts[index].to_string(), line));
            }
        }
    }

    let mut missing: Vec<String> = Vec::new();
    for (path, name, line) in &pairs {
        let Ok(source) = std::fs::read_to_string(root.join(path)) else {
            missing.push(format!(
                "{path}:{line} - the file the canon cites is not there"
            ));
            continue;
        };
        match source.lines().nth(line - 1) {
            None => missing.push(format!(
                "{path}:{line} `{name}` - the file stops at line {}",
                source.lines().count()
            )),
            Some(text) if !text.contains(name.as_str()) => missing.push(format!(
                "{path}:{line} `{name}` - that line reads: {}",
                text.trim()
            )),
            Some(_) => {}
        }
    }

    eprintln!(
        "line numbers cited beside a name: {} (floor {LINE_NUMBER_PAIRS_FLOOR}); \
         not on the line the canon gives: {}",
        pairs.len(),
        missing.len()
    );

    assert!(
        missing.is_empty(),
        "the canon sends a reader to a line that no longer holds the name: {missing:#?}\n\
         \n  A line number drifts every time something above it is edited, and the section's \
         own *Verified:* stamp does not catch it - four of these were five lines stale under \
         two stamps read after the shift. Re-read the citation, or cite the name without a line."
    );

    assert!(
        pairs.len() >= LINE_NUMBER_PAIRS_FLOOR,
        "this check now re-reads {} citations, below the floor of {LINE_NUMBER_PAIRS_FLOOR}\n\
         \n  Pairs leave this scan when a citation loses its name or a name loses its full \
         path, and both read as tidying. If a citation really did go, lower the floor in the \
         same commit and say which one.",
        pairs.len()
    );
}

/// The programs this canon records a command for. A backticked span whose first
/// word is one of these and which carries an argument is a command; anything
/// else in backticks is a name, a path or a phrase.
const RECORDED_TOOLS: &[&str] = &[
    "grep", "rg", "cargo", "git", "wc", "find", "sed", "awk", "ls", "python3", "jq", "npx", "cat",
];

/// Commands this check records but does not run. A `cargo test` belongs to the
/// quality gate, which already runs it; running it again from inside a test
/// over prose would build the workspace to re-read a sentence.
const NOT_RUN_HERE: &[&str] = &["cargo"];

/// Rust's primitive type names, which are the one thing that looks like a
/// placeholder and is not: `Vec<String>` is a type in a pattern somebody
/// greps for, `<term>` is a hole the reader has to fill.
const PRIMITIVE_TYPES: &[&str] = &[
    "u8", "u16", "u32", "u64", "u128", "i8", "i16", "i32", "i64", "i128", "usize", "isize", "f32",
    "f64", "bool", "char", "str",
];

/// Words that stand between a command and the figure it answers with. The set
/// is closed on purpose: the first word outside it that is not a number ends
/// the search, so a sentence that wanders off into prose is read as pointing at
/// nothing rather than at the next number that happens along.
const LINKING: &[&str] = &[
    "answers", "gives", "returns", "says", "reports", "gave", "is", "was", "for", "the", "and",
    "to", "at", "over", "against", "of", "in", "on", "it", "that", "which", "a", "an", "this",
];

/// Numbers this canon writes as words. None of the twenty is needed today -
/// every figure a recorded command points at is written in digits - and they
/// are here because the day one is written out, a check without them would let
/// that command **leave the class in silence** rather than fail.
const NUMERALS: &[&str] = &[
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "ten",
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen",
    "twenty",
];

const COMMANDS_EXAMINED_FLOOR: usize = 12;
const COMMANDS_POINTING_FLOOR: usize = 8;

/// A `<hole>` a reader has to fill. The angle brackets alone are not the test -
/// `grep -rn "Vec<String>"` carries a type, not a hole - so the word inside has
/// to start lowercase and not be one of Rust's primitives.
fn carries_a_placeholder(command: &str) -> bool {
    let mut rest = command;
    while let Some(open) = rest.find('<') {
        rest = &rest[open + 1..];
        let Some(close) = rest.find('>') else {
            return false;
        };
        let inside = &rest[..close];
        if inside.starts_with(|c: char| c.is_ascii_lowercase())
            && !inside.is_empty()
            && !PRIMITIVE_TYPES.contains(&inside)
        {
            return true;
        }
    }
    false
}

/// A command with nothing to act on: every word is the tool, an option, or a
/// shell connector. `grep -n` in a sentence about line numbers is prose about
/// an option rather than a command anybody could run.
fn is_a_fragment(command: &str) -> bool {
    !command.split_whitespace().skip(1).any(|word| {
        !word.starts_with('-')
            && !RECORDED_TOOLS.contains(&word)
            && !matches!(word, "|" | "&&" | ";")
    })
}

/// The figure a sentence says a command answers with, taken from the words
/// after it. A seven-character hex token is skipped because this canon states
/// what a figure **used to be** at a commit, and that historical value sits
/// exactly where the current one would: the first pass read `29 at 4ff318c`
/// and pointed at 29 for a command that answers 30.
fn figure_pointed_at(after: &str) -> Option<String> {
    let words: Vec<&str> = after.split_whitespace().take(14).collect();
    for (at, word) in words.iter().enumerate() {
        let bare = word.trim_matches(|c: char| !c.is_ascii_alphanumeric());
        if bare.is_empty() {
            continue;
        }
        let lowered = bare.to_ascii_lowercase();
        if LINKING.contains(&lowered.as_str()) || looks_like_a_commit(bare) {
            continue;
        }
        let is_a_figure = bare.chars().all(|c| c.is_ascii_digit())
            || NUMERALS.contains(&lowered.as_str())
            || lowered == "nothing";
        if !is_a_figure {
            return None;
        }
        // A figure followed by `at <hash>` is what this figure **used to be**,
        // and it sits exactly where the current one would. The first version of
        // this check read `29 at 4ff318c and 30 with the case this measurement
        // added` and pointed at 29 for a command that answers 30.
        let historical = words.get(at + 1).is_some_and(|w| {
            w.trim_matches(|c: char| !c.is_ascii_alphanumeric())
                .eq_ignore_ascii_case("at")
        }) && words.get(at + 2).is_some_and(|w| {
            looks_like_a_commit(w.trim_matches(|c: char| !c.is_ascii_alphanumeric()))
        });
        if historical {
            continue;
        }
        return Some(lowered);
    }
    None
}

/// What the sentence's word means as an answer: a count, or no output at all.
fn as_an_answer(figure: &str) -> Option<Option<usize>> {
    if figure == "nothing" {
        return Some(None);
    }
    if let Ok(count) = figure.parse::<usize>() {
        return Some(Some(count));
    }
    NUMERALS
        .iter()
        .position(|n| *n == figure)
        .map(|at| Some(at + 1))
}

#[test]
fn a_recorded_command_beside_a_figure_still_prints_it() {
    let root = repo_root();
    let canon =
        std::fs::read_to_string(root.join("docs/ROUTING-CANON.md")).expect("the canon is there");

    let mut examined = 0usize;
    let mut pointing = 0usize;
    let mut wrong: Vec<String> = Vec::new();

    // **Read off the canon itself, not off its flattened paragraphs.**
    // `canon_paragraphs` blanks anything between double quotes, which is right
    // for prose - this file quotes the sentences it replaced - and wrong for a
    // command, whose pattern lives inside those quotes. The first run of this
    // check ran `grep -h tests/fixtures/benchmark/*.kicad_pcb` with the pattern
    // gone and watched it answer 0 where the sentence says 174.
    let prose = canon_prose_only(&canon);
    // Only the hard wrap is normalised: a newline inside a backticked span is
    // the canon's line break and means a space. Every other run of whitespace
    // is the command's own - `grep -h "^  (footprint "` matches two spaces, and
    // collapsing them made it answer 0 where the sentence says 174.
    let parts: Vec<String> = prose
        .split('`')
        .map(|part| part.replace('\n', " "))
        .collect();
    {
        for index in (1..parts.len()).step_by(2) {
            let command = parts[index].as_str();
            let Some(tool) = command.split_whitespace().next() else {
                continue;
            };
            if !RECORDED_TOOLS.contains(&tool) || !command.contains(' ') {
                continue;
            }
            if NOT_RUN_HERE.contains(&tool)
                || carries_a_placeholder(command)
                || is_a_fragment(command)
            {
                continue;
            }
            examined += 1;

            // A shell, because the canon records pipes, globs and quoted
            // patterns, and because the order of the arguments has to reach
            // the tool exactly as the canon prints it - `--include` after `--`
            // stops being an option, which is a defect this project has
            // already shipped twice.
            let output = std::process::Command::new("sh")
                .arg("-c")
                .arg(command)
                .current_dir(&root)
                .output()
                .expect("a shell runs");

            // **Exit code 1 is an answer, not a failure.** grep says 1 when it
            // matched nothing, and the two commands here that answer with
            // nothing are the two sentences claiming an absence - the ones
            // telling the truth are the ones the shell calls failures. Only 2
            // means the command could not read what it was pointed at.
            assert_ne!(
                output.status.code(),
                Some(2),
                "the canon records a command that cannot run: `{command}`\n  {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );

            let printed = String::from_utf8_lossy(&output.stdout);
            let printed = printed.trim();
            let single_token = !printed.contains('\n') && !printed.contains(' ');
            if !single_token && !printed.is_empty() {
                // It ran and printed something a sentence cannot restate as one
                // figure - the rotations, for instance, which the canon quotes
                // rather than counts.
                continue;
            }

            // The words after the command, and the two spans behind them: the
            // hash that marks a figure as historical is itself backticked, so a
            // reach that stops at the next backtick cannot see it.
            let after = parts[index + 1..(index + 4).min(parts.len())].join(" ");
            let Some(figure) = figure_pointed_at(&after) else {
                continue;
            };
            let Some(answer) = as_an_answer(&figure) else {
                continue;
            };
            pointing += 1;

            let agrees = match answer {
                None => printed.is_empty(),
                Some(count) => printed.parse::<usize>() == Ok(count),
            };
            if !agrees {
                wrong.push(format!(
                    "`{command}` -> {:?}, and the sentence says {figure}",
                    printed
                ));
            }
        }
    }

    eprintln!(
        "recorded commands run: {examined} (floor {COMMANDS_EXAMINED_FLOOR}); \
         pointing at a figure: {pointing} (floor {COMMANDS_POINTING_FLOOR}); \
         disagreeing: {}",
        wrong.len()
    );

    assert!(
        wrong.is_empty(),
        "a command recorded beside a figure no longer prints it: {wrong:#?}\n\
         \n  A command written next to a number looks like provenance and is decoration until \
         somebody runs it. Re-read the sentence against what the command answers now."
    );

    assert!(
        examined >= COMMANDS_EXAMINED_FLOOR && pointing >= COMMANDS_POINTING_FLOOR,
        "this check ran {examined} commands and re-read {pointing} figures, below the floors \
         of {COMMANDS_EXAMINED_FLOOR} and {COMMANDS_POINTING_FLOOR}\n\
         \n  A command leaves this scan when its sentence stops naming the figure it answers \
         with, or when a directory moves out of the command and into the prose beside it - and \
         both read as tidying. If one really did go, lower the floor in the same commit."
    );
}

const RECORDED_RANGES_FLOOR: usize = 19;

/// **A recorded `sed -n 'A,Bp'` is a line number with a command around it, and
/// it drifts the same way one does - silently, and out of reach of the check
/// that would have caught it.** `a_line_number_beside_a_name_still_points_at_that_name`
/// reads a citation written ``name` (`path:line`)`; these live inside the
/// verification block, where `canon_prose_only` blanks them, so nothing looked
/// at them at all. One had already moved: the range that claims to show "the
/// rule that fires most is the first one the registry runs" printed a blank
/// line, five short of `ClearanceRule` - the same five that four citations had
/// drifted by.
///
/// What is asserted is the weakest thing that catches that: a range has to
/// print something that is not blank and not a comment. It cannot know what the
/// range ought to show, and a range showing the wrong code is a defect this
/// does not catch - but a range showing nothing at all is the one that happens
/// when the lines above it move.
#[test]
fn a_recorded_range_still_prints_code() {
    let root = repo_root();
    let canon =
        std::fs::read_to_string(root.join("docs/ROUTING-CANON.md")).expect("the canon is there");

    let mut ranges = 0usize;
    let mut silent: Vec<String> = Vec::new();
    for line in canon.lines() {
        let line = line.trim();
        if !line.starts_with("sed -n ") {
            continue;
        }
        ranges += 1;
        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg(line)
            .current_dir(&root)
            .output()
            .expect("a shell runs");
        assert_ne!(
            output.status.code(),
            Some(2),
            "the canon records a range command that cannot run: `{line}`"
        );
        let printed = String::from_utf8_lossy(&output.stdout);
        let says_something = printed.lines().any(|printed| {
            let printed = printed.trim();
            !printed.is_empty() && !printed.starts_with("//")
        });
        if !says_something {
            silent.push(line.to_string());
        }
    }

    eprintln!(
        "recorded ranges read: {ranges} (floor {RECORDED_RANGES_FLOOR}); printing no code: {}",
        silent.len()
    );

    assert!(
        silent.is_empty(),
        "a recorded range prints nothing a reader could check against: {silent:#?}\n\
         \n  The lines above it moved and the range stayed where it was. Re-read the file and \
         give the command the lines that now hold what its comment says it shows."
    );

    assert!(
        ranges >= RECORDED_RANGES_FLOOR,
        "this check read {ranges} recorded ranges, below the floor of {RECORDED_RANGES_FLOOR}\n\
         \n  A verification block loses a command the same way a sentence loses a figure: \
         quietly, while the prose around it still reads as current."
    );
}

/// The tools the verification blocks use. `cargo` is left out: the gate already
/// runs the workspace, and a test over prose that builds it to read a sentence
/// would cost minutes to learn nothing new.
const VERIFICATION_TOOLS: &[&str] = &[
    "grep", "sed", "awk", "ls", "wc", "find", "python3", "jq", "cat", "git", "rg",
];

const VERIFICATION_COMMANDS_FLOOR: usize = 46;

/// Figures this document states that its own verification blocks never print.
/// Each comes from a standard, a vendor's page or one run of the router, and
/// the legend says so - the blocks reach what the tree names, not what a figure
/// is worth. The sentence that says this is held here rather than left to age.
const BEYOND_THE_BLOCKS: &[&str] = &["7.5", "0.0248", "1.27", "9.7", "0.05", "1500"];

/// The control the list above needs: figures the blocks **do** print. Without
/// these, a scan that had stopped reading output would report all six absent
/// and look exactly like a passing one.
const WITHIN_THE_BLOCKS: &[&str] = &["0.254", "1000", "ClearanceRule", "10_000"];

/// **The verification block is the canon's own instrument and nothing ran it.**
/// Every section ends by naming the commands a reader can use to re-check it,
/// 51 of them, and until now the only ones anybody ran were the 11 written in
/// prose. A path that moves breaks these silently: the block still reads as a
/// way to check the section.
///
/// What is asserted is that the command **could run at all** - exit code 2,
/// which is what GNU grep, sed and awk return for a missing file or a bad
/// option, as against 1, which is an honest "no match" and is how this canon
/// states an absence. It does not check what the output says; the range check
/// above does that for the 19 that print code.
///
/// A line that does not parse on its own is a fragment of a larger shell
/// construct - one block builds a loop - and is skipped rather than guessed at.
#[test]
fn every_command_in_a_verification_block_still_runs() {
    let root = repo_root();
    let canon =
        std::fs::read_to_string(root.join("docs/ROUTING-CANON.md")).expect("the canon is there");

    let mut fenced = false;
    let mut pending = String::new();
    let mut commands: Vec<String> = Vec::new();
    for line in canon.lines() {
        if line.starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if !fenced {
            continue;
        }
        let first = line.split(' ').next().unwrap_or("");
        if pending.is_empty() && !VERIFICATION_TOOLS.contains(&first) {
            continue;
        }
        if pending.is_empty() {
            pending.push_str(line);
        } else {
            pending.push(' ');
            pending.push_str(line.trim());
        }
        // A trailing backslash is the canon's line break inside one command.
        if let Some(without) = pending.trim_end().strip_suffix('\\') {
            pending = without.trim_end().to_string();
            continue;
        }
        commands.push(std::mem::take(&mut pending));
    }

    let mut ran = 0usize;
    let mut fragments = 0usize;
    let mut broken: Vec<String> = Vec::new();
    let mut printed = String::new();
    for command in &commands {
        let parses = std::process::Command::new("sh")
            .arg("-n")
            .arg("-c")
            .arg(command)
            .current_dir(&root)
            .output()
            .expect("a shell runs");
        if !parses.status.success() {
            fragments += 1;
            continue;
        }
        ran += 1;
        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg(command)
            .current_dir(&root)
            .output()
            .expect("a shell runs");
        printed.push_str(&String::from_utf8_lossy(&output.stdout));
        if output.status.code() == Some(2) {
            broken.push(format!(
                "{command}\n    {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
    }

    let reached: Vec<&str> = BEYOND_THE_BLOCKS
        .iter()
        .copied()
        .filter(|figure| printed.contains(figure))
        .collect();
    let unreached: Vec<&str> = WITHIN_THE_BLOCKS
        .iter()
        .copied()
        .filter(|figure| !printed.contains(figure))
        .collect();

    eprintln!(
        "verification commands run: {ran} (floor {VERIFICATION_COMMANDS_FLOOR}); \
         fragments of a larger construct: {fragments}; cannot run: {}; \
         figures the legend says these blocks never print: {} of {}, control {} of {} found",
        broken.len(),
        reached.len(),
        BEYOND_THE_BLOCKS.len(),
        WITHIN_THE_BLOCKS.len() - unreached.len(),
        WITHIN_THE_BLOCKS.len()
    );

    assert!(
        unreached.is_empty(),
        "the control says these figures are printed by the blocks and they are not: \
         {unreached:#?}\n\
         \n  Nothing below this line means anything if the output stopped being read: a scan \
         over nothing reports every figure absent and looks like a passing one."
    );

    assert!(
        reached.is_empty(),
        "the legend says the verification blocks never print these, and now one does: \
         {reached:#?}\n\
         \n  That is good news and a stale sentence: a figure from a standard or a vendor's \
         page has acquired a command that reaches it. Move the figure out of that list and say \
         so in the legend."
    );

    assert!(
        broken.is_empty(),
        "a verification block records a command that cannot read what it is pointed at: \
         {broken:#?}\n\
         \n  Exit code 2 is not 'no match' - it is a file that moved or an option that stopped \
         being one. The block still reads as a way to re-check the section, and it is not."
    );

    assert!(
        ran >= VERIFICATION_COMMANDS_FLOOR,
        "this check ran {ran} verification commands, below the floor of \
         {VERIFICATION_COMMANDS_FLOOR}\n\
         \n  Commands leave this scan when a block is rewritten into a shape the joiner reads \
         as a fragment, which looks like tidying and silently empties the instrument."
    );
}

/// Rules this project reasoned out for itself rather than reading somewhere.
const DERIVED_RULES_FLOOR: usize = 4;

/// **A rule nobody else published has to bring its own command.** Of the 19
/// rules here, 12 are named by a `# R-NN` comment in the verification block and
/// 7 are not - and every one of those 7 carries `[R]`, `[P]` or `[O]`, which is
/// to say a standard, a vendor or an article vouches for it. Not one of them is
/// `[S]`.
///
/// That asymmetry is the rule this check holds. A figure from a standard can
/// stand on the standard; a rule this project derived from its own tree has
/// nothing behind it but the tree, so the block has to say how to look. The
/// check does not ask what the command shows - `every_command_in_a_verification_block_still_runs`
/// runs them all - only that a self-derived rule is named by one at all.
#[test]
fn a_rule_this_project_derived_itself_records_a_command() {
    let canon = std::fs::read_to_string(repo_root().join("docs/ROUTING-CANON.md"))
        .expect("the canon is there");

    let recorded: BTreeSet<&str> = canon
        .lines()
        .filter_map(|line| line.strip_prefix("# "))
        .filter_map(|rest| {
            rest.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
                .next()
        })
        .filter(|token| token.starts_with("R-") && token.len() > 2)
        .collect();

    let mut derived: Vec<&str> = Vec::new();
    let mut unrecorded: Vec<String> = Vec::new();
    for line in canon.lines() {
        let Some(heading) = line.strip_prefix("### ") else {
            continue;
        };
        if !heading.contains("[S]") {
            continue;
        }
        let Some(rule) = heading.split_whitespace().next() else {
            continue;
        };
        if !rule.starts_with("R-") {
            continue;
        }
        derived.push(rule);
        if !recorded.contains(rule) {
            unrecorded.push(heading.to_string());
        }
    }

    eprintln!(
        "rules this project derived itself: {} (floor {DERIVED_RULES_FLOOR}); \
         with no command recorded for them: {}; rules named by a command at all: {}",
        derived.len(),
        unrecorded.len(),
        recorded.len()
    );

    assert!(
        unrecorded.is_empty(),
        "a rule this project reasoned out for itself records no command: {unrecorded:#?}\n\
         \n  A rule read off a standard can stand on the standard. One derived from this tree \
         has nothing behind it but the tree, so the verification block has to say how to look."
    );

    assert!(
        derived.len() >= DERIVED_RULES_FLOOR,
        "this check found {} rules tagged `[S]`, below the floor of {DERIVED_RULES_FLOOR}\n\
         \n  Either a rule stopped being this project's own - which is a change worth saying \
         out loud - or the tag moved and this check is now reading nothing.",
        derived.len()
    );
}
