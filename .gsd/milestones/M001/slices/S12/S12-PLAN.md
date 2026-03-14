# S12: Web Deployment

**Goal:** Optimize the production build pipeline for web deployment: configure Vite with WASM plugins, optimize WASM binary size, and set Cargo release profile for minimal WASM output.
**Demo:** Optimize the production build pipeline for web deployment: configure Vite with WASM plugins, optimize WASM binary size, and set Cargo release profile for minimal WASM output.

## Must-Haves


## Tasks

- [x] **T01: Production Build Pipeline** `est:6m 54s`
  - Optimize the production build pipeline for web deployment: configure Vite with WASM plugins, optimize WASM binary size, and set Cargo release profile for minimal WASM output.

Purpose: WEB-01 requires <3s load on 3G. WASM optimization is the critical path — unoptimized builds are 2-5x larger.
Output: Production build pipeline producing optimized assets ready for CDN deployment.
- [x] **T02: File System Access API** `est:2min`
  - Implement File System Access API for opening and saving local files, with fallback for Firefox and older browsers.

Purpose: WEB-05 and WEB-06 require users to open/save local files without server involvement. The existing file-picker.ts uses basic input element — this adds native file handle support for save-in-place.
Output: file-access.ts module with open/save functions, integrated into main.ts.
- [x] **T03: URL State Sharing & Layout**
  - Implement URL-based design sharing and responsive layout for tablet/desktop usage.

Purpose: WEB-07/WEB-08 enable collaboration via URL sharing. WEB-02 ensures usability across devices.
Output: url-state.ts module for share links, responsive CSS for touch devices.
- [x] **T04: Cloudflare Pages Deployment**
  - Set up Cloudflare Pages deployment with GitHub Actions, proper WASM headers, and CSP configuration.

Purpose: WEB-03 (HTTPS), WEB-09 (global CDN). Production deployment with correct headers for WASM execution.
Output: GitHub Actions workflow + Cloudflare Pages config files.

## Files Likely Touched

- `viewer/vite.config.ts`
- `viewer/build-wasm.sh`
- `viewer/package.json`
- `crates/cypcb-render/Cargo.toml`
- `viewer/src/file-access.ts`
- `viewer/src/file-picker.ts`
- `viewer/src/main.ts`
- `viewer/src/url-state.ts`
- `viewer/src/main.ts`
- `viewer/index.html`
- `.github/workflows/deploy.yml`
- `viewer/public/_headers`
- `viewer/public/_redirects`
