# Phase 04 Plan 06: CLI Export Integration Summary

**One-liner:** CLI export command with JLCPCB/PCBWay presets generating complete manufacturing file set

**Phase:** 04-export
**Plan:** 06
**Type:** execute
**Status:** Complete
**Completed:** 2026-01-28

## What Was Built

Implemented CLI export command with manufacturer presets and organized output structure, providing the primary interface for headless export (DEV-01). Users can now run `cypcb export project.cypcb` to generate all manufacturing files in a single command.

### Key Features

1. **Manufacturer Presets Module** - Configuration system for different manufacturers
   - ExportPreset struct with coordinate format and layer configuration
   - FileNaming struct for manufacturer-specific file suffixes
   - ExportLayers struct for layer export configuration
   - JLCPCB 2-layer preset with KiCad-style naming (-F_Cu.gbr, -PTH.drl)
   - PCBWay standard preset with traditional naming (_top.gtl, _drill.xln)
   - Case-insensitive preset lookup via from_name()

2. **Export Job Orchestrator** - Coordinates complete file generation
   - ExportJob struct with source path, output dir, preset, board name
   - run_export() generates all files based on preset configuration
   - Organized output structure: gerber/, drill/, assembly/
   - Exports Gerber layers (copper, mask, paste, silk, outline)
   - Exports Excellon drill files (PTH)
   - Exports assembly files (BOM CSV/JSON, CPL) when enabled
   - ExportResult with file list, warnings, duration tracking
   - ExportedFile with path, type description, and size

3. **CLI Export Command** - User-facing interface
   - clap-based argument parsing with clear options
   - Input file, output directory (-o), preset selection (-p)
   - --no-assembly flag to skip BOM/CPL generation
   - --dry-run mode to preview files without generating
   - Error handling for parse errors, sync errors, unknown presets
   - Clear progress output during export
   - Success summary with file list, sizes, and duration
   - Integrated into main CLI as export subcommand

### Technical Implementation

**Preset System:**
- Preset-driven configuration avoids hardcoding manufacturer rules
- FileNaming struct enables different file suffixes per manufacturer
- ExportLayers enables selective layer export
- Future-proof for adding new manufacturers without code changes

**Job Orchestration:**
- Single function (run_export) handles complete file generation
- Creates directory structure before export
- Uses existing export modules (gerber, excellon, bom, cpl)
- Tracks all exported files with metadata
- Duration tracking for performance monitoring

**CLI Integration:**
- Uses clap derive API for automatic help generation
- Board name extracted from input filename
- Output directory defaults to ./output
- Preset defaults to jlcpcb (most common)
- Dry-run provides clear preview of what will be generated

## Files Modified

- `crates/cypcb-export/src/presets.rs` - New: Manufacturer presets
- `crates/cypcb-export/src/job.rs` - New: Export job orchestration
- `crates/cypcb-export/src/lib.rs` - Updated: Export presets and job
- `crates/cypcb-export/src/bom/mod.rs` - Updated: Re-export functions
- `crates/cypcb-export/src/cpl/mod.rs` - Updated: Re-export functions
- `crates/cypcb-cli/Cargo.toml` - Updated: Add cypcb-export dependency
- `crates/cypcb-cli/src/commands/export.rs` - New: Export command implementation
- `crates/cypcb-cli/src/commands/mod.rs` - Updated: Add export module
- `crates/cypcb-cli/src/main.rs` - Updated: Add Export variant

## Test Coverage

**Presets Module:** 11 tests
- Preset name verification (JLCPCB, PCBWay)
- Coordinate format validation
- File naming verification
- Layer configuration checks
- Case-insensitive lookup
- Unknown preset handling

**Job Module:** 4 tests
- Job creation and configuration
- Directory structure creation
- File generation verification
- Duration tracking

**CLI Command:** 3 tests
- Command construction
- Preset lookup
- Unknown preset error handling

**Integration:** 9 CLI tests pass (parse, check commands)

**Total:** 15+ new tests, all passing

## Verification Results

All verification criteria met:

1. ✅ `cargo test -p cypcb-export` - 130 tests passing
2. ✅ `cargo test -p cypcb-cli` - 9 tests passing
3. ✅ `cypcb export examples/blink.cypcb --dry-run` - Lists 13 expected files
4. ✅ `cypcb export examples/blink.cypcb -o /tmp/test-export` - Creates 13 files in 67ms
5. ✅ Generated Gerber files have correct headers (X2 attributes, format declaration)
6. ✅ Drill file has proper header (METRIC,TZ format)

