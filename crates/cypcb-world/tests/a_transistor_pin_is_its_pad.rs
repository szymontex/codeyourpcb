//! A transistor pin is the pad the design names, never a guessed number.
//!
//! `cargo test -p cypcb-world --test a_transistor_pin_is_its_pad`
//!
//! Until 2026-09-25 `Q1.B`, `Q1.C` and `Q1.E` on a footprint with numbered
//! pads went to pads 1, 2 and 3. No datasheet gives that order. onsemi's
//! SOT-23 outline (98ASB42226B, CASE 318, read 2026-09-25) lists base,
//! emitter, collector as STYLE 6, which the BC847 and the MMBT3904 use, and
//! emitter, base, collector as STYLE 7. On a BC847 the old map put the
//! collector net on the emitter and the emitter net on the collector, and
//! nothing said so.
//!
//! Now a transistor pin is a pad number or a pad of that exact name. Any
//! other name is an error that says what to write instead.
//!
//! The same error names the pin as the design wrote it. Until the same day a
//! name an alias still reads - `LED1.A` on a footprint without pad `1` - was
//! reported as "no pin '1'", a name that is nowhere in the design.

use cypcb_parser::parse;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{sync_ast_to_world, BoardWorld, NetConnections, SyncError};
use miette::Diagnostic;

const BOARD: &str = r#"
board t {
    size 30mm x 20mm
    layers 2
}
"#;

/// Each error's message and help.
fn sync(body: &str) -> (BoardWorld, Vec<(String, String)>) {
    let source = format!("{BOARD}{body}");
    let parsed = parse(&source);
    assert!(
        parsed.errors.is_empty(),
        "the fixture has to parse: {:?}",
        parsed.errors
    );
    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    let errors = sync_ast_to_world(&parsed.value, &source, &mut world, &mut library)
        .errors
        .iter()
        .map(|error: &SyncError| {
            let help = error.help().map(|h| h.to_string()).unwrap_or_default();
            (error.to_string(), help)
        })
        .collect();
    (world, errors)
}

fn pins_of(world: &mut BoardWorld, refdes: &str) -> Vec<(String, String)> {
    let entity = world.find_by_refdes(refdes).expect("the part exists");
    let mut pins: Vec<(String, String)> = world
        .get::<NetConnections>(entity)
        .expect("the part has connections")
        .iter()
        .map(|c| {
            let net = world.net_name(c.net).expect("the net is interned");
            (c.pin.clone(), net.to_string())
        })
        .collect();
    pins.sort();
    pins
}

const SOT23_WIRED_BY_LETTER: &str = r#"
component Q1 transistor "SOT-23" {
    at 10mm, 10mm
}

net BASE_DRIVE {
    Q1.B
}

net LOAD {
    Q1.C
}

net GND {
    Q1.E
}
"#;

#[test]
fn b_c_and_e_on_a_numbered_footprint_are_refused_with_what_to_write() {
    let (_world, errors) = sync(SOT23_WIRED_BY_LETTER);
    let messages: Vec<&str> = errors.iter().map(|(m, _)| m.as_str()).collect();
    assert_eq!(
        messages,
        vec![
            "component 'Q1' has no pin 'B'. It has: 1, 2, 3",
            "component 'Q1' has no pin 'C'. It has: 1, 2, 3",
            "component 'Q1' has no pin 'E'. It has: 1, 2, 3",
        ]
    );
    for (_, help) in &errors {
        assert!(help.contains("Q1.1"), "names a pad to write: {help}");
        assert!(
            help.contains("pads are named B, C and E"),
            "names the footprint way: {help}"
        );
    }
}

#[test]
fn a_transistor_is_wired_by_its_pad_numbers() {
    let (mut world, errors) = sync(
        r#"
component Q1 transistor "SOT-23" {
    at 10mm, 10mm
}

net BASE_DRIVE {
    Q1.1
}

net GND {
    Q1.2
}

net LOAD {
    Q1.3
}
"#,
    );
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(
        pins_of(&mut world, "Q1"),
        vec![
            ("1".to_string(), "BASE_DRIVE".to_string()),
            ("2".to_string(), "GND".to_string()),
            ("3".to_string(), "LOAD".to_string()),
        ]
    );
}

#[test]
fn a_footprint_with_pads_b_c_and_e_takes_the_letters() {
    let (mut world, errors) = sync(&format!(
        r#"
footprint SOT23_BEC {{
    courtyard 4mm x 4mm
    pad B rect at -1mm, 1mm size 1mm x 0.6mm
    pad E rect at -1mm, -1mm size 1mm x 0.6mm
    pad C rect at 1mm, 0mm size 1mm x 0.6mm
}}
{}"#,
        SOT23_WIRED_BY_LETTER.replace("\"SOT-23\"", "\"SOT23_BEC\"")
    ));
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(
        pins_of(&mut world, "Q1"),
        vec![
            ("B".to_string(), "BASE_DRIVE".to_string()),
            ("C".to_string(), "LOAD".to_string()),
            ("E".to_string(), "GND".to_string()),
        ]
    );
}

/// A part declared as something else, wired by a transistor's letters, gets
/// the same help: the letters are what gives it away.
#[test]
fn a_transistor_letter_on_another_kind_of_part_gets_the_same_help() {
    let (_world, errors) = sync(
        r#"
component U1 ic "SOT-23" {
    at 10mm, 10mm
}

net LOAD {
    U1.collector
}
"#,
    );
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(
        errors[0].0,
        "component 'U1' has no pin 'collector'. It has: 1, 2, 3"
    );
    assert!(errors[0].1.contains("U1.1"), "{}", errors[0].1);
}

#[test]
fn an_alias_that_misses_every_pad_is_named_as_the_design_wrote_it() {
    let (_world, errors) = sync(
        r#"
footprint TWO_LETTERS {
    courtyard 4mm x 2mm
    pad X rect at -1mm, 0mm size 1mm x 1mm
    pad Y rect at 1mm, 0mm size 1mm x 1mm
}

component LED1 led "TWO_LETTERS" {
    at 10mm, 10mm
}

net IN {
    LED1.A
}
"#,
    );
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(
        errors[0].0,
        "component 'LED1' has no pin 'A' (read as pin '1'). It has: X, Y"
    );
    assert!(
        errors[0].1.contains("X, Y"),
        "an LED keeps the plain help: {}",
        errors[0].1
    );
}
