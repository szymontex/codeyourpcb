# Phase 4: Export - Research

**Researched:** 2026-01-28
**Domain:** PCB manufacturing file generation (Gerber X2, Excellon, BOM, Pick-and-Place)
**Confidence:** MEDIUM

## Summary

Export phase requires generating industry-standard manufacturing files that PCB fabricators and assemblers can consume. The primary formats are Gerber X2 for layer images, Excellon for drill data, and CSV for BOM/pick-and-place data.

The Rust ecosystem has gerber-types (v0.7.0) for low-level Gerber X2 code generation, but no existing Excellon library. The project already uses integer nanometers (i64 Nm) which requires conversion to decimal millimeters/inches for export. The existing CLI structure (clap-based with subcommands) provides a good foundation for adding an `export` subcommand.

Key challenges include coordinate system conversion (internal Y-up to Gerber Y-up, both bottom-left origin), precision management (nanometers to decimal places), and manufacturer-specific validation (JLCPCB/PCBWay DFM requirements).

**Primary recommendation:** Use gerber-types for Gerber X2 generation, hand-roll Excellon writer (simple text format), use csv crate for BOM/CPL, implement manufacturer presets as configuration files, add gerbv validation in tests.

## Standard Stack

The established libraries/tools for this domain:

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| gerber-types | 0.7.0 | Gerber X2 code generation | Only maintained Rust Gerber library, low-level AST approach |
| csv | 1.4.0 | CSV reading/writing | Fast, flexible, Serde integration for structured data |
| clap | 4.x | CLI argument parsing | Industry standard for Rust CLIs, derive API, subcommands |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| serde | latest | Serialization framework | Already workspace dependency, needed for CSV struct serialization |
| serde_json | latest | JSON output format | BOM export in JSON format (requirement EXP-03) |
| thiserror | latest | Error handling | Already workspace dependency, export-specific errors |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| gerber-types | Hand-rolled writer | gerber-types provides type safety and format compliance, hand-rolling risks spec violations |
| csv | rust_xlsxwriter | CSV is simpler and more universal, Excel adds complexity manufacturers don't need |
| File-based presets | Hardcoded values | Configuration files allow user customization, easier to add manufacturers |

**Installation:**
```bash
# Already in workspace
cargo add gerber-types@0.7
cargo add csv@1.4
# clap, serde, serde_json, thiserror already workspace dependencies
```

## Architecture Patterns

### Recommended Project Structure
```
crates/
├── cypcb-export/        # New crate for export functionality
│   ├── src/
│   │   ├── lib.rs       # Public API
│   │   ├── gerber/      # Gerber X2 generation
│   │   │   ├── mod.rs
│   │   │   ├── layer.rs       # Per-layer export (copper, mask, silk)
│   │   │   ├── apertures.rs   # Aperture definitions from pad shapes
│   │   │   └── attributes.rs  # X2 metadata (.FileFunction, .Part, etc)
│   │   ├── excellon/    # Excellon drill file generation
│   │   │   ├── mod.rs
│   │   │   ├── writer.rs      # Text format writer
│   │   │   └── tools.rs       # Tool definitions
│   │   ├── bom/         # Bill of Materials
│   │   │   ├── mod.rs
│   │   │   ├── csv.rs         # CSV format
│   │   │   └── json.rs        # JSON format
│   │   ├── cpl/         # Component Placement List (pick-and-place)
│   │   │   ├── mod.rs
│   │   │   └── csv.rs         # CSV format with rotation handling
│   │   ├── presets/     # Manufacturer presets
│   │   │   ├── mod.rs
│   │   │   ├── jlcpcb.rs
│   │   │   └── pcbway.rs
│   │   └── validation/  # Post-generation checks
│   │       ├── mod.rs
│   │       └── dfm.rs         # Basic DFM rules
│   └── Cargo.toml
└── cypcb-cli/
    └── src/
        └── commands/
            └── export.rs    # Export subcommand
```

