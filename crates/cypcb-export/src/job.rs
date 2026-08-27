//! Export job orchestration.
//!
//! Coordinates the generation of all manufacturing files according to a preset,
//! creating an organized output directory structure.
//!
//! # Examples
//!
//! ```no_run
//! use cypcb_export::job::{ExportJob, run_export};
//! use cypcb_export::presets::from_name;
//! use cypcb_world::BoardWorld;
//! use cypcb_world::footprint::FootprintLibrary;
//! use std::path::PathBuf;
//!
//! let mut world = BoardWorld::new();
//! let library = FootprintLibrary::new();
//! let preset = from_name("jlcpcb").unwrap();
//!
//! let job = ExportJob {
//!     source_path: PathBuf::from("board.cypcb"),
//!     output_dir: PathBuf::from("output"),
//!     preset,
//!     board_name: "board".to_string(),
//! };
//!
//! let result = run_export(&job, &mut world, &library).unwrap();
//! println!("Exported {} files", result.files.len());
//! ```

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use cypcb_world::components::Layer;
use cypcb_world::footprint::FootprintLibrary;
use cypcb_world::BoardWorld;

use crate::bom::{export_bom_csv, export_bom_json};
use crate::cpl::export_cpl;
use crate::excellon::{export_excellon, DrillType};
use crate::gerber::SilkWarning;
use crate::gerber::{
    export_copper_layer_with, export_outline, export_silkscreen_reporting, export_soldermask,
    export_solderpaste, MaskPasteConfig, Side, SilkConfig,
};
use crate::presets::ExportPreset;

/// Export job configuration.
#[derive(Debug, Clone)]
pub struct ExportJob {
    /// Source .cypcb file path
    pub source_path: PathBuf,
    /// Output directory for generated files
    pub output_dir: PathBuf,
    /// Manufacturer preset defining export parameters
    pub preset: ExportPreset,
    /// Board name (used for file naming)
    pub board_name: String,
}

/// Result of an export job.
#[derive(Debug)]
pub struct ExportResult {
    /// List of successfully exported files
    pub files: Vec<ExportedFile>,
    /// Warnings generated during export
    pub warnings: Vec<String>,
    /// Total export duration in milliseconds
    pub duration_ms: u64,
}

/// Information about an exported file.
#[derive(Debug)]
pub struct ExportedFile {
    /// File path relative to output directory
    pub path: PathBuf,
    /// Human-readable file type description
    pub file_type: String,
    /// File size in bytes
    pub size_bytes: u64,
}

/// Error during export.
#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("Board not found in world")]
    NoBoardEntity,

    #[error("Board size not defined")]
    NoBoardSize,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Export failed: {0}")]
    Export(String),
}

/// How a pour is filled for this job's fabricator.
///
/// `export_copper_layer`'s doc comment has said since it was written that
/// `ExportJob` passes the fab's clearance through `export_copper_layer_with`.
/// No caller ever did: every exported pour was filled with
/// `PourOptions::default()`, whose 0.3mm is generous rather than published,
/// so every plane shipped smaller on every edge than the house asked for.
///
/// The thermal numbers stay as the pour's own defaults, which are already the
/// fabs' published figures; only the clearance is per-house data this preset
/// carries.
fn pour_options(job: &ExportJob) -> crate::pour::PourOptions {
    crate::pour::PourOptions {
        clearance: job.preset.pour_clearance,
        ..Default::default()
    }
}

/// Run export job, generating all manufacturing files.
///
/// Creates output directory structure:
/// ```text
/// output/
/// ├── gerber/
/// │   ├── board-F_Cu.gbr
/// │   ├── board-B_Cu.gbr
/// │   ├── board-F_Mask.gbr
/// │   └── ...
/// ├── drill/
/// │   └── board-PTH.drl
/// └── assembly/
///     ├── board-BOM.csv
///     ├── board-BOM.json
///     └── board-CPL.csv
/// ```
pub fn run_export(
    job: &ExportJob,
    world: &mut BoardWorld,
    library: &FootprintLibrary,
) -> Result<ExportResult, ExportError> {
    run_export_with(job, world, library, None)
}

