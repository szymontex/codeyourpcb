# Phase 5: Intelligence - Research

**Researched:** 2026-01-22
**Domain:** Developer tooling, autorouting, footprint import, electrical calculations
**Confidence:** HIGH

## Summary

Phase 5 adds professional IDE integration via LSP, autorouting via FreeRouting integration, KiCad footprint import, and IPC-2221 trace width calculation. This research establishes the standard approach for each capability.

The LSP server will use **tower-lsp** (or its community fork tower-lsp-server) as the Rust LSP framework, integrating with the existing Tree-sitter parser for incremental parsing and AST analysis. The autorouter integration uses **FreeRouting** via CLI with DSN/SES file exchange - the industry standard approach. KiCad footprint import will use the **kicad_parse_gen** crate to parse .kicad_mod S-expression files. Trace width calculation follows **IPC-2221** formulas with different constants for internal vs external layers.

**Primary recommendation:** Build incrementally - start with LSP basics (hover, diagnostics), add completion, then integrate autorouting. KiCad import and trace calculator can proceed in parallel.

## Standard Stack

The established libraries/tools for this domain:

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| tower-lsp-server | 0.3+ | LSP server framework | Community fork with updated lsp-types 0.97, active maintenance |
| lsp-types | 0.97 | LSP protocol types | Official protocol type definitions |
| tokio | 1.x | Async runtime | Required by tower-lsp, already common in ecosystem |
| FreeRouting | 2.1.0 | PCB autorouter | Industry standard, Specctra DSN compatible, CLI mode |
| kicad_parse_gen | 7.0.2 | KiCad file parser | Mature crate for .kicad_mod parsing |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| lsp-textdocument | latest | Document sync | Incremental document updates from LSP |
| serde_json | 1.0 | JSON-RPC | LSP message serialization (already in workspace) |
| symbolic_expressions | 5.0 | S-expr parsing | Alternative to kicad_parse_gen if needed |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| tower-lsp-server | tower-lsp (original) | Original has stale lsp-types 0.94, less active |
| kicad_parse_gen | kiparse | kiparse is newer but less documented |
| FreeRouting CLI | FreeRouting API | API is beta, CLI is stable and proven |

**Installation:**
```bash
# Rust dependencies (add to Cargo.toml)
cargo add tower-lsp-server tokio --features tokio/full
cargo add kicad_parse_gen

# FreeRouting (external tool)
# Download from https://github.com/freerouting/freerouting/releases
# Requires JRE 21+
```

## Architecture Patterns

### Recommended Project Structure
```
crates/
├── cypcb-lsp/           # LSP server crate
│   ├── src/
│   │   ├── lib.rs       # Backend struct, LanguageServer impl
│   │   ├── main.rs      # Server entry point (stdio/TCP)
│   │   ├── completion.rs # Completion provider
│   │   ├── hover.rs     # Hover provider
│   │   ├── diagnostics.rs # DRC integration
│   │   └── document.rs  # Document state management
│   └── Cargo.toml
├── cypcb-router/        # Autorouting integration
│   ├── src/
│   │   ├── lib.rs       # Router trait, result types
│   │   ├── dsn.rs       # DSN export (board -> .dsn)
│   │   ├── ses.rs       # SES import (.ses -> traces)
│   │   ├── freerouting.rs # FreeRouting CLI wrapper
│   │   └── trace.rs     # Trace ECS component
│   └── Cargo.toml
├── cypcb-kicad/         # KiCad import
│   ├── src/
│   │   ├── lib.rs       # Import API
│   │   ├── footprint.rs # .kicad_mod -> Footprint conversion
│   │   └── library.rs   # Library directory handling
│   └── Cargo.toml
└── cypcb-calc/          # Electrical calculations
    ├── src/
    │   ├── lib.rs       # Calculator API
    │   ├── trace_width.rs # IPC-2221 implementation
    │   └── impedance.rs # Future: IPC-2141 impedance
    └── Cargo.toml
```

### Pattern 1: LSP Backend with Shared State

