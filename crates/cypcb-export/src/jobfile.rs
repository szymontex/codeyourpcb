//! The Gerber job file: what the eleven Gerbers together are meant to be.
//!
//! A directory of Gerbers says what to image on each layer and nothing about
//! the board. How thick it is, what goes between the copper layers, which file
//! is which - a fabricator either reads that out of an email or guesses it. The
//! Gerber Job File is Ucamco's answer: one JSON sidecar, `<board>-job.gbrjob`,
//! that names the files and describes the build.
//!
//! It is also where a design's own `stackup { ... }` finally reaches the person
//! who has to press it. Until this existed, a design could state its build,
//! have it checked against the rest of the design, and then export fifteen
//! files that carried none of it.
//!
//! # What is written, and what is deliberately left out
//!
//! Everything here comes from the design or from the files that were just
//! written. Nothing is filled in from what a typical board looks like:
//!
//! - `BoardThickness` appears only when every layer of the stackup states a
//!   thickness. A partial sum is not a thickness.
//! - `MaterialStackup` appears only when the design declares one. A build
//!   invented here would be a fabrication instruction nobody wrote.
//! - `Material` is never written on a dielectric. FR4 is the usual answer and
//!   this tool has not been told it.
//! - `Finish` and `DesignRules` are absent. The fab decides the finish, and the
//!   design rules a board was checked against live in the checker's preset
//!   rather than in the exporter.
//!
//! `FileFunction` is read back out of each Gerber rather than recomputed, so
//! the job file cannot disagree with the files it describes.

use std::path::Path;

use cypcb_world::components::{Stackup, StackupLayerKind};
use cypcb_world::BoardWorld;
use serde_json::{json, Map, Value};

/// One file, as the job file describes it.
struct FileEntry {
    path: String,
    function: String,
    /// `Gerber` for an image, `NC` for an Excellon drill file.
    ///
    /// The specification lists `Gerber|XNC|NC|SM|IPC356|Other` and puts
    /// Excellon under `NC`. It matters: a CAM system told a drill file is a
    /// Gerber tries to read it as one.
    format: &'static str,
}

/// The `TF.FileFunction` a Gerber states about itself.
fn stated_function(gerber: &str) -> Option<String> {
    gerber
        .lines()
        .find_map(|line| line.split("TF.FileFunction,").nth(1))
        .map(|rest| rest.trim_end_matches('*').trim().to_string())
}

/// Whether a layer is imaged as what it keeps or as what it removes.
///
/// This exporter's mask file draws the openings - `export_soldermask` says so
/// and the file holds one flash per exposed pad - so the image is the negative
/// of the layer, which is what the job format specification's own example
/// states: `"FileFunction": "Soldermask,Top"` with `"FilePolarity":
/// "Negative"`. Everything else here draws the copper or the ink itself.
///
/// Two vocabularies meet in this file and they disagree on one letter: a file
/// function says `Soldermask` and a material stackup entry says `SolderMask`.
/// Both spellings are the specification's. The first version of this matched
/// the stackup spelling against a file function and called every mask
/// positive, which is a board that comes back with mask over its pads.
fn polarity(function: &str) -> &'static str {
    if function.to_ascii_lowercase().starts_with("soldermask") {
        "Negative"
    } else {
        "Positive"
    }
}

