# Phase 10: Library Management Foundation - Research

**Researched:** 2026-01-29
**Domain:** Component library management, multi-source integration, search indexing
**Confidence:** MEDIUM-HIGH

## Summary

Library management for PCB design tools requires handling multiple source formats (KiCad footprints, JLCPCB API, custom libraries), full-text search across component metadata, and conflict-free namespace management. The standard approach uses SQLite with FTS5 for search, S-expression parsers for KiCad formats, HTTP clients for API integration, and namespace-prefixed identifiers for conflict resolution.

Research reveals that modern component libraries store centralized metadata (symbols, footprints, 3D models, parametric data) following IPC standards. KiCad's `.kicad_mod` footprint files use S-expression syntax (since v6.0+), and each `.pretty` folder contains multiple footprint files. JLCPCB provides a Components API for accessing millions of parts, though it requires application approval and API keys.

For search implementation, SQLite FTS5 provides production-ready full-text search with BM25 ranking, avoiding the complexity of embedding dedicated search engines like Tantivy. The existing Phase 9 Storage abstraction (SQLite on native, IndexedDB path for web) can be extended with structured schemas for library metadata rather than relying on key-value storage alone.

**Primary recommendation:** Extend SQLite storage with structured tables for component metadata and FTS5 virtual tables for search, use `lexpr` for parsing KiCad S-expressions, and implement namespace prefixes (e.g., `kicad::`, `jlcpcb::`, `custom::`) to prevent conflicts across library sources.

## Standard Stack

The established libraries/tools for this domain:

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| rusqlite | 0.32+ | SQLite database access with FTS5 | Already used in Phase 9, mature Rust bindings, supports full-text search extensions |
| lexpr | 0.2.7 | S-expression parser with Serde | 253k downloads, location tracking for errors, Serde integration for easy deserialization |
| reqwest | 0.11+ | Async HTTP client | De facto standard HTTP client, Tokio integration, JSON support for API calls |
| serde_json | 1.0+ | JSON serialization | Standard for API responses, already in workspace dependencies |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| async-trait | 0.1+ | Async trait support | Already in platform crate, needed for async Storage operations |
| tokio | 1.0+ | Async runtime | Already in workspace, required for async file I/O and HTTP requests |
| serde | 1.0+ | Serialization framework | Already in workspace, needed for deserializing API responses and library metadata |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| SQLite FTS5 | Tantivy | Tantivy is faster (~2x vs Lucene) but adds complexity, requires separate index management, and doesn't integrate with existing Storage. Use Tantivy only if search performance becomes critical bottleneck. |
| lexpr | sexp, rsexp | lexpr provides better error reporting with source locations, Serde integration for direct struct deserialization. Other parsers are lower-level or OCaml-focused. |
| rusqlite | sqlx | rusqlite is simpler, already used in Phase 9, and supports FTS5 extensions. sqlx adds connection pooling but unnecessary for single-user desktop app. |

**Installation:**
```bash
# Add to cypcb-platform/Cargo.toml (most already present)
[dependencies]
rusqlite = { version = "0.32", features = ["bundled"] }  # Already present
async-trait = "0.1"  # Already present
serde = { workspace = true }  # Already present

# New dependencies for library management
lexpr = "0.2.7"
reqwest = { version = "0.11", features = ["json"] }

# Native-only (library management needs file I/O)
[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
tokio = { version = "1", features = ["fs", "rt"] }  # Already present
```

## Architecture Patterns

### Recommended Project Structure
```
crates/cypcb-library/
├── src/
│   ├── lib.rs                  # Public API
│   ├── manager.rs              # LibraryManager orchestrator
│   ├── sources/                # Library source implementations
│   │   ├── mod.rs
│   │   ├── kicad.rs           # KiCad .kicad_mod parser
│   │   ├── jlcpcb.rs          # JLCPCB API client
│   │   └── custom.rs          # User-defined libraries
│   ├── schema.rs               # Database schema and migrations
│   ├── search.rs               # Search index manager (FTS5)
│   ├── models.rs               # Component, Footprint, Library structs
│   └── error.rs                # Library-specific errors
└── Cargo.toml
```

