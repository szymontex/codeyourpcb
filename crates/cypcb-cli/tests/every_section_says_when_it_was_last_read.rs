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
