# T02: CSS Variable Migration

**Slice:** S10 — **Milestone:** M001

## Description

Migrate all hardcoded colors in the viewer UI to use CSS custom properties and wire the ThemeManager into the application so that switching themes updates all surfaces (HTML UI, canvas, grid, labels).

Purpose: This is where theming becomes visible. Plan 01 created the infrastructure; this plan makes every pixel respond to it.

Output: Fully themed viewer UI — toolbar, status bar, error panel, canvas background, grid, and component labels all update when theme changes.

## Must-Haves

- [ ] "Toolbar, status bar, error panel all use theme colors"
- [ ] "Canvas background and grid colors respond to theme"
- [ ] "All hardcoded colors in inline styles replaced with CSS variables"
- [ ] "Switching themes updates every visible UI surface"
- [ ] "Error panel (the only dialog-like component in the current viewer) is fully themed (UI-05 partial: native menus are Phase 12/Tauri scope, HTML menus are Phase 13/web scope)"

## Files

- `viewer/index.html`
- `viewer/src/main.ts`
- `viewer/src/layers.ts`
- `viewer/src/renderer.ts`
