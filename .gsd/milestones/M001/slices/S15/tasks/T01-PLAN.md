# T01: WASM Production Verification

**Slice:** S15 — **Milestone:** M001

## Description

Verify WASM bundling works correctly in production build and confirm load time meets requirements.

Purpose: Close WEB-01 gap (3s load time) and verify WEB-09 (CDN deployment readiness). Analysis shows Vite's wasm plugin correctly bundles WASM to dist/assets/ with proper import paths, so this plan focuses on verification rather than fixes.
Output: Verified production build with WASM loading, documented load time metrics.

## Must-Haves

- [ ] "WASM loads successfully in production build (not MockPcbEngine fallback)"
- [ ] "Board rendering works correctly with real WASM engine"
- [ ] "Web application loads in less than 3 seconds on simulated 3G connection"

## Files

- `viewer/src/main.ts`