**What:** Single Backend struct holds Client handle and document state, implements LanguageServer trait
**When to use:** All LSP implementations with tower-lsp

**Example:**
```rust
// Source: tower-lsp-boilerplate pattern
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};
use std::sync::Arc;
use tokio::sync::RwLock;

struct Backend {
    client: Client,
    documents: Arc<RwLock<HashMap<Url, DocumentState>>>,
}

struct DocumentState {
    content: String,
    version: i32,
    ast: Option<SourceFile>,  // Parsed AST from cypcb-parser
    diagnostics: Vec<Diagnostic>,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL  // Start with full sync, optimize later
                )),
                completion_provider: Some(CompletionOptions::default()),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        // Query document state, find symbol at position
        // Return hover info from AST/footprint library
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        // Context-aware completion based on cursor position in AST
    }
}
```

### Pattern 2: FreeRouting CLI Integration

**What:** Spawn FreeRouting as subprocess, exchange data via DSN/SES files
**When to use:** Autorouting workflow

**Example:**
```rust
// Source: FreeRouting documentation
use std::process::{Command, Stdio};
use std::path::Path;

pub struct FreeRoutingRunner {
    jar_path: PathBuf,
    timeout_secs: u64,
}

impl FreeRoutingRunner {
    pub async fn route(&self, dsn_path: &Path, ses_path: &Path) -> Result<RoutingResult> {
        // FreeRouting CLI: java -jar freerouting.jar -de input.dsn -do output.ses
        let status = Command::new("java")
            .args([
                "-jar", self.jar_path.to_str().unwrap(),
                "-de", dsn_path.to_str().unwrap(),
                "-do", ses_path.to_str().unwrap(),
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?
            .wait_with_output()?;

        if status.status.success() && ses_path.exists() {
            // Parse SES file and extract routes
            let routes = parse_ses(ses_path)?;
            Ok(RoutingResult::Complete(routes))
        } else {
            Ok(RoutingResult::Failed(String::from_utf8_lossy(&status.stderr).into()))
        }
    }
}
```

### Pattern 3: DSN Export Structure

**What:** Export board model to Specctra DSN format for autorouter
**When to use:** Before calling FreeRouting

**Example:**
```rust
// Source: Specctra DSN specification
pub fn export_dsn(world: &BoardWorld, output: &mut impl Write) -> Result<()> {
    writeln!(output, "(pcb \"board\"")?;
    writeln!(output, "  (parser")?;
    writeln!(output, "    (string_quote \")")?;
    writeln!(output, "    (space_in_quoted_tokens on)")?;
    writeln!(output, "  )")?;
    writeln!(output, "  (resolution mil 10)")?;  // 0.1 mil resolution
    writeln!(output, "  (unit mil)")?;

    // Structure section: layers, boundary, rules
    write_structure(world, output)?;

    // Placement section: component positions
    write_placement(world, output)?;

    // Library section: footprints and padstacks
    write_library(world, output)?;

    // Network section: nets and pin connections
    write_network(world, output)?;

    writeln!(output, ")")?;
    Ok(())
}
```

### Pattern 4: IPC-2221 Trace Width Calculation

**What:** Calculate minimum trace width from current, temperature rise, copper weight
**When to use:** Trace width suggestions, constraint validation

**Example:**
```rust
// Source: IPC-2221 standard
/// IPC-2221 trace width calculator
pub struct TraceWidthCalculator;

impl TraceWidthCalculator {
    /// Calculate minimum trace width for given current
    ///
    /// Formula: I = k * dT^0.44 * A^0.725
    /// Solve for A: A = (I / (k * dT^0.44))^(1/0.725)
    /// Width = A / (thickness * 1.378)
    pub fn min_width_for_current(
        current_amps: f64,
        temp_rise_c: f64,      // Typically 10C
        copper_oz: f64,        // 1oz = 35um = 1.378 mil thickness
        is_external: bool,     // External layers dissipate heat better
    ) -> Nm {
        // k = 0.048 for external, 0.024 for internal
        let k = if is_external { 0.048 } else { 0.024 };

        // Calculate cross-sectional area in mils^2
        let area_mils2 = (current_amps / (k * temp_rise_c.powf(0.44))).powf(1.0 / 0.725);

        // Convert to width: thickness in mils = copper_oz * 1.378
        let thickness_mils = copper_oz * 1.378;
        let width_mils = area_mils2 / thickness_mils;

        // Convert mils to nanometers (1 mil = 25400 nm)
        Nm((width_mils * 25_400.0) as i64)
    }
}
```

