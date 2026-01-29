---
phase: 11-dark-mode-ui-polish
plan: 04
subsystem: viewer
tags: [monaco, theme, editor, typescript]

dependency_graph:
  requires: [11-01]
  provides: [monaco-theme-definitions, monaco-theme-wiring]
  affects: [14-01]

tech_stack:
  added: []
  patterns:
    - monaco-theme-data-structure
    - theme-manager-subscription
    - zero-dependency-theme-prep

key_files:
  created:
    - viewer/src/theme/monaco-theme.ts
  modified: []

decisions:
  - id: DEC-11-04-01
    title: Local MonacoThemeData interface
    choice: Define interface locally instead of importing from monaco-editor
    reason: Avoids monaco-editor dependency, allows Phase 14 to add Monaco without breaking this infrastructure

  - id: DEC-11-04-02
    title: Monaco parameter typed as any
    choice: applyMonacoTheme(monaco: any) instead of monaco.editor.IStandaloneEditor
    reason: No monaco-editor types available, Phase 14 passes actual monaco module

  - id: DEC-11-04-03
    title: PCB DSL syntax tokens
    choice: Standard language tokens (comment, keyword, string, number, type, operator)
    reason: DSL syntax highlighting can use generic tokens, specific grammar comes in Phase 14

  - id: DEC-11-04-04
    title: Editor chrome colors hardcoded
    choice: Hex values instead of reading CSS custom properties
    reason: Monaco themes require hex strings, CSS var() not supported by Monaco API

metrics:
  duration: 51s
  completed: 2026-01-29
---

# Phase 11 Plan 4: Monaco Editor Themes Summary

Monaco editor theme definitions (light/dark) with ThemeManager wiring, ready for Phase 14 integration

## What Was Built

### Monaco Theme Data Structure (monaco-theme.ts, 108 lines)

**MonacoThemeData Interface:**
- Matches `monaco.editor.IStandaloneThemeData` shape
- Defined locally (no monaco-editor import)
- Fields: base ('vs' | 'vs-dark'), inherit (boolean), rules (token array), colors (Record<string, string>)

**lightTheme Object:**
- base: 'vs', inherit: true
- Syntax tokens (PCB DSL):
  - comment: #6a9955 (green)
  - keyword: #0000ff (blue)
  - string: #a31515 (red)
  - number: #098658 (teal)
  - type: #267f99 (cyan)
  - operator: #000000 (black)
- Editor chrome (matches colors.css light theme):
  - editor.background: #ffffff
  - editor.foreground: #1a1a1a
  - editor.lineHighlightBackground: #f5f5f5
  - editorLineNumber.foreground: #999999
  - editorGutter.background: #f5f5f5
  - editor.selectionBackground: #add6ff
  - editorCursor.foreground: #000000

**darkTheme Object:**
- base: 'vs-dark', inherit: true
- Syntax tokens (dark variant):
  - comment: #6a9955 (same green, visible on dark)
  - keyword: #569cd6 (lighter blue)
  - string: #ce9178 (lighter red/orange)
  - number: #b5cea8 (lighter teal)
  - type: #4ec9b0 (lighter cyan)
  - operator: #d4d4d4 (light gray)
- Editor chrome (matches colors.css dark theme):
  - editor.background: #1e1e1e
  - editor.foreground: #e0e0e0
  - editor.lineHighlightBackground: #252525
  - editorLineNumber.foreground: #808080
  - editorGutter.background: #1e1e1e
  - editor.selectionBackground: #264f78
  - editorCursor.foreground: #ffffff

**applyMonacoTheme Function:**
- Signature: `applyMonacoTheme(monaco: any): void`
- Defines themes: `monaco.editor.defineTheme('cypcb-light', lightTheme)` and `monaco.editor.defineTheme('cypcb-dark', darkTheme)`
- Sets initial theme: `monaco.editor.setTheme(themeManager.getResolvedTheme() === 'dark' ? 'cypcb-dark' : 'cypcb-light')`
- Subscribes to theme changes: `themeManager.subscribe((resolved) => monaco.editor.setTheme(...))`
- Phase 14 will call after loading Monaco

**Exports:**
- `export const lightTheme`
- `export const darkTheme`
- `export function applyMonacoTheme`

## Key Implementation Details

**Zero Dependency Design:**
- NO `import * as monaco from 'monaco-editor'`
- NO monaco-editor package.json dependency
- MonacoThemeData interface shape matches Monaco API but defined locally
- Phase 14 adds monaco-editor, calls applyMonacoTheme(monaco)

