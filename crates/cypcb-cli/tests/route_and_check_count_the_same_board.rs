//! What `cypcb route` says it wrote is what `cypcb check` finds in the file.
//!
//! `cargo test -p cypcb-cli --test route_and_check_count_the_same_board`
//!
//! `esp32_starter` is the board compared against other tools, and the two
//! commands gave it two numbers: `route --fast` printed 129 violations with 44
//! shorts, `check` on the file it had just written 131 with 46. The segments
//! were the same on both sides, every one of them. What differed was how they
//! were grouped: the router holds one trace entity per net and layer, the
//! reader makes one per `path`, and the clearance check counted contacts per
//! pair of entities - so a contact against a net split in two was one row in
//! memory and two in the file. It counts per place now, whatever the grouping;
//! `a_contact_is_one_row` holds that.
//!
//! `route` now reads the text it writes through the checker's own reader and
//! reports on that, so the two agree by construction. That leaves one way for
//! the file to stop being the board the router made - the writer losing
//! something on the way out - and the second test here is for that: the
//! copper read back from the text is the copper the router laid, segment for
//! segment and via for via.

use std::path::{Path, PathBuf};
use std::process::Command;

use cypcb_rules::presets::RulesPreset;
use cypcb_world::components::trace::{RouterPlaced, Trace, TraceSource, Via};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{sync_ast_to_world, BoardWorld};

fn cypcb() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cypcb"))
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/fixtures/benchmark")
        .join(name)
}

/// Total rows and shorts, as a pair.
type Counted = (usize, usize);

/// The numbers after `DRC on the routed board:`.
fn what_route_said(log: &str) -> Option<Counted> {
    let line = log
        .lines()
        .find(|line| line.starts_with("DRC on the routed board: "))?;
    let mut words = line.split_whitespace();
    let total = words.nth(5)?.parse().ok()?;
    let shorts = if line.contains("none of them touching") {
        0
    } else {
        line.split(", ").nth(1)?.split(' ').next()?.parse().ok()?
    };
    Some((total, shorts))
}

/// The header count and the shorts line of `cypcb check`.
fn what_check_said(log: &str) -> Option<Counted> {
    if log.contains("passed DRC") {
        return Some((0, 0));
    }
    let total = log
        .lines()
        .find_map(|line| line.split(" DRC violation(s)").next()?.parse().ok())?;
    let shorts = log
        .lines()
        .find_map(|line| {
            line.trim()
                .strip_prefix("copper touching copper at 0.00mm: ")?
                .parse()
                .ok()
        })
        .unwrap_or(0);
    Some((total, shorts))
}

/// Route one benchmark board one way and hold the answer to the checker's.
fn agree(name: &str, mode: &str) {
    let extension = Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .expect("a fixture has an extension");
    let home = cypcb_fixtures::scratch_dir(&format!("cypcb-route-check-{name}{mode}"));
    let written = home.join(format!("routed.{extension}"));
    let routed = cypcb()
        .arg("route")
        .arg(fixture(name))
        .arg(mode)
        .arg("-o")
        .arg(&written)
        .output()
        .expect("the binary runs");
    let route_log = String::from_utf8_lossy(&routed.stderr).to_string();
    assert!(
        routed.status.success(),
        "{name} {mode} did not route:\n{route_log}"
    );

    let checked = cypcb()
        .arg("check")
        .arg(&written)
        .output()
        .expect("the binary runs");
    let check_log = format!(
        "{}{}",
        String::from_utf8_lossy(&checked.stdout),
        String::from_utf8_lossy(&checked.stderr)
    );

    let said = what_route_said(&route_log)
        .unwrap_or_else(|| panic!("{name} {mode}: route printed no DRC line:\n{route_log}"));
    let found = what_check_said(&check_log)
        .unwrap_or_else(|| panic!("{name} {mode}: check printed no count:\n{check_log}"));
    assert_eq!(
        said, found,
        "{name} {mode}: route said (violations, shorts) {said:?}, check on the file it wrote \
         found {found:?}"
    );
}

#[test]
fn esp32_starter() {
    agree("esp32_starter.cypcb", "--fast");
    agree("esp32_starter.cypcb", "--variants");
}

#[test]
fn led_blink() {
    agree("led_blink.kicad_pcb", "--fast");
    agree("led_blink.kicad_pcb", "--variants");
}

#[test]
fn multi_ic_fast() {
    agree("multi_ic.kicad_pcb", "--fast");
}

/// The one pair left out of the gate, for its cost alone: routing `multi_ic`
/// through every variant took 165.9s in the test profile, where `--fast` took
/// 2.6s and the slowest other board here 37s, and nextest's pool waits for its
/// longest test. Run it by name before a change to how either command reads
/// or writes a four-layer board.
#[test]
#[ignore = "165.9s in the test profile; run by name"]
fn multi_ic_variants() {
    agree("multi_ic.kicad_pcb", "--variants");
}

#[test]
fn plane_board() {
    agree("plane_board.kicad_pcb", "--fast");
    agree("plane_board.kicad_pcb", "--variants");
}

#[test]
fn qfp_fanout() {
    agree("qfp_fanout.kicad_pcb", "--fast");
    agree("qfp_fanout.kicad_pcb", "--variants");
}

