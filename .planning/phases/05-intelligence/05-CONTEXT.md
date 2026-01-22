# Phase 5: Intelligence - Context

**Gathered:** 2026-01-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Developer tooling and automation for PCB design workflow. Includes:
- LSP server for IDE integration (completions, hover, diagnostics)
- Autorouter integration with FreeRouting
- KiCad footprint import
- Electrical-aware constraints and trace width calculation

Does NOT include: visual editor, manual trace drawing UI, component placement assistant.

</domain>

<decisions>
## Implementation Decisions

### LSP Experience
- Context-aware autocomplete (Claude's discretion on implementation)
- Hover info: basic by default (footprint, value, position), expanded details with Ctrl/Shift modifier
- DRC errors appear as squiggles in editor, updated on file save (not real-time)
- Go-to-definition: Claude's discretion on scope

### Autorouter Workflow
- Triggered on file save (same as DRC) — seamless hot-reload workflow
- Routes stored in separate file (e.g., `.routes`) — keeps source clean, routing is regenerable
- Users can lock traces to prevent re-routing (DSL syntax for locking)
- Manual traces definable in DSL with waypoints
- Router iterates to find solutions; if physically impossible, show partial results + explain blockers
- Progress indicator required (logs, status bar, or modal) — must show work is happening; live preview deferred
- User can cancel routing at any time
- "Satisfaction score" based on routing quality metrics — needs research into how FreeRouting handles this
- Routing strategies: research should investigate per-route vs global, cost optimization (minimize vias), signal-type-aware routing
- Net constraints in DSL are mandatory — router must respect them
- Ratsnest: toggle option in layer controls
- Full trace rendering: actual width, copper layer colors, vias visible

### KiCad Footprint Import
- Import mechanism: Claude's discretion (CLI command, DSL reference, or library directory)
- Storage format: Claude's discretion (convert to DSL vs binary reference)
- Goal is full KiCad compatibility — if it exports to Gerber, we support it
- Comprehensive bundled library — ship with hundreds of common footprints (resistors, caps, ICs, connectors)

### Electrical Constraints
- Fully definable per net: width, clearance, current, impedance
- Current-based calculation: IPC-2221 calculator for trace width from current/copper weight/temp rise
- Intelligent defaults: tool should calculate constraints automatically from datasheets, simulations, or AI inference when not specified
- Beginner-friendly: user can omit constraints, system figures it out; experts can override
- Impedance control: research what's needed for high-speed signals; include if required for proper design
- Electrical issue warnings: Claude's discretion on how to surface (DRC-style vs separate panel)

### Claude's Discretion
- LSP autocomplete implementation details
- LSP go-to-definition scope
- KiCad import mechanism and storage format
- Routing result storage format (exact file structure)
- Electrical warning display approach

</decisions>

<specifics>
## Specific Ideas

- "I want to save the file and see it routed in the viewer automatically" — same workflow as current DRC
- "The tool should be smart enough that a beginner with a chatbot can design a working board" — AI-friendly, intelligent defaults
- "If senior engineer knows the values, they can specify them; if not, system calculates"
- Router should consider manufacturing cost (vias cost money, layer count matters)
- Signal-type-aware routing (power vs signal vs high-speed have different requirements)
- Lock traces workflow: autoroute once, lock good traces, let router figure out the rest

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 05-intelligence*
*Context gathered: 2026-01-22*
