use serde::{Deserialize, Serialize};
use std::fmt;

/// Component identifier with source namespace to prevent conflicts
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ComponentId {
    /// Source identifier (e.g., "kicad", "jlcpcb", "custom")
    pub source: String,
    /// Component name within the source
    pub name: String,
}

impl ComponentId {
    pub fn new(source: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            name: name.into(),
        }
    }

    /// Returns the fully qualified component name in format "source::name"
    pub fn full_name(&self) -> String {
        format!("{}::{}", self.source, self.name)
    }
}

impl fmt::Display for ComponentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.full_name())
    }
}

/// Component representation with footprint data and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Component {
    pub id: ComponentId,
    pub library: String,
    pub category: Option<String>,
    /// Raw footprint data for preview (e.g., S-expression for KiCad)
    pub footprint_data: Option<String>,
    pub metadata: ComponentMetadata,
}

/// Component metadata including manufacturer, datasheet, and physical properties
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ComponentMetadata {
    pub description: Option<String>,
    pub datasheet_url: Option<String>,
    pub manufacturer: Option<String>,
    /// Manufacturer Part Number
    pub mpn: Option<String>,
    /// Component value (e.g., "10k", "100nF")
    pub value: Option<String>,
    /// Package type (e.g., "0805", "SOT-23")
    pub package: Option<String>,
    /// Path to 3D STEP model file
    pub step_model_path: Option<String>,
}

/// Library information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryInfo {
    pub source: String,
    pub name: String,
    pub path: Option<String>,
    pub version: Option<String>,
    pub enabled: bool,
    pub component_count: usize,
}

/// Search filters for component queries
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchFilters {
    pub category: Option<String>,
    pub manufacturer: Option<String>,
    pub source: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    50
}

/// Search result with component and relevance ranking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub component: Component,
    /// BM25 ranking score (lower/more negative = better match)
    pub rank: f64,
}