### Pattern 1: Namespace-Prefixed Components
**What:** Prevent name conflicts by prefixing component names with source identifier (e.g., `kicad::R_0805`, `jlcpcb::R_0805`, `custom::R_0805`)

**When to use:** Always for multi-source libraries. Components from different sources may have identical names but different implementations.

**Example:**
```rust
// models.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentId {
    pub source: String,      // "kicad", "jlcpcb", "custom"
    pub name: String,        // "R_0805"
}

impl ComponentId {
    pub fn new(source: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            name: name.into(),
        }
    }

    pub fn full_name(&self) -> String {
        format!("{}::{}", self.source, self.name)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Component {
    pub id: ComponentId,
    pub footprint: String,
    pub category: String,
    pub metadata: ComponentMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentMetadata {
    pub description: Option<String>,
    pub datasheet_url: Option<String>,
    pub manufacturer: Option<String>,
    pub mpn: Option<String>,        // Manufacturer Part Number
    pub value: Option<String>,      // e.g., "10k", "100nF"
    pub package: Option<String>,    // e.g., "0805", "SOT-23"
    pub step_model: Option<String>, // Path to 3D STEP file
}
```

### Pattern 2: Multi-Table Schema with FTS5
**What:** Structured SQLite schema with separate tables for libraries, components, footprints, and FTS5 virtual table for search

**When to use:** When extending Phase 9's Storage abstraction for structured library data

**Example:**
```rust
// schema.rs - Database schema initialization
pub const LIBRARY_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS libraries (
    source TEXT NOT NULL,           -- "kicad", "jlcpcb", "custom"
    name TEXT NOT NULL,             -- Library name
    path TEXT,                      -- File path for local libraries
    version TEXT,                   -- Version or import timestamp
    enabled INTEGER NOT NULL DEFAULT 1,
    PRIMARY KEY (source, name)
);

CREATE TABLE IF NOT EXISTS components (
    source TEXT NOT NULL,
    name TEXT NOT NULL,
    library TEXT NOT NULL,
    category TEXT,
    footprint TEXT,
    description TEXT,
    datasheet_url TEXT,
    manufacturer TEXT,
    mpn TEXT,
    value TEXT,
    package TEXT,
    step_model TEXT,
    metadata_json TEXT,             -- JSON blob for extensibility
    PRIMARY KEY (source, name),
    FOREIGN KEY (source, library) REFERENCES libraries(source, name)
);

CREATE INDEX IF NOT EXISTS idx_components_category ON components(category);
CREATE INDEX IF NOT EXISTS idx_components_manufacturer ON components(manufacturer);
CREATE INDEX IF NOT EXISTS idx_components_value ON components(value);

-- FTS5 virtual table for full-text search
CREATE VIRTUAL TABLE IF NOT EXISTS components_fts USING fts5(
    source,
    name,
    category,
    description,
    manufacturer,
    mpn,
    value,
    package,
    content=components,
    content_rowid=rowid
);

-- Triggers to keep FTS5 in sync with components table
CREATE TRIGGER IF NOT EXISTS components_ai AFTER INSERT ON components BEGIN
    INSERT INTO components_fts(rowid, source, name, category, description, manufacturer, mpn, value, package)
    VALUES (new.rowid, new.source, new.name, new.category, new.description, new.manufacturer, new.mpn, new.value, new.package);
END;

CREATE TRIGGER IF NOT EXISTS components_ad AFTER DELETE ON components BEGIN
    DELETE FROM components_fts WHERE rowid = old.rowid;
END;

CREATE TRIGGER IF NOT EXISTS components_au AFTER UPDATE ON components BEGIN
    UPDATE components_fts SET
        source = new.source,
        name = new.name,
        category = new.category,
        description = new.description,
        manufacturer = new.manufacturer,
        mpn = new.mpn,
        value = new.value,
        package = new.package
    WHERE rowid = old.rowid;
END;
"#;

// Search with FTS5 BM25 ranking
pub async fn search_components(
    conn: &rusqlite::Connection,
    query: &str,
    filters: SearchFilters,
) -> Result<Vec<Component>, LibraryError> {
    let mut sql = String::from(
        "SELECT c.* FROM components c
         JOIN components_fts fts ON c.rowid = fts.rowid
         WHERE components_fts MATCH ?1"
    );

    // Add optional filters
    if let Some(category) = filters.category {
        sql.push_str(&format!(" AND c.category = '{}'", category));
    }
    if let Some(manufacturer) = filters.manufacturer {
        sql.push_str(&format!(" AND c.manufacturer = '{}'", manufacturer));
    }

    // BM25 ranking for relevance
    sql.push_str(" ORDER BY bm25(components_fts) LIMIT ?2");

    // Execute query...
}
```

