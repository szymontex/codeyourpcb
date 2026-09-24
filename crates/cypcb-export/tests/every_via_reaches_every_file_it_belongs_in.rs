//! Every via is in every file that describes the copper or the hole it makes.
//!
//! `cargo test -p cypcb-export --test every_via_reaches_every_file_it_belongs_in`
//!
//! Until 2026-09-24 each export decided for itself which layers a via is on,
//! from `start_layer` and `end_layer` as the router wrote them. The router
//! writes a via in the direction it climbed, and the copper files knew three
//! spans in one order: on a routed four-layer board about half the vias had no
//! land in the Gerber, IPC-2581 drew its inner layers as copies of the top,
//! the netlist called a through via written from the bottom a buried one, and
//! a drill file for a via climbing from the bottom said `Buried` and `4,2`.
//!
//! Here the expected layers come from where each end sits in the stack, not
//! from the model's own answer, and every ordered pair of distinct layers on a
//! four-layer board is tried.

use cypcb_core::{Nm, Point};
use cypcb_export::coords::CoordinateFormat;
use cypcb_export::excellon::{export_excellon, export_excellon_span, non_through_spans, DrillType};
use cypcb_export::gerber::export_copper_layer;
use cypcb_export::ipc2581::{export_ipc2581, HouseTolerances};
use cypcb_export::ipc356::export_ipc356;
use cypcb_export::nm_to_gerber;
use cypcb_world::components::trace::Via;
use cypcb_world::components::Layer;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{BoardWorld, NetId};

const STACK: [Layer; 4] = [
    Layer::TopCopper,
    Layer::Inner(0),
    Layer::Inner(1),
    Layer::BottomCopper,
];
const FORMAT: CoordinateFormat = CoordinateFormat::FORMAT_MM_2_6;

/// A via between stack positions `from` and `to`, and the positions it is
/// drilled through.
struct Case {
    via: Via,
    upper: usize,
    lower: usize,
}

impl Case {
    fn is_on(&self, position: usize) -> bool {
        self.upper <= position && position <= self.lower
    }
    fn is_through(&self) -> bool {
        self.upper == 0 && self.lower == STACK.len() - 1
    }
}

/// Every ordered pair of distinct layers, each via at its own place.
fn cases() -> Vec<Case> {
    let mut cases = Vec::new();
    for (from, &start_layer) in STACK.iter().enumerate() {
        for (to, &end_layer) in STACK.iter().enumerate() {
            if from == to {
                continue;
            }
            let n = cases.len() as f64;
            let mut via = Via::new(Point::from_mm(2.0 + 2.0 * n, 5.0), NetId::new(0));
            via.drill = Nm::from_mm(0.2);
            via.start_layer = start_layer;
            via.end_layer = end_layer;
            cases.push(Case {
                via,
                upper: from.min(to),
                lower: from.max(to),
            });
        }
    }
    cases
}

fn board(vias: &[&Case]) -> (BoardWorld, FootprintLibrary) {
    let mut world = BoardWorld::new();
    world.set_board(
        "t".to_string(),
        (Nm::from_mm(40.0), Nm::from_mm(10.0)),
        STACK.len() as u8,
    );
    let net = world.intern_net("GND");
    for case in vias {
        let mut via = case.via;
        via.net_id = net;
        world.ecs_mut().spawn((via, net));
    }
    (world, FootprintLibrary::new())
}

fn flash(via: &Via) -> String {
    format!(
        "X{}Y{}D03*",
        nm_to_gerber(via.position.x.0, &FORMAT),
        nm_to_gerber(via.position.y.0, &FORMAT)
    )
}

/// The sections of an IPC-2581 document that hold a via, by layer name.
fn ipc2581_via_layers(xml: &str) -> Vec<String> {
    xml.split("<LayerFeature layerRef=\"")
        .skip(1)
        .filter(|section| {
            section
                .split("</LayerFeature>")
                .next()
                .is_some_and(|body| body.contains("<Set padUsage=\"VIA\">"))
        })
        .map(|section| section.split('"').next().unwrap_or_default().to_string())
        .collect()
}