### Pattern 1: Layer-Based Export
**What:** Generate one Gerber file per board layer, each self-contained with apertures and drawing commands.
**When to use:** All Gerber X2 exports (copper, mask, silkscreen, paste, outline).
**Example:**
```rust
// Conceptual structure based on gerber-types low-level API
use gerber_types::{GerberCode, ExtendedCode, CoordinateFormat, Unit};

pub struct LayerExporter {
    format: CoordinateFormat,
    apertures: HashMap<PadShape, ApertureDefinition>,
}

impl LayerExporter {
    pub fn export_copper_layer(&self, world: &World, layer: Layer) -> Result<String> {
        let mut output = String::new();

        // Header with X2 attributes
        output.push_str(&self.write_header(layer)?);

        // Aperture definitions
        for aperture in &self.apertures {
            output.push_str(&aperture.to_code());
        }

        // Drawing commands for traces, pads, zones
        output.push_str(&self.write_traces(world, layer)?);
        output.push_str(&self.write_pads(world, layer)?);
        output.push_str(&self.write_zones(world, layer)?);

        // Footer
        output.push_str("M02*\n");
        Ok(output)
    }
}
```

### Pattern 2: Coordinate Conversion Pipeline
**What:** Convert internal i64 nanometer coordinates to Gerber decimal format with proper precision.
**When to use:** All coordinate output (Gerber, Excellon, CPL).
**Example:**
```rust
pub struct CoordinateConverter {
    unit: Unit, // Millimeters or Inches
    decimal_places: u8, // Typically 6 for mm, 4 for inches
}

impl CoordinateConverter {
    pub fn nm_to_gerber(&self, nm: i64) -> String {
        match self.unit {
            Unit::Millimeters => {
                // 1mm = 1_000_000 nm
                let mm = nm as f64 / 1_000_000.0;
                format!("{:.precision$}", mm, precision = self.decimal_places as usize)
            }
            Unit::Inches => {
                // 1 inch = 25_400_000 nm
                let inches = nm as f64 / 25_400_000.0;
                format!("{:.precision$}", inches, precision = self.decimal_places as usize)
            }
        }
    }
}
```

### Pattern 3: Preset-Driven Configuration
**What:** Load manufacturer requirements from preset files that define layer stack, file naming, validation rules.
**When to use:** Different manufacturers have different requirements (JLCPCB vs PCBWay).
**Example:**
```rust
#[derive(Deserialize)]
pub struct ManufacturerPreset {
    pub name: String,
    pub units: Unit,
    pub coordinate_format: (u8, u8), // (integer_places, decimal_places)
    pub file_naming: FileNamingScheme,
    pub required_layers: Vec<LayerType>,
    pub dfm_rules: DfmRules,
}

pub fn load_preset(name: &str) -> Result<ManufacturerPreset> {
    match name {
        "jlcpcb" => Ok(JLCPCB_PRESET),
        "pcbway" => Ok(PCBWAY_PRESET),
        _ => Err(Error::UnknownPreset(name.to_string())),
    }
}
```

### Pattern 4: CSV Serialization with Serde
**What:** Define structs matching BOM/CPL columns, serialize directly to CSV.
**When to use:** BOM and pick-and-place file generation.
**Example:**
```rust
use serde::Serialize;
use csv::Writer;

#[derive(Serialize)]
struct BomEntry {
    #[serde(rename = "Designator")]
    designator: String,
    #[serde(rename = "Comment")]
    comment: String, // Value (100nF, etc)
    #[serde(rename = "Footprint")]
    footprint: String,
    #[serde(rename = "Quantity")]
    quantity: u32,
}

pub fn write_bom(entries: &[BomEntry], path: &Path) -> Result<()> {
    let mut wtr = Writer::from_path(path)?;
    for entry in entries {
        wtr.serialize(entry)?;
    }
    wtr.flush()?;
    Ok(())
}
```

### Anti-Patterns to Avoid
- **Floating-point intermediate coordinates:** Converting nm → float → decimal string loses precision. Go directly nm → decimal string with integer arithmetic.
- **Mixed units in single file:** Gerber spec requires consistent units. Set once in header, never change.
- **Missing aperture definitions:** Every D-code must have definition before use. Track defined apertures, reuse when possible.
- **Hardcoded manufacturer rules:** DFM rules differ per manufacturer. Use preset system from start.
- **Ignoring rotation conventions:** Pick-and-place rotation varies by manufacturer (counterclockwise vs clockwise from 0°). Document and test.

## Don't Hand-Roll

Problems that look simple but have existing solutions:

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| CSV file writing | Custom string formatting with commas | csv crate with Serde | Handles escaping, quotes, special chars, encoding edge cases |
| Gerber format validation | Custom parser to check output | gerbv (external tool) or gerber_parser crate | Gerber spec is complex, existing tools catch edge cases |
| Decimal number formatting | format!() with floating point | Integer arithmetic with division | Avoids floating-point rounding errors in coordinates |
| File compression (zip) | Custom compression logic | zip crate or external zip tool | Manufacturers expect specific zip formats, use proven library |