**Export output:**
- 13 files generated (9 Gerber, 1 drill, 3 assembly)
- Total size: 4.5 KB
- Duration: 67ms
- Files: Top/Bottom copper, mask, paste, silk, outline, drill, BOM CSV/JSON, CPL

## Success Criteria

- ✅ `cypcb export` command works (DEV-01 requirement)
- ✅ JLCPCB preset generates correct file set
- ✅ Output organized in gerber/drill/assembly folders
- ✅ Dry-run mode shows file list without writing
- ✅ All tests pass (18 tests total)

## Deviations from Plan

None - plan executed exactly as written.

## Architecture Insights

**Preset vs Hardcoded Configuration:**
The preset system provides excellent extensibility. Adding a new manufacturer requires only creating a new preset function, no code changes to job orchestration or CLI. The FileNaming struct avoids brittle string concatenation.

**Job Orchestration Pattern:**
Single run_export() function handles all complexity, making CLI integration simple. The function creates directories, generates all files, and returns complete metadata. This separation of concerns makes testing straightforward.

**CLI Dry-Run:**
Dry-run mode is valuable for users to preview output before generation, especially for large boards or when testing new presets. Implementation is simple - just print preset configuration and return early.

**Export Result Metadata:**
Returning ExportResult with file list and sizes enables CLI to provide detailed feedback. Duration tracking helps identify performance issues. This pattern scales well for future features like progress bars.

## Performance Notes

- Export of blink.cypcb (2 components, 2-layer): 67ms
- Includes: parsing, world building, Gerber generation, drill, BOM, CPL
- File I/O dominates duration (13 file writes)
- No optimization needed for MVP boards (<100 components)

## Next Phase Readiness

**Ready for Phase 4 completion:**
- Export infrastructure complete (04-01 through 04-06)
- All manufacturing file formats implemented
- CLI provides headless export capability (DEV-01)

**Remaining Phase 4 plans:**
- 04-07: Gerber job file (.gbrjob metadata)
- 04-08: ZIP packaging for upload
- 04-09: Export integration testing

**No blockers identified.**

## Dependencies

**Required by this plan:**
- 04-01: Export foundation (coordinates, apertures)
- 04-02: Gerber layer export (copper, mask, paste)
- 04-03: Board outline and silkscreen
- 04-04: Excellon drill file export
- 04-05: BOM and pick-and-place export

**Enables future plans:**
- 04-07: Job file can reference presets
- 04-08: ZIP can use preset file naming
- Phase 6: Desktop can use CLI export backend

## Decisions Made

| Decision | Rationale | Impact |
|----------|-----------|--------|
| Preset-based configuration | Avoids hardcoding, extensible for new manufacturers | Easy to add presets without code changes |
| Organized directory structure | Clear separation: gerber/, drill/, assembly/ | Standard industry practice, easy to navigate |
| Dry-run mode | Preview without generating | User confidence before large exports |
| Board name from filename | Sensible default, no extra config needed | Convenient for most use cases |
| Default JLCPCB preset | Most common hobbyist manufacturer | Zero configuration for typical users |
| Assembly included by default | Most users want complete files | --no-assembly available if needed |
| CLI as primary interface | Headless operation requirement (DEV-01) | Enables automation, CI/CD integration |

## Future Improvements

**Not blocking, deferred:**

1. **Additional presets** - OSH Park, ALLPCB, etc.
2. **Custom preset files** - User-defined YAML/JSON presets
3. **Zip packaging** - Single file upload (04-08)
4. **Gerber viewer** - Visual verification before export
5. **Progress bar** - For large boards with many files
6. **Parallel file generation** - Speed up multi-layer boards
7. **Export validation** - Gerber file syntax check
8. **Manufacturer DFM checks** - Warn about rule violations

## Metadata

**Duration:** 8.7 minutes (519 seconds)
**Commits:** 4 (d8b64d9, 87f98b7, b186cd8, 1c40375)
**Lines Added:** ~900 (presets, job, CLI command)
**Wave:** 3 (Integration)
**Depends On:** [04-02, 04-03, 04-04, 04-05]
