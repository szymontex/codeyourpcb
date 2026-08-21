//! The fabricator gets told what the board is, not just what to image.
//!
//! `cargo test -p cypcb-export --test the_job_file_describes_the_board_it_ships_with`
//!
//! A directory of Gerbers says what to draw on each layer and nothing about the
//! board: how thick it is, what goes between the copper, which file is which.
//! A design could state `stackup { copper 0.035mm core 1.5mm copper 0.035mm }`,
//! have it checked against the rest of the design, and then export fifteen
//! files carrying none of it.
//!
//! The Gerber Job File is where that belongs - `<board>-job.gbrjob`, JSON,
//! Ucamco's format. What is checked here is that it agrees with the files it
//! ships beside, because a job file that disagrees is worse than none: a CAM
//! operator who trusts it builds the wrong board.

use std::path::PathBuf;

use cypcb_core::Nm;
use cypcb_export::job::{run_export, ExportJob};
use cypcb_export::presets::from_name;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::{BoardWorld, Stackup, StackupLayer, StackupLayerKind};
use serde_json::Value;

use StackupLayerKind::{Copper, Core, Mask, Paste, Prepreg, Silk};

/// A four-layer board, optionally stating what it is made of.
fn board(stackup: Option<&[(StackupLayerKind, Option<f64>)]>) -> BoardWorld {
    let mut world = BoardWorld::new();
    world.set_board(
        "sensor_hub".to_string(),
        (Nm::from_mm(40.0), Nm::from_mm(30.0)),
        4,
    );
    if let Some(spec) = stackup {
        world.set_stackup(Stackup {
            layers: spec
                .iter()
                .map(|(kind, thickness)| StackupLayer::new(*kind, thickness.map(Nm::from_mm)))
                .collect(),
        });
    }
    world
}

/// Export into a directory of its own and read the job file back.
fn exported(name: &str, world: &mut BoardWorld) -> (Value, PathBuf) {
    let dir = std::env::temp_dir().join(format!("cypcb-jobfile-{name}"));
    let _ = std::fs::remove_dir_all(&dir);

    let job = ExportJob {
        source_path: PathBuf::from("in-memory.cypcb"),
        output_dir: dir.clone(),
        preset: from_name("jlcpcb").expect("the preset is there"),
        board_name: "sensor_hub".to_string(),
    };
    let library = FootprintLibrary::new();
    run_export(&job, world, &library).expect("the export runs");

    let path = dir.join("sensor_hub-job.gbrjob");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("no job file at {}: {err}", path.display()));
    (
        serde_json::from_str(&text).expect("the job file is JSON"),
        dir,
    )
}

const FOUR_LAYER: &[(StackupLayerKind, Option<f64>)] = &[
    (Copper, Some(0.035)),
    (Prepreg, Some(0.2)),
    (Copper, Some(0.0175)),
    (Core, Some(1.065)),
    (Copper, Some(0.0175)),
    (Prepreg, Some(0.2)),
    (Copper, Some(0.035)),
];

/// Every manufacturing file in the output, as a path relative to its root.
fn manufacturing_files(dir: &std::path::Path) -> Vec<String> {
    let mut found: Vec<String> = ["gerber", "drill"]
        .iter()
        .flat_map(|sub| {
            std::fs::read_dir(dir.join(sub))
                .into_iter()
                .flatten()
                .filter_map(move |entry| {
                    let name = entry.ok()?.file_name().to_string_lossy().to_string();
                    (name.ends_with(".gbr") || name.ends_with(".drl") || name.ends_with(".xln"))
                        .then(|| format!("{sub}/{name}"))
                })
        })
        .collect();
    found.sort();
    found
}

#[test]
fn every_file_written_is_named_in_the_job_file() {
    let (job, dir) = exported("named", &mut board(Some(FOUR_LAYER)));

    let mut described: Vec<String> = job["FilesAttributes"]
        .as_array()
        .expect("FilesAttributes is an array")
        .iter()
        .map(|entry| entry["Path"].as_str().unwrap_or_default().to_string())
        .collect();
    described.sort();

    assert_eq!(
        described,
        manufacturing_files(&dir),
        "the job file has to name the files it ships with"
    );
}

#[test]
fn every_path_it_names_resolves_from_where_the_job_file_sits() {
    // The job file is at the root of the set and the files are in `gerber/`
    // and `drill/`, so a bare file name would be unresolvable for half of
    // them. A fabricator following these paths has to find every file.
    let (job, dir) = exported("paths", &mut board(Some(FOUR_LAYER)));

    for entry in job["FilesAttributes"].as_array().expect("an array") {
        let named = entry["Path"].as_str().expect("a path");
        assert!(
            dir.join(named).is_file(),
            "{named} is named in the job file and is not there"
        );
    }
}