/// A stable identifier for the project, derived from its name.
///
/// The Gerber job format asks for a GUID. This project has no identity to
/// remember one by and no random source it wants in an export, so the name's
/// own bytes are laid out in the shape of a UUID - the same thing KiCad writes,
/// and stable across exports of the same board, which is what a fabricator
/// comparing two revisions actually needs.
fn guid_from_name(name: &str) -> String {
    let mut bytes: Vec<u8> = name.bytes().take(16).collect();
    bytes.resize(16, b'.');
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

/// The job file's word for a stackup layer, or `None` for a layer that is not
/// a material of the bare board.
///
/// Solder paste is the only `None`. The specification asks for "all layers of
/// the PCB, and only those materials", and paste is deposited through a
/// stencil at assembly - it is not part of what the fabricator delivers. A
/// design can still declare one, because KiCad's stackup carries it; this file
/// is where it stops.
fn material_type(kind: StackupLayerKind) -> Option<&'static str> {
    Some(match kind {
        StackupLayerKind::Copper => "Copper",
        // Prepreg and core are both dielectric to a fabricator reading this
        // file; which one it is stays in the note beside it.
        StackupLayerKind::Prepreg | StackupLayerKind::Core => "Dielectric",
        StackupLayerKind::Mask => "SolderMask",
        StackupLayerKind::Silk => "Legend",
        // The film over a flexible section, in the specification's own word.
        StackupLayerKind::Coverlay => "Coverlay",
        // A stiffener is bonded on after the board is built, the way paste is
        // deposited at assembly: it is not a material of the bare board, so
        // this file is where it stops.
        StackupLayerKind::Stiffener => return None,
        StackupLayerKind::Paste => return None,
    })
}

/// Add the surface materials the export wrote but the design did not declare.
///
/// The specification is strict about this array: *"If the Material Stackup is
/// included, it must be complete - all layers of the PCB, must be present, and
/// only those materials."* A design states its stackup to control what is
/// between the copper layers, and most say nothing about mask or silkscreen -
/// so writing the declaration through unchanged would claim a board with no
/// solder mask, in the same file that lists two solder mask Gerbers.
///
/// What is added is not a guess: it is the set of files this export just
/// wrote. No thickness and no colour, because neither is known. A design that
/// does mention mask or silk in its stackup is taken as complete and passed
/// through - at that point the designer is describing the whole board and this
/// has no business adding to it.
fn complete_stackup(declared: Vec<Value>, stackup: &Stackup, files: &[FileEntry]) -> Vec<Value> {
    let mentions_surfaces = stackup
        .layers
        .iter()
        .any(|layer| matches!(layer.kind, StackupLayerKind::Mask | StackupLayerKind::Silk));
    if mentions_surfaces {
        return declared;
    }

    let wrote = |function: &str| files.iter().any(|file| file.function.starts_with(function));
    let surface = |kind: &str, side: &str| json!({ "Type": kind, "Notes": format!("{side} of the board, from the exported files") });

    let mut complete = Vec::new();
    if wrote("Legend,Top") {
        complete.push(surface("Legend", "top"));
    }
    if wrote("Soldermask,Top") {
        complete.push(surface("SolderMask", "top"));
    }
    complete.extend(declared);
    if wrote("Soldermask,Bot") {
        complete.push(surface("SolderMask", "bottom"));
    }
    if wrote("Legend,Bot") {
        complete.push(surface("Legend", "bottom"));
    }
    complete
}