**Key insight:** Manufacturing file formats have decades of edge cases and ambiguities. Parsers/validators from the community catch issues you won't discover until manufacturer rejection. Use existing tools for validation even if you generate files yourself.

## Common Pitfalls

### Pitfall 1: Coordinate Format Mismatches
**What goes wrong:** Gerber file declares format %FSLAX26Y26*% (2 integer, 6 decimal) but coordinates use different precision, causing misaligned features or parser errors.
**Why it happens:** Converting from nanometers to decimal string without matching declared format precision.
**How to avoid:** Generate format string and coordinate converter together, enforce consistent precision throughout file. Test with multiple viewers (gerbv, online viewers).
**Warning signs:** Pads appear in wrong locations in viewer, manufacturer queries about misalignment, drill holes don't match copper.

### Pitfall 2: Missing or Empty Layer Files
**What goes wrong:** Exporter creates zero-byte files or omits critical layers (board outline, drill file), manufacturer rejects order.
**Why it happens:** Layer has no content (empty copper layer), exporter still creates file but with no drawing commands.
**How to avoid:** Always generate outline and drill files. For optional layers (silkscreen), skip file generation if no content. Validate file set completeness before zipping.
**Warning signs:** Manufacturer email asking for missing files, DFM check fails with "missing outline", zero-byte files in output directory.

### Pitfall 3: Excellon Units Ambiguity
**What goes wrong:** Drill file missing header with units (METRIC/INCH), manufacturer guesses wrong, holes drilled at wrong size.
**Why it happens:** Excellon spec makes header "optional", some generators skip it.
**How to avoid:** ALWAYS write Excellon header with M48, unit declaration (METRIC or INCH), tool definitions, and M95/%. Never omit header.
**Warning signs:** Manufacturer queries about drill sizes, PCB arrives with giant holes (interpreted inches as millimeters) or tiny holes (opposite).

### Pitfall 4: Aperture Definition Before Use
**What goes wrong:** Gerber file uses D10 before defining it with %ADD10C,...*%, parser fails or renders incorrectly.
**Why it happens:** gerber-types is low-level and doesn't enforce semantic correctness, allows syntactically valid but semantically invalid files.
**How to avoid:** Generate all aperture definitions immediately after format declaration, before any drawing commands. Track defined D-codes, reuse existing apertures when possible.
**Warning signs:** Gerber viewer errors, features render with wrong shape/size, manufacturer DFM check fails.

### Pitfall 5: BOM Designator Consolidation
**What goes wrong:** BOM lists each component separately (C1, C2, C3...) instead of consolidating identical parts (C1,C2,C3 as single row), causing unnecessarily long BOM and confusion.
**Why it happens:** Naive iteration over component entities without grouping by value+footprint.
**How to avoid:** Group components by (value, footprint, part_number), collect designators, output single row with comma-separated designators and quantity count.
**Warning signs:** BOM has hundreds of rows for simple board, manufacturer questions about duplicate parts, assembly quote is higher than expected.

### Pitfall 6: Pick-and-Place Rotation Conventions
**What goes wrong:** Components placed with 90° or 180° rotation error, requiring manual correction or causing assembly failures.
**Why it happens:** Different manufacturers/machines use different 0° reference (IPC-7351 vs proprietary), different rotation directions (counterclockwise vs clockwise).
**How to avoid:** Document rotation convention in preset, provide rotation offset parameter, test with manufacturer's preview tool before ordering, include rotation angle validation.
**Warning signs:** Manufacturer preview shows sideways components, assembly requires manual intervention, assembled board has backwards ICs.

### Pitfall 7: Mixed Layer Coordinate Origins
**What goes wrong:** Copper layers align but drill holes or silkscreen are offset, causing registration errors.
**Why it happens:** Using different origin points for different layers (board corner vs user origin vs datum point).
**How to avoid:** Use single consistent origin (board bottom-left corner) for all exports, document in Gerber job file. Project already uses bottom-left Y-up which matches Gerber standard.
**Warning signs:** Layers don't align in viewer, manufacturer queries about registration, drill holes miss pads.

## Code Examples

Verified patterns from official sources:

### Gerber X2 File Header with Attributes
```gerber
G04 Gerber X2 format - generated by cypcb*
G04 Board outline layer*
%FSLAX26Y26*%
%MOMM*%
G04 #@! TF.GenerationSoftware,CodeYourPCB,cypcb,1.0*
G04 #@! TF.CreationDate,2026-01-28T13:20:00*
G04 #@! TF.FileFunction,Profile,NP*
G04 #@! TF.Part,Single*
%ADD10C,0.100000*%
D10*
```
Source: [Gerber X2 specification](https://www.ucamco.com/en/gerber) - Standard X2 attributes for file metadata

### Excellon Drill File with Header
```excellon
M48
; DRILL file generated by cypcb
; FORMAT={2:4/ absolute / metric / suppress trailing zeros}
METRIC,TZ
T1C0.8000
T2C1.0000
T3C3.2000
%
T1
X50.800Y30.480
X55.880Y30.480
T2
X50.800Y40.640
T3
X63.500Y50.800
M30
```
Source: [Excellon format specification](https://www.artwork.com/gerber/drl2laser/excellon/index.htm) - Header is mandatory for tool definitions and units

### BOM CSV Format (JLCPCB)
```rust
#[derive(Serialize)]
struct BomEntry {
    #[serde(rename = "Designator")]
    designator: String,        // "C1,C2,C3"
    #[serde(rename = "Footprint")]
    footprint: String,         // "0805"
    #[serde(rename = "Quantity")]
    quantity: u32,             // 3
    #[serde(rename = "Comment")]
    comment: String,           // "100nF"
    #[serde(rename = "LCSC Part #")]
    lcsc_part: Option<String>, // "C1234"
}
```
Source: [JLCPCB BOM requirements](https://jlcpcb.com/help/article/bill-of-materials-for-pcb-assembly) - Required columns and format

### Pick-and-Place CSV Format
```rust
#[derive(Serialize)]
struct CplEntry {
    #[serde(rename = "Designator")]
    designator: String,    // "U1"
    #[serde(rename = "Mid X")]
    x_mm: f64,             // 50.800 (center point in mm)
    #[serde(rename = "Mid Y")]
    y_mm: f64,             // 30.480
    #[serde(rename = "Layer")]
    layer: String,         // "Top" or "Bottom"
    #[serde(rename = "Rotation")]
    rotation: f64,         // 0.0 (degrees, counterclockwise)
}
```
Source: [JLCPCB pick-and-place requirements](https://jlcpcb.com/help/article/pick-place-file-for-pcb-assembly) - Standard centroid format with rotation

### Coordinate Conversion (Nanometers to Gerber)
```rust
// Integer-based conversion avoiding floating point
pub fn nm_to_gerber_mm(nm: i64, decimal_places: u8) -> String {
    let divisor = 10_i64.pow((9 - decimal_places) as u32);
    let integer_part = nm / 1_000_000;
    let fractional_part = (nm % 1_000_000).abs();

    // Format with zero-padding for fractional part
    format!("{}.{:0width$}",
        integer_part,
        fractional_part / divisor,
        width = decimal_places as usize
    )
}
```
Source: Research finding - avoids floating-point precision loss

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Gerber RS-274D (separate aperture file) | Gerber RS-274X (embedded apertures) | 1998 | Unified format, reduced file count |
| RS-274X (no metadata) | Gerber X2 (attributes) | 2014 | Machine-readable layer identification, reduced errors |
| Separate BOM + CPL files | Gerber X3 (embedded component data) | 2020 | Unified fabrication+assembly data (not widely adopted yet) |
| Manual DFM checks | Automated online DFM tools | 2020s | Instant validation, faster turnaround |

**Deprecated/outdated:**
- **RS-274D:** Requires separate aperture file (.apt), obsolete since 1998. Don't implement.
- **Gerber X1:** No attributes, ambiguous layer identification. Use X2 minimum.
- **Inches as primary unit:** Industry moving to metric (millimeters). Support both but default to mm.
- **Tab-delimited files:** BOM/CPL should use CSV (comma-separated), not TSV (tab-separated). CSV more universal.

**Gerber X3 status:** Released 2020, embeds component data in Gerber files. Not widely supported by manufacturers yet (2026). Stick with X2 + separate BOM/CPL for maximum compatibility.

## Open Questions

Things that couldn't be fully resolved:

1. **gerber-types production readiness**
   - What we know: v0.7.0, actively maintained (Dominic Clifton as of 2025), low-level AST approach
   - What's unclear: Real-world adoption, edge case handling, performance characteristics
   - Recommendation: Test extensively with gerbv and online viewers, implement validation layer, be prepared to contribute fixes or fork if needed

2. **Excellon library gap**
   - What we know: No Rust crate exists for Excellon generation
   - What's unclear: Best approach - hand-roll writer vs adapt gerber-types patterns vs port from another language
   - Recommendation: Hand-roll simple text writer for MVP (format is simpler than Gerber), extract to separate crate if successful for community benefit

3. **Manufacturer preset completeness**
   - What we know: JLCPCB and PCBWay have documented requirements (BOM columns, file formats)
   - What's unclear: Full DFM rule sets, edge cases, changes over time, other manufacturers (OSH Park, PCBWay, etc)
   - Recommendation: Start with JLCPCB preset (most documented), add validation hooks for community contributions, version presets for updates

4. **Rotation angle conventions**
   - What we know: Counterclockwise from 0° is most common, IPC-7351 exists but not universal
   - What's unclear: Exact conventions for JLCPCB/PCBWay machines, how to test before ordering
   - Recommendation: Default to counterclockwise, add rotation offset parameter in presets, document convention in output files, recommend test board

5. **Coordinate precision requirements**
   - What we know: Gerber supports up to 6 decimal places, common formats are 2.4 or 2.6 (mm)
   - What's unclear: Practical precision needed for modern PCB manufacturing (sub-micron possible?)
   - Recommendation: Default to 2.6 format (0.001mm = 1µm precision), allow preset override, validate with manufacturer

## Sources

### Primary (HIGH confidence)
- [Gerber Format Specification revision 2024.05](https://www.ucamco.com/en/gerber) - Official Ucamco specification
- [gerber-types crate documentation v0.7.0](https://docs.rs/gerber-types/latest/gerber_types/) - Rust library API
- [gerber-types GitHub repository](https://github.com/dbrgn/gerber-types-rs) - Maintenance status, examples
- [csv crate documentation v1.4.0](https://docs.rs/csv) - CSV reading/writing API
- [JLCPCB BOM requirements](https://jlcpcb.com/help/article/bill-of-materials-for-pcb-assembly) - Required columns
- [JLCPCB Pick-and-Place requirements](https://jlcpcb.com/help/article/pick-place-file-for-pcb-assembly) - CPL format
- [Excellon Format Specification](https://www.artwork.com/gerber/drl2laser/excellon/index.htm) - Drill file format

### Secondary (MEDIUM confidence)
- [Gerber X2 vs X3 comparison](https://resources.altium.com/p/pcb-production-file-format-wars) - Feature differences (WebSearch verified with Altium)
- [Common Gerber file mistakes](https://www.acceleratedassemblies.com/blog/pcb-gerber-files-common-issues-and-ways-to-fix-them) - Industry pitfalls (multiple sources agree)
- [Gerber file naming conventions](https://www.allaboutcircuits.com/industry-articles/getting-to-know-the-gerber-file-format-and-file-names/) - Industry standards
- [Pick-and-place rotation conventions](https://www.unisoft-cim.com/rotation-understanding-component-rotations.html) - Rotation angle standards
- [gerbv viewer documentation](https://gerbv.github.io/) - Validation tool capabilities

### Tertiary (LOW confidence)
- rust_xlsxwriter for Excel output - not needed for MVP, CSV sufficient
- Fixed-point decimal crates (fpdec, rust-decimal) - may help with precision, but integer arithmetic likely sufficient
- Gerber job file (.gbrjob) specification - optional metadata file, defer to later phase

## Metadata

**Confidence breakdown:**
- Standard stack: MEDIUM - gerber-types verified but limited real-world usage data, csv and clap are HIGH confidence
- Architecture: MEDIUM - Patterns based on format specifications and existing Rust patterns, not verified in production
- Pitfalls: HIGH - Multiple manufacturer sources document same issues consistently across years

**Research date:** 2026-01-28
**Valid until:** 2026-04-28 (90 days - stable domain, Gerber spec changes slowly)

**Research gaps:**
- No hands-on testing of gerber-types library
- No direct verification of manufacturer DFM tools
- No performance benchmarks for large boards
- Limited information on Gerber X3 adoption timeline

**Validation needed:**
- Test gerber-types output with gerbv and online viewers
- Verify BOM/CPL formats with JLCPCB preview tool
- Confirm rotation conventions with test board order
- Validate Excellon output with manufacturer upload
