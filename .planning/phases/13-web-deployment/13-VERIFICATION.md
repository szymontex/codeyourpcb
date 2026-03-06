---
phase: 13-web-deployment
verified: 2026-01-30T15:30:00Z
status: gaps_found
score: 6/9 must-haves verified
gaps:
  - truth: "Web application loads in less than 3 seconds on 3G connection"
    status: failed
    reason: "WASM file not accessible in production deployment - import path broken"
    artifacts:
      - path: "viewer/src/wasm.ts"
        issue: "Line 590 imports from '../pkg/cypcb_render.js' but pkg/ not deployed"
      - path: ".github/workflows/deploy.yml"
        issue: "Only deploys viewer/dist, not viewer/pkg where WASM lives"
    missing:
      - "Copy WASM files from pkg/ to dist/ during build, OR"
      - "Deploy both dist/ and pkg/ directories, OR"
      - "Refactor wasm.ts to use bundled WASM path"
  - truth: "User can share designs via URL with project state"
    status: partial
    reason: "Share feature intentionally disabled in code pending design decision"
    artifacts:
      - path: "viewer/src/main.ts"
        issue: "Lines 868-869 comment out shareBtn event listener"
      - path: "viewer/index.html"
        issue: "Share button exists but remains hidden (class='hidden')"
    missing:
      - "Design decision: share full board state vs viewport-only"
      - "Enable Share button if viewport-only is acceptable"
      - "OR implement full state sharing with file content in URL"
  - truth: "Deployment triggers automatically on push to main"
    status: uncertain
    reason: "Workflow exists but secrets may not be configured"
    artifacts:
      - path: ".github/workflows/deploy.yml"
        issue: "Requires CLOUDFLARE_API_TOKEN and CLOUDFLARE_ACCOUNT_ID secrets"
    missing:
      - "Verify GitHub repository secrets are configured"
      - "Test deployment to confirm workflow runs successfully"
---

# Phase 13: Web Deployment Verification Report

**Phase Goal:** Users can access CodeYourPCB via browser without installation, with fast loading and file access
**Verified:** 2026-01-30T15:30:00Z
**Status:** gaps_found
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Production build completes without errors | ✓ VERIFIED | vite.config.ts has WASM plugins, package.json has build:web script, TypeScript compiles |
| 2 | WASM binary is optimized for size (wasm-opt applied) | ✓ VERIFIED | build-wasm.sh runs wasm-opt -O4, Cargo.toml has opt-level="z", WASM file is 264KB |
| 3 | Web application loads in <3s on 3G connection | ✗ FAILED | WASM not accessible in deployment - import path broken |
| 4 | User can open local files via File System Access API | ✓ VERIFIED | file-access.ts exports openFile(), main.ts uses it on line 423, Ctrl+O works |
| 5 | User can save files without save-as dialog (when handle available) | ✓ VERIFIED | file-access.ts exports saveFile(), main.ts Ctrl+S handler on line 884-886 |
| 6 | User can share designs via URL with project state | ⚠️ PARTIAL | url-state.ts exists, but Share button disabled (TODO comment line 867) |
| 7 | Application is responsive on tablet and desktop viewports | ✓ VERIFIED | index.html has @media (max-width: 768px) and @media (pointer: coarse) |
| 8 | WASM files served with correct Content-Type and CSP | ✓ VERIFIED | _headers has application/wasm and wasm-unsafe-eval |
| 9 | Deployment triggers automatically on push to main | ? UNCERTAIN | deploy.yml exists but requires secrets configuration |