### Pattern 3: Trait-Based Source Abstraction
**What:** Define a `LibrarySource` trait that each source (KiCad, JLCPCB, custom) implements for uniform access

**When to use:** When adding multiple library sources with different loading mechanisms

**Example:**
```rust
// sources/mod.rs
#[async_trait]
pub trait LibrarySource {
    /// List all available libraries from this source
    async fn list_libraries(&self) -> Result<Vec<LibraryInfo>, LibraryError>;

    /// Import/sync a specific library by name
    async fn import_library(&self, name: &str) -> Result<Vec<Component>, LibraryError>;

    /// Search within this source (optional, can delegate to FTS5)
    async fn search(&self, query: &str) -> Result<Vec<Component>, LibraryError>;
}

// sources/kicad.rs
pub struct KiCadSource {
    search_paths: Vec<PathBuf>,
}

#[async_trait(?Send)]
impl LibrarySource for KiCadSource {
    async fn list_libraries(&self) -> Result<Vec<LibraryInfo>, LibraryError> {
        // Scan search_paths for .pretty folders
        let mut libraries = Vec::new();
        for path in &self.search_paths {
            // Use tokio::fs::read_dir with spawn_blocking to avoid blocking async runtime
            // Pattern: Parse .kicad_mod files with lexpr
        }
        Ok(libraries)
    }

    async fn import_library(&self, name: &str) -> Result<Vec<Component>, LibraryError> {
        // Find .pretty folder matching name
        // Parse each .kicad_mod file with lexpr
        // Use tokio::task::spawn_blocking for CPU-intensive parsing
        todo!()
    }
}

// sources/jlcpcb.rs
pub struct JLCPCBSource {
    client: reqwest::Client,
    api_key: String,
}

#[async_trait]
impl LibrarySource for JLCPCBSource {
    async fn list_libraries(&self) -> Result<Vec<LibraryInfo>, LibraryError> {
        // JLCPCB treats entire catalog as one "library"
        Ok(vec![LibraryInfo {
            source: "jlcpcb".into(),
            name: "JLCPCB Parts".into(),
            version: chrono::Utc::now().to_rfc3339(),
        }])
    }

    async fn import_library(&self, _name: &str) -> Result<Vec<Component>, LibraryError> {
        // Full catalog import is impractical (millions of parts)
        // Instead, implement search() for on-demand queries
        Err(LibraryError::NotSupported("Use search() for JLCPCB parts"))
    }

    async fn search(&self, query: &str) -> Result<Vec<Component>, LibraryError> {
        // Call JLCPCB API with search query
        let response = self.client
            .get("https://api.jlcpcb.com/components/search")
            .query(&[("q", query)])
            .header("Authorization", &self.api_key)
            .send()
            .await?;

        let components: Vec<JLCPCBComponent> = response.json().await?;
        // Convert to Component with source = "jlcpcb"
        Ok(components.into_iter().map(|c| c.into_component()).collect())
    }
}
```

### Pattern 4: Async File Parsing with spawn_blocking
**What:** Use `tokio::task::spawn_blocking` for CPU-intensive parsing to avoid blocking async runtime

**When to use:** When parsing many files (e.g., importing KiCad library with hundreds of footprints)

