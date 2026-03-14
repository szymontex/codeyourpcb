# S15: Web Verification Polish

**Goal:** Verify WASM bundling works correctly in production build and confirm load time meets requirements.
**Demo:** Verify WASM bundling works correctly in production build and confirm load time meets requirements.

## Must-Haves


## Tasks

- [x] **T01: WASM Production Verification**
  - Verify WASM bundling works correctly in production build and confirm load time meets requirements.

Purpose: Close WEB-01 gap (3s load time) and verify WEB-09 (CDN deployment readiness). Analysis shows Vite's wasm plugin correctly bundles WASM to dist/assets/ with proper import paths, so this plan focuses on verification rather than fixes.
Output: Verified production build with WASM loading, documented load time metrics.
- [x] **T02: Share URL Feature**
  - Enable the Share URL feature that was intentionally disabled pending design decision.

Purpose: Close WEB-07 gap. The viewport-only share approach is the correct design decision - sharing full board state via URL is impractical (file content too large) and unnecessary (users can share files directly). URL sharing is for collaboration on specific views of a design.
Output: Working Share button that copies viewport URL to clipboard.

## Files Likely Touched

- `viewer/src/main.ts`
- `viewer/src/main.ts`
- `viewer/index.html`
