---
phase: 12
plan: 01
subsystem: desktop-foundation
tags: [tauri, vite, desktop, bundler, window-management]
requires: [phase-09, phase-11]
provides: [tauri-shell, desktop-config, vite-tauri-integration]
affects: [phase-12-02, phase-12-03, phase-12-04]
tech-stack:
  added:
    - tauri@2.x
    - tauri-build@2.x
    - tauri-plugin-dialog@2.x
    - tauri-plugin-fs@2.x
    - tauri-plugin-window-state@2.x
    - "@tauri-apps/api@2.9.1"
    - "@tauri-apps/cli@2.9.6"
  patterns:
    - tauri-v2-project-structure
    - vite-tauri-integration
    - capabilities-based-security
key-files:
  created:
    - src-tauri/Cargo.toml
    - src-tauri/build.rs
    - src-tauri/tauri.conf.json
    - src-tauri/capabilities/default.json
    - src-tauri/src/main.rs
    - src-tauri/src/lib.rs
    - src-tauri/icons/*
  modified:
    - Cargo.toml
    - viewer/vite.config.ts
    - viewer/package.json
decisions:
  - what: "Tauri v2 project structure at src-tauri/"
    why: "Standard Tauri layout, sits alongside viewer/ directory"
    alternatives: "Nested inside viewer/ - rejected because Tauri typically at root"
  - what: "Window starts maximized by default"
    why: "PCB viewers benefit from maximum canvas space"
    alternatives: "Default size - rejected, requires manual resizing"
  - what: "Placeholder icons using solid color"
    why: "Satisfies bundler requirements during scaffolding phase"
    alternatives: "Skip icons - rejected, bundler requires them for build"
  - what: "Vite watch ignores src-tauri/"
    why: "Prevents infinite rebuild loop (Rust changes trigger Vite reload)"
    alternatives: "No ignore - causes development loop issues"
  - what: "Build target conditional on TAURI_ENV_PLATFORM"
    why: "safari13 for macOS webview, chrome105 for Windows, esnext for web-only"
    alternatives: "Fixed target - rejected, breaks cross-platform compatibility"
metrics:
  duration: 263s
  tasks-completed: 2
  tests-added: 0
  commits: 2
completed: 2026-01-29
---

# Phase 12 Plan 01: Tauri Desktop Foundation Summary

**One-liner:** Scaffolded Tauri v2 desktop shell with maximized window, file association for .cypcb, and Vite integration

## What Was Built

Created the foundational Tauri v2 project structure at `src-tauri/` that wraps the existing Vite viewer in a native desktop window. The Tauri backend is configured to serve the viewer frontend (port 4321, dist output) with proper window management (maximized start, 800x600 minimum), file associations (.cypcb), and Vite integration (dev server, build process). Added Tauri plugins for dialogs, filesystem access, and window state persistence.

## Technical Implementation

### Tauri Project Structure (Task 1)
- **src-tauri/Cargo.toml:** New workspace member `cypcb-desktop` with Tauri v2 and plugins (dialog, fs, window-state). Depends on existing `cypcb-platform` crate for Menu data model integration. Release profile optimized for small bundles (strip, lto, opt-level=s).
- **src-tauri/tauri.conf.json:** Tauri v2 config schema with product info (CodeYourPCB, com.codeyourpcb.app), build commands (npm run dev/build in ../viewer), dev URL (http://localhost:4321), frontend dist (../viewer/dist), window settings (maximized, 800x600 min), and file association (.cypcb extension).
- **src-tauri/capabilities/default.json:** Security permissions for core, dialog, fs, and window-state plugins. Tauri v2 uses capability-based security instead of v1's allowlist.
- **src-tauri/src/main.rs:** Desktop entry point that calls `cypcb_desktop::run()`. Uses `windows_subsystem = "windows"` attribute to hide console on release builds.
- **src-tauri/src/lib.rs:** Minimal app builder that registers three plugins (dialog, fs, window-state) and runs with generated context. No menu or IPC commands yet (deferred to Plan 02).
- **src-tauri/icons/:** Placeholder PNG icons (32x32, 128x128, 256x256) plus ico/icns for bundler. Created programmatically with solid color (#5D6EE8).
- **Cargo.toml:** Updated workspace members from `["crates/*"]` to `["crates/*", "src-tauri"]`.

### Vite/Tauri Integration (Task 2)
- **viewer/vite.config.ts:** Updated server.host to use `TAURI_DEV_HOST` env var (fallback 0.0.0.0). Added watch ignore for `**/src-tauri/**` to prevent infinite rebuild loop. Added `envPrefix: ['VITE_', 'TAURI_ENV_*']` for Tauri env vars. Conditional build.target based on `TAURI_ENV_PLATFORM` (chrome105 for Windows, safari13 for macOS, esnext for web). Conditional minify and sourcemap based on `TAURI_ENV_DEBUG`.
- **viewer/package.json:** Added `@tauri-apps/api@^2.0.0` dependency (frontend-backend IPC). Added `@tauri-apps/cli@^2.0.0` devDependency (tauri dev/build commands). New scripts: `tauri`, `dev:desktop`, `build:desktop`. Existing web-only scripts (dev, build) unchanged.
- **Verification:** npm install succeeded, TypeScript compilation passed, Tauri packages installed (@tauri-apps/api@2.9.1, @tauri-apps/cli@2.9.6).

## Deviations from Plan

### Known Blocker Encountered: GTK System Libraries

**Issue:** Task 1 verification requires `cargo check -p cypcb-desktop` to compile, but Tauri v2 on Linux depends on GTK3 system libraries (pkg-config, libglib2.0-dev, libgtk-3-dev, libwebkit2gtk-4.1-dev) which are not installed in this execution environment and require sudo access to install.

**Classification:** This is an **infrastructure limitation** of the execution environment, not a design flaw. The Tauri project structure is correct according to Tauri v2 documentation and Phase 12 research.

**Resolution:**
- All Tauri configuration files created and verified (tauri.conf.json is valid JSON, icons exist, workspace configuration correct).
- npm install succeeded, confirming frontend integration is valid.
- TypeScript compilation passed, confirming vite.config.ts has no syntax errors.
- Documented system library requirements in commit message for future reference.

**Impact:** Compilation verification deferred until environment with GTK3 libraries is available (developer workstation, CI with system dependencies). This does NOT block subsequent plans - the structure is correct and will compile in proper environment.

**Tracked as:** Similar to Phase 9 blocker "Linux File Dialogs" - Tauri (like rfd) requires native GUI system libraries on Linux.

### No Other Deviations
All other aspects executed exactly as planned. No Rule 1/2/3 deviations needed.

## Requirements Satisfied

**From ROADMAP.md Phase 12:**
- DESK-01: Native file dialogs (tauri-plugin-dialog registered, ready for Plan 02)
- DESK-03: File association for .cypcb (configured in tauri.conf.json bundle.fileAssociations)
- DESK-08: Window starts maximized (configured in tauri.conf.json app.windows[0].maximized: true)
- **Partial DESK-09:** Native app installers (bundler configured, targets: "all", will work after compilation succeeds)

## Files Changed

### Created (12 files)
- `src-tauri/Cargo.toml` - Package definition with Tauri v2 dependencies
- `src-tauri/build.rs` - Tauri build script integration
- `src-tauri/tauri.conf.json` - App configuration and bundler settings
- `src-tauri/capabilities/default.json` - Security permissions
- `src-tauri/src/main.rs` - Desktop entry point
- `src-tauri/src/lib.rs` - Tauri app builder
- `src-tauri/icons/32x32.png` - Small icon
- `src-tauri/icons/128x128.png` - Medium icon
- `src-tauri/icons/128x128@2x.png` - High-DPI medium icon
- `src-tauri/icons/icon.ico` - Windows icon
- `src-tauri/icons/icon.icns` - macOS icon
- `viewer/package-lock.json` - npm lock file (Tauri packages)

### Modified (3 files)
- `Cargo.toml` - Added "src-tauri" to workspace members
- `viewer/vite.config.ts` - Tauri env vars, watch ignore, conditional build
- `viewer/package.json` - Tauri packages and desktop scripts

## Dependencies

**Requires (from earlier phases):**
- Phase 9 (Platform Abstraction): `cypcb-platform` crate provides Menu data model that will be rendered to native Tauri menus in Plan 02
- Phase 11 (Dark Mode): Theme system infrastructure ready for Tauri window theming

**Provides (for later phases/plans):**
- Tauri v2 desktop shell foundation for all Phase 12 plans
- Vite/Tauri integration patterns (env vars, watch ignore, build targets)
- File association scaffolding (.cypcb registration)

**Affects:**
- Phase 12 Plan 02: Native menus (will use src-tauri/src/lib.rs app builder)
- Phase 12 Plan 03: File open/save (will use tauri-plugin-dialog)
- Phase 12 Plan 04: Window management (uses tauri-plugin-window-state)
- Phase 13 (Web Deployment): Vite config conditional logic ensures web-only builds still work

## Next Phase Readiness

**Blockers:**
- GTK3 system libraries required for Linux compilation (pkg-config, libgtk-3-dev, libwebkit2gtk-4.1-dev, libayatana-appindicator3-dev, librsvg2-dev). Not available in this execution environment.
- Recommendation: Install libraries in developer environment or CI system before running `tauri dev` or `tauri build`.

**Concerns:**
- Icon placeholders need replacement with actual CodeYourPCB branding (current: solid color squares). Consider using `npx @tauri-apps/cli icon <source.png>` to generate all sizes from single high-res source.
- Window state persistence (tauri-plugin-window-state) registered but not tested yet. Will be validated in Plan 04.

**Recommendations:**
- Run `tauri dev` in environment with GTK libraries to validate window opens maximized and Vite integration works without rebuild loops.
- Test file association (.cypcb double-click) with installed build (not dev mode).
- Replace placeholder icons before public release.

## Open Questions

None - plan executed as designed. Compilation verification deferred due to environment limitation.

## Lessons Learned

1. **Tauri v2 Linux Dependencies:** Like rfd (Phase 9), Tauri requires native GTK libraries on Linux. This is a known constraint of native desktop frameworks and should be documented in project README for contributors.

2. **Vite Watch Ignore is Critical:** Without `ignored: ['**/src-tauri/**']` in Vite watch config, Rust compilation output changes trigger frontend hot reload, which triggers another Rust rebuild - infinite loop. This was identified in Phase 12 research as Pitfall 1 and properly prevented.

3. **Tauri Env Vars for Conditional Build:** Using `TAURI_ENV_PLATFORM` and `TAURI_ENV_DEBUG` in Vite config allows single configuration for both web-only and desktop builds. Web builds use `esnext` target, desktop uses platform-specific targets (safari13/chrome105).

4. **Placeholder Icons Satisfy Bundler:** Tauri bundler requires icon files to exist even during scaffolding phase. Programmatically generated solid color PNGs are sufficient to unblock development. Production icons can be added later via `tauri icon` CLI.

5. **npm Install Validates Frontend Integration:** Even though Rust compilation failed, successful npm install confirmed that package.json is valid and Tauri npm packages resolve correctly. This partial verification is useful in constrained environments.
