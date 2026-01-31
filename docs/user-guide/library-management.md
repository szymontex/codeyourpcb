# Library Management Guide

CodeYourPCB's library system allows you to import, organize, and search component footprints from multiple sources. This guide explains how to work with libraries effectively.

## Overview

The library system provides:
- **Multi-source support**: KiCad, JLCPCB, and custom libraries
- **Namespace isolation**: Prevent naming conflicts across sources
- **Full-text search**: Find components quickly with FTS5 search
- **Metadata tracking**: Descriptions, manufacturers, part numbers, datasheets
- **Footprint preview**: Visual verification of pad layouts
- **3D model association**: Link 3D models for realistic board visualization

## Library Sources

### KiCad Libraries

KiCad has an extensive collection of high-quality footprints covering thousands of common components.

**Format:** `.kicad_mod` files in `.pretty` directories

**Structure:**
```
KiCad/footprints/
├── Resistor_SMD.pretty/
│   ├── R_0402_1005Metric.kicad_mod
│   ├── R_0603_1608Metric.kicad_mod
│   ├── R_0805_2012Metric.kicad_mod
│   └── ...
├── Capacitor_SMD.pretty/
│   ├── C_0402_1005Metric.kicad_mod
│   ├── C_0603_1608Metric.kicad_mod
│   └── ...
└── Package_SO.pretty/
    ├── SOIC-8_3.9x4.9mm_P1.27mm.kicad_mod
    └── ...
```

**Auto-organization:** When importing a `.pretty` folder, components are automatically organized with the folder name as the category.

**Namespace:** All KiCad components are prefixed with `kicad::` to prevent conflicts.

### JLCPCB Library (Optional)

JLCPCB is a PCB manufacturer with an extensive catalog of in-stock components. The JLCPCB library integration allows searching their catalog directly.

**Requirements:**
- JLCPCB API key (requires manual application approval)
- CodeYourPCB compiled with `jlcpcb` feature flag

**Namespace:** All JLCPCB components are prefixed with `jlcpcb::`

**Note:** JLCPCB integration is optional and can be enabled/disabled at compile time.

### Custom Libraries

Create your own component libraries for:
- Company-specific footprints
- Modified standard footprints
- Custom connectors and mechanical parts
- Footprints not available in KiCad

**Format:** Same as KiCad (`.kicad_mod` S-expression format)

**Namespace:** Custom libraries use the source name you provide (e.g., `mycompany::`)

## Namespace-Prefixed Component IDs

To prevent naming conflicts, all components use namespace-prefixed IDs:

**Format:** `source::name`

**Examples:**
- `kicad::R_0805_2012Metric` - KiCad 0805 resistor footprint
- `jlcpcb::R_0805` - JLCPCB 0805 resistor footprint
- `mycompany::CUSTOM_CONNECTOR` - Your company's custom footprint

**Why namespaces?**
- Multiple sources may have components with the same name
- `kicad::R_0805` and `jlcpcb::R_0805` are different footprints
- Prevents accidental conflicts when merging libraries
- Clear indication of component source

**In .cypcb files:**
```
component R1 resistor "kicad::R_0805_2012Metric" {
    value "330"
    at 15mm, 15mm
}
```

## Importing KiCad Libraries

### Find KiCad Footprint Libraries

**Linux:**
```bash
/usr/share/kicad/footprints/
```

**macOS:**
```bash
/Applications/KiCad/KiCad.app/Contents/SharedSupport/footprints/
```

**Windows:**
```
C:\Program Files\KiCad\share\kicad\footprints\
```

### Import a Single Library

```rust
use cypcb_library::LibraryManager;

let mut manager = LibraryManager::new()?;

// Import Resistor_SMD.pretty folder
manager.import_kicad_library(
    "/usr/share/kicad/footprints/Resistor_SMD.pretty"
)?;
```

**Result:** All resistor footprints are imported with namespace `kicad::` and category `Resistor_SMD`.

### Import Multiple Libraries

Set search paths and import everything:

```rust
let mut manager = LibraryManager::new()?;

// Add KiCad footprint directories
manager.set_kicad_search_paths(vec![
    "/usr/share/kicad/footprints/".to_string(),
]);

// Import all .pretty folders in search paths
manager.import_all_kicad_libraries()?;
```

**Result:** Thousands of components imported and organized by category.

### Auto-Organization

When importing `Resistor_SMD.pretty`:
- Folder name becomes category: `Resistor_SMD`
- Components get namespace: `kicad::`
- Full component ID: `kicad::R_0805_2012Metric`