**Score:** 6/9 truths verified, 1 partial, 1 failed, 1 uncertain

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `viewer/vite.config.ts` | WASM plugins configured | ✓ VERIFIED | Lines 2-3: imports wasm and topLevelAwait, lines 7-8: plugins enabled |
| `viewer/build-wasm.sh` | wasm-opt post-processing | ✓ VERIFIED | Lines 37-46: wasm-opt -O4 with bulk-memory and nontrapping flags |
| `viewer/package.json` | build:web script | ✓ VERIFIED | Line 12: "build:web": "npm run build:wasm && tsc && vite build" |
| `Cargo.toml` | Release profile for size | ✓ VERIFIED | Lines 59-64: opt-level="z", lto=true, strip=true |
| `crates/cypcb-render/Cargo.toml` | wasm-pack config | ✓ VERIFIED | Lines 40-41: wasm-opt -O4 flags in metadata |
| `viewer/src/file-access.ts` | File System Access API wrapper | ✓ VERIFIED | 221 lines, exports openFile/saveFile, has TypeScript declarations |
| `viewer/src/url-state.ts` | URL state encoding/decoding | ✓ VERIFIED | 39 lines, exports encodeViewState/decodeViewState, uses URLSearchParams |
| `viewer/index.html` | Responsive CSS | ✓ VERIFIED | Lines 301-313: touch targets and narrow viewport rules |
| `.github/workflows/deploy.yml` | Cloudflare deployment | ⚠️ PARTIAL | Exists but only deploys dist/, not pkg/ |
| `viewer/public/_headers` | WASM headers and CSP | ✓ VERIFIED | Lines 1-3: WASM Content-Type, lines 5-8: CSP with wasm-unsafe-eval |
| `viewer/public/_redirects` | SPA routing | ✓ VERIFIED | Line 1: "/* /index.html 200" |

### Key Link Verification

| From | To | Via | Status | Details |
|------|-----|-----|--------|---------|
| vite.config.ts | WASM plugins | import | ✓ WIRED | Line 2-3 imports, lines 7-8 use in plugins array |
| vite.config.ts | WASM exclude | optimizeDeps | ✓ WIRED | Line 37 excludes cypcb-render from optimization |
| build-wasm.sh | wasm-opt | command | ✓ WIRED | Lines 37-46 run wasm-opt with correct flags |
| Cargo.toml | opt-level | profile.release | ✓ WIRED | Line 60 sets opt-level="z" |
| main.ts | file-access.ts | import + use | ✓ WIRED | Line 15 imports, line 423 calls openFile(), line 807 calls saveFile() |
| main.ts | url-state.ts | import + use | ⚠️ PARTIAL | Line 17 imports, line 286 calls decodeViewState(), but encodeViewState() NOT USED (Share disabled) |
| main.ts | Ctrl+S handler | keyboard event | ✓ WIRED | Lines 884-886 intercept Ctrl+S and call handleSaveFile() |
| wasm.ts | pkg/cypcb_render.js | dynamic import | ✗ BROKEN | Line 590 imports '../pkg/cypcb_render.js' but pkg/ not in dist/ |
| deploy.yml | build:web | npm script | ✓ WIRED | Line 23 runs "cd viewer && npm run build:web" |
| deploy.yml | viewer/dist | deployment path | ✗ BROKEN | Line 29 deploys viewer/dist but WASM is in viewer/pkg |

### Requirements Coverage

| Requirement | Status | Blocking Issue |
|-------------|--------|----------------|
| WEB-01: <3s load on 3G | ✗ BLOCKED | WASM file not accessible - deployment path broken |
| WEB-02: Responsive | ✓ SATISFIED | Responsive CSS verified in index.html |
| WEB-03: HTTPS | ✓ SATISFIED | Cloudflare Pages provides HTTPS |
| WEB-04: Cross-browser | ✓ SATISFIED | CSP headers configured, File System Access API has fallback |
| WEB-05: Open local files | ✓ SATISFIED | file-access.ts with File System Access API |
| WEB-06: Save local files | ✓ SATISFIED | file-access.ts with save-in-place support |
| WEB-07: Share via URL | ⚠️ PARTIAL | url-state.ts exists but Share button disabled |
| WEB-08: URL restores state | ✓ SATISFIED | decodeViewState() called on init (line 286) |
| WEB-09: Global CDN | ✓ SATISFIED | Cloudflare Pages configured |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| viewer/src/main.ts | 867 | TODO comment | 🛑 BLOCKER | Share feature disabled, WEB-07 not fully satisfied |
| viewer/src/main.ts | 868-869 | Commented-out code | 🛑 BLOCKER | Share button exists but not wired |
| viewer/src/main.ts | 164 | Unused variable | ⚠️ WARNING | TypeScript error: shareBtn declared but never used |
| viewer/src/wasm.ts | 590 | Hardcoded relative path | 🛑 BLOCKER | WASM import will fail in deployment |
| .github/workflows/deploy.yml | 29 | Missing pkg/ in deploy path | 🛑 BLOCKER | Only dist/ deployed, WASM unreachable |

### Human Verification Required

