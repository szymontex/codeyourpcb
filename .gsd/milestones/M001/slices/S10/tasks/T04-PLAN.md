# T04: Monaco Editor Themes

**Slice:** S10 — **Milestone:** M001

## Description

Create Monaco editor theme definitions (light and dark) and export a function that Phase 14 will call to apply them. This prepares the theming infrastructure for Monaco integration without installing or instantiating Monaco.

Purpose: Satisfies UI-08 (Monaco editor theme syncs with application theme) at the infrastructure level. Phase 14 (Monaco Editor Integration) will import and use these definitions when it adds the actual editor.

Output: `monaco-theme.ts` with light/dark theme token maps and an `applyMonacoTheme` function.

## Must-Haves

- [ ] "Monaco theme definitions exist for both light and dark modes"
- [ ] "Theme definitions map semantic CSS variables to Monaco token colors"
- [ ] "An applyMonacoTheme function is exported for Phase 14 to call"

## Files

- `viewer/src/theme/monaco-theme.ts`