/// Run the same export, filleting where tracks meet pads.
///
/// Separate from [`run_export`] rather than a field on [`ExportJob`] because a
/// board that has never asked for teardrops must keep receiving the copper it
/// received yesterday: the default is the absence, and a caller has to say the
/// word. Item 1 of the KiCad parity audit.
pub fn run_export_with(
    job: &ExportJob,
    world: &mut BoardWorld,
    library: &FootprintLibrary,
    teardrops: Option<cypcb_world::teardrop::TeardropRatios>,
) -> Result<ExportResult, ExportError> {
    let start = Instant::now();
    let mut files = Vec::new();
    let mut warnings = Vec::new();

    // Create output directory structure
    let gerber_dir = job.output_dir.join("gerber");
    let drill_dir = job.output_dir.join("drill");
    let assembly_dir = job.output_dir.join("assembly");

    fs::create_dir_all(&gerber_dir)?;
    fs::create_dir_all(&drill_dir)?;
    if job.preset.assembly {
        fs::create_dir_all(&assembly_dir)?;
    }

    // Export Gerber layers
    if job.preset.layers.top_copper {
        let filename = format!("{}{}", job.board_name, job.preset.file_naming.top_copper);
        let path = gerber_dir.join(&filename);
        let content = export_copper_layer_with(
            world,
            library,
            Layer::TopCopper,
            &job.preset.coordinate_format,
            &pour_options(job),
            teardrops,
        )
        .map_err(|e| ExportError::Export(format!("{:?}", e)))?;
        let file = write_export_file(&path, &content, "Top Copper")?;
        files.push(file);
    }

    if job.preset.layers.bottom_copper {
        let filename = format!("{}{}", job.board_name, job.preset.file_naming.bottom_copper);
        let path = gerber_dir.join(&filename);
        let content = export_copper_layer_with(
            world,
            library,
            Layer::BottomCopper,
            &job.preset.coordinate_format,
            &pour_options(job),
            teardrops,
        )
        .map_err(|e| ExportError::Export(format!("{:?}", e)))?;
        let file = write_export_file(&path, &content, "Bottom Copper")?;
        files.push(file);
    }

    // Inner copper, driven by the board rather than by the preset.
    //
    // Both presets ship `inner_copper: vec![]`, so a four-layer design exported
    // as though it were two-layer: every trace the router put on In1 or In2 was
    // absent from the file set, silently. Measured on the multi_ic benchmark,
    // which declares F.Cu, In1.Cu, In2.Cu and B.Cu - the router uses the inner
    // pair and nothing carried it. The board's own stack is the truth here; a
    // preset that lists inner layers can still add to it.
    let inner_from_board = world
        .board_info()
        .map(|(_, stack)| stack.count.saturating_sub(2))
        .unwrap_or(0);
    let inner_count = inner_from_board.max(job.preset.layers.inner_copper.len() as u8);
    for index in 0..inner_count {
        let number = index + 1;
        let suffix = inner_layer_suffix(job.preset.file_naming.top_copper, number);
        let filename = format!("{}{}", job.board_name, suffix);
        let path = gerber_dir.join(&filename);
        let content = export_copper_layer_with(
            world,
            library,
            Layer::Inner(index),
            &job.preset.coordinate_format,
            &pour_options(job),
            teardrops,
        )
        .map_err(|e| ExportError::Export(format!("{:?}", e)))?;
        let file = write_export_file(&path, &content, &format!("Inner Copper {number}"))?;
        files.push(file);
    }

    if job.preset.layers.top_mask {
        let filename = format!("{}{}", job.board_name, job.preset.file_naming.top_mask);
        let path = gerber_dir.join(&filename);
        // The fab's own expansion rather than the constant the default
        // carries: a house asking for 0.04mm got its openings drawn at
        // 0.05mm, so the files disagreed with the rules the board was
        // checked against. Paste below keeps its own shrink.
        let config = MaskPasteConfig::default().with_mask_expansion(job.preset.mask_expansion);
        let content = export_soldermask(
            world,
            library,
            Side::Top,
            &job.preset.coordinate_format,
            &config,
        )
        .map_err(|e| ExportError::Export(format!("{:?}", e)))?;
        let file = write_export_file(&path, &content, "Top Soldermask")?;
        files.push(file);
    }

    if job.preset.layers.bottom_mask {
        let filename = format!("{}{}", job.board_name, job.preset.file_naming.bottom_mask);
        let path = gerber_dir.join(&filename);
        // The fab's own expansion rather than the constant the default
        // carries: a house asking for 0.04mm got its openings drawn at
        // 0.05mm, so the files disagreed with the rules the board was
        // checked against. Paste below keeps its own shrink.
        let config = MaskPasteConfig::default().with_mask_expansion(job.preset.mask_expansion);
        let content = export_soldermask(
            world,
            library,
            Side::Bottom,
            &job.preset.coordinate_format,
            &config,
        )
        .map_err(|e| ExportError::Export(format!("{:?}", e)))?;
        let file = write_export_file(&path, &content, "Bottom Soldermask")?;
        files.push(file);
    }

    if job.preset.layers.top_paste {
        let filename = format!("{}{}", job.board_name, job.preset.file_naming.top_paste);
        let path = gerber_dir.join(&filename);
        let config = MaskPasteConfig::default();
        let content = export_solderpaste(
            world,
            library,
            Side::Top,
            &job.preset.coordinate_format,
            &config,
        )
        .map_err(|e| ExportError::Export(format!("{:?}", e)))?;
        let file = write_export_file(&path, &content, "Top Solderpaste")?;
        files.push(file);
    }

    if job.preset.layers.bottom_paste {
        let filename = format!("{}{}", job.board_name, job.preset.file_naming.bottom_paste);
        let path = gerber_dir.join(&filename);
        let config = MaskPasteConfig::default();
        let content = export_solderpaste(
            world,
            library,
            Side::Bottom,
            &job.preset.coordinate_format,
            &config,
        )
        .map_err(|e| ExportError::Export(format!("{:?}", e)))?;
        let file = write_export_file(&path, &content, "Bottom Solderpaste")?;
        files.push(file);
    }

    if job.preset.layers.top_silk {
        let filename = format!("{}{}", job.board_name, job.preset.file_naming.top_silk);
        let path = gerber_dir.join(&filename);
        let config = SilkConfig {
            clearance: job.preset.silk_clearance,
            ..SilkConfig::default()
        };
        let (content, silk_warnings) = export_silkscreen_reporting(
            world,
            library,
            Side::Top,
            &job.preset.coordinate_format,
            &config,
        )
        .map_err(|e| ExportError::Export(format!("{:?}", e)))?;
        let file = write_export_file(&path, &content, "Top Silkscreen")?;
        files.push(file);
        warnings.extend(describe_clipped_names(&silk_warnings, "top"));
    }

    if job.preset.layers.bottom_silk {
        let filename = format!("{}{}", job.board_name, job.preset.file_naming.bottom_silk);
        let path = gerber_dir.join(&filename);
        let config = SilkConfig {
            clearance: job.preset.silk_clearance,
            ..SilkConfig::default()
        };
        let (content, silk_warnings) = export_silkscreen_reporting(
            world,
            library,
            Side::Bottom,
            &job.preset.coordinate_format,
            &config,
        )
        .map_err(|e| ExportError::Export(format!("{:?}", e)))?;
        let file = write_export_file(&path, &content, "Bottom Silkscreen")?;
        files.push(file);
        warnings.extend(describe_clipped_names(&silk_warnings, "bottom"));
    }

    if job.preset.layers.outline {
        let filename = format!("{}{}", job.board_name, job.preset.file_naming.outline);
        let path = gerber_dir.join(&filename);
        let content = export_outline(world, &job.preset.coordinate_format)
            .map_err(|e| ExportError::Export(format!("{:?}", e)))?;
        let file = write_export_file(&path, &content, "Board Outline")?;
        files.push(file);
    }

    // Export drill files
    if job.preset.layers.drill {
        let filename = format!("{}{}", job.board_name, job.preset.file_naming.drill_pth);
        let path = drill_dir.join(&filename);
        let content = export_excellon(
            world,
            library,
            &job.preset.coordinate_format,
            Some(DrillType::Plated),
        )
        .map_err(|e| ExportError::Export(format!("{:?}", e)))?;
        let file = write_export_file(&path, &content, "Drill PTH")?;
        files.push(file);

        // Every preset already names a file for holes that must not be plated
        // - `-NPTH.drl` for JLCPCB, `_npth.xln` for PCBWay - and nothing ever
        // wrote one. A mounting hole went into the plated file with the pads
        // and came back plated: narrower than the screw it was drilled for,
        // and connected to whatever copper it passes.
        //
        // Written only when the board has such a hole. An empty NPTH file is
        // not neutral - a fabricator reading one has to decide whether it
        // means "no mounting holes" or "the CAM step went wrong".
        let npth = export_excellon(
            world,
            library,
            &job.preset.coordinate_format,
            Some(DrillType::NonPlated),
        )
        .map_err(|e| ExportError::Export(format!("{:?}", e)))?;
        if npth.lines().any(|line| line.starts_with('X')) {
            let filename = format!("{}{}", job.board_name, job.preset.file_naming.drill_npth);
            let path = drill_dir.join(&filename);
            let file = write_export_file(&path, &npth, "Drill NPTH")?;
            files.push(file);
        }

        // Blind and buried vias join layers the through file cannot describe.
        // A drill file with no stated pair means "through the whole board" to
        // every fabricator, so these get one file per pair, named for it.
        let spans = crate::excellon::non_through_spans(world, library)
            .map_err(|e| ExportError::Export(format!("{:?}", e)))?;
        for (start, end) in spans {
            let pair = format!("{}-{}", layer_tag(start), layer_tag(end));
            let suffix = job
                .preset
                .file_naming
                .drill_pth
                .rsplit_once('.')
                .map(|(stem, extension)| format!("{stem}-{pair}.{extension}"))
                .unwrap_or_else(|| format!("{}-{pair}", job.preset.file_naming.drill_pth));
            let filename = format!("{}{}", job.board_name, suffix);
            let path = drill_dir.join(&filename);
            let content = crate::excellon::export_excellon_span(
                world,
                library,
                &job.preset.coordinate_format,
                Some(DrillType::Plated),
                (start, end),
            )
            .map_err(|e| ExportError::Export(format!("{:?}", e)))?;
            let file = write_export_file(&path, &content, &format!("Drill {pair}"))?;
            files.push(file);
        }
    }

    // Export assembly files
    if job.preset.assembly {
        // BOM CSV
        let filename = format!("{}{}", job.board_name, job.preset.file_naming.bom);
        let path = assembly_dir.join(&filename);
        let content = export_bom_csv(world).map_err(|e| ExportError::Export(format!("{:?}", e)))?;
        let file = write_export_file(&path, &content, "BOM CSV")?;
        files.push(file);

        // BOM JSON
        let filename = format!("{}.json", job.board_name);
        let path = assembly_dir.join(&filename);
        let content = export_bom_json(world, Some(&job.board_name))
            .map_err(|e| ExportError::Export(format!("{:?}", e)))?;
        let file = write_export_file(&path, &content, "BOM JSON")?;
        files.push(file);

        // CPL
        let filename = format!("{}{}", job.board_name, job.preset.file_naming.cpl);
        let path = assembly_dir.join(&filename);
        let content = export_cpl(world, library, None)
            .map_err(|e| ExportError::Export(format!("{:?}", e)))?;
        let file = write_export_file(&path, &content, "Pick-and-Place")?;
        files.push(file);
    }

    // What the export passed over without stopping.
    //
    // This vector existed and was always empty, which promises warnings that
    // never come. A fabricator does what the files say, so the things worth
    // saying out loud are the ones that produce a board nobody wanted: copper
    // with no traces on it is an unrouted design sent to be made, and a board
    // whose only holes are its through-hole pads has no vias at all - fine on
    // a one-layer design, a sign of an unfinished one otherwise.
    {
        let trace_count = {
            let ecs = world.ecs_mut();
            let mut query = ecs.query::<&cypcb_world::components::trace::Trace>();
            query.iter(ecs).count()
        };
        if trace_count == 0 {
            warnings.push("the board has no traces: this exports an unrouted design".to_string());
        }

        // A stiffener is part of the board a fabricator hands over and not
        // part of the bare board the job file describes: it is bonded on after
        // the stack is pressed, the way paste is deposited at assembly, and
        // `material_type` returns nothing for it on purpose. That is the right
        // call about the file and the wrong thing to do in silence - a design
        // that states one is asking for a board nobody can make from this set
        // of files alone.
        if let Some(stiffener) = world.stackup().and_then(|stackup| {
            stackup
                .layers
                .iter()
                .find(|layer| {
                    matches!(
                        layer.kind,
                        cypcb_world::components::StackupLayerKind::Stiffener
                    )
                })
                .cloned()
        }) {
            let thickness = stiffener
                .thickness
                .map(|nm| format!("{:.3}mm", nm.raw() as f64 / 1_000_000.0))
                .unwrap_or_else(|| "an unstated thickness".to_string());
            let material = stiffener
                .material
                .clone()
                .unwrap_or_else(|| "unstated material".to_string());
            warnings.push(format!(
                "the stiffener this design states ({thickness} of {material}) is not in these \
                 files: the job file describes the bare board and a stiffener is bonded on after \
                 it is built, so the fabricator has to be told about it another way"
            ));
        }

        // Nothing is said about copper pours, and that is the point.
        //
        // This warning has been wrong twice. It first said a declared pour was
        // not exported at all, which stopped being true when
        // `export_copper_layer` started filling zones; it was then changed to
        // say the pour is flooded solid without thermal relief, which stopped
        // being true in the very next commit, when a pad on the pour's own net
        // started getting a gap ring and four spokes. It kept printing on
        // every export of every board with a plane on it for as long as both
        // of those were false, and `the_pour_keeps_the_fabs_distance` and
        // `a_pour_keeps_clear_of_other_nets_and_reaches_its_own` were passing
        // the whole time. A warning is a claim, and an export that cries about
        // a plane it drew correctly teaches a user to skip the warnings that
        // matter.

        for file in &files {
            // Only the outline. A silkscreen or a paste layer with nothing on
            // it is ordinary - a board with parts on one side has an empty
            // legend on the other - and warning about those buries the one
            // that matters. A board with no cut path cannot be made at all.
            let is_outline = file
                .path
                .file_name()
                .map(|name| name.to_string_lossy().contains("Edge_Cuts"))
                .unwrap_or(false);
            if is_outline {
                if let Ok(content) = fs::read_to_string(&file.path) {
                    let draws = content
                        .lines()
                        .filter(|line| {
                            line.contains("D01") || line.contains("D02") || line.contains("D03")
                        })
                        .count();
                    if draws == 0 {
                        warnings.push(format!(
                            "{} has no geometry: a board with no cut path cannot be made",
                            file.path
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_default()
                        ));
                    }
                }
            }
        }
    }

    // The job file describes the set, so it is written last and reads the
    // files rather than being told about them: what it claims about each one
    // is what that file says about itself.
    //
    // The drill files are in it too: they state their own function now, so a
    // fabricator reading the job file no longer sees a board with no holes.
    let described: Vec<PathBuf> = files
        .iter()
        .filter(|file| {
            file.path
                .extension()
                .is_some_and(|ext| ext == "gbr" || ext == "drl" || ext == "xln")
        })
        .map(|file| file.path.clone())
        .collect();
    if !described.is_empty() {
        let borrowed: Vec<&Path> = described.iter().map(PathBuf::as_path).collect();
        let content =
            crate::jobfile::build_job_file(world, &job.board_name, &borrowed, &job.output_dir);
        // At the root of the set rather than inside `gerber/`: it describes
        // the drill files too, and a path written from inside one subdirectory
        // cannot name a file in another without climbing out of it.
        let path = job
            .output_dir
            .join(format!("{}-job.gbrjob", job.board_name));
        let file = write_export_file(&path, &content, "Gerber job file")?;
        files.push(file);
    }

    let duration_ms = start.elapsed().as_millis() as u64;

    Ok(ExportResult {
        files,
        warnings,
        duration_ms,
    })
}