#[test]
fn shift_driver() {
    agree("shift_driver.kicad_pcb", "--fast");
    agree("shift_driver.kicad_pcb", "--variants");
}

#[test]
fn stm32_breakout() {
    agree("stm32_breakout.kicad_pcb", "--fast");
    agree("stm32_breakout.kicad_pcb", "--variants");
}

/// Every benchmark board has both of its runs above: a board added to the
/// directory without them would be a board this promise quietly does not
/// cover.
#[test]
fn every_benchmark_board_is_held_to_it() {
    let this_file = include_str!("route_and_check_count_the_same_board.rs");
    let mut boards: Vec<String> = std::fs::read_dir(fixture(""))
        .expect("the benchmark directory is there")
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .filter(|name| name.ends_with(".kicad_pcb") || name.ends_with(".cypcb"))
        .collect();
    boards.sort();
    assert!(boards.len() >= 7, "found only {boards:?}");
    for board in boards {
        for mode in ["--fast", "--variants"] {
            assert!(
                this_file.contains(&format!("agree(\"{board}\", \"{mode}\")")),
                "{board} is a benchmark board with no {mode} route-against-check test here"
            );
        }
    }
}

fn world_from(source: &str) -> (BoardWorld, FootprintLibrary) {
    let parsed = cypcb_parser::parse(source);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    let result = sync_ast_to_world(&parsed.value, source, &mut world, &mut library);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    (world, library)
}

/// Every segment as net, layer, width and both ends in nanometres, each
/// segment's ends in one order so a segment written backwards is the same
/// segment. Sorted, so grouping into entities does not show.
fn copper(world: &mut BoardWorld, only_routed: bool) -> (Vec<String>, Vec<String>) {
    let traces: Vec<Trace> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<&Trace>();
        query
            .iter(ecs)
            .filter(|trace| !only_routed || trace.source == TraceSource::Autorouted)
            .cloned()
            .collect()
    };
    let vias: Vec<Via> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<(&Via, Option<&RouterPlaced>)>();
        query
            .iter(ecs)
            .filter(|(_, placed)| !only_routed || placed.is_some())
            .map(|(via, _)| *via)
            .collect()
    };
    let mut segments = Vec::new();
    for trace in &traces {
        let net = world.net_name(trace.net_id).unwrap_or("?").to_string();
        for segment in &trace.segments {
            let (a, b) = (
                (segment.start.x.0, segment.start.y.0),
                (segment.end.x.0, segment.end.y.0),
            );
            let (a, b) = if a <= b { (a, b) } else { (b, a) };
            segments.push(format!(
                "{net} {:?} {} {:?} {a:?} {b:?}",
                trace.layer,
                trace.width.0,
                segment.width.map(|w| w.0)
            ));
        }
    }
    let mut holes: Vec<String> = vias
        .iter()
        .map(|via| {
            format!(
                "{} {:?} {} {} {:?} {:?}",
                world.net_name(via.net_id).unwrap_or("?"),
                (via.position.x.0, via.position.y.0),
                via.drill.0,
                via.outer_diameter.0,
                via.start_layer,
                via.end_layer
            )
        })
        .collect();
    segments.sort();
    holes.sort();
    (segments, holes)
}

#[test]
fn the_written_copper_is_the_copper_the_router_laid() {
    let source =
        std::fs::read_to_string(fixture("esp32_starter.cypcb")).expect("the board is there");
    let (mut world, library) = world_from(&source);
    let (drawn_by_hand, _) = copper(&mut world, false);
    assert!(
        drawn_by_hand.is_empty(),
        "the fixture carries copper of its own, and this test compares everything read back \
         against what the router laid"
    );

    let rules = cypcb_drc::ruleset_for_world(RulesPreset::JlcpcbStandard2Layer, &world);
    let result = cypcb_autoroute::route_board(
        &mut world,
        &library,
        &rules,
        &cypcb_autoroute::AutorouteConfig::default(),
    );
    cypcb_router::apply_routes(&mut world, &result);
    let laid = copper(&mut world, true);
    assert!(
        !laid.0.is_empty(),
        "the router laid nothing, so nothing is compared"
    );
    assert!(
        !laid.1.is_empty(),
        "the router placed no via, so no via is compared"
    );

    let written = format!(
        "{source}\n{}",
        cypcb_world::dsl::routed_traces_as_dsl(&mut world)
    );
    let (mut reread, _) = world_from(&written);
    let read_back = copper(&mut reread, false);

    let lost: Vec<&String> = laid.0.iter().filter(|s| !read_back.0.contains(s)).collect();
    let gained: Vec<&String> = read_back.0.iter().filter(|s| !laid.0.contains(s)).collect();
    assert!(
        lost.is_empty() && gained.is_empty() && laid.0.len() == read_back.0.len(),
        "{} segments laid, {} read back; not in the file: {:?}; only in the file: {:?}",
        laid.0.len(),
        read_back.0.len(),
        lost.iter().take(5).collect::<Vec<_>>(),
        gained.iter().take(5).collect::<Vec<_>>()
    );
    assert_eq!(
        laid.1, read_back.1,
        "the vias read back are not the vias placed"
    );
}