**Example:**
```rust
// sources/kicad.rs
use tokio::task;

async fn parse_kicad_footprint(path: PathBuf) -> Result<Component, LibraryError> {
    // Move file reading and parsing to blocking thread pool
    task::spawn_blocking(move || {
        let content = std::fs::read_to_string(&path)?;

        // Parse S-expression with lexpr
        let value = lexpr::from_str(&content)?;

        // Extract footprint data
        let footprint = parse_footprint_sexp(&value)?;

        Ok(Component {
            id: ComponentId::new("kicad", footprint.name.clone()),
            footprint: footprint.name,
            category: footprint.category,
            metadata: footprint.metadata,
        })
    })
    .await
    .map_err(|e| LibraryError::Parse(format!("Task failed: {}", e)))?
}

// Parse multiple footprints in parallel
async fn import_kicad_library(pretty_path: PathBuf) -> Result<Vec<Component>, LibraryError> {
    let mut entries = tokio::fs::read_dir(&pretty_path).await?;
    let mut tasks = Vec::new();

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.extension() == Some(std::ffi::OsStr::new("kicad_mod")) {
            tasks.push(parse_kicad_footprint(path));
        }
    }

    // Wait for all parses to complete
    let components = futures::future::try_join_all(tasks).await?;
    Ok(components)
}
```

### Anti-Patterns to Avoid
- **Blocking I/O in async context:** Never use `std::fs` directly in async functions. Always use `tokio::fs` or `spawn_blocking`. Blocking operations can kill throughput by 60%+ under load (2026 research).
- **Hand-rolling S-expression parser:** KiCad's format is well-defined but has edge cases. Use `lexpr` with Serde for automatic deserialization.
- **Storing raw file paths in database:** Store library metadata and parsed component data in SQLite. Only reference file paths for 3D models and original library locations.
- **Global namespace without prefixes:** Always namespace components by source to avoid conflicts when importing multiple libraries with same component names.
- **Scanning filesystem on every search:** Index components into database during import. Search queries hit FTS5, not filesystem.

## Don't Hand-Roll

Problems that look simple but have existing solutions:

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| S-expression parsing | Recursive descent parser | lexpr (0.2.7) | KiCad format has edge cases (quoted strings, escaped chars, nested lists). lexpr provides source locations for errors and Serde integration. |
| Full-text search | String contains/regex | SQLite FTS5 | FTS5 provides BM25 ranking, stemming, prefix matching, and phrase queries. Building competitive search requires understanding TF-IDF, tokenization, and ranking algorithms. |
| HTTP retry logic | Manual retry loops | reqwest with retry middleware | Network requests need exponential backoff, jitter, timeout handling. Production HTTP clients handle these correctly. |
| JSON API parsing | Manual string parsing | serde_json with typed structs | API responses have nested structures, null handling, type conversions. Serde provides compile-time validation. |
| File format versioning | Custom version checks | lexpr with enum deserialization | KiCad footprints have version tokens. Deserialize to enums for exhaustive version handling. |
| Hierarchical categories | String splitting | Hierarchical FTS5 fields or separate category table | Faceted search requires proper indexing. Splitting "Electronics > Resistors > 0805" on every query is slow. |

**Key insight:** Component library management is data-intensive, not algorithm-intensive. Use proven tools for parsing, search, and storage rather than building custom solutions. The complexity lies in handling multiple formats, conflicting names, and incremental updates—not in the core parsing/search algorithms.

## Common Pitfalls

### Pitfall 1: Blocking Async Runtime with Synchronous File I/O
**What goes wrong:** Using `std::fs` operations in async functions blocks the Tokio runtime, causing throughput to drop 60%+ under concurrent load. Manifests as slow library imports when user adds multiple libraries simultaneously.

**Why it happens:** Rust's `std::fs` API is synchronous and blocking. Calling it inside an async function prevents the executor from running other tasks. Not obvious in development with single imports, but critical under load.

**How to avoid:**
- Use `tokio::fs` for file operations when possible
- Use `tokio::task::spawn_blocking` for CPU-intensive parsing (lexpr parsing large files)
- Never use `std::fs::read_*` directly in async context

**Warning signs:**
- Library import hangs when multiple sources are added
- Other operations freeze during import
- `tokio::task::spawn_blocking` shows high contention in profiling

### Pitfall 2: Not Namespacing Components from Multiple Sources
**What goes wrong:** When importing KiCad and JLCPCB libraries, components like "R_0805" exist in both. Without namespaces, later import overwrites earlier one, or database constraint fails on duplicate key.

**Why it happens:** Component names are not globally unique. Different library sources use same naming conventions (IPC-7351 standard). Natural to use component name as primary key.

**How to avoid:**
- Always use composite key: (source, name) as primary key in database
- Include source prefix in UI display: "kicad::R_0805" vs "jlcpcb::R_0805"
- Allow user to choose preferred source when ambiguous

