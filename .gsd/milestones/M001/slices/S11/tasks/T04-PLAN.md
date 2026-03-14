# T04: Installer & File Association

**Slice:** S11 — **Milestone:** M001

## Description

Configure installers, file associations, auto-updater, and verify performance targets.

Purpose: Completes DESK-06 (installers), DESK-07 (updates), DESK-08 (bundle size), DESK-09 (memory), DESK-10 (startup time).
Output: Build configuration producing platform installers with .cypcb file association, updater plugin, and optimized release profile.

## Must-Haves

- [ ] "Application bundle targets configured for all platforms"
- [ ] "File association for .cypcb registered in bundle config"
- [ ] "Update checker plugin configured"
- [ ] "Release profile produces small binary (<10MB target)"
- [ ] "Application starts in under 1 second"

## Files

- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json`
- `src-tauri/src/lib.rs`