fn ipc2581_name(position: usize) -> String {
    match position {
        0 => "F_Cu".to_string(),
        last if last == STACK.len() - 1 => "B_Cu".to_string(),
        inner => format!("In{inner}_Cu"),
    }
}

#[test]
fn every_copper_file_flashes_every_via_that_has_copper_on_it() {
    let cases = cases();
    let all: Vec<&Case> = cases.iter().collect();
    let (mut world, library) = board(&all);

    for (position, layer) in STACK.iter().enumerate() {
        let gerber = export_copper_layer(&mut world, &library, *layer, &FORMAT).unwrap();
        let flashes = gerber.lines().filter(|line| line.ends_with("D03*")).count();
        let expected = cases.iter().filter(|case| case.is_on(position)).count();
        assert_eq!(
            flashes, expected,
            "{layer:?}: the board holds nothing but vias, so every flash is one"
        );
        for case in &cases {
            assert_eq!(
                gerber.contains(&flash(&case.via)),
                case.is_on(position),
                "{:?} -> {:?} on {layer:?}",
                case.via.start_layer,
                case.via.end_layer
            );
        }
    }
}

#[test]
fn every_ipc2581_layer_carries_the_vias_that_have_copper_on_it() {
    for case in cases() {
        let (mut world, library) = board(&[&case]);
        let (xml, _) = export_ipc2581(
            &mut world,
            &library,
            HouseTolerances::default(),
            "2026-09-24T00:00:00Z",
        );
        let expected: Vec<String> = (0..STACK.len())
            .filter(|position| case.is_on(*position))
            .map(ipc2581_name)
            .collect();
        assert_eq!(
            ipc2581_via_layers(&xml),
            expected,
            "{:?} -> {:?}",
            case.via.start_layer,
            case.via.end_layer
        );
    }
}

#[test]
fn the_netlist_knows_a_through_via_and_the_face_a_blind_one_opens_on() {
    for case in cases() {
        let (mut world, library) = board(&[&case]);
        let (netlist, _) = export_ipc356(&mut world, &library, "t");
        let line = netlist
            .lines()
            .find(|line| line.get(20..23) == Some("VIA"))
            .unwrap_or_else(|| panic!("no via line in:\n{netlist}"));
        let top = case.upper == 0;
        let bottom = case.lower == STACK.len() - 1;
        let access = match (top, bottom) {
            (true, true) => "A00",
            (true, false) => "A01",
            _ => "A02",
        };
        let code = if case.is_through() { "317" } else { "307" };
        assert_eq!(
            (&line[..3], &line[38..41]),
            (code, access),
            "{:?} -> {:?}: {line}",
            case.via.start_layer,
            case.via.end_layer
        );
    }
}

#[test]
fn every_via_is_drilled_once_in_the_file_for_its_pair() {
    for case in cases() {
        let (mut world, library) = board(&[&case]);
        let hole = format!(
            "X{:.6}Y{:.6}",
            case.via.position.x.0 as f64 / 1e6,
            case.via.position.y.0 as f64 / 1e6
        );
        let through =
            export_excellon(&mut world, &library, &FORMAT, Some(DrillType::Plated)).unwrap();
        let spans = non_through_spans(&mut world, &library).unwrap();
        let label = format!("{:?} -> {:?}", case.via.start_layer, case.via.end_layer);

        if case.is_through() {
            assert!(through.contains(&hole), "{label}:\n{through}");
            assert!(spans.is_empty(), "{label}: {spans:?}");
            continue;
        }
        assert!(!through.contains(&hole), "{label}:\n{through}");
        assert_eq!(
            spans,
            vec![(STACK[case.upper], STACK[case.lower])],
            "{label}"
        );
        let file = export_excellon_span(
            &mut world,
            &library,
            &FORMAT,
            Some(DrillType::Plated),
            spans[0],
        )
        .unwrap();
        let kind = if case.upper == 0 || case.lower == STACK.len() - 1 {
            "Blind"
        } else {
            "Buried"
        };
        let function = format!(
            "TF.FileFunction,Plated,{},{},{kind}",
            case.upper + 1,
            case.lower + 1
        );
        assert!(file.contains(&hole), "{label}:\n{file}");
        assert!(file.contains(&function), "{label}: want {function}\n{file}");
    }
}
