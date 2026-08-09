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

    assert!(
        out.len() >= 10,
        "expected the examples to be boards, got {}",
        out.len()
    );
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
    assert!(!result.files.is_empty(), "{name} exported nothing at all");
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

/// Where a file's `%FS...%` line says the decimal point is.
///
/// This test used to read `X3.730000` with `parse::<f64>()` and take the
/// answer as millimetres, which worked only because the exporter wrote a
/// decimal point the header said would not be there. `%FSLAX26Y26*%` declares
/// six implied decimals; a reader that ignores the declaration and trusts a
/// point in the data is not reading Gerber, and neither was this test.
///
/// Reading the declaration is also the only way this file can catch the two
/// disagreeing again: change the header to `X25` without changing the writer
/// and every coordinate below comes out ten times too big.
fn decimal_places(gerber: &str) -> u32 {
    let line = gerber
        .lines()
        .find(|line| line.starts_with("%FS"))
        .expect("a Gerber file states its coordinate format");
    let digits = line
        .split_once('X')
        .and_then(|(_, rest)| rest.get(..2))
        .expect("the format declaration carries an X field");
    digits[1..2]
        .parse()
        .unwrap_or_else(|_| panic!("unreadable coordinate format: {line}"))
}

/// Pull `X`/`Y` out of a Gerber command, in millimetres.
fn coordinates(line: &str, decimals: u32) -> Option<(f64, f64)> {
    let scale = 10f64.powi(decimals as i32);
    let x = between(line, 'X')? / scale;
    let y = between(line, 'Y')? / scale;
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
    let decimals = decimal_places(gerber);
    gerber
        .lines()
        .filter(|line| line.contains("D03"))
        .filter_map(|line| coordinates(line, decimals))
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
            // Zero, because a drill file carries its own decimal point: its
            // `METRIC,TZ` header names no digit count, so the point has to be
            // in the data. Gerber is the other way round. Reading a Gerber
            // coordinate that still has a point in it now lands a thousandth
            // of a millimetre from the pad and fails loudly, which is the
            // point of taking the scale from the header instead of the data.
            // A slot is one milled path, `X..Y..G85X..Y..`, between the two
            // end centres of the bit's travel - so the hole itself is at their
            // midpoint, which is where the pad is. Reading only the first pair
            // puts every slot half its own length away from the pad it belongs
            // to, which is what this guard reported the day slots reached the
            // language.
            match line.split_once("G85") {
                Some((start, end)) => {
                    if let (Some((x0, y0)), Some((x1, y1))) =
                        (coordinates(start, 0), coordinates(end, 0))
                    {
                        out.push(((x0 + x1) / 2.0, (y0 + y1) / 2.0, current));
                    }
                }
                None => {
                    if let Some((x, y)) = coordinates(line, 0) {
                        out.push((x, y, current));
                    }
                }
            }
        }
    }
    out
}