#[test]
fn what_each_file_is_called_is_what_that_file_says_it_is() {
    // Read back rather than recomputed: this is what stops the job file and
    // the Gerbers drifting apart, and it is how the four-layer numbering fault
    // would have shown up in a second place.
    let (job, dir) = exported("agrees", &mut board(Some(FOUR_LAYER)));

    for entry in job["FilesAttributes"].as_array().expect("an array") {
        let path = dir.join(entry["Path"].as_str().unwrap());
        let gerber = std::fs::read_to_string(&path).expect("the file is there");
        let stated = gerber
            .lines()
            .find_map(|line| line.split("TF.FileFunction,").nth(1))
            .map(|rest| rest.trim_end_matches('*').to_string())
            .expect("the file states its function");

        assert_eq!(
            entry["FileFunction"].as_str().unwrap(),
            stated,
            "{} is described as one thing and says another",
            entry["Path"]
        );
    }
}

#[test]
fn the_drill_file_is_in_the_set_the_fabricator_is_told_about() {
    // Eleven Gerbers described and the drill file mentioned nowhere is a board
    // with no holes. It states its own function now, so it is described the
    // same way everything else is - by reading it.
    let (job, _) = exported("drill", &mut board(Some(FOUR_LAYER)));

    let drills: Vec<&Value> = job["FilesAttributes"]
        .as_array()
        .expect("an array")
        .iter()
        .filter(|entry| entry["FileFormat"] == "NC")
        .collect();

    assert_eq!(drills.len(), 1, "one plated through file: {drills:#?}");
    assert_eq!(drills[0]["FileFunction"], "Plated,1,4,PTH");
    assert!(
        drills[0].get("FilePolarity").is_none(),
        "a drill file images nothing, so it has no polarity: {:#?}",
        drills[0]
    );
}

#[test]
fn every_entry_says_which_format_it_is_in() {
    // The specification lists Gerber|XNC|NC|SM|IPC356|Other and puts Excellon
    // under NC. A CAM system told a drill file is a Gerber reads it as one.
    let (job, _) = exported("formats", &mut board(Some(FOUR_LAYER)));

    for entry in job["FilesAttributes"].as_array().expect("an array") {
        let format = entry["FileFormat"].as_str().unwrap_or("missing");
        let expected = if entry["Path"].as_str().unwrap().ends_with(".gbr") {
            "Gerber"
        } else {
            "NC"
        };
        assert_eq!(format, expected, "{}", entry["Path"]);
    }
}

#[test]
fn the_solder_mask_is_the_negative_it_draws() {
    // The exporter images the openings, so the mask files are negatives. The
    // specification's own example says the same. Getting this wrong is a board
    // that comes back with solder mask over every pad.
    let (job, _) = exported("polarity", &mut board(Some(FOUR_LAYER)));

    for entry in job["FilesAttributes"].as_array().expect("an array") {
        let function = entry["FileFunction"].as_str().unwrap();
        // Drill files image nothing and carry no polarity at all.
        let Some(polarity) = entry["FilePolarity"].as_str() else {
            continue;
        };
        let expected = if function.starts_with("Soldermask") {
            "Negative"
        } else {
            "Positive"
        };
        assert_eq!(polarity, expected, "{function}");
    }
}

#[test]
fn the_stackup_the_design_declared_is_in_the_file() {
    let (job, _) = exported("stackup", &mut board(Some(FOUR_LAYER)));

    let stackup = job["MaterialStackup"].as_array().expect("an array");
    let copper: Vec<f64> = stackup
        .iter()
        .filter(|entry| entry["Type"] == "Copper")
        .filter_map(|entry| entry["Thickness"].as_f64())
        .collect();
    assert_eq!(
        copper,
        vec![0.035, 0.0175, 0.0175, 0.035],
        "four copper layers, the thicknesses the design stated"
    );

    let dielectrics = stackup.iter().filter(|e| e["Type"] == "Dielectric").count();
    assert_eq!(dielectrics, 3, "prepreg, core, prepreg");

    // 0.035 + 0.2 + 0.0175 + 1.065 + 0.0175 + 0.2 + 0.035
    assert_eq!(job["GeneralSpecs"]["BoardThickness"].as_f64(), Some(1.57));
}

