//! The SVG, DXF and PDF plots of a board are the same bytes each time.
//!
//! `cargo test -p cypcb-cli --test a_plot_is_the_same_each_run`
//!
//! The plot writers walked a bevy query, whose archetypes come in the order of
//! a hash map seeded once per process, so a board with traces that carry a
//! curve and traces that do not - `examples/curved-track.cypcb` is one - wrote
//! its plots in one order in one run and in another in the next. They now draw
//! in the order the board was built. What is held here: two exports in one
//! process, several in separate processes, and a golden of the top copper SVG
//! made by the writer as it now stands.
//!
//! The process runs cannot be a proof by themselves: with two archetypes the
//! seed puts either first, so a plain query comes out right in about half of
//! the runs. The order itself is held, without the coin, by
//! `a_plot_comes_in_the_order_the_board_was_built` in `cypcb-export`.

use std::path::{Path, PathBuf};
use std::process::Command;

use cypcb_world::{sync_ast_to_world, BoardWorld, Layer};

/// How many separate processes export the board. Six give a missed fault
/// about one chance in thirty-two when the order is a coin.
const PROCESSES: usize = 6;

fn example() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/curved-track.cypcb")
}

fn golden() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/plot-order/curved-track-F_Cu.svg")
}

fn export(who: &str) -> cypcb_fixtures::ScratchPath {
    let home = cypcb_fixtures::scratch_dir(&format!("cypcb-plot-order-{who}"));
    let dir = home.join("out");
    std::fs::create_dir_all(&dir).expect("the directory is creatable");
    let out = home.holding(dir);
    let status = Command::new(env!("CARGO_BIN_EXE_cypcb"))
        .env("SOURCE_DATE_EPOCH", "1760000000")
        .arg("export")
        .arg(example())
        .arg("-o")
        .arg(&out)
        .args(["--svg", "--dxf", "--pdf"])
        .status()
        .expect("the binary runs");
    assert!(status.success(), "the export failed");
    out
}

const PLOTS: [&str; 6] = [
    "curved-track-F_Cu.svg",
    "curved-track-B_Cu.svg",
    "curved-track-F_Cu.dxf",
    "curved-track-B_Cu.dxf",
    "curved-track-F_Cu.pdf",
    "curved-track-B_Cu.pdf",
];

#[test]
fn separate_processes_write_the_same_plots() {
    let first = export("0");
    for run in 1..PROCESSES {
        let again = export(&run.to_string());
        for name in PLOTS {
            let a = std::fs::read(first.join("plot").join(name)).expect("written");
            let b = std::fs::read(again.join("plot").join(name)).expect("written");
            assert!(!a.is_empty(), "{name} is a file");
            assert!(a == b, "{name} differs between process 0 and process {run}");
        }
    }
}

fn plots_in_this_process() -> Vec<String> {
    let source = std::fs::read_to_string(example()).expect("the example reads");
    let parsed = cypcb_parser::parse(&source);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors.first());
    let mut world = BoardWorld::new();
    let mut library = cypcb_world::footprint::FootprintLibrary::new();
    let result = sync_ast_to_world(&parsed.value, &source, &mut world, &mut library);
    assert!(result.errors.is_empty(), "{:?}", result.errors.first());
    world.rebuild_spatial_index_from_library(&library);
    let mut plots = Vec::new();
    for layer in [Layer::TopCopper, Layer::BottomCopper] {
        plots.push(cypcb_export::svg::plot_layer(&mut world, &library, layer));
        plots.push(cypcb_export::dxf::plot_layer(&mut world, &library, layer));
        plots.push(cypcb_export::pdf::plot_layer(&mut world, &library, layer));
    }
    plots
}

#[test]
fn one_process_writes_the_same_plots_twice() {
    let first = plots_in_this_process();
    let second = plots_in_this_process();
    assert_eq!(first.len(), 6);
    assert!(first.iter().all(|p| !p.is_empty()));
    assert!(
        first == second,
        "the same board plotted twice in one process differs"
    );
}

#[test]
fn the_top_copper_svg_is_the_golden() {
    let out = export("golden");
    let got = std::fs::read(out.join("plot/curved-track-F_Cu.svg")).expect("written");
    let want = std::fs::read(golden()).expect("the golden reads");
    assert!(
        got == want,
        "curved-track-F_Cu.svg is not the golden ({} bytes against {})",
        got.len(),
        want.len()
    );
}
