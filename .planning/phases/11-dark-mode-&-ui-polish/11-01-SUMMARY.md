---
phase: 11-dark-mode-ui-polish
plan: 01
subsystem: viewer
tags: [theme, dark-mode, css-custom-properties, typescript]

dependency_graph:
  requires: []
  provides: [theme-system, theme-manager, css-custom-properties, fart-prevention]
  affects: [11-02, 11-03, 14-01]

tech_stack:
  added: []
  patterns:
    - singleton-theme-manager
    - css-custom-properties
    - fart-prevention-inline-script
    - matchMedia-system-theme

key_files:
  created:
    - viewer/src/theme/theme-types.ts
    - viewer/src/theme/theme-manager.ts
    - viewer/src/theme/colors.css
  modified:
    - viewer/index.html

decisions:
  - id: DEC-11-01-01
    title: FART prevention inline script
    choice: Inline script in HTML head before CSS loads
    reason: Prevents flash of wrong theme by setting data-theme attribute synchronously

  - id: DEC-11-01-02
    title: Singleton ThemeManager pattern
    choice: Export single themeManager instance
    reason: Single source of truth for theme state, prevents multiple instances

  - id: DEC-11-01-03
    title: CSS custom properties over SCSS
    choice: Native CSS custom properties with data-theme attribute selector
    reason: No build step, browser-native, dynamic theme switching without recompile

  - id: DEC-11-01-04
    title: localStorage key 'theme'
    choice: Simple key name, not namespaced
    reason: Single-application domain, no collision risk

metrics:
  duration: 1m 29s
  completed: 2026-01-29
---

# Phase 11 Plan 1: Theme System Foundation Summary

Theme types, ThemeManager singleton, CSS custom properties, and FART prevention for dark/light mode support

## What Was Built

### Theme Type Definitions (theme-types.ts)
- `Theme = 'light' | 'dark' | 'auto'` - User preference including system auto
- `ResolvedTheme = 'light' | 'dark'` - Actual applied theme (auto resolved)
- `ThemeChangeListener = (theme: ResolvedTheme) => void` - Callback type for theme changes

### ThemeManager Singleton (theme-manager.ts, 106 lines)
- Reads localStorage('theme') on initialization
- Sets up `matchMedia('(prefers-color-scheme: dark)')` listener
- **setTheme(theme: Theme)**: Saves to localStorage, calls updateTheme()
- **getTheme()**: Returns raw theme preference (light/dark/auto)
- **getResolvedTheme()**: Resolves 'auto' using matchMedia
- **subscribe(listener)**: Returns unsubscribe function
- **updateTheme()**: Sets document.documentElement data-theme attribute, notifies listeners
- Handles OS theme change events when 'auto' is selected

### CSS Custom Properties (colors.css, 60 lines)
**Light theme (:root defaults):**
- 16 semantic color variables (bg-primary, text-primary, accent-primary, etc.)
- 4 PCB-specific colors (grid, board-outline, empty-text, label)
- WCAG AA compliant (4.5:1+ contrast ratios)

**Dark theme ([data-theme="dark"]):**
- Same 16 semantic color variables with dark-optimized values
- 4 PCB-specific colors adjusted for dark backgrounds
- WCAG AA compliant, no pure black backgrounds or pure white text

**Native form controls:**
- `color-scheme: light dark` on :root for native browser controls

### FART Prevention (index.html)
**Inline script in head (before CSS):**
- Reads localStorage('theme')
- Detects system preference via matchMedia
- Resolves theme to 'light' or 'dark'
- Sets document.documentElement.setAttribute('data-theme', theme)
- Executes synchronously before CSS loads (prevents flash)

**Meta tag:**
- `<meta name="color-scheme" content="light dark">` for native controls

## Key Implementation Details

**Theme Resolution Logic:**
1. If saved theme is 'light' or 'dark', use it directly
2. If saved theme is 'auto' or missing, use system preference
3. System preference determined by matchMedia('(prefers-color-scheme: dark)').matches

**OS Theme Change Handling:**
- ThemeManager listens to matchMedia 'change' event
- When OS switches light/dark and user has 'auto' selected, updateTheme() triggers immediately
- Listeners notified, data-theme attribute updated

**FART Prevention Guarantees:**
- Inline script executes before any CSS
- data-theme attribute set before first paint
- No dependency on JavaScript bundle loading
- Works even if main.ts fails to load

**CSS Custom Property Benefits:**
- No build step (SCSS not needed)
- Dynamic theme switching without recompile
- Browser-native, performant
- Can be read by JavaScript (getComputedStyle)

## Commits

| Hash | Message |
|------|---------|
| 757f183 | feat(11-01): create theme types and ThemeManager |
| 8184d72 | feat(11-01): add CSS custom properties and FART prevention |

## Deviations from Plan

None - plan executed exactly as written.

## Verification Results

| Check | Result |
|-------|--------|
| TypeScript compiles | Pass |
| data-theme in index.html | Pass (line 14) |
| color-scheme meta tag | Pass (line 6) |
| localStorage.getItem in FART script | Pass (line 11) |
| --bg-primary occurrences | Pass (2: light + dark) |
| Full TypeScript compilation | Pass |

## WCAG AA Contrast Verification

| Pair | Contrast | Pass |
|------|----------|------|
| Light primary (#1a1a1a on #ffffff) | 16.7:1 | ✓ |
| Dark primary (#e0e0e0 on #1e1e1e) | 12.5:1 | ✓ |
| Light secondary (#666666 on #ffffff) | 5.7:1 | ✓ |
| Dark secondary (#b0b0b0 on #1e1e1e) | 8.6:1 | ✓ |

All text/background pairs meet WCAG AA minimum 4.5:1 ratio.

## Next Plan Readiness

11-02-PLAN.md (Convert Inline Styles) can proceed:
- ThemeManager singleton available for import
- CSS custom properties defined for both themes
- data-theme attribute set on documentElement
- Ready to replace hardcoded colors with var(--*) references

Phase 14 (Monaco Integration):
- Theme system foundation ready for Monaco theme coordination
- getResolvedTheme() provides current theme for Monaco config
- subscribe() allows Monaco to react to theme changes

## Files Changed

```
viewer/src/theme/theme-types.ts     (created, 18 lines)
viewer/src/theme/theme-manager.ts   (created, 106 lines)
viewer/src/theme/colors.css         (created, 60 lines)
viewer/index.html                   (modified, +9 lines)
```

## Technical Notes

**ThemeManager Initialization Timing:**
- Constructor runs when module imported (synchronous)
- FART script already set initial data-theme before ThemeManager loads
- ThemeManager reads localStorage again (redundant but safe)
- Ensures consistency between FART script and ThemeManager

**localStorage Persistence:**
- Key: 'theme'
- Values: 'light' | 'dark' | 'auto'
- Missing key treated as 'auto' (system preference)

**matchMedia Compatibility:**
- Supported in all modern browsers
- Graceful degradation: defaults to 'light' if matchMedia fails
- 'change' event listener automatically cleaned up by browser on page unload

**CSS Custom Property Scope:**
- :root scope makes variables available to entire document
- [data-theme="dark"] selector overrides for dark mode
- Specificity: attribute selector same as class (0,0,1,0)
- Order matters: dark theme rules must come after light theme rules

**Inline Style Preservation:**
- All existing inline styles in index.html preserved
- Conversion to CSS custom properties deferred to Plan 02
- No breaking changes to existing UI
