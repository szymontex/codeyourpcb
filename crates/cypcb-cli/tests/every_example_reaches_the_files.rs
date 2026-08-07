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

#[test]
fn every_pad_is_flashed_where_its_footprint_puts_it() {
    // The layers are read for completeness above; this reads them for content.
    // A pad that never reaches copper is a part that cannot be soldered, and
    // the file set looks perfectly well formed without it.
    use cypcb_world::components::{FootprintRef, Layer, Position, Rotation};

    let mut missing: Vec<String> = Vec::new();
    let mut pads_checked = 0usize;

    for (name, mut world, library) in boards() {
        // Where every pad ends up, per copper layer, in board coordinates.
        let placements: Vec<(cypcb_core::Point, f64, String)> = {
            let ecs = world.ecs_mut();
            let mut query = ecs.query::<(&Position, &Rotation, &FootprintRef)>();
            query
                .iter(ecs)
                .map(|(position, rotation, footprint)| {
                    (
                        position.0,
                        rotation.to_degrees(),
                        footprint.as_str().to_string(),
                    )
                })
                .collect()
        };

        let mut expected: Vec<(Layer, f64, f64)> = Vec::new();
        for (position, degrees, footprint_name) in &placements {
            let Some(footprint) = library.get(footprint_name) else {
                continue;
            };
            let (sin, cos) = degrees.to_radians().sin_cos();
            for pad in &footprint.pads {
                let px = pad.position.x.0 as f64;
                let py = pad.position.y.0 as f64;
                let x = position.x.0 as f64 + px * cos - py * sin;
                let y = position.y.0 as f64 + px * sin + py * cos;
                for layer in &pad.layers {
                    if matches!(layer, Layer::TopCopper | Layer::BottomCopper) {
                        expected.push((*layer, x / 1_000_000.0, y / 1_000_000.0));
                    }
                }
            }
        }

        if expected.is_empty() {
            continue;
        }

        let dir = export_to_temp("pads", &name, &mut world, &library);
        let files = files_under(&dir);
        // The preset decides the names: JLCPCB writes `-F_Cu.gbr`, others
        // write `.gtl`. Matching one spelling silently read an empty file and
        // reported every pad as missing - which is how this test failed its
        // first run, on itself rather than on the exporter.
        let read_layer = |markers: &[&str]| -> String {
            files
                .iter()
                .find(|path| {
                    let name = path.to_string_lossy();
                    markers.iter().any(|marker| name.ends_with(marker))
                })
                .map(|path| std::fs::read_to_string(path).expect("a readable layer"))
                .unwrap_or_else(|| panic!("{name} exported no top or bottom copper: {files:?}"))
        };
        let top = flashes(&read_layer(&["F_Cu.gbr", ".gtl"]));
        let bottom = flashes(&read_layer(&["B_Cu.gbr", ".gbl"]));

        for (layer, x, y) in expected {
            pads_checked += 1;
            let candidates = match layer {
                Layer::TopCopper => &top,
                _ => &bottom,
            };
            let found = candidates
                .iter()
                .any(|(fx, fy)| (fx - x).abs() < 0.002 && (fy - y).abs() < 0.002);
            if !found {
                missing.push(format!(
                    "{name}: no copper flashed at ({x:.3}mm, {y:.3}mm) on {layer:?}"
                ));
            }
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    assert!(
        missing.is_empty(),
        "pads that never reach copper:\n{}",
        missing.join("\n")
    );
    assert!(pads_checked >= 40, "only {pads_checked} pads were checked");
    eprintln!("{pads_checked} pads found in the copper their footprint puts them on");
}

/// Every flash coordinate in a layer, in millimetres.
fn flashes(gerber: &str) -> Vec<(f64, f64)> {
    gerber
        .lines()
        .filter(|line| line.contains("D03"))
        .filter_map(coordinates)
        .collect()
}

#[test]
fn every_drilled_pad_gets_a_hole_of_the_size_it_asked_for() {
    // Copper without a hole is a through-hole part that cannot be fitted, and
    // a hole of the wrong size is a lead that does not go in or a joint that
    // never wets. Neither shows in a Gerber preview: the drill file is a
    // separate format that most previews do not overlay.
    use cypcb_world::components::{FootprintRef, Position, Rotation};

    let mut wrong: Vec<String> = Vec::new();
    let mut holes_checked = 0usize;

    for (name, mut world, library) in boards() {
        let placements: Vec<(cypcb_core::Point, f64, String)> = {
            let ecs = world.ecs_mut();
            let mut query = ecs.query::<(&Position, &Rotation, &FootprintRef)>();
            query
                .iter(ecs)
                .map(|(position, rotation, footprint)| {
                    (
                        position.0,
                        rotation.to_degrees(),
                        footprint.as_str().to_string(),
                    )
                })
                .collect()
        };

        // Where a hole has to be, and how wide, in millimetres.
        let mut expected: Vec<(f64, f64, f64)> = Vec::new();
        for (position, degrees, footprint_name) in &placements {
            let Some(footprint) = library.get(footprint_name) else {
                continue;
            };
            let (sin, cos) = degrees.to_radians().sin_cos();
            for pad in &footprint.pads {
                let Some(drill) = pad.drill else {
                    continue;
                };
                let px = pad.position.x.0 as f64;
                let py = pad.position.y.0 as f64;
                expected.push((
                    (position.x.0 as f64 + px * cos - py * sin) / 1_000_000.0,
                    (position.y.0 as f64 + px * sin + py * cos) / 1_000_000.0,
                    drill.0 as f64 / 1_000_000.0,
                ));
            }
        }

        if expected.is_empty() {
            continue;
        }

        let dir = export_to_temp("drills", &name, &mut world, &library);
        let drill_file = files_under(&dir)
            .into_iter()
            .find(|path| path.extension().and_then(|e| e.to_str()) == Some("drl"))
            .unwrap_or_else(|| panic!("{name} has drilled pads and exported no drill file"));
        let text = std::fs::read_to_string(&drill_file).expect("a readable drill file");
        let holes = excellon_holes(&text);
        assert!(
            !holes.is_empty(),
            "{name}: the drill file carries no holes at all:\n{text}"
        );

        for (x, y, diameter) in expected {
            holes_checked += 1;
            let hit = holes
                .iter()
                .find(|(hx, hy, _)| (hx - x).abs() < 0.002 && (hy - y).abs() < 0.002);
            match hit {
                None => wrong.push(format!("{name}: no hole at ({x:.3}mm, {y:.3}mm)")),
                Some((_, _, drilled)) if (drilled - diameter).abs() > 0.002 => {
                    wrong.push(format!(
                        "{name}: hole at ({x:.3}mm, {y:.3}mm) is {drilled:.3}mm, the footprint asks for {diameter:.3}mm"
                    ));
                }
                Some(_) => {}
            }
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    assert!(
        wrong.is_empty(),
        "the drill file and the copper disagree:\n{}",
        wrong.join("\n")
    );
    assert!(holes_checked > 0, "no drilled pads were found to check");
    eprintln!("{holes_checked} drilled pads matched to holes of the size they asked for");
}

/// Every hole in an Excellon file, as (x mm, y mm, diameter mm).
///
/// The exporter writes metric decimals with a tool table: `T1C1.000000` sets
/// the size, `T1` selects it, and the coordinates that follow use it.
fn excellon_holes(text: &str) -> Vec<(f64, f64, f64)> {
    let mut tools: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
    let mut current = 0.0;
    let mut out = Vec::new();

    for line in text.lines().map(str::trim) {
        if let Some((tool, size)) = line.split_once('C') {
            if tool.starts_with('T') && !tool.is_empty() {
                if let Ok(diameter) = size.parse::<f64>() {
                    tools.insert(tool.to_string(), diameter);
                    continue;
                }
            }
        }
        if line.starts_with('T') && line.len() > 1 && !line.contains('C') {
            current = tools.get(line).copied().unwrap_or(0.0);
            continue;
        }
        if line.starts_with('X') {
            if let Some((x, y)) = coordinates(line) {
                out.push((x, y, current));
            }
        }
    }
    out
}