#[test]
fn the_stackup_is_complete_or_it_is_not_written() {
    // The specification: "If the Material Stackup is included, it must be
    // complete - all layers of the PCB, must be present". A design states its
    // dielectrics and says nothing about mask or silkscreen, so passing the
    // declaration through unchanged would claim a board with no solder mask in
    // the same file that lists two solder mask Gerbers.
    let (job, _) = exported("complete", &mut board(Some(FOUR_LAYER)));
    let stackup = job["MaterialStackup"].as_array().expect("an array");

    let types: Vec<&str> = stackup
        .iter()
        .map(|entry| entry["Type"].as_str().unwrap_or_default())
        .collect();

    assert_eq!(types.first(), Some(&"Legend"));
    assert_eq!(types.get(1), Some(&"SolderMask"));
    assert_eq!(types[types.len() - 2], "SolderMask");
    assert_eq!(types[types.len() - 1], "Legend");
    assert_eq!(
        types.iter().filter(|t| **t == "Copper").count(),
        4,
        "and the copper the design declared is untouched: {types:?}"
    );
}

#[test]
fn a_design_that_describes_its_own_surfaces_is_left_alone() {
    // At that point the designer is describing the whole board, and adding to
    // it would be this tool overruling them.
    let spec: &[(StackupLayerKind, Option<f64>)] = &[
        (Silk, None),
        (Mask, Some(0.025)),
        (Copper, Some(0.035)),
        (Core, Some(1.5)),
        (Copper, Some(0.035)),
        (Mask, Some(0.025)),
        (Silk, None),
    ];
    let (job, _) = exported("surfaces", &mut board(Some(spec)));

    let stackup = job["MaterialStackup"].as_array().expect("an array");
    assert_eq!(stackup.len(), 7, "nothing was added: {stackup:#?}");
    assert_eq!(stackup[1]["Thickness"].as_f64(), Some(0.025));
}

#[test]
fn a_board_that_declares_no_stackup_gets_no_stackup_written() {
    // Most designs take the fab's standard build. Inventing one here would be
    // a fabrication instruction nobody wrote.
    let (job, _) = exported("bare", &mut board(None));

    assert!(job.get("MaterialStackup").is_none(), "{job:#?}");
    assert!(
        job["GeneralSpecs"].get("BoardThickness").is_none(),
        "and no thickness either"
    );
    // What is known is still there.
    assert_eq!(job["GeneralSpecs"]["LayerNumber"].as_u64(), Some(4));
    assert_eq!(job["GeneralSpecs"]["Size"]["X"].as_f64(), Some(40.0));
}

#[test]
fn a_partial_thickness_is_not_a_board_thickness() {
    let spec: &[(StackupLayerKind, Option<f64>)] =
        &[(Copper, Some(0.035)), (Core, None), (Copper, Some(0.035))];
    let (job, _) = exported("partial", &mut board(Some(spec)));

    assert!(
        job["GeneralSpecs"].get("BoardThickness").is_none(),
        "a sum missing one of its three terms is not a thickness"
    );
    assert!(
        job.get("MaterialStackup").is_some(),
        "the stackup itself is still described"
    );
}

#[test]
fn solder_paste_is_left_out_of_the_material_stackup() {
    // The specification asks for "all layers of the PCB, and only those
    // materials". Paste is deposited through a stencil at assembly and is not
    // part of what the fabricator delivers, so a design may declare one and
    // this file still describes the bare board. The declaration is not an
    // error and nothing is added to make up for the omission - the design
    // named its own surfaces, so `complete_stackup` leaves it alone.
    let spec: &[(StackupLayerKind, Option<f64>)] = &[
        (Silk, None),
        (Paste, Some(0.1)),
        (Mask, Some(0.025)),
        (Copper, Some(0.035)),
        (Core, Some(1.5)),
        (Copper, Some(0.035)),
        (Mask, Some(0.025)),
        (Paste, Some(0.1)),
        (Silk, None),
    ];
    let (job, _) = exported("paste", &mut board(Some(spec)));

    let stackup = job["MaterialStackup"].as_array().expect("an array");
    let types: Vec<&str> = stackup
        .iter()
        .map(|entry| entry["Type"].as_str().unwrap_or_default())
        .collect();
    assert_eq!(
        types,
        vec![
            "Legend",
            "SolderMask",
            "Copper",
            "Dielectric",
            "Copper",
            "SolderMask",
            "Legend"
        ],
        "nine layers were declared and the two paste ones do not belong here: {stackup:#?}"
    );
}
