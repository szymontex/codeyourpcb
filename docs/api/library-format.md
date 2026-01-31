# Library File Formats

CodeYourPCB's library system supports multiple component sources with a unified storage backend. This document describes the database schema, component data model, and supported file formats.

## Overview

The library system is built on SQLite for storage with FTS5 full-text search indexing. Components can be imported from:

1. **KiCad .kicad_mod files** (S-expression format)
2. **JLCPCB API** (optional, requires API access)
3. **Custom JSON libraries** (user-defined format)

All sources share the same SQLite schema and search infrastructure.

## SQLite Schema

### Tables

The library database consists of three main tables:

#### `libraries` Table

Tracks all library sources.

```sql
CREATE TABLE libraries (
    source TEXT NOT NULL,           -- Source identifier (e.g., "kicad", "jlcpcb", "custom")
    name TEXT NOT NULL,             -- Library name (e.g., "Resistor_SMD")
    path TEXT,                      -- Optional filesystem path
    version TEXT,                   -- Optional version identifier
    enabled INTEGER NOT NULL DEFAULT 1,  -- Enable/disable flag (1 = enabled, 0 = disabled)
    component_count INTEGER DEFAULT 0,   -- Cached component count
    PRIMARY KEY (source, name)
);
```

**Example rows:**
```
source="kicad", name="Resistor_SMD", path="/usr/share/kicad/footprints/Resistor_SMD.pretty", enabled=1, component_count=142
source="custom", name="MyParts", path="/home/user/my-parts.db", enabled=1, component_count=8
```

#### `components` Table

Stores all component data.

```sql
CREATE TABLE components (
    rowid INTEGER PRIMARY KEY AUTOINCREMENT,
    source TEXT NOT NULL,           -- Source identifier (matches libraries.source)
    name TEXT NOT NULL,             -- Component name (unique within source)
    library TEXT NOT NULL,          -- Library name (foreign key to libraries)
    category TEXT,                  -- Component category (e.g., "Resistors", "Capacitors")
    footprint_data TEXT,            -- Raw footprint data (S-expression for KiCad)
    description TEXT,               -- Human-readable description
    datasheet_url TEXT,             -- URL to component datasheet
    manufacturer TEXT,              -- Manufacturer name
    mpn TEXT,                       -- Manufacturer Part Number
    value TEXT,                     -- Component value (e.g., "10k", "100nF")
    package TEXT,                   -- Package type (e.g., "0805", "SOT-23")
    step_model_path TEXT,           -- Optional path to 3D STEP model file
    metadata_json TEXT,             -- Full ComponentMetadata as JSON (extensibility)
    UNIQUE(source, name),
    FOREIGN KEY (source, library) REFERENCES libraries(source, name)
);
```

**Field purposes:**

- **Individual columns** (description, manufacturer, mpn, etc.) enable SQL WHERE clauses and FTS5 indexing
- **metadata_json** preserves full ComponentMetadata structure for source-specific fields
- **Composite UNIQUE constraint** (source, name) enforces namespace isolation

**Example row:**
```
rowid=1,
source="kicad",
name="R_0805_2012Metric",
library="Resistor_SMD",
category="Resistors",
footprint_data="(footprint \"R_0805_2012Metric\" ...)",
description="Resistor SMD 0805 (2012 Metric), square (rectangular) end terminal",
datasheet_url="~",
manufacturer=NULL,
mpn=NULL,
value=NULL,
package="0805",
step_model_path=NULL,
metadata_json="{\"description\":\"Resistor SMD 0805...\",\"package\":\"0805\",...}"
```

#### Indexes

Performance indexes for common queries:

```sql
CREATE INDEX idx_components_category ON components(category);
CREATE INDEX idx_components_manufacturer ON components(manufacturer);
CREATE INDEX idx_components_value ON components(value);
```

#### `components_fts` Virtual Table (FTS5)

Full-text search index using SQLite FTS5 with BM25 ranking.

```sql
CREATE VIRTUAL TABLE components_fts USING fts5(
    source,
    name,
    category,
    description,
    manufacturer,
    mpn,
    value,
    package
);
```

