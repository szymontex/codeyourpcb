//! A diode's or a polarised part's pin is the pad the design names, never a
//! guessed number.
//!
//! `cargo test -p cypcb-world --test a_diode_pin_is_its_pad`
//!
//! Until 2026-09-26 `LED1.A` and `LED1.K` on a footprint with pads 1 and 2
//! went to pads 1 and 2, and `C1.POS` and `C1.NEG` the same. The built-in
//! two-pad footprints carry no polarity mark: their pads are the same shape
//! and size, and they print no silkscreen. Nothing on the board said which
//! end of the LED the anode net was on. Where a footprint does say, it says
//! the opposite: KiCad's `LED` and `D` symbols number the cathode 1, and its
//! LED_SMD and Diode_SMD footprints print the cathode mark beside pad 1
//! (read 2026-09-26).
//!
//! Now such a pin is a pad number, or a pad of that exact name. Any other
//! anode, cathode or polarity name is an error that says what to write
//! instead.

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

/// The premise: no built-in footprint an LED, a diode or a polarised
/// capacitor lands on says which pad is which. If one grows a mark, what the
/// letters mean on it has a source, and this file needs reading again.
#[test]
fn the_builtin_two_pad_footprints_carry_no_polarity_mark() {
    let library = FootprintLibrary::new();
    for name in ["0402", "0603", "0805", "1206", "2512", "AXIAL-300"] {
        let footprint = library.get(name).expect("a built-in footprint");
        assert_eq!(footprint.pads.len(), 2, "{name}");
        assert!(footprint.silk.is_empty(), "{name} prints a legend");
        let (one, two) = (&footprint.pads[0], &footprint.pads[1]);
        assert_eq!(one.shape, two.shape, "{name}: pad shapes differ");
        assert_eq!(one.size, two.size, "{name}: pad sizes differ");
    }
}

#[test]
fn a_and_k_on_a_builtin_two_pad_footprint_are_refused_with_what_to_write() {
    let (_world, errors) = sync(
        r#"
component LED1 led "0805" {
    at 10mm, 10mm
}

net LED_ANODE {
    LED1.A
}

net GND {
    LED1.K
}
"#,
    );
    let messages: Vec<&str> = errors.iter().map(|(m, _)| m.as_str()).collect();
    assert_eq!(
        messages,
        vec![
            "component 'LED1' has no pin 'A'. It has: 1, 2",
            "component 'LED1' has no pin 'K'. It has: 1, 2",
        ]
    );
    for (_, help) in &errors {
        assert!(help.contains("LED1.1"), "names a pad to write: {help}");
        assert!(
            help.contains("pads are named A and K and whose silkscreen marks the cathode"),
            "names the footprint way: {help}"
        );
    }
}

#[test]
fn every_name_the_diode_alias_read_is_refused_the_same_way() {
    let (_world, errors) = sync(
        r#"
component D1 diode "AXIAL-300" {
    at 10mm, 10mm
}

net IN {
    D1.anode
}

net OUT {
    D1.cathode
}

net ALSO_OUT {
    D1.ka
}
"#,
    );
    assert_eq!(errors.len(), 3, "{errors:?}");
    for (message, help) in &errors {
        assert!(
            message.starts_with("component 'D1' has no pin '"),
            "{message}"
        );
        assert!(help.contains("marks the cathode"), "{help}");
    }
}

#[test]
fn a_polarity_name_on_a_builtin_two_pad_footprint_is_refused() {
    let (_world, errors) = sync(
        r#"
component C1 capacitor "1206" {
    at 10mm, 10mm
}

net VBUS {
    C1.POS
}

net GND {
    C1.NEG
}
"#,
    );
    let messages: Vec<&str> = errors.iter().map(|(m, _)| m.as_str()).collect();
    assert_eq!(
        messages,
        vec![
            "component 'C1' has no pin 'POS'. It has: 1, 2",
            "component 'C1' has no pin 'NEG'. It has: 1, 2",
        ]
    );
    for (_, help) in &errors {
        assert!(help.contains("C1.1"), "{help}");
        assert!(
            help.contains("marks the positive or the negative pad"),
            "{help}"
        );
    }
}

#[test]
fn an_led_is_wired_by_its_pad_numbers() {
    let (mut world, errors) = sync(
        r#"
component LED1 led "0805" {
    at 10mm, 10mm
}

net GND {
    LED1.1
}

net LED_ANODE {
    LED1.2
}
"#,
    );
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(
        pins_of(&mut world, "LED1"),
        vec![
            ("1".to_string(), "GND".to_string()),
            ("2".to_string(), "LED_ANODE".to_string()),
        ]
    );
}

#[test]
fn a_footprint_with_pads_k_and_a_takes_the_letters() {
    let (mut world, errors) = sync(
        r#"
footprint LED_KA {
    courtyard 3mm x 2mm
    pad K rect at -1mm, 0mm size 1mm x 1.2mm
    pad A rect at 1mm, 0mm size 1mm x 1.2mm
    silk line -1.8mm, -0.8mm to -1.8mm, 0.8mm width 0.12mm
}

component LED1 led "LED_KA" {
    at 10mm, 10mm
}

net LED_ANODE {
    LED1.A
}

net GND {
    LED1.K
}
"#,
    );
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(
        pins_of(&mut world, "LED1"),
        vec![
            ("A".to_string(), "LED_ANODE".to_string()),
            ("K".to_string(), "GND".to_string()),
        ]
    );
}
