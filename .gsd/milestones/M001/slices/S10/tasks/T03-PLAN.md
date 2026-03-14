# T03: Theme Toggle & WCAG Verification

**Slice:** S10 — **Milestone:** M001

## Description

Add user-facing theme toggle control to the toolbar and wire it to ThemeManager. The toggle cycles through light → dark → auto modes.

Purpose: Users need a way to manually control their theme preference. This satisfies UI-04 (manual toggle) and connects UI-03 (OS preference via auto mode).

Output: Theme toggle button in toolbar, cycling light/dark/auto, persisted via ThemeManager.

## Must-Haves

- [ ] "User can toggle between light, dark, and auto modes via a UI control"
- [ ] "Auto mode follows OS preference and updates live"
- [ ] "Toggle state persists across page reloads"
- [ ] "Theme toggle is accessible (keyboard navigable, labeled)"

## Files

- `viewer/index.html`
- `viewer/src/main.ts`
