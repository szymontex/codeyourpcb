# Phase 12: Tauri Desktop Foundation - Context

**Gathered:** 2026-01-29
**Status:** Ready for planning

<domain>
## Phase Boundary

Native desktop application that wraps the existing web viewer with OS-level integration. Provides file dialogs, native menus, installers, and keyboard shortcuts. Desktop is a superset of web capabilities - validates platform abstraction layer works correctly.

</domain>

<decisions>
## Implementation Decisions

### Window behavior
- **First launch:** Maximized by default
- **Multi-window support:** Claude's discretion (choose based on PCB workflow patterns)
- **Unsaved changes handling:** Claude's discretion (follow desktop conventions)
- **Window state persistence:** Claude's discretion (desktop UX best practices)

### Native menus
- **Menu structure:** Claude's discretion (design based on available features)
- **Keyboard shortcuts display:** Claude's discretion (follow platform conventions)
- **Unavailable actions:** Claude's discretion (desktop UX conventions - likely disable/gray out)
- **Context menus:** Claude's discretion (add where they make sense)

### File operations
- **File picker behavior:** Claude's discretion (desktop file dialog conventions)
- **Recent files list:** Claude's discretion (include if improves workflow)
- **File association:** Yes - register .cypcb extension, double-click opens in CodeYourPCB
- **Autosave/backup:** Claude's discretion (desktop app conventions)

### Installer experience
- **Install location:** Claude's discretion (follow platform conventions - varies by OS)
- **Shortcuts creation:** Claude's discretion (OS-specific installer conventions)
- **Update mechanism:** Claude's discretion (Tauri capabilities and best practices)
- **Uninstall behavior:** Claude's discretion (uninstaller best practices)

### Claude's Discretion
Claude has significant flexibility across all areas except:
- **Must:** Start maximized on first launch
- **Must:** Register .cypcb file association at install time

All other decisions should follow desktop application best practices, platform conventions, and Tauri ecosystem patterns.

</decisions>

<specifics>
## Specific Ideas

- User expects professional desktop PCB design tool experience
- File association is important - double-clicking .cypcb files should "just work"
- Trust Claude to follow platform-specific conventions (Windows vs macOS vs Linux differences are expected)

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 12-tauri-desktop-foundation*
*Context gathered: 2026-01-29*
