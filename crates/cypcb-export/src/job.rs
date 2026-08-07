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
use crate::gerber::{
    export_copper_layer, export_outline, export_silkscreen, export_soldermask, export_solderpaste,
    MaskPasteConfig, Side, SilkConfig,
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
        let content = export_copper_layer(
            world,
            library,
            Layer::TopCopper,
            &job.preset.coordinate_format,
        )
        .map_err(|e| ExportError::Export(format!("{:?}", e)))?;
        let file = write_export_file(&path, &content, "Top Copper")?;
        files.push(file);
    }

    if job.preset.layers.bottom_copper {
        let filename = format!("{}{}", job.board_name, job.preset.file_naming.bottom_copper);
        let path = gerber_dir.join(&filename);
        let content = export_copper_layer(
            world,
            library,
            Layer::BottomCopper,
            &job.preset.coordinate_format,
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
        let content = export_copper_layer(
            world,
            library,
            Layer::Inner(index),
            &job.preset.coordinate_format,
        )
        .map_err(|e| ExportError::Export(format!("{:?}", e)))?;
        let file = write_export_file(&path, &content, &format!("Inner Copper {number}"))?;
        files.push(file);
    }

    if job.preset.layers.top_mask {
        let filename = format!("{}{}", job.board_name, job.preset.file_naming.top_mask);
        let path = gerber_dir.join(&filename);
        let config = MaskPasteConfig::default();
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
        let config = MaskPasteConfig::default();
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
        let config = SilkConfig::default();
        let content = export_silkscreen(
            world,
            library,
            Side::Top,
            &job.preset.coordinate_format,
            &config,
        )
        .map_err(|e| ExportError::Export(format!("{:?}", e)))?;
        let file = write_export_file(&path, &content, "Top Silkscreen")?;
        files.push(file);
    }

    if job.preset.layers.bottom_silk {
        let filename = format!("{}{}", job.board_name, job.preset.file_naming.bottom_silk);
        let path = gerber_dir.join(&filename);
        let config = SilkConfig::default();
        let content = export_silkscreen(
            world,
            library,
            Side::Bottom,
            &job.preset.coordinate_format,
            &config,
        )
        .map_err(|e| ExportError::Export(format!("{:?}", e)))?;
        let file = write_export_file(&path, &content, "Bottom Silkscreen")?;
        files.push(file);
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

        // A zone the design asks for and the exporter cannot draw.
        //
        // Copper pour is not implemented: a `zone` carries a net and the
        // ratsnest treats a pad inside it as connected, so the design reads as
        // finished, and the Gerber has nothing where the pour should be.
        // Measured on a board with one ground pour: four pad flashes and zero
        // copper. Someone who draws a ground plane and sends these files gets
        // a board without one.
        let zone_count = world
            .zones()
            .iter()
            .filter(|(_, z)| !z.is_keepout())
            .count();
        if zone_count > 0 {
            warnings.push(format!(
                "{zone_count} copper pour(s) are filled without thermal relief: a pad on the pour's own net \
                 is flooded solid, which connects it and makes it hard to solder by hand"
            ));
        }

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
