# S10: Dark Mode & Ui Polish

**Goal:** Create the theme system foundation: CSS custom properties for both themes, ThemeManager singleton for coordinating theme changes, and FART prevention inline script in HTML.
**Demo:** Create the theme system foundation: CSS custom properties for both themes, ThemeManager singleton for coordinating theme changes, and FART prevention inline script in HTML.

## Must-Haves


## Tasks

- [x] **T01: Theme System Foundation**
  - Create the theme system foundation: CSS custom properties for both themes, ThemeManager singleton for coordinating theme changes, and FART prevention inline script in HTML.

Purpose: Establishes the theming infrastructure that all subsequent UI work builds upon. Without this, theme-aware components have nothing to hook into.

Output: Theme types, ThemeManager class, CSS custom properties file, FART-preventing index.html.
- [x] **T02: CSS Variable Migration**
  - Migrate all hardcoded colors in the viewer UI to use CSS custom properties and wire the ThemeManager into the application so that switching themes updates all surfaces (HTML UI, canvas, grid, labels).

Purpose: This is where theming becomes visible. Plan 01 created the infrastructure; this plan makes every pixel respond to it.

Output: Fully themed viewer UI — toolbar, status bar, error panel, canvas background, grid, and component labels all update when theme changes.
- [x] **T03: Theme Toggle & WCAG Verification**
  - Add user-facing theme toggle control to the toolbar and wire it to ThemeManager. The toggle cycles through light → dark → auto modes.

Purpose: Users need a way to manually control their theme preference. This satisfies UI-04 (manual toggle) and connects UI-03 (OS preference via auto mode).

Output: Theme toggle button in toolbar, cycling light/dark/auto, persisted via ThemeManager.
- [x] **T04: Monaco Editor Themes**
  - Create Monaco editor theme definitions (light and dark) and export a function that Phase 14 will call to apply them. This prepares the theming infrastructure for Monaco integration without installing or instantiating Monaco.

Purpose: Satisfies UI-08 (Monaco editor theme syncs with application theme) at the infrastructure level. Phase 14 (Monaco Editor Integration) will import and use these definitions when it adds the actual editor.

Output: `monaco-theme.ts` with light/dark theme token maps and an `applyMonacoTheme` function.

## Files Likely Touched

- `viewer/src/theme/theme-manager.ts`
- `viewer/src/theme/theme-types.ts`
- `viewer/src/theme/colors.css`
- `viewer/index.html`
- `viewer/index.html`
- `viewer/src/main.ts`
- `viewer/src/layers.ts`
- `viewer/src/renderer.ts`
- `viewer/index.html`
- `viewer/src/main.ts`
- `viewer/src/theme/monaco-theme.ts`