### Anti-Patterns to Avoid

- **Blocking LSP thread:** Never do heavy computation in LSP request handlers - spawn background tasks
- **Full document sync forever:** Start with FULL sync but plan incremental sync for large files
- **Embedded FreeRouting:** Don't try to embed FreeRouting as a library - CLI integration is the supported path
- **Custom DSN parser:** Don't write a full DSN parser - only need to export and import what we produce
- **Floating point in coordinates:** Continue using integer nanometers - convert only at DSN export

## Don't Hand-Roll

Problems that look simple but have existing solutions:

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| LSP protocol handling | Custom JSON-RPC | tower-lsp-server | Protocol is complex, edge cases in notifications |
| KiCad S-expr parsing | nom/manual parser | kicad_parse_gen | KiCad format has many undocumented quirks |
| Autorouting algorithm | Maze router | FreeRouting | Autorouting is PhD-level, FreeRouting is battle-tested |
| IPC-2221 tables | Lookup tables | Formulas | Formulas more flexible, tables have gaps |
| LSP position conversion | Byte offset math | lsp-textdocument | UTF-16 code unit handling is tricky |

**Key insight:** The intelligence phase is about integration, not invention. FreeRouting spent decades on routing algorithms. tower-lsp handles LSP protocol complexities. kicad_parse_gen understands KiCad's quirks. Use these, focus on integration.

## Common Pitfalls

### Pitfall 1: LSP Position Encoding Mismatch

**What goes wrong:** Editor reports position as UTF-16 code units, server uses byte offsets, cursor lands wrong
**Why it happens:** LSP protocol defaults to UTF-16 (for historical JavaScript reasons)
**How to avoid:** Use lsp-textdocument crate for document management, or negotiate UTF-8 position encoding in capabilities
**Warning signs:** Completion/hover off by one character, worse on non-ASCII

### Pitfall 2: FreeRouting Timeout on Complex Boards

**What goes wrong:** FreeRouting runs forever, user thinks it's frozen
**Why it happens:** FreeRouting doesn't have built-in timeout, complex boards take minutes
**How to avoid:**
- Implement timeout (kill process after N seconds)
- Show progress indicator ("Routing... this may take a few minutes")
- Provide cancel button
**Warning signs:** No output for >30 seconds

### Pitfall 3: DSN Coordinate System Mismatch

**What goes wrong:** Components end up mirrored or offset after routing import
**Why it happens:** DSN uses different origin/Y-direction than internal model
**How to avoid:**
- DSN uses mils, project uses nanometers - convert carefully
- DSN Y-axis may be inverted - verify with simple test board
- Round-trip test: export -> route -> import should match
**Warning signs:** Routes don't connect to pads, visual offset

### Pitfall 4: KiCad Footprint Version Incompatibility

**What goes wrong:** Parser fails on some .kicad_mod files
**Why it happens:** KiCad format evolves, files from different versions have different structures
**How to avoid:**
- Use kicad_parse_gen which tracks KiCad updates
- Test with files from KiCad 5, 6, 7, and 8
- Graceful degradation: skip unsupported features, log warning
**Warning signs:** Some footprints import, others fail silently

### Pitfall 5: IPC-2221 Accuracy Limits

**What goes wrong:** Calculated trace width doesn't match reality
**Why it happens:** IPC-2221 formulas are approximations, accurate only for specific conditions
**How to avoid:**
- Document that this is an estimate, not a guarantee
- Only accurate for ~50 ohm impedance target
- Only valid up to 35A, 0.4" width, 10-100C rise
- Recommend 10% margin
**Warning signs:** User expects precise values, gets surprised by fab results