**1. Production Build End-to-End Test**

**Test:** Run `cd viewer && npm run build:web`, then `npx vite preview`, open http://localhost:4173 in Chrome
**Expected:** 
- Open button shows native file picker
- Load a .cypcb file, board renders
- Ctrl+S saves file without dialog (second save)
- Resize browser to 768px width, toolbar wraps
- Enable touch emulation, buttons enlarge to 48px

**Why human:** Visual verification of responsive layout and touch targets, actual file picker behavior varies by browser

**2. Cross-Browser Compatibility**

**Test:** Open production build in Chrome, Firefox, Safari, and Edge
**Expected:**
- Chrome/Edge/Safari: File System Access API (native picker, save-in-place)
- Firefox: Input element fallback (open works, save triggers download)
- All browsers: Board renders, WASM loads, routing works

**Why human:** Browser-specific APIs require manual testing across browsers

**3. Share URL Workflow (If Enabled)**

**Test:** Click Share button (after uncommenting code), paste URL in new tab
**Expected:**
- URL contains ?l=layers&z=zoom&x=panX&y=panY parameters
- Opening shared URL restores exact viewport and layer visibility
- Share URL under 200 characters

**Why human:** Currently disabled, pending design decision on full state vs viewport-only

**4. Deployment Verification**

**Test:** Push to main branch, verify GitHub Actions workflow runs successfully, visit deployed URL
**Expected:**
- Workflow completes without errors
- Site accessible at codeyourpcb.pages.dev (or custom domain)
- WASM loads successfully in production
- All features work same as local build

**Why human:** Requires GitHub secrets configuration and actual deployment to test

### Gaps Summary

**CRITICAL GAP: WASM Deployment Path Broken**

The WASM file is built to `viewer/pkg/cypcb_render_bg.wasm` (264KB), but the production build only deploys `viewer/dist`. The JavaScript code imports from `../pkg/cypcb_render.js`, which will result in a 404 error in production.

**Evidence:**
- `viewer/src/wasm.ts` line 590: `await import("../pkg/cypcb_render.js")`
- `.github/workflows/deploy.yml` line 29: `pages deploy viewer/dist` (only dist/, not pkg/)
- Build output: `dist/` contains HTML/JS/CSS, `pkg/` contains WASM files, they're separate directories

**Impact:** Web application will fail to load WASM in production, falling back to MockPcbEngine. Users won't get real rendering, DRC, or spatial queries. This breaks the core value proposition of the web deployment.

**Solution Options:**
1. **Copy pkg/ to dist/ during build:** Add Vite plugin or post-build script to copy pkg/* to dist/pkg/
2. **Deploy both directories:** Change deploy command to deploy viewer/ (parent of both dist/ and pkg/)
3. **Bundle WASM in dist/:** Configure Vite to copy WASM to dist/assets/ and update import path

**PARTIAL GAP: Share Feature Disabled**

The Share feature (WEB-07) is implemented but intentionally disabled in code. The TODO comment on line 867 indicates a design decision is needed: "share full board state or just viewport?"

**Evidence:**
- `viewer/src/url-state.ts` fully implemented (encodeViewState, decodeViewState)
- `viewer/src/main.ts` has handleShareView() function
- `viewer/index.html` has Share button element
- Lines 868-869 comment out shareBtn.classList.remove('hidden') and event listener

**Impact:** Users cannot share designs via URL. URL state decoding works (opening shared URLs restores viewport), but encoding (creating share URLs) is disabled.

**Solution:** Decide on share scope:
- **Viewport-only:** Uncomment lines 868-869, Share button works immediately (current implementation)
- **Full state:** Extend URL state to include file content (Base64 encoded, size concern) or use server-side storage

**UNCERTAIN: Deployment Secrets**

The GitHub Actions workflow requires `CLOUDFLARE_API_TOKEN` and `CLOUDFLARE_ACCOUNT_ID` secrets. These may not be configured in the repository.

**Evidence:**
- `.github/workflows/deploy.yml` lines 27-28 reference secrets
- No way to verify secret presence programmatically

**Impact:** Workflow will fail on push to main if secrets missing. Deployment won't happen.

**Solution:** Verify secrets are configured in GitHub repository settings, or add them.

---

_Verified: 2026-01-30T15:30:00Z_
_Verifier: Claude (gsd-verifier)_
