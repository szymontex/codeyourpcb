//! Manufacturer presets for export configuration.
//!
//! Presets define file naming conventions, coordinate formats, and layer configurations
//! for specific PCB manufacturers (JLCPCB, PCBWay, etc.).
//!
//! # Examples
//!
//! ```
//! use cypcb_export::presets::{ExportPreset, from_name};
//!
//! // Load JLCPCB preset
//! let preset = from_name("jlcpcb").unwrap();
//! assert_eq!(preset.name, "JLCPCB 2-Layer");
//! assert!(preset.assembly);
//! ```

use crate::coords::{CoordinateFormat, Unit};

/// Export preset defining manufacturer-specific requirements.
#[derive(Debug, Clone)]
pub struct ExportPreset {
    /// Human-readable preset name
    pub name: String,
    /// Coordinate format (units and decimal places)
    pub coordinate_format: CoordinateFormat,
    /// File naming convention
    pub file_naming: FileNaming,
    /// Layer export configuration
    pub layers: ExportLayers,
    /// Include assembly files (BOM, CPL)
    pub assembly: bool,
}

/// File naming convention for exported files.
#[derive(Debug, Clone)]
pub struct FileNaming {
    pub top_copper: &'static str,
    pub bottom_copper: &'static str,
    pub top_mask: &'static str,
    pub bottom_mask: &'static str,
    pub top_silk: &'static str,
    pub bottom_silk: &'static str,
    pub top_paste: &'static str,
    pub bottom_paste: &'static str,
    pub outline: &'static str,
    pub drill_pth: &'static str,
    pub drill_npth: &'static str,
    pub bom: &'static str,
    pub cpl: &'static str,
}

/// Layer export configuration.
#[derive(Debug, Clone)]
pub struct ExportLayers {
    pub top_copper: bool,
    pub bottom_copper: bool,
    pub inner_copper: Vec<u8>,
    pub top_mask: bool,
    pub bottom_mask: bool,
    pub top_silk: bool,
    pub bottom_silk: bool,
    pub top_paste: bool,
    pub bottom_paste: bool,
    pub outline: bool,
    pub drill: bool,
}

/// JLCPCB 2-layer preset.
///
/// Standard 2-layer board preset for JLCPCB manufacturing and assembly.
/// Uses KiCad-style file naming with metric coordinates (2.6 format).
pub fn jlcpcb_2layer() -> ExportPreset {
    ExportPreset {
        name: "JLCPCB 2-Layer".to_string(),
        coordinate_format: CoordinateFormat {
            unit: Unit::Millimeters,
            integer_places: 2,
            decimal_places: 6,
        },
        file_naming: FileNaming {
            top_copper: "-F_Cu.gbr",
            bottom_copper: "-B_Cu.gbr",
            top_mask: "-F_Mask.gbr",
            bottom_mask: "-B_Mask.gbr",
            top_silk: "-F_SilkS.gbr",
            bottom_silk: "-B_SilkS.gbr",
            top_paste: "-F_Paste.gbr",
            bottom_paste: "-B_Paste.gbr",
            outline: "-Edge_Cuts.gbr",
            drill_pth: "-PTH.drl",
            drill_npth: "-NPTH.drl",
            bom: "-BOM.csv",
            cpl: "-CPL.csv",
        },
        layers: ExportLayers {
            top_copper: true,
            bottom_copper: true,
            inner_copper: vec![],
            top_mask: true,
            bottom_mask: true,
            top_silk: true,
            bottom_silk: true,
            top_paste: true,
            bottom_paste: true,
            outline: true,
            drill: true,
        },
        assembly: true,
    }
}

