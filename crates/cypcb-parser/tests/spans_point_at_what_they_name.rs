//! Every span has to slice the source back to the thing it describes.
//!
//! Spans are what an editor underlines and what `cypcb check` points at. A
//! span that is off by a token underlines the wrong word, and nothing else in
//! this project would notice: the parse succeeds, the model is right, and the
//! only symptom is a squiggle in the wrong place.
//!
//! This walks the parsed AST as JSON and checks every `{ value, span }` pair
//! against the text it came from, so a construct added to the grammar is
//! covered without anyone remembering to add a case here.

use std::path::Path;

/// Walk the JSON and yield every `{ "value": ..., "span": { start, end } }`.
fn spanned_values(node: &serde_json::Value, out: &mut Vec<(String, usize, usize)>) {
    match node {
        serde_json::Value::Object(map) => {
            if let (Some(value), Some(span)) = (map.get("value"), map.get("span")) {
                if let (Some(start), Some(end)) = (
                    span.get("start").and_then(|v| v.as_u64()),
                    span.get("end").and_then(|v| v.as_u64()),
                ) {
                    // Only the ones whose value is text can be compared to the
                    // source. A number's span covers `10kohm`, whose value is
                    // 10000 - a different thing, and not this test's business.
                    if let Some(text) = value.as_str() {
                        out.push((text.to_string(), start as usize, end as usize));
                    }
                }
            }
            for child in map.values() {
                spanned_values(child, out);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                spanned_values(item, out);
            }
        }
        _ => {}
    }
}

fn check_file(relative: &str) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative);
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{} is in the repo: {e}", path.display()));

    let parsed = cypcb_parser::parse(&source);
    assert!(
        parsed.errors.is_empty(),
        "{relative} has to parse: {:?}",
        parsed.errors
    );

    let json = serde_json::to_value(&parsed.value).expect("the AST serialises");
    let mut spans = Vec::new();
    spanned_values(&json, &mut spans);
    assert!(
        !spans.is_empty(),
        "{relative} produced no spans at all, so this test proves nothing"
    );

    for (text, start, end) in &spans {
        assert!(
            *end <= source.len() && start <= end,
            "{relative}: span {start}..{end} is outside a {} byte file, for {text:?}",
            source.len()
        );
        // A string literal's span covers its quotes, which is what an editor
        // should underline, while its value is the text between them.
        let sliced = &source[*start..*end];
        let matches = sliced == text || sliced == format!("\"{text}\"");
        assert!(
            matches,
            "{relative}: the span at {start}..{end} covers {sliced:?} for a value of {text:?}"
        );
    }
}

#[test]
fn the_blink_example_spans_point_at_their_own_text() {
    check_file("examples/blink.cypcb");
}

#[test]
fn the_language_showcase_spans_point_at_their_own_text() {
    // The file that uses every construct the language has, so a new one is
    // covered the day it is written into an example.
    check_file("examples/v2-modules.cypcb");
}