**Features:**
- BM25 relevance ranking (lower/more negative scores = better matches)
- Automatic sync via INSERT/UPDATE/DELETE triggers
- Searches across all indexed text fields

**Why FTS5 instead of LIKE?**
- Orders results by relevance (BM25 algorithm)
- Tokenizes text (matches word stems, ignores punctuation)
- Faster for large datasets (100k+ components)

### Triggers

Automatic FTS5 synchronization:

```sql
-- After INSERT: add to search index
CREATE TRIGGER components_ai AFTER INSERT ON components BEGIN
    INSERT INTO components_fts(source, name, category, description, manufacturer, mpn, value, package)
    VALUES (new.source, new.name, new.category, new.description, new.manufacturer, new.mpn, new.value, new.package);
END;

-- After DELETE: remove from search index
CREATE TRIGGER components_ad AFTER DELETE ON components BEGIN
    DELETE FROM components_fts WHERE source = old.source AND name = old.name;
END;

-- After UPDATE: remove old entry, add new entry
CREATE TRIGGER components_au AFTER UPDATE ON components BEGIN
    DELETE FROM components_fts WHERE source = old.source AND name = old.name;
    INSERT INTO components_fts(source, name, category, description, manufacturer, mpn, value, package)
    VALUES (new.source, new.name, new.category, new.description, new.manufacturer, new.mpn, new.value, new.package);
END;
```

**Why DELETE + INSERT instead of UPDATE?**
SQLite FTS5 external content tables don't support UPDATE operations. The workaround is to DELETE the old entry and INSERT the new one.

### Metadata Schema (Version Tracking)

Additional table for import history:

```sql
CREATE TABLE library_versions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source TEXT NOT NULL,
    library_name TEXT NOT NULL,
    version_id TEXT,                -- Optional version identifier
    imported_at TEXT NOT NULL,      -- ISO 8601 timestamp
    component_count INTEGER NOT NULL,
    notes TEXT
);

CREATE INDEX idx_library_versions_lookup ON library_versions(source, library_name, imported_at);
```

**Purpose:** Track when libraries were imported, enabling rollback or change tracking.

## Component Data Model

### `ComponentId` Struct

Namespace-prefixed component identifier.

```rust
pub struct ComponentId {
    pub source: String,  // e.g., "kicad", "jlcpcb", "custom"
    pub name: String,    // e.g., "R_0805_2012Metric"
}

impl ComponentId {
    pub fn full_name(&self) -> String {
        format!("{}::{}", self.source, self.name)
    }
}

impl Display for ComponentId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.full_name())
    }
}
```

**Format:** `source::name`

**Examples:**
- `kicad::R_0805_2012Metric`
- `jlcpcb::C17414` (JLCPCB part number)
- `custom::MyCustomResistor`

**Why namespace prefixes?**
Prevents conflicts when importing components from multiple sources. Without namespaces, `R_0805` from KiCad would conflict with `R_0805` from a custom library.

### `Component` Struct

Complete component representation.

```rust
pub struct Component {
    pub id: ComponentId,
    pub library: String,             // Library name within source
    pub category: Option<String>,
    pub footprint_data: Option<String>,  // Raw footprint for preview
    pub metadata: ComponentMetadata,
}
```

### `ComponentMetadata` Struct

Extensible metadata fields.

```rust
#[derive(Default, Serialize, Deserialize)]
pub struct ComponentMetadata {
    pub description: Option<String>,
    pub datasheet_url: Option<String>,
    pub manufacturer: Option<String>,
    pub mpn: Option<String>,              // Manufacturer Part Number
    pub value: Option<String>,            // e.g., "10k", "100nF"
    pub package: Option<String>,          // e.g., "0805", "SOT-23"
    pub step_model_path: Option<String>,  // Path to 3D model
}
```

**All fields are optional** because different sources provide different metadata:
- KiCad: Has description, package, sometimes 3D model path
- JLCPCB: Has manufacturer, MPN, value, package, datasheet URL
- Custom: May have any subset

### Dual Storage Strategy

Component metadata is stored in **two ways**:

1. **Individual SQL columns:** Enable WHERE clauses and FTS5 indexing
2. **metadata_json TEXT column:** Preserves full structure for extensibility

**Why both?**