/// PCBWay standard preset.
///
/// Standard preset for PCBWay manufacturing. Uses more traditional file naming
/// with metric coordinates.
pub fn pcbway_standard() -> ExportPreset {
    ExportPreset {
        name: "PCBWay Standard".to_string(),
        coordinate_format: CoordinateFormat {
            unit: Unit::Millimeters,
            integer_places: 2,
            decimal_places: 6,
        },
        file_naming: FileNaming {
            top_copper: "_top.gtl",
            bottom_copper: "_bottom.gbl",
            top_mask: "_topsoldermask.gts",
            bottom_mask: "_bottomsoldermask.gbs",
            top_silk: "_topsilkscreen.gto",
            bottom_silk: "_bottomsilkscreen.gbo",
            top_paste: "_toppaste.gtp",
            bottom_paste: "_bottompaste.gbp",
            outline: "_outline.gko",
            drill_pth: "_drill.xln",
            drill_npth: "_npth.xln",
            bom: "_bom.csv",
            cpl: "_cpl.csv",
        },
        layers: ExportLayers {
            top_copper: true,
            bottom_copper: true,
            inner_copper: vec![],
            top_mask: true,
            bottom_mask: true,
            top_silk: true,
            bottom_silk: true,
            top_paste: true,
            bottom_paste: true,
            outline: true,
            drill: true,
        },
        assembly: true,
    }
}

/// Look up a preset by name.
///
/// # Arguments
///
/// * `name` - Preset name (case-insensitive). Accepts:
///   - "jlcpcb" -> JLCPCB 2-layer
///   - "pcbway" -> PCBWay standard
///
/// # Examples
///
/// ```
/// use cypcb_export::presets::from_name;
///
/// let preset = from_name("jlcpcb").unwrap();
/// assert_eq!(preset.name, "JLCPCB 2-Layer");
///
/// let preset = from_name("pcbway").unwrap();
/// assert_eq!(preset.name, "PCBWay Standard");
///
/// assert!(from_name("unknown").is_none());
/// ```
pub fn from_name(name: &str) -> Option<ExportPreset> {
    match name.to_lowercase().as_str() {
        "jlcpcb" => Some(jlcpcb_2layer()),
        "pcbway" => Some(pcbway_standard()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jlcpcb_preset_name() {
        let preset = jlcpcb_2layer();
        assert_eq!(preset.name, "JLCPCB 2-Layer");
    }

    #[test]
    fn test_jlcpcb_coordinate_format() {
        let preset = jlcpcb_2layer();
        assert!(matches!(preset.coordinate_format.unit, Unit::Millimeters));
        assert_eq!(preset.coordinate_format.integer_places, 2);
        assert_eq!(preset.coordinate_format.decimal_places, 6);
    }

    #[test]
    fn test_jlcpcb_file_naming() {
        let preset = jlcpcb_2layer();
        assert_eq!(preset.file_naming.top_copper, "-F_Cu.gbr");
        assert_eq!(preset.file_naming.bottom_copper, "-B_Cu.gbr");
        assert_eq!(preset.file_naming.outline, "-Edge_Cuts.gbr");
        assert_eq!(preset.file_naming.drill_pth, "-PTH.drl");
    }

    #[test]
    fn test_jlcpcb_layers_enabled() {
        let preset = jlcpcb_2layer();
        assert!(preset.layers.top_copper);
        assert!(preset.layers.bottom_copper);
        assert!(preset.layers.outline);
        assert!(preset.layers.drill);
    }

    #[test]
    fn test_jlcpcb_assembly_enabled() {
        let preset = jlcpcb_2layer();
        assert!(preset.assembly);
    }

    #[test]
    fn test_pcbway_preset_name() {
        let preset = pcbway_standard();
        assert_eq!(preset.name, "PCBWay Standard");
    }

    #[test]
    fn test_pcbway_file_naming() {
        let preset = pcbway_standard();
        assert_eq!(preset.file_naming.top_copper, "_top.gtl");
        assert_eq!(preset.file_naming.bottom_copper, "_bottom.gbl");
        assert_eq!(preset.file_naming.outline, "_outline.gko");
    }

    #[test]
    fn test_from_name_jlcpcb() {
        let preset = from_name("jlcpcb").unwrap();
        assert_eq!(preset.name, "JLCPCB 2-Layer");
    }

    #[test]
    fn test_from_name_pcbway() {
        let preset = from_name("pcbway").unwrap();
        assert_eq!(preset.name, "PCBWay Standard");
    }

    #[test]
    fn test_from_name_case_insensitive() {
        assert!(from_name("JLCPCB").is_some());
        assert!(from_name("JlcPcb").is_some());
        assert!(from_name("PCBWAY").is_some());
    }

    #[test]
    fn test_from_name_unknown() {
        assert!(from_name("unknown").is_none());
        assert!(from_name("oshpark").is_none());
    }
}
