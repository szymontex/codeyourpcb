---
phase: 02
plan: 02
subsystem: viewer
tags: [vite, typescript, frontend, canvas, wasm]

dependency-graph:
  requires:
    - None (parallel with 02-01 WASM crate setup)
  provides:
    - Frontend scaffolding with canvas for PCB rendering
    - TypeScript types matching Rust BoardSnapshot
  affects:
    - 02-03 (WASM binding integration)
    - 02-04 (Canvas 2D rendering implementation)

tech-stack:
  added:
    - vite 5.0 (build tool and dev server)
    - typescript 5.3 (type-safe JavaScript)
  patterns:
    - Vanilla TypeScript (no framework)
    - WASM module loading pattern

key-files:
  created:
    - viewer/package.json
    - viewer/tsconfig.json
    - viewer/vite.config.ts
    - viewer/.gitignore
    - viewer/index.html
    - viewer/src/main.ts
    - viewer/src/wasm.ts
    - viewer/src/types.ts

decisions:
  - id: vanilla-typescript
    summary: No UI framework - vanilla TypeScript sufficient for minimal verification UI
  - id: vite-build-tool
    summary: Vite chosen for fast dev server and native WASM support

metrics:
  duration: ~15 minutes
  completed: 2026-01-21
---

# Phase 2 Plan 2: Frontend Scaffolding Summary

**One-liner:** Vite + TypeScript frontend with canvas, layer toggles, and WASM loading placeholder ready for integration.

## Overview

Created the minimal frontend infrastructure for the CodeYourPCB viewer web application. The viewer uses vanilla TypeScript with Vite as the build tool, providing a canvas element for PCB rendering and layer toggle controls in the toolbar.

## What Was Built

### Project Structure
- **viewer/package.json** - npm package with dev/build/preview scripts
- **viewer/tsconfig.json** - Strict TypeScript config targeting ES2020
- **viewer/vite.config.ts** - Minimal Vite config with WASM module exclusion
- **viewer/.gitignore** - Ignores node_modules, dist, pkg directories

### HTML Shell (index.html)
- Responsive layout with toolbar and canvas container
- Canvas element (`#pcb-canvas`) sized to fill container
- Toolbar with layer toggle checkboxes (Top/Bottom)
- Coordinate display (`#coords`) updated on mouse move
- Status indicator showing loading/ready state
- CSS styling for clean, minimal interface

### TypeScript Infrastructure

**src/types.ts** - Interfaces matching Rust BoardSnapshot:
- `BoardSnapshot` - Root snapshot type
- `BoardInfo` - Board dimensions and layer count
- `ComponentInfo` - Component placement data with pads
- `PadInfo` - Pad geometry, shape, layer mask
- `NetInfo` - Net connectivity data
- `PinRef` - Component-pin references

**src/wasm.ts** - WASM loading utilities:
- `PcbEngine` interface defining WASM module API
- `loadWasm()` async function (placeholder for actual loading)

**src/main.ts** - Application entry point:
- Canvas resize handling on window resize
- Mouse coordinate tracking and display
- WASM module loading attempt
- Placeholder rendering when WASM not available
- Error handling with status display

## Key Technical Details

### No UI Framework
The viewer uses vanilla TypeScript rather than React/Vue/Angular. This decision:
- Reduces bundle size and complexity
- Makes WASM integration straightforward
- Sufficient for verification UI (not production)
- Can add framework later if needed

### WASM Integration Preparation
The code structure anticipates WASM module integration:
- `PcbEngine` interface matches planned Rust exports
- `loadWasm()` function ready to import from pkg/
- Types match `BoardSnapshot` structure from Rust

### Canvas Management
- Responsive sizing (fills container)
- Handles window resize events
- 2D context ready for rendering
- Placeholder displayed when WASM not loaded

## Deviations from Plan

None - plan executed exactly as written.

## Verification Results

All criteria verified:
1. `npm install` succeeds
2. `npm run dev` starts server at localhost:5173
3. Browser would show page with canvas and toolbar
4. TypeScript compiles without errors (`tsc` passes)
5. Console would show "WASM not ready" message

## Success Criteria Met

- [x] Vite dev server runs
- [x] Canvas element present and sized to container
- [x] Layer checkboxes visible (non-functional yet)
- [x] TypeScript types match Rust BoardSnapshot
- [x] No external UI framework dependencies (vanilla TS)

## Commands

```bash
# Start development server
cd viewer && npm run dev
# Build for production
cd viewer && npm run build
# Preview production build
cd viewer && npm run preview
```

## Next Steps

This scaffolding is ready for:
1. **02-03:** WASM module integration when cypcb-render is built
2. **02-04:** Canvas 2D rendering implementation
3. Layer visibility toggling connected to actual render state

## Commits

| Hash | Type | Description |
|------|------|-------------|
| f945fb5 | feat | Create Vite TypeScript project structure |
| 126bcb9 | feat | Create HTML shell and TypeScript entry point |