- SQL columns: Fast filtering (`WHERE manufacturer = 'Texas Instruments'`)
- JSON column: Preserves all fields without schema changes (forward compatibility)

**Deserialization:**
When reading components from database, `metadata_json` is parsed back into `ComponentMetadata` struct.

## Search System

### FTS5 Full-Text Search

Components are searchable via SQLite FTS5 with BM25 ranking.

**Basic search query:**
```sql
SELECT source, name, category, description, manufacturer, mpn, value, package
FROM components_fts
WHERE components_fts MATCH 'resistor 0805'
ORDER BY rank ASC
LIMIT 50;
```

**BM25 ranking:**
- Scores are **negative** (SQLite FTS5 implementation detail)
- **Lower (more negative) = better match**
- ORDER BY rank ASC gives best matches first

**Example scores:**
```
rank=-2.5  → Excellent match (keyword in name)
rank=-1.2  → Good match (keyword in description)
rank=-0.3  → Weak match (keyword in metadata)
```

### Search Filters

Optional filters narrow results:

```rust
pub struct SearchFilters {
    pub category: Option<String>,      // e.g., "Resistors"
    pub manufacturer: Option<String>,  // e.g., "Texas Instruments"
    pub source: Option<String>,        // e.g., "kicad"
    pub limit: usize,                  // Default: 50
}
```

**Dynamic SQL generation:**
```sql
SELECT c.*, -bm25(fts.rowid) as score
FROM components_fts fts
JOIN components c ON c.source = fts.source AND c.name = fts.name
WHERE fts MATCH ?1
  AND (?2 IS NULL OR c.category = ?2)
  AND (?3 IS NULL OR c.manufacturer = ?3)
  AND (?4 IS NULL OR c.source = ?4)
ORDER BY score DESC
LIMIT ?5;
```

**Parameters:**
- `?1` = search query
- `?2` = category filter (NULL if not set)
- `?3` = manufacturer filter (NULL if not set)
- `?4` = source filter (NULL if not set)
- `?5` = limit

### Search Results

```rust
pub struct SearchResult {
    pub component: Component,
    pub rank: f64,  // BM25 score (negative, lower = better)
}
```

## Supported File Formats

### 1. KiCad .kicad_mod Format

KiCad footprints use S-expression format (Lisp-style).

**Example file structure:**
```lisp
(footprint "R_0805_2012Metric"
  (version 20221018)
  (generator pcbnew)
  (layer "F.Cu")
  (descr "Resistor SMD 0805 (2012 Metric), square (rectangular) end terminal")
  (tags "resistor")
  (property "Reference" "R"
    (at 0 -1.65 0)
    (layer "F.SilkS")
    (uuid "...")
    (effects (font (size 1 1) (thickness 0.15)))
  )
  (property "Value" "R_0805_2012Metric"
    (at 0 1.65 0)
    (layer "F.Fab")
    (uuid "...")
    (effects (font (size 1 1) (thickness 0.15)))
  )
  (attr smd)
  (fp_line
    (start -0.227064 -0.735)
    (end 0.227064 -0.735)
    (stroke (width 0.12) (type solid))
    (layer "F.SilkS")
    (uuid "...")
  )
  (pad "1" smd roundrect
    (at -0.9125 0)
    (size 1.025 1.4)
    (layers "F.Cu" "F.Paste" "F.Mask")
    (roundrect_rratio 0.243902)
    (uuid "...")
  )
  (pad "2" smd roundrect
    (at 0.9125 0)
    (size 1.025 1.4)
    (layers "F.Cu" "F.Paste" "F.Mask")
    (roundrect_rratio 0.243902)
    (uuid "...")
  )
  (model "${KICAD7_3DMODEL_DIR}/Resistor_SMD.3dshapes/R_0805_2012Metric.wrl"
    (offset (xyz 0 0 0))
    (scale (xyz 1 1 1))
    (rotate (xyz 0 0 0))
  )
)
```

**Parsing approach:**

CodeYourPCB uses **manual S-expression tree walking** (not Serde derive):