**Database structure:**
```
+--------+-------------------+------------------+
| source | name              | category         |
+--------+-------------------+------------------+
| kicad  | R_0805_2012Metric | Resistor_SMD     |
| kicad  | R_0603_1608Metric | Resistor_SMD     |
| kicad  | C_0805_2012Metric | Capacitor_SMD    |
| kicad  | SOIC-8_3.9x4.9mm  | Package_SO       |
+--------+-------------------+------------------+
```

## Creating Custom Libraries

### Manual Component Creation

```rust
use cypcb_library::{LibraryManager, Component, ComponentMetadata};

let mut manager = LibraryManager::new()?;

let component = Component {
    source: "mycompany".to_string(),
    name: "CUSTOM_CONNECTOR_4PIN".to_string(),
    category: Some("Connectors".to_string()),
    footprint_data: "...kicad_mod content...".to_string(),
    metadata: ComponentMetadata {
        description: Some("Custom 4-pin connector for product X".to_string()),
        manufacturer: Some("ACME Corp".to_string()),
        mpn: Some("CONN-4P-001".to_string()),
        datasheet_url: Some("https://example.com/datasheet.pdf".to_string()),
        package: Some("Custom".to_string()),
        library_version: Some("1.0".to_string()),
        tags: vec!["connector".to_string(), "4pin".to_string()],
        custom_fields: HashMap::new(),
    },
    thumbnail: None,
    model_3d_path: None,
};

manager.add_component(component)?;
```

**Result:** Component `mycompany::CUSTOM_CONNECTOR_4PIN` is now searchable.

### Import Custom .pretty Folder

```rust
let mut manager = LibraryManager::new()?;

manager.import_custom_library(
    "mycompany",  // Namespace
    "/path/to/MyCompany_Footprints.pretty"
)?;
```

**Result:** All `.kicad_mod` files in the folder are imported with namespace `mycompany::`.

## Searching Components

### Basic Full-Text Search

The search engine uses SQLite FTS5 with BM25 ranking for relevance scoring.

```rust
let manager = LibraryManager::new()?;

// Search for "resistor"
let results = manager.search("resistor", None)?;

for result in results {
    println!("{}: {}", result.component_id, result.rank);
}
```

**Output:**
```
kicad::R_0805_2012Metric: -2.34
kicad::R_0603_1608Metric: -2.45
kicad::R_0402_1005Metric: -2.56
...
```

**BM25 scores:**
- Scores are NEGATIVE (SQLite FTS5 implementation detail)
- Lower (more negative) = better match
- Results are ordered by relevance (best matches first)

### Search with Filters

Narrow down results with optional filters:

```rust
use cypcb_library::SearchFilters;

let filters = SearchFilters {
    source: Some("kicad".to_string()),          // Only KiCad components
    category: Some("Resistor_SMD".to_string()), // Only resistors
    package: Some("0805".to_string()),          // Only 0805 package
    manufacturer: None,
    tags: vec![],
};

let results = manager.search("resistor", Some(filters))?;
```

**Filter fields:**
- `source`: Filter by library source (kicad, jlcpcb, custom)
- `category`: Filter by category (auto-organized from folder names)
- `package`: Filter by package type (0805, SOIC8, etc.)
- `manufacturer`: Filter by manufacturer name
- `tags`: Filter by tags (AND logic - component must have all tags)

### Search Examples

**Find all 0805 resistors:**
```rust
let filters = SearchFilters {
    category: Some("Resistor_SMD".to_string()),
    package: Some("0805".to_string()),
    ..Default::default()
};
let results = manager.search("", Some(filters))?;
```

**Find specific IC by part number:**
```rust
let results = manager.search("ATmega328P", None)?;
```

**Find connectors with USB in the name:**
```rust
let filters = SearchFilters {
    tags: vec!["connector".to_string()],
    ..Default::default()
};
let results = manager.search("usb", Some(filters))?;
```

### Search Performance

- **FTS5 full-text index**: Sub-millisecond search on 100K+ components
- **BM25 ranking**: Relevance-based ordering
- **Automatic sync**: Triggers maintain index on INSERT/UPDATE/DELETE
- **No manual indexing**: Index management is transparent

**Upgrade path:** If search performance becomes a bottleneck with millions of components, consider upgrading to Tantivy (Rust native full-text search engine).

## Component Metadata

### Metadata Fields

Each component can have rich metadata for search and documentation:

```rust
pub struct ComponentMetadata {
    pub description: Option<String>,        // Human-readable description
    pub manufacturer: Option<String>,       // Manufacturer name
    pub mpn: Option<String>,                // Manufacturer part number
    pub datasheet_url: Option<String>,      // Link to datasheet PDF
    pub package: Option<String>,            // Package type (0805, SOIC8)
    pub library_version: Option<String>,    // Library version for tracking
    pub tags: Vec<String>,                  // Searchable tags
    pub custom_fields: HashMap<String, String>, // Extensible key-value pairs
}
```

