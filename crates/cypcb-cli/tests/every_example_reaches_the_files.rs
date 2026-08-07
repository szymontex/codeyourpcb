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

/// Every flash coordinate in a layer, in millimetres.
fn flashes(gerber: &str) -> Vec<(f64, f64)> {
    gerber
        .lines()
        .filter(|line| line.contains("D03"))
        .filter_map(coordinates)
        .collect()
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

/// Every placed part on a board, as (refdes, x mm, y mm).
fn placed_parts(world: &mut BoardWorld) -> Vec<(String, f64, f64)> {
    use cypcb_world::components::{Position, RefDes};

    let ecs = world.ecs_mut();
    let mut query = ecs.query::<(&RefDes, &Position)>();
    let mut parts: Vec<(String, f64, f64)> = query
        .iter(ecs)
        .map(|(refdes, position)| {
            (
                refdes.as_str().to_string(),
                position.0.x.0 as f64 / 1_000_000.0,
                position.0.y.0 as f64 / 1_000_000.0,
            )
        })
        .collect();
    // By reference designator: the coordinates are floats and the order only
    // has to be stable for the report to read the same twice.
    parts.sort_by(|a, b| a.0.cmp(&b.0));
    parts
}

/// Split a CSV line, keeping quoted fields whole.
///
/// The BOM groups designators as `"C1,C2"`, so splitting on every comma turns
/// one part into two and the count comes out right for the wrong reason.
fn csv_fields(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    for c in line.chars() {
        match c {
            '"' => quoted = !quoted,
            ',' if !quoted => fields.push(std::mem::take(&mut current)),
            _ => current.push(c),
        }
    }
    fields.push(current);
    fields
}

fn assembly_file(dir: &Path, suffix: &str, name: &str) -> String {
    let path = files_under(dir)
        .into_iter()
        .find(|path| path.to_string_lossy().ends_with(suffix))
        .unwrap_or_else(|| panic!("{name} exported no {suffix}"));
    std::fs::read_to_string(path).expect("a readable assembly file")
}

/// How much was actually looked at, so a sweep that inspected nothing cannot
/// pass by inspecting nothing.
#[derive(Default)]
struct Counts {
    layers: usize,
    outlines: usize,
    pads: usize,
    holes: usize,
    placements: usize,
    bom_parts: usize,
}

/// Export one board and hold its files to everything they can be held to.
///
/// Written once and called twice: for the examples as their designers drew
/// them, and for a board the autorouter has been over. The router is what
/// changes copper after the designer stops, and until this was reused nothing
/// checked what it writes.
fn check_the_files(
    tag: &str,
    name: &str,
    world: &mut BoardWorld,
    library: &FootprintLibrary,
    counts: &mut Counts,
) -> Vec<String> {
    use cypcb_world::components::{FootprintRef, Layer, Position, Rotation};

    let mut wrong: Vec<String> = Vec::new();

    {
        let dir = export_to_temp(tag, name, world, library);
        let files = files_under(&dir);

        // ---- every layer is a complete Gerber -------------------------------
        //
        // A layer without `M02*` is a file a fabricator's loader stops reading
        // part way through, and one truncated layer is a board with a side
        // missing.
        for path in &files {
            let Some(extension) = path.extension().and_then(|e| e.to_str()) else {
                continue;
            };
            if !extension.starts_with('g') && extension != "gbr" {
                continue;
            }
            let text = std::fs::read_to_string(path).expect("a readable layer");
            counts.layers += 1;
            let file = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
            if !text.contains("M02*") {
                wrong.push(format!("{name}/{file}: no end-of-file marker"));
            }
            if !text.contains("%FSLAX") {
                wrong.push(format!("{name}/{file}: no coordinate format header"));
            }
        }

        // ---- the outline is the board the source declares -------------------
        //
        // A board that declares 50x30mm and cuts to something else is scrap,
        // and nothing else in the file set would show it.
        let (size, _) = world.board_info().expect("a board");
        let outline = files
            .iter()
            .find(|path| {
                path.extension().and_then(|e| e.to_str()) == Some("gko")
                    || path.to_string_lossy().contains("Edge_Cuts")
            })
            .unwrap_or_else(|| panic!("{name} exported no outline"));
        let text = std::fs::read_to_string(outline).expect("a readable outline");
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
        counts.outlines += 1;
        let (width, height) = (max_x - min_x, max_y - min_y);
        if (width - size.width.to_mm()).abs() > 0.001 || (height - size.height.to_mm()).abs() > 0.001
        {
            wrong.push(format!(
                "{name}: declares {:.3}mm x {:.3}mm, cuts {width:.3}mm x {height:.3}mm",
                size.width.to_mm(),
                size.height.to_mm()
            ));
        }

        // ---- every pad reaches copper, every drilled pad gets its hole ------
        //
        // A pad that never reaches copper is a part that cannot be soldered; a
        // hole of the wrong size is a lead that will not go in. The file set
        // looks perfectly well formed without either.
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

        let mut expected_pads: Vec<(Layer, f64, f64)> = Vec::new();
        let mut expected_holes: Vec<(f64, f64, f64)> = Vec::new();
        for (position, degrees, footprint_name) in &placements {
            let Some(footprint) = library.get(footprint_name) else {
                continue;
            };
            let (sin, cos) = degrees.to_radians().sin_cos();
            for pad in &footprint.pads {
                let px = pad.position.x.0 as f64;
                let py = pad.position.y.0 as f64;
                let x = (position.x.0 as f64 + px * cos - py * sin) / 1_000_000.0;
                let y = (position.y.0 as f64 + px * sin + py * cos) / 1_000_000.0;
                for layer in &pad.layers {
                    if matches!(layer, Layer::TopCopper | Layer::BottomCopper) {
                        expected_pads.push((*layer, x, y));
                    }
                }
                if let Some(drill) = pad.drill {
                    expected_holes.push((x, y, drill.0 as f64 / 1_000_000.0));
                }
            }
        }

        if !expected_pads.is_empty() {
            // The preset decides the names: JLCPCB writes `-F_Cu.gbr`, others
            // write `.gtl`. Matching one spelling read an empty file and
            // reported every pad as missing, which is the silent zero this
            // project keeps banning.
            let read_layer = |markers: &[&str]| -> String {
                files
                    .iter()
                    .find(|path| {
                        let file = path.to_string_lossy();
                        markers.iter().any(|marker| file.ends_with(marker))
                    })
                    .map(|path| std::fs::read_to_string(path).expect("a readable layer"))
                    .unwrap_or_else(|| panic!("{name} exported no top or bottom copper"))
            };
            let top = flashes(&read_layer(&["F_Cu.gbr", ".gtl"]));
            let bottom = flashes(&read_layer(&["B_Cu.gbr", ".gbl"]));

            for (layer, x, y) in expected_pads {
                counts.pads += 1;
                let candidates = match layer {
                    Layer::TopCopper => &top,
                    _ => &bottom,
                };
                if !candidates
                    .iter()
                    .any(|(fx, fy)| (fx - x).abs() < 0.002 && (fy - y).abs() < 0.002)
                {
                    wrong.push(format!(
                        "{name}: no copper flashed at ({x:.3}mm, {y:.3}mm) on {layer:?}"
                    ));
                }
            }
        }

        if !expected_holes.is_empty() {
            let drill_file = files
                .iter()
                .find(|path| path.extension().and_then(|e| e.to_str()) == Some("drl"))
                .unwrap_or_else(|| panic!("{name} has drilled pads and exported no drill file"));
            let holes = excellon_holes(
                &std::fs::read_to_string(drill_file).expect("a readable drill file"),
            );
            assert!(!holes.is_empty(), "{name}: the drill file carries no holes");

            for (x, y, diameter) in expected_holes {
                counts.holes += 1;
                match holes
                    .iter()
                    .find(|(hx, hy, _)| (hx - x).abs() < 0.002 && (hy - y).abs() < 0.002)
                {
                    None => wrong.push(format!("{name}: no hole at ({x:.3}mm, {y:.3}mm)")),
                    Some((_, _, drilled)) if (drilled - diameter).abs() > 0.002 => wrong.push(
                        format!("{name}: hole at ({x:.3}mm, {y:.3}mm) is {drilled:.3}mm, the footprint asks for {diameter:.3}mm"),
                    ),
                    Some(_) => {}
                }
            }
        }

        // ---- the assembly files account for the same board -------------------
        //
        // The CPL is what a machine follows and the BOM is what somebody buys
        // from. A board whose CPL is short by one part is assembled with a gap.
        let parts = placed_parts(world);
        if !parts.is_empty() {
            let cpl = assembly_file(&dir, "CPL.csv", &name);
            let rows: Vec<Vec<String>> = cpl
                .lines()
                .skip(1)
                .filter(|line| !line.trim().is_empty())
                .map(csv_fields)
                .collect();

            if rows.len() != parts.len() {
                wrong.push(format!(
                    "{name}: {} parts on the board, {} rows in the CPL",
                    parts.len(),
                    rows.len()
                ));
            }

            for (refdes, x, y) in &parts {
                counts.placements += 1;
                let Some(row) = rows
                    .iter()
                    .find(|row| row.first().map(String::as_str) == Some(refdes))
                else {
                    wrong.push(format!("{name}: {refdes} is on the board and not in the CPL"));
                    continue;
                };
                let read = |field: Option<&String>| -> Option<f64> {
                    field?.trim().trim_end_matches("mm").parse::<f64>().ok()
                };
                match (read(row.get(1)), read(row.get(2))) {
                    (Some(cx), Some(cy)) if (cx - x).abs() < 0.002 && (cy - y).abs() < 0.002 => {}
                    (Some(cx), Some(cy)) => wrong.push(format!(
                        "{name}: {refdes} sits at ({x:.3}mm, {y:.3}mm) and the CPL says ({cx:.3}mm, {cy:.3}mm)"
                    )),
                    _ => wrong.push(format!("{name}: {refdes} has no readable position in the CPL")),
                }
            }

            let bom = assembly_file(&dir, "BOM.csv", &name);
            let mut listed: Vec<String> = Vec::new();
            let mut quantity_total = 0usize;
            for line in bom.lines().skip(1).filter(|line| !line.trim().is_empty()) {
                let fields = csv_fields(line);
                for refdes in fields.first().map(String::as_str).unwrap_or("").split(',') {
                    let refdes = refdes.trim();
                    if !refdes.is_empty() {
                        listed.push(refdes.to_string());
                    }
                }
                quantity_total += fields
                    .get(2)
                    .and_then(|q| q.trim().parse::<usize>().ok())
                    .unwrap_or(0);
            }
            let unique: std::collections::BTreeSet<&String> = listed.iter().collect();
            if unique.len() != listed.len() {
                wrong.push(format!("{name}: a part is listed twice in the BOM"));
            }
            if quantity_total != parts.len() {
                wrong.push(format!(
                    "{name}: {} parts on the board, {quantity_total} accounted for in the BOM",
                    parts.len()
                ));
            }
            for (refdes, _, _) in &parts {
                counts.bom_parts += 1;
                if !unique.contains(refdes) {
                    wrong.push(format!("{name}: {refdes} is on the board and not in the BOM"));
                }
            }
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    wrong
}

#[test]
fn the_files_say_what_the_board_says() {
    let mut wrong: Vec<String> = Vec::new();
    let mut counts = Counts::default();

    for (name, mut world, library) in boards() {
        wrong.extend(check_the_files(
            "sweep",
            &name,
            &mut world,
            &library,
            &mut counts,
        ));
    }

    assert!(
        wrong.is_empty(),
        "the files do not say what the board says:\n{}",
        wrong.join("\n")
    );

    assert!(counts.layers >= 100, "only {} layers read", counts.layers);
    assert!(counts.outlines >= 10, "only {} outlines", counts.outlines);
    assert!(counts.pads >= 40, "only {} pads", counts.pads);
    assert!(counts.holes > 0, "no drilled pads were checked");
    assert!(counts.placements >= 40, "only {} placements", counts.placements);
    assert!(counts.bom_parts >= 40, "only {} BOM parts", counts.bom_parts);

    eprintln!(
        "{} layers, {} outlines, {} pads, {} holes, {} placements, {} BOM lines checked",
        counts.layers,
        counts.outlines,
        counts.pads,
        counts.holes,
        counts.placements,
        counts.bom_parts
    );
}

#[test]
fn a_routed_board_reaches_the_files_too() {
    // Every example is checked as its designer drew it. The router is what
    // changes copper after the designer stops, and nothing held what it writes
    // to the same standard: a trace that reaches the board model and not the
    // Gerber is a connection the fabricator never makes.
    use cypcb_autoroute::{route_board, AutorouteConfig};
    use cypcb_router::apply_routes;
    use cypcb_rules::presets::{PresetRuleSet, RulesPreset};
    use cypcb_world::components::trace::Trace;

    let file = examples_dir().join("blink.cypcb");
    let source = std::fs::read_to_string(&file).expect("the example is there");
    let parsed = cypcb_parser::parse(&source);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);

    let mut world = BoardWorld::new();
    let mut library = FootprintLibrary::new();
    let result = sync_ast_to_world(&parsed.value, &source, &mut world, &mut library);
    assert!(result.errors.is_empty(), "{:?}", result.errors);

    let rules = PresetRuleSet::new(RulesPreset::from_name("jlcpcb").expect("the preset"));
    let routing = route_board(&mut world, &library, &rules, &AutorouteConfig::default());
    assert!(
        routing.route_count() > 0,
        "the router produced nothing to check: {:?}",
        routing.status
    );
    apply_routes(&mut world, &routing);
    world.rebuild_spatial_index_from_library(&library);

    // Everything the drawn boards are held to.
    let mut counts = Counts::default();
    let wrong = check_the_files("routed", "blink-routed", &mut world, &library, &mut counts);
    assert!(
        wrong.is_empty(),
        "a routed board does not reach its files:\n{}",
        wrong.join("\n")
    );

    // And the part only a routed board has: the copper the router laid has to
    // be in the layer files, not only in the model.
    let dir = export_to_temp("routed-copper", "blink-routed", &mut world, &library);
    let files = files_under(&dir);
    let copper: String = files
        .iter()
        .filter(|path| {
            let name = path.to_string_lossy();
            name.ends_with("F_Cu.gbr") || name.ends_with("B_Cu.gbr")
        })
        .map(|path| std::fs::read_to_string(path).expect("a readable layer"))
        .collect();
    let draws = copper.lines().filter(|line| line.contains("D01")).count();

    let segments: usize = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<&Trace>();
        query.iter(ecs).map(|trace| trace.segments.len()).sum()
    };

    assert!(
        draws >= segments,
        "the router laid {segments} segments and the copper layers draw {draws} times"
    );
    eprintln!(
        "{segments} routed segments, {draws} copper draws, {} pads and {} placements checked",
        counts.pads, counts.placements
    );

    let _ = std::fs::remove_dir_all(&dir);
}
