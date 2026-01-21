# Phase 2: Rendering - Context

**Gathered:** 2026-01-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Visual feedback with hot reload — render the board design so users can see their PCB layout and get instant updates when they save the source file. This includes 2D views, layer visibility, zoom/pan navigation, component/net highlighting, and grid display.

**In scope:** Rendering engine, layer visibility, navigation, hot reload, selection highlighting
**Out of scope:** Creating/editing via GUI, 3D preview, DRC visualization (Phase 3), export (Phase 4)

</domain>

<decisions>
## Implementation Decisions

### Visual styling
- KiCad-style layer colors: red=top copper, blue=bottom copper, green for inner layers
- Pads rendered as filled shapes with thin outline
- Dark/light mode toggle with **light mode as default** (user preference)
- Layer visibility controls similar to KiCad — each layer can be shown/hidden independently

### Navigation & interaction
- Zoom to cursor position (scroll wheel zooms centered on mouse)
- Pan via middle-click drag (CAD standard)
- Selection and click behavior: Claude's discretion for initial implementation

### Layer presentation
- Flip view button — mirrors display and swaps layers as if turning the board over
- Layer control panel location: Claude's discretion
- Layer blending mode: Claude's discretion
- Layer presets (top only, bottom only, all copper): Claude's discretion

### Hot reload experience
- Viewport preserved exactly on reload — same zoom and position
- Status message notification ("Reloaded") shown in corner after file update
- Selection state persists across reloads (if U1 was selected, keep it selected)
- Error handling on parse failure: Claude's discretion (too early to lock this down)

### Claude's Discretion
- Selection mechanics and info panel behavior
- Hover tooltips (whether to show, what content)
- Layer panel positioning (left, right, floating)
- Layer blending/transparency approach
- Layer view presets
- Parse error display strategy
- Grid display styling and snap behavior

</decisions>

<specifics>
## Specific Ideas

- Light mode default — user explicitly prefers this over dark mode
- KiCad-style layer colors are familiar to PCB designers
- Flip view should feel like physically turning the board over

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 02-rendering*
*Context gathered: 2026-01-21*