```rust
fn parse_footprint(sexpr: &lexpr::Value) -> Result<Component, LibraryError> {
    // Navigate Cons cells (Lisp-style linked lists)
    // Pattern match on Value::Symbol, Value::Cons, Value::String
    // Recursively search for fields: descr, tags, property, attr, model, pad
}
```

**Why manual parsing?**
- KiCad S-expressions have **variable structure** (not fixed schema)
- Nested properties at arbitrary depth
- Optional fields in varying order
- More flexible than custom Serde deserializer

**Extracted fields:**
- `descr` → `Component.metadata.description`
- `tags` → Ignored (not used in current schema)
- `property "Reference"` → Ignored (refdes assigned at use time)
- `property "Value"` → `Component.metadata.value` (if different from footprint name)
- `model` path → `Component.metadata.step_model_path`
- `pad` count → Derived metadata (number of pads)

**Footprint data storage:**
Entire S-expression stored as string in `Component.footprint_data` for future preview rendering.

### 2. JLCPCB API Format (Optional)

JLCPCB integration is **optional** (feature-gated, requires API access).

**Assumed endpoint:**
```
GET https://api.jlcpcb.com/components/search?q=resistor&limit=50
```

**Expected JSON response:**
```json
{
  "components": [
    {
      "lcsc_part": "C17414",
      "mfr_part": "0805W8F1001T5E",
      "manufacturer": "Uniroyal",
      "description": "10kΩ ±1% 0.125W ±100ppm/℃ 0805 Chip Resistor",
      "datasheet": "https://datasheet.lcsc.com/...",
      "price": "0.0023",
      "stock": 12500,
      "package": "0805",
      "category": "Resistors - Chip SMD"
    }
  ]
}
```

**Mapping to Component:**
- `lcsc_part` → `ComponentId.name` (e.g., "C17414")
- `mfr_part` → `ComponentMetadata.mpn`
- `manufacturer` → `ComponentMetadata.manufacturer`
- `description` → `ComponentMetadata.description`
- `datasheet` → `ComponentMetadata.datasheet_url`
- `package` → `ComponentMetadata.package`
- `category` → `Component.category`

**Note:** Actual JLCPCB API format may differ. Implementation will need adjustment once API access is obtained.

### 3. Custom Library JSON Format

User-defined components can be stored in JSON format.

**Example custom library:**
```json
{
  "library_name": "MyParts",
  "components": [
    {
      "name": "MyCustomResistor",
      "category": "Resistors",
      "description": "Custom high-power resistor",
      "value": "10k",
      "package": "1206",
      "manufacturer": "Custom Mfg",
      "mpn": "CR-1206-10K",
      "datasheet_url": "https://example.com/datasheet.pdf",
      "footprint_data": "(footprint \"Custom_R_1206\" ...)"
    }
  ]
}
```

**Parsing:**
Straightforward JSON deserialization using Serde.

**Storage:**
Same SQLite schema as other sources, with `source = "custom"`.

## Import Pipeline

### High-Level Flow

```
Source → Parse → Import → Index → Search

1. Source: Read files from KiCad .pretty folder, JLCPCB API, or custom JSON
2. Parse: Extract component metadata using source-specific parser
3. Import: Insert components into SQLite database (INSERT or UPDATE)
4. Index: FTS5 triggers automatically update search index
5. Search: Components are now searchable via LibraryManager
```

### Import Operations

#### INSERT (New Component)

```sql
INSERT INTO components
    (source, name, library, category, footprint_data, description, datasheet_url,
     manufacturer, mpn, value, package, step_model_path, metadata_json)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13);
```

#### UPDATE (Existing Component)

If component exists (UNIQUE constraint violation), update it:

```sql
UPDATE components SET
    library = ?1,
    category = ?2,
    footprint_data = ?3,
    description = ?4,
    datasheet_url = ?5,
    manufacturer = ?6,
    mpn = ?7,
    value = ?8,
    package = ?9,
    step_model_path = ?10,
    metadata_json = ?11
WHERE source = ?12 AND name = ?13;
```

**Why separate INSERT try/UPDATE pattern?**
`INSERT ... ON CONFLICT ... DO UPDATE` doesn't fire UPDATE triggers in SQLite. Must use explicit INSERT try/catch UPDATE to ensure FTS5 sync triggers fire.