### Pitfall 6: LSP Diagnostics Flooding

**What goes wrong:** Editor becomes slow, thousands of diagnostics spam the problems panel
**Why it happens:** DRC reports every violation, large boards have many
**How to avoid:**
- Limit diagnostics to first N per file (e.g., 100)
- Group similar violations
- Only report on save, not on every keystroke
**Warning signs:** Editor lag, scroll bar in problems panel

## Code Examples

Verified patterns from official sources:

### LSP Server Main Entry Point
```rust
// Source: tower-lsp examples
use tower_lsp::{LspService, Server};

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| Backend {
        client,
        documents: Arc::new(RwLock::new(HashMap::new())),
    });

    Server::new(stdin, stdout, socket).serve(service).await;
}
```

### Tree-sitter Integration for Completions
```rust
// Source: GitHub discussion on tree-sitter + LSP
async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
    let uri = params.text_document_position.text_document.uri;
    let position = params.text_document_position.position;

    let docs = self.documents.read().await;
    let doc = docs.get(&uri).ok_or_else(|| Error::invalid_params("Document not found"))?;

    // Convert LSP position to byte offset
    let offset = position_to_offset(&doc.content, position);

    // Use existing Tree-sitter AST to find context
    if let Some(ast) = &doc.ast {
        let context = find_completion_context(ast, offset);
        let items = match context {
            Context::ComponentFootprint => footprint_completions(),
            Context::NetName => net_completions(ast),
            Context::ComponentType => component_type_completions(),
            Context::PropertyKey => property_completions(&context),
            _ => vec![],
        };
        return Ok(Some(CompletionResponse::Array(items)));
    }

    Ok(None)
}
```

### DSN Network Section Export
```rust
// Source: Specctra DSN gist + KiCad implementation
fn write_network(world: &BoardWorld, output: &mut impl Write) -> Result<()> {
    writeln!(output, "  (network")?;

    // Export each net with its pin connections
    for (net_id, net_name) in world.net_registry().iter() {
        writeln!(output, "    (net {}", quote_dsn(net_name))?;
        write!(output, "      (pins")?;

        // Find all pins connected to this net
        for (refdes, pin) in world.pins_on_net(net_id) {
            write!(output, " {}-{}", refdes, pin)?;
        }
        writeln!(output, ")")?;
        writeln!(output, "    )")?;
    }

    // Define net class with default rules
    writeln!(output, "    (class default")?;
    for (_, net_name) in world.net_registry().iter() {
        write!(output, " {}", quote_dsn(net_name))?;
    }
    writeln!(output, "")?;
    writeln!(output, "      (rule (width 8))")?;  // 8 mils default
    writeln!(output, "    )")?;

    writeln!(output, "  )")?;
    Ok(())
}
```

### SES Import for Trace Extraction
```rust
// Source: Specctra specification
fn parse_ses_wiring(content: &str, world: &mut BoardWorld) -> Result<Vec<Trace>> {
    let mut traces = Vec::new();

    // SES file contains (routes (network_out ...))
    // Within network_out: (wire (path layer width x1 y1 x2 y2 ...))

    for wire in extract_wires(content)? {
        let layer = parse_layer(&wire.layer)?;
        let width = Nm::from_mils(wire.width);

        // Convert path coordinates from mils to nm
        let points: Vec<Point> = wire.path
            .chunks(2)
            .map(|chunk| Point::from_mils(chunk[0], chunk[1]))
            .collect();

        // Create trace segments
        for window in points.windows(2) {
            traces.push(Trace {
                layer,
                width,
                start: window[0],
                end: window[1],
                net_id: wire.net_id,
            });
        }
    }

    Ok(traces)
}
```