/// Build the job file for a board and the Gerbers just written for it.
///
/// `gerbers` are paths to files already on disk; each is read for the function
/// it states about itself, and a file that states none is left out rather than
/// described by a guess.
pub fn build_job_file(
    world: &BoardWorld,
    board_name: &str,
    written: &[&Path],
    output_dir: &Path,
) -> String {
    let entries: Vec<FileEntry> = written
        .iter()
        .filter_map(|path| {
            let content = std::fs::read_to_string(path).ok()?;
            let is_gerber = path.extension().is_some_and(|ext| ext == "gbr");
            // Relative to the job file, which sits at the root of the set:
            // `gerber/board-F_Cu.gbr`, `drill/board-PTH.drl`. A bare file name
            // would be unresolvable for anything outside the job file's own
            // directory.
            let relative = path.strip_prefix(output_dir).unwrap_or(path);
            Some(FileEntry {
                path: relative.to_string_lossy().replace('\\', "/"),
                function: stated_function(&content)?,
                format: if is_gerber { "Gerber" } else { "NC" },
            })
        })
        .collect();

    let files: Vec<Value> = entries
        .iter()
        .map(|entry| {
            let mut file = Map::new();
            file.insert("Path".to_string(), json!(entry.path));
            file.insert("FileFunction".to_string(), json!(entry.function));
            // A drill file images nothing, so it has no polarity - the
            // specification's own drill entry has none either, and writing
            // `Positive` there would describe an image that does not exist.
            if entry.format == "Gerber" {
                file.insert("FilePolarity".to_string(), json!(polarity(&entry.function)));
            }
            file.insert("FileFormat".to_string(), json!(entry.format));
            Value::Object(file)
        })
        .collect();

    let mut general = Map::new();
    general.insert(
        "ProjectId".to_string(),
        json!({
            "Name": board_name,
            "GUID": guid_from_name(board_name),
            "Revision": "1",
        }),
    );

    if let Some((size, layers)) = world.board_info() {
        general.insert(
            "Size".to_string(),
            json!({ "X": size.width.to_mm(), "Y": size.height.to_mm() }),
        );
        general.insert("LayerNumber".to_string(), json!(layers.count));
    }

    let stackup = world.stackup();
    if let Some(total) = stackup.and_then(|stackup| stackup.total_thickness()) {
        general.insert("BoardThickness".to_string(), json!(total.to_mm()));
    }

    let mut job = Map::new();
    job.insert(
        "Header".to_string(),
        json!({
            "GenerationSoftware": {
                "Vendor": "CodeYourPCB",
                "Application": "cypcb",
                "Version": env!("CARGO_PKG_VERSION"),
            },
            "CreationDate": chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%z").to_string(),
        }),
    );
    job.insert("GeneralSpecs".to_string(), Value::Object(general));
    job.insert("FilesAttributes".to_string(), Value::Array(files));

    if let Some(stackup) = stackup {
        let declared: Vec<Value> = stackup
            .layers
            .iter()
            .filter_map(|layer| {
                let material = material_type(layer.kind)?;
                let mut entry = Map::new();
                entry.insert("Type".to_string(), json!(material));
                if let Some(thickness) = layer.thickness {
                    entry.insert("Thickness".to_string(), json!(thickness.to_mm()));
                }
                entry.insert(
                    "Notes".to_string(),
                    json!(format!("declared as {}", layer.kind.as_str())),
                );
                Some(Value::Object(entry))
            })
            .collect();

        job.insert(
            "MaterialStackup".to_string(),
            Value::Array(complete_stackup(declared, stackup, &entries)),
        );
    }

    serde_json::to_string_pretty(&Value::Object(job)).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_solder_mask_is_the_only_negative() {
        // The spelling in a file function, which is the one that reaches here.
        assert_eq!(polarity("Soldermask,Top"), "Negative");
        assert_eq!(polarity("Soldermask,Bot"), "Negative");
        // And the stackup's spelling, so neither vocabulary can slip past.
        assert_eq!(polarity("SolderMask,Top"), "Negative");
        assert_eq!(polarity("Copper,L1,Top"), "Positive");
        assert_eq!(polarity("Legend,Top"), "Positive");
        assert_eq!(polarity("Profile,NP"), "Positive");
    }

    #[test]
    fn the_identifier_is_the_same_every_export() {
        assert_eq!(guid_from_name("board"), guid_from_name("board"));
        assert_ne!(guid_from_name("board"), guid_from_name("other"));
    }

    #[test]
    fn the_identifier_is_shaped_like_a_uuid_whatever_the_name() {
        for name in ["a", "", "a_very_long_board_name_beyond_sixteen_bytes"] {
            let guid = guid_from_name(name);
            let parts: Vec<usize> = guid.split('-').map(str::len).collect();
            assert_eq!(parts, vec![8, 4, 4, 4, 12], "{name} gave {guid}");
        }
    }

    #[test]
    fn a_file_that_says_nothing_about_itself_is_not_described() {
        assert_eq!(stated_function("G04 nothing here*\nM02*\n"), None);
        assert_eq!(
            stated_function("G04 #@! TF.FileFunction,Copper,L2,Inr*\n").as_deref(),
            Some("Copper,L2,Inr")
        );
    }
}