### Why Dual Storage?

The database schema uses dual storage:

1. **Individual columns** (description, manufacturer, mpn, package)
   - Enable SQL WHERE clauses: `WHERE manufacturer = 'TI'`
   - Enable FTS5 indexing for fast full-text search
   - Queryable and filterable

2. **JSON column** (metadata_json)
   - Preserves complete ComponentMetadata struct
   - Extensible for source-specific fields
   - Future-proof for schema evolution

**Trade-off:** Requires JSON deserialization when reading components, but enables powerful search and filtering.

### Adding Metadata

```rust
let component = Component {
    source: "kicad".to_string(),
    name: "R_0805_2012Metric".to_string(),
    category: Some("Resistor_SMD".to_string()),
    footprint_data: "...".to_string(),
    metadata: ComponentMetadata {
        description: Some("0805 (2012 Metric) resistor footprint".to_string()),
        package: Some("0805".to_string()),
        tags: vec!["smd".to_string(), "resistor".to_string()],
        custom_fields: {
            let mut map = HashMap::new();
            map.insert("power_rating".to_string(), "0.125W".to_string());
            map
        },
        ..Default::default()
    },
    thumbnail: None,
    model_3d_path: None,
};

manager.add_component(component)?;
```

## Footprint Preview

### Extracting Thumbnail

Footprint thumbnails are generated from pad layout:

```rust
use cypcb_library::preview::extract_thumbnail;

let thumbnail_data = extract_thumbnail(&footprint_data)?;

// Store with component
let component = Component {
    // ...
    thumbnail: Some(thumbnail_data),
    // ...
};
```

**Use cases:**
- Visual verification before placing component
- Library browser UI showing footprint previews
- Quick identification of similar footprints

### Preview Format

Thumbnails are stored as PNG image data (binary blob).

**Rendering:**
- Pads rendered with copper color
- Silkscreen outline shown
- Scaled to fit preview size (e.g., 200x200 pixels)

## 3D Model Association

### Linking 3D Models

Associate STEP or WRL files with footprints for realistic board visualization:

```rust
let component = Component {
    // ...
    model_3d_path: Some("models/resistor_0805.step".to_string()),
    // ...
};
```

**Model path:**
- Relative to footprint library location
- Standard formats: STEP (.step), VRML (.wrl)
- Used by 3D viewer to render realistic board

**Benefits:**
- Verify component clearances in 3D
- Export 3D board model for enclosure design
- Marketing/documentation renders

### Model Search Paths

LibraryManager can search for 3D models in configured directories:

```rust
manager.set_model_search_paths(vec![
    "/usr/share/kicad/3dmodels/".to_string(),
]);
```

## Version Tracking

### Component Versions

Track library versions for reproducibility:

```rust
let metadata = ComponentMetadata {
    library_version: Some("2024.01".to_string()),
    ..Default::default()
};
```

**Use cases:**
- Ensure designs use compatible library versions
- Migrate designs when library updates
- Audit which footprints need updates

### Version Queries

```rust
// Find all components from a specific library version
let filters = SearchFilters::default();
let results = manager.search("library_version:2024.01", Some(filters))?;
```

## LibraryManager API

### Initialization

```rust
use cypcb_library::LibraryManager;

// Create manager with default SQLite database
let manager = LibraryManager::new()?;

// Or specify database path
let manager = LibraryManager::with_path("/path/to/library.db")?;
```

### Configuration Methods (Mutable)

```rust
let mut manager = LibraryManager::new()?;

// Set KiCad search paths
manager.set_kicad_search_paths(vec![
    "/usr/share/kicad/footprints/".to_string(),
]);

// Set 3D model search paths
manager.set_model_search_paths(vec![
    "/usr/share/kicad/3dmodels/".to_string(),
]);
```

### Import Methods

```rust
// Import single KiCad library
manager.import_kicad_library("/path/to/Resistor_SMD.pretty")?;

// Import all KiCad libraries in search paths
manager.import_all_kicad_libraries()?;

// Import custom library with namespace
manager.import_custom_library("mycompany", "/path/to/MyLibrary.pretty")?;

// Import JLCPCB component (requires API access)
#[cfg(feature = "jlcpcb")]
manager.import_jlcpcb_component("C12345")?;
```

### Query Methods (Immutable)

```rust
// Search with optional filters
let results = manager.search("resistor", Some(filters))?;

// Get component by ID
let component = manager.get_component("kicad::R_0805_2012Metric")?;

// List all libraries
let libraries = manager.list_libraries()?;

// Get library info
let info = manager.get_library_info("kicad")?;
```