### Batch Import

For performance, components are imported in batches within a transaction:

```rust
pub fn insert_components_batch(
    conn: &mut Connection,
    components: &[Component],
) -> Result<usize, LibraryError> {
    let tx = conn.transaction()?;

    for component in components {
        // INSERT with UNIQUE constraint handling
        // Falls back to UPDATE if constraint violated
    }

    tx.commit()?;
    Ok(components.len())
}
```

**Transaction benefits:**
- Atomic import (all or nothing)
- Faster (single disk write at commit)
- FTS5 triggers run once per component, but in single transaction

## Platform Differences

### Desktop (Native SQLite)

- **Storage:** SQLite file on local filesystem
- **Path:** Platform-specific (XDG_DATA_HOME on Linux, AppData on Windows)
- **Performance:** Native rusqlite performance (~10k components/sec import)
- **Concurrency:** Mutex-protected Connection for thread safety

### Web (IndexedDB via SQL.js)

- **Storage:** IndexedDB (browser's object store)
- **Performance:** Slower than native SQLite (~1k components/sec import)
- **Limitations:** No native filesystem access, 50MB-1GB quota
- **Future:** SQLite compiled to WASM with virtual filesystem backed by IndexedDB

**Current status:** Web library management not yet implemented (Phase 13 scope).

## LibraryManager API

High-level API for library operations:

```rust
pub struct LibraryManager {
    conn: Arc<Mutex<Connection>>,
    sources: Vec<Box<dyn LibrarySource>>,
}

impl LibraryManager {
    // Initialize from config
    pub fn new(db_path: impl AsRef<Path>) -> Result<Self, LibraryError>;

    // Configure KiCad search paths
    pub fn set_kicad_search_paths(&mut self, paths: Vec<PathBuf>);

    // List all libraries
    pub fn list_libraries(&self) -> Result<Vec<LibraryInfo>, LibraryError>;

    // Import a library
    pub fn import_library(&self, source: &str, library_name: &str) -> Result<usize, LibraryError>;

    // Search components
    pub fn search(&self, query: &str, filters: SearchFilters) -> Result<Vec<SearchResult>, LibraryError>;

    // Get component by ID
    pub fn get_component(&self, id: &ComponentId) -> Result<Option<Component>, LibraryError>;
}
```

## Related Documentation

- **Library Management Workflows:** See `docs/user-guide/library-management.md`
- **Schema Implementation:** See `crates/cypcb-library/src/schema.rs`
- **Data Models:** See `crates/cypcb-library/src/models.rs`
- **Search Engine:** See `crates/cypcb-library/src/search.rs`
- **KiCad Parser:** See `crates/cypcb-library/src/sources/kicad.rs`

## Troubleshooting

### FTS5 Sync Issues

**Symptom:** Components imported but not searchable.

**Check:**
1. Are triggers enabled? `SELECT * FROM sqlite_master WHERE type='trigger';`
2. Does FTS5 table have data? `SELECT COUNT(*) FROM components_fts;`
3. Did transaction commit? INSERT/UPDATE in uncommitted transaction won't trigger.

### Namespace Collisions

**Symptom:** Component import fails with UNIQUE constraint violation.

**Cause:** Same `(source, name)` combination already exists.

**Resolution:**
- If intentional update: Code handles this (UPDATE path)
- If accidental: Check source identifier consistency

### Import Performance

**Symptom:** Importing KiCad library takes too long.

**Optimizations:**
1. Use batch import (`insert_components_batch`) instead of individual inserts
2. Ensure transaction wraps entire batch
3. Disable FTS5 temporarily during bulk import, rebuild after (advanced)

## Future Enhancements

Potential improvements:

1. **Component thumbnails:** Extract footprint preview images during import
2. **3D model validation:** Check STEP model paths resolve correctly
3. **Parametric search:** Filter by numeric values (e.g., resistance range)
4. **Tag-based categorization:** Multiple tags per component for flexible organization
5. **Incremental sync:** Track library file mtimes, only re-import if changed
6. **Compression:** Compress footprint_data to reduce database size
7. **Tantivy upgrade:** Replace FTS5 with Tantivy for advanced search (if scale demands it)