/// Write content to file and return exported file info.
fn write_export_file(
    path: &Path,
    content: &str,
    file_type: &str,
) -> Result<ExportedFile, ExportError> {
    let mut file = fs::File::create(path)?;
    file.write_all(content.as_bytes())?;
    file.sync_all()?;

    let metadata = fs::metadata(path)?;

    Ok(ExportedFile {
        path: path.to_path_buf(),
        file_type: file_type.to_string(),
        size_bytes: metadata.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presets::from_name;
    use cypcb_core::Nm;

    fn setup_minimal_board() -> (BoardWorld, FootprintLibrary) {
        let mut world = BoardWorld::new();
        let library = FootprintLibrary::new();

        // Set board properties
        world.set_board(
            "test-board".to_string(),
            (Nm::from_mm(50.0), Nm::from_mm(50.0)),
            2,
        );

        (world, library)
    }

    #[test]
    fn test_export_job_creation() {
        let preset = from_name("jlcpcb").unwrap();
        let job = ExportJob {
            source_path: PathBuf::from("test.cypcb"),
            output_dir: PathBuf::from("/tmp/test-export"),
            preset,
            board_name: "test".to_string(),
        };

        assert_eq!(job.board_name, "test");
        assert_eq!(job.preset.name, "JLCPCB 2-Layer");
    }

    #[test]
    fn test_run_export_creates_directories() {
        let (mut world, library) = setup_minimal_board();
        let preset = from_name("jlcpcb").unwrap();

        let temp_dir = std::env::temp_dir().join(format!("cypcb-test-dirs-{}", std::process::id()));

        let job = ExportJob {
            source_path: PathBuf::from("test.cypcb"),
            output_dir: temp_dir.clone(),
            preset,
            board_name: "test".to_string(),
        };

        let _result = run_export(&job, &mut world, &library).unwrap();

        assert!(temp_dir.join("gerber").exists());
        assert!(temp_dir.join("drill").exists());
        assert!(temp_dir.join("assembly").exists());

        // Cleanup
        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_export_result_has_files() {
        let (mut world, library) = setup_minimal_board();
        let preset = from_name("jlcpcb").unwrap();

        let temp_dir =
            std::env::temp_dir().join(format!("cypcb-test-files-{}", std::process::id()));

        let job = ExportJob {
            source_path: PathBuf::from("test.cypcb"),
            output_dir: temp_dir.clone(),
            preset,
            board_name: "test".to_string(),
        };

        let result = run_export(&job, &mut world, &library).unwrap();

        // Should have generated multiple files
        assert!(!result.files.is_empty());

        // At least some files should have content (board outline, BOM, etc)
        let non_empty = result.files.iter().filter(|f| f.size_bytes > 0).count();
        assert!(non_empty > 0, "Expected at least one non-empty file");

        // Cleanup
        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_export_duration_tracked() {
        let (mut world, library) = setup_minimal_board();
        let preset = from_name("jlcpcb").unwrap();

        let temp_dir =
            std::env::temp_dir().join(format!("cypcb-test-duration-{}", std::process::id()));

        let job = ExportJob {
            source_path: PathBuf::from("test.cypcb"),
            output_dir: temp_dir.clone(),
            preset,
            board_name: "test".to_string(),
        };

        let result = run_export(&job, &mut world, &library).unwrap();

        // Duration should be tracked (u64 is always >= 0, just verify it exists)
        let _duration = result.duration_ms;

        // Cleanup
        let _ = fs::remove_dir_all(temp_dir);
    }
}

/// The file name an inner copper layer takes, in the preset's own style.
///
/// A preset that calls the top layer `-F_Cu.gbr` calls the first inner one
/// `-In1_Cu.gbr`; one that calls it `_top.gtl` gets `_inner1.gtl`. Guessing
/// from the top layer's name keeps the set looking like one set.
pub fn inner_layer_suffix(top_copper: &str, number: u8) -> String {
    if top_copper.contains("F_Cu") {
        return top_copper.replace("F_Cu", &format!("In{number}_Cu"));
    }
    let extension = top_copper.rsplit_once('.').map(|(_, e)| e).unwrap_or("gbr");
    format!("_inner{number}.{extension}")
}

#[cfg(test)]
mod inner_layer_naming {
    use super::inner_layer_suffix;

    #[test]
    fn an_inner_layer_is_named_the_way_the_preset_names_the_top_one() {
        assert_eq!(inner_layer_suffix("-F_Cu.gbr", 1), "-In1_Cu.gbr");
        assert_eq!(inner_layer_suffix("-F_Cu.gbr", 2), "-In2_Cu.gbr");
        assert_eq!(inner_layer_suffix("_top.gtl", 1), "_inner1.gtl");
    }
}

/// A layer as a drill file name spells it.
fn layer_tag(layer: Layer) -> String {
    match layer {
        Layer::TopCopper => "Top".to_string(),
        Layer::BottomCopper => "Bottom".to_string(),
        Layer::Inner(n) => format!("In{}", n + 1),
        other => format!("{other:?}"),
    }
}

/// Turn clipped designators into sentences a user can act on.
///
/// A board house clips silkscreen off solderable copper, and this exporter
/// does the clipping itself so the file is the file that gets made. That is
/// only safe if the person sending it knows which labels it cost them: a
/// designator eaten by the pads around it leaves a part nobody can identify
/// on the board.
fn describe_clipped_names(clipped: &[SilkWarning], side: &str) -> Vec<String> {
    clipped
        .iter()
        .map(|warning| {
            format!(
                "{} is unreadable on the {side} legend: {} of its {} strokes were clipped off \
                 copper. Move the part, shorten its name, or place its designator by hand.",
                warning.refdes,
                warning.strokes_wanted - warning.strokes_drawn,
                warning.strokes_wanted,
            )
        })
        .collect()
}