### Adding Components

```rust
// Add single component
manager.add_component(component)?;

// Batch add components
manager.add_components(vec![component1, component2, component3])?;
```

## Import Pipeline

The complete flow from source to searchable component:

1. **Source scan**: Find `.kicad_mod` files in `.pretty` folders
2. **Parse**: Extract footprint data (pads, silkscreen, courtyard)
3. **Namespace**: Prefix component name with source (`kicad::`, `mycompany::`)
4. **Organize**: Auto-assign category from folder name
5. **Index**: Insert into SQLite with metadata
6. **FTS5 sync**: Trigger automatically updates full-text search index
7. **Verify**: Component is now searchable via `manager.search()`

**End-to-end verification:**
```rust
// Import
manager.import_kicad_library("/usr/share/kicad/footprints/Resistor_SMD.pretty")?;

// Search
let results = manager.search("0805", None)?;
assert!(!results.is_empty());

// Retrieve
let component = manager.get_component("kicad::R_0805_2012Metric")?;
assert_eq!(component.category, Some("Resistor_SMD".to_string()));
```

## Storage Backends

### Desktop (Native)

**Storage:** SQLite database
**Location:** User data directory (platform-specific)
- Linux: `~/.local/share/codeyourpcb/libraries.db`
- macOS: `~/Library/Application Support/codeyourpcb/libraries.db`
- Windows: `C:\Users\<user>\AppData\Local\codeyourpcb\libraries.db`

**Features:**
- Full SQL query support
- FTS5 full-text search
- Efficient joins and aggregations
- Transaction support

### Web (WASM)

**Storage:** localStorage (v1.1)
**Limitations:**
- ~5MB quota (browser-dependent)
- Key-value store (no SQL joins)
- Synchronous API only

**Future:** Upgrade to IndexedDB for larger libraries (Phase 16+)

## Best Practices

### Organize Libraries by Source

Keep libraries separated by source for easier management:

```rust
manager.import_kicad_library("/path/to/kicad/Resistor_SMD.pretty")?;
manager.import_custom_library("mycompany", "/path/to/custom/MyParts.pretty")?;
```

### Use Descriptive Namespaces

For custom libraries, use company or project names:
- `acme::` - ACME Corp custom footprints
- `project_x::` - Project-specific footprints
- `legacy::` - Old designs (for migration)

### Tag Generously

Add tags to improve searchability:
```rust
tags: vec![
    "smd".to_string(),
    "resistor".to_string(),
    "0805".to_string(),
    "power-rating-0.125W".to_string(),
]
```

### Document Custom Footprints

Always add descriptions to custom components:
```rust
metadata: ComponentMetadata {
    description: Some("Custom connector for X project - DO NOT MODIFY".to_string()),
    // ...
}
```

### Version Control Custom Libraries

Commit your custom `.pretty` folders to git:
```
my_project/
├── libraries/
│   └── mycompany.pretty/
│       ├── CUSTOM_PART_1.kicad_mod
│       └── CUSTOM_PART_2.kicad_mod
└── README.md
```

## Troubleshooting

### Component Not Found

**Symptom:** Search returns no results for known component

**Fixes:**
1. Verify import completed: `manager.list_libraries()?`
2. Check namespace: Try `kicad::R_0805` instead of `R_0805`
3. Re-import library: `manager.import_kicad_library(...)?`
4. Check FTS5 index: Search for wildcard `*` to see all components

### Namespace Conflicts

**Symptom:** Two libraries have same namespace

**Fixes:**
1. Use different namespaces for different sources
2. Re-import with explicit namespace: `import_custom_library("namespace", path)`
3. Merge libraries if duplication is intentional

### Slow Search

**Symptom:** Search takes >100ms

**Possible causes:**
1. Database not indexed (shouldn't happen - automatic)
2. Extremely large library (>1M components)
3. Complex filter queries

**Fixes:**
1. Verify FTS5 triggers exist: Check `components_fts` table
2. Consider upgrading to Tantivy for large-scale search
3. Simplify filters or add more specific search terms

## Summary

- **Multi-source**: KiCad, JLCPCB, custom libraries with namespace isolation
- **Import**: Auto-organize from `.pretty` folders with category extraction
- **Search**: FTS5 full-text search with BM25 ranking and optional filters
- **Metadata**: Rich component information for selection and documentation
- **Namespace IDs**: `source::name` prevents conflicts (e.g., `kicad::R_0805`)
- **LibraryManager**: Single entry point for all library operations

For more information:
- [Getting Started](getting-started.md) - Learn to use components in .cypcb files
- [Project Structure](project-structure.md) - Organize library files in projects
- [Platform Differences](platform-differences.md) - Library storage on desktop vs web
