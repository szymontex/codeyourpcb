# T02: Share URL Feature

**Slice:** S15 — **Milestone:** M001

## Description

Enable the Share URL feature that was intentionally disabled pending design decision.

Purpose: Close WEB-07 gap. The viewport-only share approach is the correct design decision - sharing full board state via URL is impractical (file content too large) and unnecessary (users can share files directly). URL sharing is for collaboration on specific views of a design.
Output: Working Share button that copies viewport URL to clipboard.

## Must-Haves

- [ ] "User can click Share button to copy view state URL to clipboard"
- [ ] "Share URL includes layer visibility, zoom, and pan position"
- [ ] "Keyboard shortcut Ctrl+Shift+S triggers share action on web"

## Files

- `viewer/src/main.ts`
- `viewer/index.html`