/// Every placed part on a board, as (refdes, x mm, y mm).
fn placed_parts(world: &mut BoardWorld, library: &FootprintLibrary) -> Vec<(String, f64, f64)> {
    use cypcb_world::components::{FootprintRef, Position, RefDes};

    let ecs = world.ecs_mut();
    let mut query = ecs.query::<(&RefDes, &Position, &FootprintRef)>();
    let mut parts: Vec<(String, f64, f64)> = query
        .iter(ecs)
        // A mounting hole is a part on the board and not a part anybody
        // places: it has no copper, so there is nothing to solder and nothing
        // for a machine to pick. The placement file leaves it out on purpose,
        // and this counted it as missing.
        .filter(|(_, _, footprint_ref)| {
            library
                .get(&footprint_ref.0)
                .is_none_or(|footprint| !footprint.is_mechanical())
        })
        .map(|(refdes, position, _)| {
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
            // The job file sits beside the Gerbers and is not one: it is the
            // JSON that describes them, so it has no `M02*` and no coordinate
            // format. `gbrjob` starts with a g like every Gerber extension
            // does, which is how it landed here.
            if extension == "gbrjob" {
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
        let decimals = decimal_places(&text);
        let (mut min_x, mut min_y) = (f64::MAX, f64::MAX);
        let (mut max_x, mut max_y) = (f64::MIN, f64::MIN);
        for line in text
            .lines()
            .filter(|l| l.contains("D01") || l.contains("D02"))
        {
            let Some((x, y)) = coordinates(line, decimals) else {
                continue;
            };
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
        counts.outlines += 1;
        let (width, height) = (max_x - min_x, max_y - min_y);
        if (width - size.width.to_mm()).abs() > 0.001
            || (height - size.height.to_mm()).abs() > 0.001
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
        let mut expected_holes: Vec<(f64, f64, f64, bool)> = Vec::new();
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
                    expected_holes.push((x, y, drill.0 as f64 / 1_000_000.0, pad.is_non_plated()));
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
            // Two drill files now, and which one a hole is in is the whole
            // point of the split: a mounting hole in the plated file comes
            // back narrower than its screw and shorted to the copper it
            // passes. This used to take the first `.drl` it found, which
            // sorts to `-NPTH.drl`, and then reported every plated pin as
            // missing.
            let read_drill = |ending: &str| -> Option<Vec<(f64, f64, f64)>> {
                files
                    .iter()
                    .find(|path| {
                        path.file_name()
                            .is_some_and(|n| n.to_string_lossy().ends_with(ending))
                    })
                    .map(|path| {
                        excellon_holes(
                            &std::fs::read_to_string(path).expect("a readable drill file"),
                        )
                    })
            };
            let plated = read_drill("-PTH.drl").unwrap_or_default();
            let unplated = read_drill("-NPTH.drl").unwrap_or_default();
            assert!(
                !plated.is_empty() || !unplated.is_empty(),
                "{name} has drilled pads and exported no drill file with holes in it"
            );

            for (x, y, diameter, is_non_plated) in expected_holes {
                counts.holes += 1;
                let (wanted, other, which) = if is_non_plated {
                    (&unplated, &plated, "unplated")
                } else {
                    (&plated, &unplated, "plated")
                };

                match wanted
                    .iter()
                    .find(|(hx, hy, _)| (hx - x).abs() < 0.002 && (hy - y).abs() < 0.002)
                {
                    None => wrong.push(format!(
                        "{name}: no hole at ({x:.3}mm, {y:.3}mm) in the {which} drill file"
                    )),
                    Some((_, _, drilled)) if (drilled - diameter).abs() > 0.002 => wrong.push(
                        format!("{name}: hole at ({x:.3}mm, {y:.3}mm) is {drilled:.3}mm, the footprint asks for {diameter:.3}mm"),
                    ),
                    Some(_) => {}
                }

                // And not in the other file, which is the failure that costs
                // real money: a hole in both is drilled twice, and a mounting
                // hole in the plated file is plated.
                if other
                    .iter()
                    .any(|(hx, hy, _)| (hx - x).abs() < 0.002 && (hy - y).abs() < 0.002)
                {
                    wrong.push(format!(
                        "{name}: the hole at ({x:.3}mm, {y:.3}mm) is in both drill files"
                    ));
                }
            }
        }

        // ---- the assembly files account for the same board -------------------
        //
        // The CPL is what a machine follows and the BOM is what somebody buys
        // from. A board whose CPL is short by one part is assembled with a gap.
        let parts = placed_parts(world, library);
        if !parts.is_empty() {
            let cpl = assembly_file(&dir, "CPL.csv", name);
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
                    wrong.push(format!(
                        "{name}: {refdes} is on the board and not in the CPL"
                    ));
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

            let bom = assembly_file(&dir, "BOM.csv", name);
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
                    wrong.push(format!(
                        "{name}: {refdes} is on the board and not in the BOM"
                    ));
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
    assert!(
        counts.placements >= 40,
        "only {} placements",
        counts.placements
    );
    assert!(
        counts.bom_parts >= 40,
        "only {} BOM parts",
        counts.bom_parts
    );

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

    // A count cannot see copper that moved. `source_to_fabrication.rs` learned
    // that once: a polyline replacing a short segment with a long one leaves
    // the count unchanged and the board different. So every routed segment's
    // endpoints have to lie on copper actually drawn in the layer that carries
    // it - on a stroke, not merely equal to a vertex, because the smoother is
    // allowed to merge collinear runs.
    let read = |marker: &str| -> Vec<((f64, f64), (f64, f64))> {
        files
            .iter()
            .find(|path| path.to_string_lossy().ends_with(marker))
            .map(|path| drawn_strokes(&std::fs::read_to_string(path).expect("a layer")))
            .unwrap_or_default()
    };
    let top_strokes = read("F_Cu.gbr");
    let bottom_strokes = read("B_Cu.gbr");

    /// One end of a drawn line, in millimetres.
    type Point2 = (f64, f64);

    let routed: Vec<(cypcb_world::components::Layer, Point2, Point2)> = {
        let ecs = world.ecs_mut();
        let mut query = ecs.query::<&Trace>();
        query
            .iter(ecs)
            .flat_map(|trace| {
                trace.segments.iter().map(move |segment| {
                    (
                        trace.layer,
                        (
                            segment.start.x.0 as f64 / 1_000_000.0,
                            segment.start.y.0 as f64 / 1_000_000.0,
                        ),
                        (
                            segment.end.x.0 as f64 / 1_000_000.0,
                            segment.end.y.0 as f64 / 1_000_000.0,
                        ),
                    )
                })
            })
            .collect()
    };

    let mut off_copper: Vec<String> = Vec::new();
    let mut endpoints_checked = 0usize;
    for (layer, start, end) in &routed {
        let strokes = match layer {
            cypcb_world::components::Layer::BottomCopper => &bottom_strokes,
            _ => &top_strokes,
        };
        for point in [start, end] {
            endpoints_checked += 1;
            if !strokes.iter().any(|stroke| on_stroke(*point, *stroke)) {
                off_copper.push(format!(
                    "a routed end at ({:.3}mm, {:.3}mm) on {layer:?} has no copper under it",
                    point.0, point.1
                ));
            }
        }
    }

    assert!(
        off_copper.is_empty(),
        "the copper does not follow the routing:\n{}",
        off_copper.join("\n")
    );
    assert!(
        endpoints_checked >= 2 * segments,
        "only {endpoints_checked} endpoints checked"
    );
    eprintln!("{endpoints_checked} routed endpoints found on copper actually drawn");
    eprintln!(
        "{segments} routed segments, {draws} copper draws, {} pads and {} placements checked",
        counts.pads, counts.placements
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// Every stroke a Gerber layer draws, as pairs of points in millimetres.
///
/// `D02` lifts the pen and moves; `D01` draws from wherever it is to the point
/// given. Flashes (`D03`) are pads, not strokes.
fn drawn_strokes(gerber: &str) -> Vec<((f64, f64), (f64, f64))> {
    let mut strokes = Vec::new();
    let mut pen: Option<(f64, f64)> = None;
    let decimals = decimal_places(gerber);

    for line in gerber.lines() {
        let Some(point) = coordinates(line, decimals) else {
            continue;
        };
        if line.contains("D02") {
            pen = Some(point);
        } else if line.contains("D01") {
            if let Some(from) = pen {
                strokes.push((from, point));
            }
            pen = Some(point);
        }
    }
    strokes
}

/// Whether a point sits on a drawn stroke, within a tenth of the narrowest
/// trace this project allows.
fn on_stroke(point: (f64, f64), stroke: ((f64, f64), (f64, f64))) -> bool {
    let ((x1, y1), (x2, y2)) = stroke;
    let (dx, dy) = (x2 - x1, y2 - y1);
    let length_squared = dx * dx + dy * dy;

    let (nearest_x, nearest_y) = if length_squared < f64::EPSILON {
        (x1, y1)
    } else {
        let t = (((point.0 - x1) * dx + (point.1 - y1) * dy) / length_squared).clamp(0.0, 1.0);
        (x1 + t * dx, y1 + t * dy)
    };

    let (ex, ey) = (point.0 - nearest_x, point.1 - nearest_y);
    (ex * ex + ey * ey).sqrt() < 0.0127
}
