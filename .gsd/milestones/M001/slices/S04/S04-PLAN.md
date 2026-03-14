# S04: Export

**Goal:** Create the cypcb-export crate foundation with coordinate conversion and aperture management.
**Demo:** Create the cypcb-export crate foundation with coordinate conversion and aperture management.

## Must-Haves


## Tasks

- [x] **T01: Export Crate Setup**
  - Create the cypcb-export crate foundation with coordinate conversion and aperture management.

Purpose: Export functionality requires precise coordinate conversion from internal nanometers to Gerber decimal format, plus aperture definitions for all pad shapes. This foundation is used by all Gerber exporters.

Output: New cypcb-export crate with coordinate conversion and aperture utilities.
- [x] **T02: Gerber Copper/Mask/Paste Export** `est:35min`
  - Implement Gerber X2 export for copper layers (top, bottom, inner).

Purpose: Copper layers are the primary manufacturing files containing all conductive features - pads, traces, and copper zones. This is the core export functionality.

Output: Functions to export copper layer Gerber files with X2 attributes, pads, and traces.
- [x] **T03: Board Outline & Silkscreen Export** `est:5min`
  - Implement board outline and silkscreen Gerber export.

Purpose: Board outline defines the physical board shape for routing/cutting. Silkscreen provides component labels and assembly markings. Both are essential manufacturing files.

Output: Functions to export board outline and silkscreen Gerber files.
- [x] **T04: Excellon Drill Export**
  - Implement Excellon drill file export for through-hole pads and vias.

Purpose: Excellon drill files tell the manufacturer where to drill holes. Essential for through-hole components, mounting holes, and vias. Misaligned drills = unusable boards.

Output: Excellon drill file export with proper header, tool definitions, and coordinates.
- [x] **T05: BOM & Pick-and-Place Export** `est:539s`
  - Implement BOM (Bill of Materials) and CPL (Component Placement List/Pick-and-Place) file export.

Purpose: BOM tells assemblers what parts to order. CPL tells pick-and-place machines where to place them. Both are essential for PCBA (PCB assembly).

Output: BOM export in CSV/JSON formats, CPL export in CSV format matching JLCPCB requirements.
- [x] **T06: CLI Export Command**
  - Implement CLI export command with manufacturer presets and organized file output.

Purpose: The CLI is the primary interface for headless export (requirement DEV-01). Users run `cypcb export project.cypcb` to generate all manufacturing files in a single command.

Output: CLI export subcommand that generates complete manufacturing file set.
- [x] **T07: Visual Verification**
  - Visual verification of export functionality using external viewers and validation.

Purpose: Manufacturing files must be validated with real tools before production. This checkpoint ensures exported files are correctly formatted and usable.

Output: Verified export functionality ready for manufacturer submission.

## Files Likely Touched

- `crates/cypcb-export/Cargo.toml`
- `crates/cypcb-export/src/lib.rs`
- `crates/cypcb-export/src/coords.rs`
- `crates/cypcb-export/src/apertures.rs`
- `Cargo.toml`
- `crates/cypcb-export/src/gerber/mod.rs`
- `crates/cypcb-export/src/gerber/header.rs`
- `crates/cypcb-export/src/gerber/copper.rs`
- `crates/cypcb-export/src/gerber/mask.rs`
- `crates/cypcb-export/src/lib.rs`
- `crates/cypcb-export/src/gerber/outline.rs`
- `crates/cypcb-export/src/gerber/silk.rs`
- `crates/cypcb-export/src/gerber/mod.rs`
- `crates/cypcb-export/src/excellon/mod.rs`
- `crates/cypcb-export/src/excellon/writer.rs`
- `crates/cypcb-export/src/excellon/tools.rs`
- `crates/cypcb-export/src/lib.rs`
- `crates/cypcb-export/src/bom/mod.rs`
- `crates/cypcb-export/src/bom/csv.rs`
- `crates/cypcb-export/src/bom/json.rs`
- `crates/cypcb-export/src/cpl/mod.rs`
- `crates/cypcb-export/src/cpl/csv.rs`
- `crates/cypcb-export/src/lib.rs`
- `crates/cypcb-export/src/job.rs`
- `crates/cypcb-export/src/presets.rs`
- `crates/cypcb-export/src/lib.rs`
- `crates/cypcb-cli/src/commands/export.rs`
- `crates/cypcb-cli/src/commands/mod.rs`
- `crates/cypcb-cli/src/main.rs`