### KiCad Footprint Import
```rust
// Source: kicad_parse_gen crate
use kicad_parse_gen::footprint::{read_module, Module};

pub fn import_kicad_footprint(path: &Path) -> Result<Footprint> {
    let module: Module = read_module(path)?;

    let mut pads = Vec::new();
    for kicad_pad in &module.elements {
        if let Element::Pad(p) = kicad_pad {
            pads.push(PadDef {
                number: p.name.clone(),
                shape: convert_shape(&p.shape),
                position: Point::from_mm(p.at.x, p.at.y),
                size: (Nm::from_mm(p.size.x), Nm::from_mm(p.size.y)),
                drill: p.drill.map(|d| Nm::from_mm(d.size)),
                layers: convert_layers(&p.layers),
            });
        }
    }

    Ok(Footprint {
        name: module.name.clone(),
        description: module.descr.unwrap_or_default(),
        pads,
        bounds: calculate_bounds(&pads),
        courtyard: extract_courtyard(&module).unwrap_or_else(|| calculate_courtyard(&pads)),
    })
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| tower-lsp original | tower-lsp-server fork | 2024 | Updated lsp-types, better async_trait |
| KiCad .mod files | .kicad_mod S-expr | KiCad 4.0 (2015) | Modern format is standard |
| FreeRouting GUI only | FreeRouting CLI + API | 2023-2024 | Enables automation |
| IPC-2221 lookup tables | Closed-form equations | Always | Equations from same standard |

**Deprecated/outdated:**
- **lsp-types 0.94**: Use 0.97+ for latest protocol features
- **tower-lsp 0.20**: Community fork tower-lsp-server is more active
- **KiCad .mod format**: Legacy, only for KiCad 4 compatibility

## Open Questions

Things that couldn't be fully resolved:

1. **FreeRouting scoring system details**
   - What we know: Version 2.1.0 added a scoring system for routing quality
   - What's unclear: Exact metrics and how to extract score programmatically
   - Recommendation: Parse FreeRouting stdout for score, or use API when stable

2. **Trace routing result storage format**
   - What we know: User wants routes in separate file (e.g., .routes)
   - What's unclear: Exact format - reuse DSN/SES, or custom format?
   - Recommendation: Start with SES-derived format, iterate based on needs

3. **LSP incremental document sync performance**
   - What we know: Full sync is simpler, incremental is faster for large files
   - What's unclear: At what file size does incremental matter for .cypcb?
   - Recommendation: Start with FULL sync, profile, add incremental if needed

4. **KiCad library directory structure**
   - What we know: .pretty folders contain .kicad_mod files
   - What's unclear: How to handle library search paths, environment variables
   - Recommendation: Start with explicit path, add library management later

## Sources

### Primary (HIGH confidence)
- [tower-lsp docs.rs](https://docs.rs/tower-lsp) - API reference, setup patterns
- [tower-lsp-boilerplate](https://github.com/IWANABETHATGUY/tower-lsp-boilerplate) - Complete example
- [FreeRouting GitHub](https://github.com/freerouting/freerouting) - CLI documentation
- [KiCad dev-docs](https://dev-docs.kicad.org/en/file-formats/sexpr-footprint/index.html) - Footprint format spec
- [kicad_parse_gen docs.rs](https://docs.rs/kicad_parse_gen) - Rust parser API

### Secondary (MEDIUM confidence)
- [LSP Specification 3.17](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/) - Protocol reference
- [IPC-2221 Calculator Guide](https://resources.altium.com/p/ipc-2221-calculator-pcb-trace-current-and-heating) - Formula explanation
- [Specctra DSN gist](https://gist.github.com/bert/727553) - DSN format example

### Tertiary (LOW confidence)
- FreeRouting API documentation - Beta, may change
- Various blog posts on LSP implementation - Patterns vary

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - Well-established libraries with documentation
- Architecture: HIGH - Patterns from working examples
- LSP integration: HIGH - tower-lsp-boilerplate provides reference
- FreeRouting integration: MEDIUM - CLI proven, API beta
- KiCad import: HIGH - kicad_parse_gen is mature
- IPC-2221: HIGH - Standard formulas, well-documented
- Pitfalls: MEDIUM - Based on common issues in similar projects

**Research date:** 2026-01-22
**Valid until:** 2026-03-22 (60 days - stable domain, slow-moving ecosystem)