**Warning signs:**
- "Duplicate key" errors during library import
- Components disappearing after importing second library
- User reports "wrong footprint" for component (got JLCPCB version instead of KiCad)

### Pitfall 3: Scanning Filesystem for Libraries on Every Search
**What goes wrong:** Search query triggers filesystem scan of `.pretty` folders, parsing `.kicad_mod` files to find matches. Search takes seconds instead of milliseconds. User perceives UI as frozen.

**Why it happens:** Treating filesystem as source of truth instead of database. "Just scan the folder" approach works for demos but doesn't scale to libraries with 1000+ components.

**How to avoid:**
- Import/index libraries into SQLite during initial setup
- Search queries only hit FTS5 index, never filesystem
- Background task syncs filesystem changes to database (use `notify` crate from workspace)
- Only re-parse individual files when detected as modified

**Warning signs:**
- Search latency grows linearly with library size
- Disk I/O spikes during search
- Search times inconsistent based on disk cache state

### Pitfall 4: Missing FTS5 Triggers for Index Synchronization
**What goes wrong:** Components table gets updated (INSERT/UPDATE/DELETE), but FTS5 virtual table not updated. Search returns stale results or misses newly added components.

**Why it happens:** FTS5 virtual table is separate from base table. Must explicitly keep them in sync with triggers. Easy to forget when focused on application logic.

**How to avoid:**
- Create INSERT/UPDATE/DELETE triggers when creating FTS5 table (see Pattern 2)
- Test search immediately after component import to verify indexing
- Consider using `content=components` option which creates triggers automatically (but verify in rusqlite)

**Warning signs:**
- Search doesn't find newly imported components until app restart
- Deleted components still appear in search results
- Updated component descriptions don't match search results

### Pitfall 5: JLCPCB API Rate Limiting Without Caching
**What goes wrong:** Each search query hits JLCPCB API. User types "resistor" → 7 API calls (one per keystroke). Hits rate limit, gets 429 errors, search fails.

**Why it happens:** JLCPCB API has rate limits (unknown specifics, requires API application). Natural to implement search as direct API call. Works in manual testing but fails with typeahead search.

**How to avoid:**
- Implement debouncing: only search after 300ms typing pause
- Cache JLCPCB search results in SQLite with timestamp (24hr expiry)
- For popular searches (resistors, capacitors), pre-populate cache
- Check cache before calling API

**Warning signs:**
- 429 Too Many Requests errors in logs
- Search fails intermittently
- Search works in demo but fails when used actively

### Pitfall 6: Not Handling KiCad Version Differences
**What goes wrong:** Parser assumes KiCad 8.0 format, user imports library from KiCad 5.x. Parser fails with cryptic S-expression error. Import silently fails or crashes.

**Why it happens:** KiCad footprint format evolved across versions. Pre-v6: minimally quoted strings. v6+: standardized quoting, version token. Parser written against latest docs may not handle older formats.

**How to avoid:**
- Check `(version ...)` token in footprint S-expression
- Support KiCad 6.0+ initially (2018+), document minimum version
- Provide clear error message if older version detected: "KiCad 5.x libraries not supported. Please use KiCad 6.0 or later, or convert library using KiCad."

**Warning signs:**
- Import fails on older community libraries
- lexpr returns parsing errors on valid KiCad files
- Different behavior on files from different KiCad versions

### Pitfall 7: Assuming JLCPCB API is Publicly Available
**What goes wrong:** Hardcode JLCPCB API endpoints, ship feature enabled by default. Users report "JLCPCB search doesn't work." API requires application approval and key.

**Why it happens:** Documentation mentions API exists at api.jlcpcb.com. Natural assumption it's public like many REST APIs. But JLCPCB requires manual approval: "not all applications will be approved; each application undergoes a review based on the partner's previous orders."

**How to avoid:**
- Make JLCPCB integration optional, disabled by default
- Require user to provide API key in settings
- Document API application process with link: https://jlcpcb.com/help/article/jlcpcb-online-api-available-now
- Provide fallback: link to JLCPCB parts catalog in browser if no API key

