# Phase 3: Validation - Context

**Gathered:** 2026-01-21
**Status:** Ready for planning

<domain>
## Phase Boundary

DRC system that prevents manufacturing-invalid designs. Includes clearance checking, trace width validation, drill size validation, unconnected pin detection, and real-time feedback. Also adds QFP/SOIC/SOT footprints and custom footprint definition syntax.

This phase does NOT include: routing, autorouting, or trace editing (those are Phase 5+).

</domain>

<decisions>
## Implementation Decisions

### Error Display
- **Non-invasive** — errors are a transient state, must not block user actions
- Status bar at bottom (VS Code style) — small, always visible when errors exist
- Circle/ring markers on board at violation locations (KiCad style)
- Single severity level (errors only) — all violations are must-fix
- Click error in list → viewport zooms to fit that location
- Overlay/popup for full error list (badge shows count)

### Rule Configuration
- Manufacturer presets as base (JLCPCB, PCBWay)
- Board file can override individual rules
- Net classes for per-net overrides (standard CAD pattern: define class with rules, assign nets to class)
- Basic rules for MVP: clearance, min trace width, min drill size
- Expand rule sophistication later — keep simple for initial testing

### Feedback Timing
- DRC runs on file save (hot reload trigger)
- Non-blocking — board renders immediately, DRC markers appear when check completes
- Progress indicator while DRC runs (status bar shows "Checking...")
- Like ESLint — save triggers validation, results appear

### Custom Footprints
- Numeric pin numbering (1, 2, 3...) — standard for ICs
- Basic syntax for MVP — enough to define simple pads
- Full syntax design is Claude's discretion (match existing DSL style)
- Courtyard handling is Claude's discretion (follow IPC standards)
- Location (inline vs separate files) is Claude's discretion

### Claude's Discretion
- DSL syntax for manufacturer preset selection
- DSL syntax for footprint definitions
- Courtyard calculation (auto vs manual)
- DRC blocking behavior (render first, markers after)
- Specific error message wording
- Rule configuration syntax details

</decisions>

<specifics>
## Specific Ideas

- "Status bar like VS Code" — minimal, non-invasive error reporting
- "Like ESLint" — on-save validation pattern
- Goal is to validate super simple circuits first — confirm the system works
- Footprint library should eventually be extensive (various connectors, database integrations) but start basic
- Follow industry standards from existing PCB CAD tools (KiCad, Altium patterns)

</specifics>

<deferred>
## Deferred Ideas

- Extensive footprint library with connector database integrations — v2
- Advanced rule configuration UI — later phases
- Warning severity level (advisory violations) — keep simple with errors-only for now

</deferred>

---

*Phase: 03-validation*
*Context gathered: 2026-01-21*
