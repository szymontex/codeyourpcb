# T01: Theme System Foundation

**Slice:** S10 — **Milestone:** M001

## Description

Create the theme system foundation: CSS custom properties for both themes, ThemeManager singleton for coordinating theme changes, and FART prevention inline script in HTML.

Purpose: Establishes the theming infrastructure that all subsequent UI work builds upon. Without this, theme-aware components have nothing to hook into.

Output: Theme types, ThemeManager class, CSS custom properties file, FART-preventing index.html.

## Must-Haves

- [ ] "Page loads with correct theme (no flash of wrong theme)"
- [ ] "OS dark/light preference is detected and applied automatically"
- [ ] "Theme preference persists in localStorage across page reloads"
- [ ] "CSS custom properties define all semantic colors for both themes"

## Files

- `viewer/src/theme/theme-manager.ts`
- `viewer/src/theme/theme-types.ts`
- `viewer/src/theme/colors.css`
- `viewer/index.html`