**Warning signs:**
- Users expect JLCPCB search to work out-of-box
- Authentication errors without clear guidance
- Confusion about why feature is present but non-functional

## Code Examples

Verified patterns from research sources:

### Parsing KiCad Footprint with lexpr
```rust
// Source: https://crates.io/crates/lexpr + KiCad dev docs
use lexpr::Value;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct KiCadFootprint {
    #[serde(rename = "footprint")]
    _tag: (),  // Match "footprint" token
    name: String,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    generator: Option<String>,
    #[serde(default)]
    layer: Option<String>,
    #[serde(default)]
    descr: Option<String>,
    // Add more fields as needed (pads, fp_line, etc.)
}

fn parse_footprint_file(path: &Path) -> Result<KiCadFootprint, LibraryError> {
    let content = std::fs::read_to_string(path)?;

    // Parse S-expression
    let value = lexpr::from_str(&content)
        .map_err(|e| LibraryError::Parse(format!("Invalid S-expression: {}", e)))?;

    // Deserialize to struct
    let footprint: KiCadFootprint = lexpr::from_value(&value)
        .map_err(|e| LibraryError::Parse(format!("Invalid footprint structure: {}", e)))?;

    // Check version support
    if let Some(version) = &footprint.version {
        if version.parse::<i32>().unwrap_or(0) < 20180101 {
            return Err(LibraryError::UnsupportedVersion(
                "KiCad 6.0+ required (2018+)".into()
            ));
        }
    }

    Ok(footprint)
}
```

### FTS5 Search Query with BM25 Ranking
```rust
// Source: https://sqlite.org/fts5.html + rusqlite docs
use rusqlite::{Connection, params};

#[derive(Debug)]
struct SearchResult {
    source: String,
    name: String,
    category: String,
    description: String,
    rank: f64,
}

fn search_components_fts5(
    conn: &Connection,
    query: &str,
    limit: usize,
) -> Result<Vec<SearchResult>, rusqlite::Error> {
    // FTS5 query with BM25 ranking
    // bm25() returns negative scores; lower (more negative) = better match
    let mut stmt = conn.prepare(
        "SELECT
            c.source,
            c.name,
            c.category,
            c.description,
            bm25(components_fts) as rank
         FROM components c
         JOIN components_fts fts ON c.rowid = fts.rowid
         WHERE components_fts MATCH ?1
         ORDER BY rank
         LIMIT ?2"
    )?;

    let results = stmt.query_map(params![query, limit], |row| {
        Ok(SearchResult {
            source: row.get(0)?,
            name: row.get(1)?,
            category: row.get(2)?,
            description: row.get(3)?,
            rank: row.get(4)?,
        })
    })?
    .collect::<Result<Vec<_>, _>>()?;

    Ok(results)
}

// Example FTS5 query syntax:
// "resistor" -> matches any field containing "resistor"
// "resistor AND 0805" -> both terms must match
// "resistor*" -> prefix match (resistors, resistance)
// "manufacturer:TI" -> field-specific match
// NEAR(resistor capacitor, 5) -> words within 5 tokens
```

### Async JLCPCB API Search
```rust
// Source: https://crates.io/crates/reqwest + JLCPCB API docs
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
struct JLCPCBResponse {
    data: Vec<JLCPCBComponent>,
    total: usize,
}

#[derive(Debug, Deserialize, Serialize)]
struct JLCPCBComponent {
    #[serde(rename = "componentCode")]
    component_code: String,

    #[serde(rename = "componentName")]
    component_name: String,

    #[serde(rename = "componentDesignator")]
    designator: Option<String>,

    #[serde(rename = "componentModel")]
    model: Option<String>,

    #[serde(rename = "stockCount")]
    stock_count: u32,

    price: Option<String>,
    describe: Option<String>,

    #[serde(rename = "manufacturerName")]
    manufacturer: Option<String>,
}

async fn search_jlcpcb_api(
    client: &Client,
    api_key: &str,
    query: &str,
) -> Result<Vec<JLCPCBComponent>, reqwest::Error> {
    let response = client
        .get("https://api.jlcpcb.com/components/search")
        .header("Authorization", format!("Bearer {}", api_key))
        .query(&[
            ("keyword", query),
            ("page", "1"),
            ("pageSize", "50"),
        ])
        .send()
        .await?
        .error_for_status()?;

    let result: JLCPCBResponse = response.json().await?;
    Ok(result.data)
}

// Note: Actual JLCPCB API endpoints and authentication may differ
// This is based on common REST API patterns; verify with official docs
```