**Theme Coordination:**
- Imports `themeManager` from `./theme-manager` (11-01)
- Reads current theme: `themeManager.getResolvedTheme()`
- Subscribes to changes: `themeManager.subscribe(callback)`
- Automatic sync when user switches light/dark/auto

**Color Palette Alignment:**
- Light theme colors match colors.css :root variables
- Dark theme colors match colors.css [data-theme="dark"] variables
- Ensures Monaco editor blends seamlessly with application UI
- No jarring color differences between editor and surrounding chrome

**PCB DSL Token Strategy:**
- Generic tokens (comment, keyword, string, number, type, operator)
- Phase 14 will define .cypcb grammar that maps DSL constructs to these tokens
- Example: 'board', 'component', 'net' keywords → token 'keyword' → #0000ff (light) or #569cd6 (dark)

## Commits

| Hash | Message |
|------|---------|
| ce376db | feat(11-04): create Monaco editor theme definitions |

## Deviations from Plan

None - plan executed exactly as written.

## Verification Results

| Check | Result |
|-------|--------|
| TypeScript compiles (monaco-theme.ts only) | Pass |
| TypeScript compiles (full viewer project) | Pass |
| Exports lightTheme, darkTheme, applyMonacoTheme | Pass (3 exports) |
| No monaco-editor imports | Pass (0 actual imports, comments don't count) |
| Theme colors align with colors.css | Pass (manual verification) |

## Phase 14 Integration Pattern

**Phase 14 will:**
```typescript
// 1. Install monaco-editor
// npm install monaco-editor

// 2. Import Monaco and theme function
import * as monaco from 'monaco-editor';
import { applyMonacoTheme } from './theme/monaco-theme';

// 3. Apply themes and wire subscription
applyMonacoTheme(monaco);

// 4. Create editor instance
const editor = monaco.editor.create(container, {
  value: code,
  language: 'cypcb', // Custom language defined in Phase 14
  theme: 'cypcb-light', // Already set by applyMonacoTheme
});

// Theme changes automatically applied via subscription
```

**What Phase 14 adds:**
- Install monaco-editor package
- Define .cypcb language grammar (registers token types)
- Create editor instance(s)
- Configure worker paths (minimize bundle size)

**What already works (this plan):**
- Theme definitions ready
- Theme switching wired to ThemeManager
- No dependency on monaco-editor

## Next Plan Readiness

Phase 14 (Monaco Editor Integration):
- Theme definitions ready for import
- applyMonacoTheme function ready to call
- ThemeManager subscription wired for automatic theme sync
- No breaking changes needed to theme infrastructure

Phase 11 completion:
- Plan 11-04 is final plan in Phase 11
- All theme infrastructure complete
- Ready for Phase 12 (Desktop Application) or Phase 14 (Monaco Integration)

## Files Changed

```
viewer/src/theme/monaco-theme.ts (created, 108 lines)
```

## Technical Notes

**MonacoThemeData Shape:**
- Matches `monaco.editor.IStandaloneThemeData` from monaco-editor types
- `base`: Inherits from 'vs' (light) or 'vs-dark' (dark) built-in themes
- `inherit`: true means our rules extend base theme (don't replace entirely)
- `rules`: Token-specific foreground/background/fontStyle overrides
- `colors`: Editor chrome color overrides (background, gutter, selection, etc.)

**Token vs Colors:**
- `rules`: Syntax highlighting (code tokens like keywords, strings)
- `colors`: Editor UI chrome (background, line numbers, cursor, selection)
- Both required for complete theme

**Hex Color Format:**
- Monaco expects hex without '#' prefix in some contexts, with '#' in others
- We use '#' prefix consistently (Monaco tolerates both)
- Example: '#ffffff' not 'ffffff'

**Theme Inheritance:**
- `inherit: true` means we don't define every possible token
- Tokens not in our rules array fall back to base theme ('vs' or 'vs-dark')
- Reduces boilerplate, makes themes easier to maintain

**Subscription Cleanup:**
- applyMonacoTheme doesn't unsubscribe from themeManager
- Intended: subscription lives for entire application lifetime
- Monaco editor theme should always track application theme
- Subscription cleaned up when page unloads

**Comment Color Consistency:**
- Both light and dark themes use #6a9955 for comments
- Green color visible on both white and dark backgrounds
- Matches VS Code default comment color
