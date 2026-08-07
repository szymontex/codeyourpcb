//! Every example board, exported and read back.
//!
//! `source_to_fabrication.rs` checks one board in depth. This checks all of
//! them shallowly, which catches a different thing: a construct that parses,
//! passes DRC and exports into the wrong copper. The sweep that preceded this
//! found two boards that could not be exported at all; running the files back
//! is the half that was still missing.
//!
//! The invariants are the ones that must hold for any board whatsoever, so a
//! new example is covered the moment it lands in the directory.

use std::path::{Path, PathBuf};

use cypcb_export::{run_export, ExportJob};
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{sync_ast_to_world, BoardWorld};

fn examples_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("the crate sits two levels below the repo root")
        .join("examples")
}

/// Every example that declares a board, as (name, world, library).
fn boards() -> Vec<(String, BoardWorld, FootprintLibrary)> {
    let mut out = Vec::new();

    let mut files: Vec<PathBuf> = std::fs::read_dir(examples_dir())
        .expect("the examples directory is there")
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "cypcb"))
        .collect();
    files.sort();

    for file in files {
        let name = file
            .file_name()
            .and_then(|n| n.to_str())
            .expect("a file name")
            .to_string();
        let Ok(source) = std::fs::read_to_string(&file) else {
            continue;
        };

        let parsed = cypcb_parser::parse(&source);
        if !parsed.errors.is_empty() {
            // Two examples exist to demonstrate a parse error;
            // `the_examples_still_say_what_they_show` is what holds them to it.
            continue;
        }

        let mut import_errors = Vec::new();
        let ast = cypcb_parser::resolve_imports(&parsed.value, &file, &mut import_errors);

        let mut world = BoardWorld::new();
        let mut library = FootprintLibrary::new();
        let result = sync_ast_to_world(&ast, &source, &mut world, &mut library);
        assert!(
            result.errors.is_empty(),
            "{name} builds no board model: {:?}",
            result.errors
        );

        // A file of modules or interfaces is a library, not a design.
        if world.board_info().is_none() {
            continue;
        }

        out.push((name, world, library));
    }

    assert!(out.len() >= 10, "expected the examples to be boards, got {}", out.len());
    out
}

/// `tag` keeps two tests from sharing a directory: they run in parallel, and
/// the first one to finish was deleting the files the other was still reading.
fn export_to_temp(
    tag: &str,
    name: &str,
    world: &mut BoardWorld,
    library: &FootprintLibrary,
) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cypcb-sweep-{tag}-{}", name.replace('.', "-")));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a temp directory");

    let preset = cypcb_export::presets::from_name("jlcpcb").expect("the jlcpcb preset");
    let job = ExportJob {
        source_path: PathBuf::from(name),
        output_dir: dir.clone(),
        preset,
        board_name: name.trim_end_matches(".cypcb").to_string(),
    };
    let result = run_export(&job, world, library)
        .unwrap_or_else(|e| panic!("{name} failed to export: {e:?}"));
    assert!(
        !result.files.is_empty(),
        "{name} exported nothing at all"
    );
    dir
}

#[test]
fn every_exported_layer_is_a_complete_gerber() {
    // A Gerber that never says `M02*` is a file a fabricator's loader stops
    // reading part way through - and one truncated layer is a board with a
    // whole side missing, which is not the kind of thing to find at the fab.
    let mut broken = Vec::new();
    // A sweep that inspected nothing passes every assertion it makes, so the
    // denominator is part of the result.
    let mut layers_read = 0usize;

    for (name, mut world, library) in boards() {
        let dir = export_to_temp("layers", &name, &mut world, &library);

        for path in files_under(&dir) {
            let Some(extension) = path.extension().and_then(|e| e.to_str()) else {
                continue;
            };
            if !extension.starts_with("g") && extension != "gbr" {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("a readable layer");
            layers_read += 1;
            let file = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
            if !text.contains("M02*") {
                broken.push(format!("{name}/{file}: no end-of-file marker"));
            }
            if !text.contains("%FSLAX") {
                broken.push(format!("{name}/{file}: no coordinate format header"));
            }
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    assert!(broken.is_empty(), "incomplete Gerbers:\n{}", broken.join("\n"));
    assert!(
        layers_read >= 100,
        "expected roughly nine Gerbers per board across the examples, read {layers_read}"
    );
    eprintln!("{layers_read} Gerber layers read back across the examples");
}

#[test]
fn the_outline_matches_the_board_every_example_declares() {
    // The outline is what the fabricator cuts. A board that declares 50x30mm
    // and cuts to something else is scrap, and nothing else in the file set
    // would show it.
    let mut wrong = Vec::new();
    let mut measured = 0usize;

    for (name, mut world, library) in boards() {
        let (size, _) = world.board_info().expect("a board");
        let expected_width = size.width.to_mm();
        let expected_height = size.height.to_mm();

        let dir = export_to_temp("outline", &name, &mut world, &library);
        let outline = files_under(&dir)
            .into_iter()
            .find(|path| {
                path.extension().and_then(|e| e.to_str()) == Some("gko")
                    || path.to_string_lossy().contains("Edge_Cuts")
            })
            .unwrap_or_else(|| panic!("{name} exported no outline"));

        let text = std::fs::read_to_string(&outline).expect("a readable outline");
        let (mut min_x, mut min_y) = (f64::MAX, f64::MAX);
        let (mut max_x, mut max_y) = (f64::MIN, f64::MIN);
        for line in text.lines().filter(|l| l.contains("D01") || l.contains("D02")) {
            let Some((x, y)) = coordinates(line) else {
                continue;
            };
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }

        let width = max_x - min_x;
        let height = max_y - min_y;
        measured += 1;
        if (width - expected_width).abs() > 0.001 || (height - expected_height).abs() > 0.001 {
            wrong.push(format!(
                "{name}: declares {expected_width:.3}mm x {expected_height:.3}mm, cuts {width:.3}mm x {height:.3}mm"
            ));
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    assert!(
        wrong.is_empty(),
        "the outline does not match the board:\n{}",
        wrong.join("\n")
    );
    assert!(measured >= 10, "only {measured} outlines were measured");
    eprintln!("{measured} outlines measured against the size their source declares");
}

/// Every file the export wrote, at any depth - the preset puts Gerbers in one
/// subdirectory and assembly files in another.
fn files_under(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(files_under(&path));
        } else {
            out.push(path);
        }
    }
    out.sort();
    out
}

/// Pull `X`/`Y` out of a Gerber command in the 2.6 format the exporter writes.
fn coordinates(line: &str) -> Option<(f64, f64)> {
    let x = between(line, 'X')?;
    let y = between(line, 'Y')?;
    Some((x, y))
}

fn between(line: &str, marker: char) -> Option<f64> {
    let start = line.find(marker)? + 1;
    let rest = &line[start..];
    let end = rest
        .find(|c: char| !c.is_ascii_digit() && c != '-' && c != '.')
        .unwrap_or(rest.len());
    rest[..end].parse::<f64>().ok()
}
