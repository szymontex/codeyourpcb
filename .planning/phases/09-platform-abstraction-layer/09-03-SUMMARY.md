---
phase: 09-platform-abstraction-layer
plan: 03
subsystem: platform-abstraction
tags: [menu, platform-facade, cross-platform, serde, rust]
requires:
  - phase: 09-01
    provides: FileSystem trait and implementations
  - phase: 09-02
    provides: Dialog and Storage abstractions
provides:
  - Menu data model for declarative menu construction
  - Platform facade providing unified access to all platform services
  - Complete platform abstraction layer (PLAT-01 through PLAT-05)
affects: [10, 12, 13, 14]
tech-stack:
  added:
    - serde (for menu serialization)
  patterns:
    - Declarative data models over trait abstractions when platforms differ fundamentally
    - Facade pattern for single entry point to platform services
    - Build-time conditional compilation hides platform differences from application code
key-files:
  created:
    - crates/cypcb-platform/src/menu.rs
    - crates/cypcb-platform/src/platform.rs
  modified:
    - crates/cypcb-platform/src/lib.rs
    - crates/cypcb-platform/Cargo.toml
key-decisions:
  - "Menu as data model (not trait) - Tauri native menus and HTML menus are fundamentally different rendering paradigms"
  - "Platform facade aggregates all services - application code never imports platform-specific types"
  - "Platform accessor methods return concrete types behind cfg - simpler than trait objects or generics"
patterns-established:
  - "Menu data model can be serialized and rendered by either platform"
  - "Platform struct is the single import for all platform services"
  - "Conditional compilation at Platform struct level keeps platform differences contained"
duration: 4min
completed: 2026-01-29
---

# Phase 09 Plan 03: Menu & Platform Facade Summary

**Menu data model with builder pattern and Platform facade providing single entry point to all platform services (FileSystem, Dialog, Storage, Menu)**

## Performance

- **Duration:** 4 minutes
- **Started:** 2026-01-29T09:36:02Z
- **Completed:** 2026-01-29T09:39:36Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments

- Menu data model (MenuBar, Menu, MenuItem) supports declarative menu construction with shortcuts, separators, and submenus
- Platform facade aggregates all platform services with conditional compilation for native vs web
- All 5 PLAT requirements complete: conditional compilation (PLAT-01), FileSystem (PLAT-02), Dialog (PLAT-03), Menu (PLAT-04), Storage (PLAT-05)
- Crate compiles cleanly for both native and wasm32-unknown-unknown targets
- Full workspace compilation verified - no breaking changes

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement Menu types for declarative menu construction** - `199b0b3` (feat)
   - Created menu.rs with MenuBar, Menu, MenuItem types
   - Serializable data model with builder pattern
   - Added serde dependency

2. **Task 2: Create Platform facade and verify dual-target compilation** - `402dcf2` (feat)
   - Created platform.rs with Platform struct
   - Platform::new_native() and Platform::new_web() constructors
   - Accessor methods: fs(), dialog(), storage()
   - Verified dual-target compilation succeeds

## Files Created/Modified

**Created:**
- `crates/cypcb-platform/src/menu.rs` - Declarative menu data model (MenuBar, Menu, MenuItem)
- `crates/cypcb-platform/src/platform.rs` - Platform facade aggregating all services

**Modified:**
- `crates/cypcb-platform/src/lib.rs` - Added menu and platform modules, exported all public types
- `crates/cypcb-platform/Cargo.toml` - Added serde workspace dependency

## Decisions Made

**Menu as data model (not trait):**
Research (09-RESEARCH.md Pitfall 5) warned against premature Menu trait abstraction. Tauri uses native OS menus with compile-time static APIs, while web uses HTML with runtime DOM manipulation. These are fundamentally different paradigms. Solution: Declarative data model that both platforms can read and render natively. Rendering deferred to Phase 12 (Desktop) and Phase 13 (Web).

**Platform accessor methods return concrete types:**
Could have used trait objects (`&dyn FileSystem`) or generics (`impl FileSystem`), but returning concrete types behind `#[cfg(native)]`/`#[cfg(wasm)]` is simpler and gives better type inference. Application code gets the exact type for their platform, and the trait methods are still available.

**Platform facade as single entry point:**
Application code should never write `use cypcb_platform::NativeFileSystem` or conditionally import platform-specific types. Instead, `use cypcb_platform::Platform` once, call `platform.fs()`, and get the right implementation. This prevents platform checks from scattering through business logic (the 800% duplication risk identified in research).

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None - straightforward implementation following established patterns from 09-01 and 09-02.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

**Platform abstraction layer complete:**
- All 5 PLAT requirements implemented and verified
- Dual-target compilation verified (native + wasm32-unknown-unknown)
- Application code can now import Platform and use all services without platform checks

**Ready for dependent phases:**
- Phase 10 (Library Management): Can use Storage for caching parsed libraries
- Phase 12 (Desktop): Can render Menu to Tauri native menus, use Platform for all services
- Phase 13 (Web): Can render Menu to HTML, use Platform for all services
- Phase 14 (Monaco Editor): Can use Storage for editor preferences

**Blockers:** None

**Concerns:** None - Phase 9 complete with all requirements satisfied

---
*Phase: 09-platform-abstraction-layer*
*Completed: 2026-01-29*