### Library Version Tracking
```rust
// Source: File version control best practices research
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LibraryVersion {
    source: String,
    name: String,
    version_id: String,      // Git SHA, timestamp, or KiCad version
    imported_at: DateTime<Utc>,
    component_count: usize,
    metadata: serde_json::Value,
}

// Track library versions for rollback capability
async fn save_library_version(
    conn: &Connection,
    library: &LibraryVersion,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT INTO library_versions
         (source, name, version_id, imported_at, component_count, metadata)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            &library.source,
            &library.name,
            &library.version_id,
            &library.imported_at.to_rfc3339(),
            &library.component_count,
            &library.metadata.to_string(),
        ],
    )?;
    Ok(())
}

// List all versions for rollback UI
async fn list_library_versions(
    conn: &Connection,
    source: &str,
    name: &str,
) -> Result<Vec<LibraryVersion>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT version_id, imported_at, component_count, metadata
         FROM library_versions
         WHERE source = ?1 AND name = ?2
         ORDER BY imported_at DESC"
    )?;

    let versions = stmt.query_map(params![source, name], |row| {
        Ok(LibraryVersion {
            source: source.to_string(),
            name: name.to_string(),
            version_id: row.get(0)?,
            imported_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(1)?)
                .unwrap().with_timezone(&Utc),
            component_count: row.get(2)?,
            metadata: serde_json::from_str(&row.get::<_, String>(3)?).unwrap(),
        })
    })?
    .collect::<Result<Vec<_>, _>>()?;

    Ok(versions)
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Manual library file scanning | Database-backed indexing with FTS5 | Established pattern | Search latency: seconds → milliseconds |
| String search in memory | SQLite FTS5 with BM25 ranking | FTS5 stable since 2015 | Relevance ranking, prefix matching, phrase queries |
| Nom parser combinators | Winnow (nom fork) or lexpr for S-expressions | Winnow 0.5 in 2023 | Winnow prioritizes future parsers; lexpr better for Serde integration |
| Tantivy for all search | SQLite FTS5 for component search | Tantivy 0.10+ mature | FTS5 sufficient for component search; Tantivy overkill unless >1M components |
| Blocking file I/O | tokio::fs + spawn_blocking | Standard since async/await | Prevents runtime blocking, maintains throughput |
| Global component names | Namespace-prefixed identifiers | Industry pattern (R, Python) | Prevents conflicts from multiple sources |

**Deprecated/outdated:**
- **KiCad legacy `.mod` format**: Replaced by `.kicad_mod` S-expression format in ~2014. Modern libraries use `.kicad_mod` exclusively.
- **nom for new projects**: Winnow (nom fork) is recommended for new parsers; nom in maintenance mode prioritizing existing users. For S-expressions, `lexpr` is higher-level and preferred.
- **Manual HTTP retry logic**: Use reqwest middleware or tower-http for retry/backoff. Hand-rolled retry loops miss edge cases (idempotency, jitter).

## Open Questions

Things that couldn't be fully resolved:

1. **JLCPCB API Endpoint Specification**
   - What we know: API exists at api.jlcpcb.com, requires application approval, provides component search with pricing/inventory
   - What's unclear: Exact endpoint paths, authentication method (Bearer token? API key header?), rate limits, response schema
   - Recommendation: Treat JLCPCB integration as optional feature requiring user-provided API key. Document need for API application. Implement generic REST client that can be adapted once API access obtained. Consider scraping JLCPCB web catalog as fallback (but check ToS).

2. **KiCad .pretty Folder Metadata**
   - What we know: `.pretty` folders contain multiple `.kicad_mod` files, each file = one footprint
   - What's unclear: How to extract library-level metadata (description, author, license). Individual footprints have metadata, but library-level info unclear. May be in separate `.json` or `.dcm` file.
   - Recommendation: Scan `.pretty` folder for all `.kicad_mod` files, treat folder name as library name. If metadata file found, parse it; otherwise derive metadata from aggregating individual footprint data.

3. **3D STEP Model Storage Strategy**
   - What we know: Components can have associated STEP files (ISO 10303-21), footprints reference them
   - What's unclear: Store STEP files in database as BLOBs, or store file paths? Large STEP files (100KB-1MB each) could bloat SQLite database. Web platform can't access arbitrary file paths.
   - Recommendation: Store relative paths in database for desktop (assume STEP models in library folder hierarchy). For web, support upload/store in IndexedDB as BLOBs. Phase 10 focus on footprint/metadata; defer full 3D model integration to future phase.

4. **Library Update/Sync Strategy**
   - What we know: Libraries change over time (new components, corrections), need version tracking for rollback
   - What's unclear: How often to check for updates? Auto-update vs manual? Git-based libraries vs file-based?
   - Recommendation: Phase 10 implements manual import/re-import with version snapshots. Store import timestamp as version. Future phase could add git integration for community libraries (KiCad official library is on GitHub).

5. **Category/Tag Hierarchy Structure**
   - What we know: Components need organization by category (Resistors > SMD > 0805), faceted search benefits from hierarchy
   - What's unclear: Fixed taxonomy vs user-defined categories? Multi-parent categories (component in both "Power" and "0805" categories)?
   - Recommendation: Start with simple string category field supporting "/" hierarchy (e.g., "Passive/Resistor/0805"). Store in single column, search with LIKE for substring match. Future phase can add proper hierarchical faceting with separate category table and many-to-many relationship.

## Sources

### Primary (HIGH confidence)
- KiCad Developer Documentation - Footprint File Format: https://dev-docs.kicad.org/en/file-formats/sexpr-footprint/index.html (Official specification for .kicad_mod S-expression format, KiCad 6.0+)
- SQLite FTS5 Extension: https://sqlite.org/fts5.html (Official SQLite documentation for full-text search)
- lexpr crate documentation: https://crates.io/crates/lexpr (S-expression parser with Serde, 253k downloads)
- rusqlite crate documentation: https://crates.io/crates/rusqlite (Already used in Phase 9, supports FTS5)
- reqwest crate documentation: https://crates.io/crates/reqwest (Standard async HTTP client)

### Secondary (MEDIUM confidence)
- JLCPCB API Platform: https://api.jlcpcb.com/ (Overview of Components API, lacks detailed documentation)
- Tantivy GitHub: https://github.com/quickwit-oss/tantivy (Full-text search engine library, ~2x faster than Lucene)
- VS Code docfind article (2026): https://code.visualstudio.com/blogs/2026/01/15/docfind (FST-based search implementation)
- Component Library Best Practices: https://www.ultralibrarian.com/about/standards (Industry standards: IPC-7351B, ISO 10303-21)
- Rust Async Performance (2026): https://medium.com/@shkmonty35/rust-async-just-killed-your-throughput-and-you-didnt-notice-c38dd119aae5 (Critical: avoid blocking ops in async)

### Tertiary (LOW confidence)
- KiCad Library Management Forum: https://forum.kicad.info/t/kicad-library-management-and-users/45805 (Community reports of manual library management issues)
- Meilisearch Hierarchical Facets: https://www.meilisearch.com/blog/nested-hierarchical-facets-guide (Hierarchical search patterns, not Rust-specific)
- Winnow vs nom vs pest comparison: https://blog.wesleyac.com/posts/rust-parsing (Parser comparison, predates Winnow 0.5)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - All libraries are established with high download counts and already used in project (rusqlite, tokio, serde) or well-documented with examples (lexpr, reqwest)
- Architecture: MEDIUM-HIGH - Patterns derived from official documentation (SQLite FTS5, KiCad format) and established practices (namespace prefixing, trait abstraction). Specific to this project's multi-source needs.
- Pitfalls: HIGH - Based on 2026 research (async blocking), official KiCad forums (library management issues), and SQLite documentation (FTS5 triggers). Directly applicable warnings.
- JLCPCB integration: LOW - API exists but documentation requires application for access. Endpoints and schemas not publicly verified.

**Research date:** 2026-01-29
**Valid until:** 2026-04-29 (90 days for stable domain - database patterns, file formats don't change rapidly; 30 days for JLCPCB API as it may evolve)
