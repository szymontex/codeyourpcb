# Knowledge Base — CodeYourPCB

Append-only register of project-specific rules, patterns, and lessons learned.

---

### K001: Toolbar button styling consistency
**Context:** Route button was originally a filled green button (`background: var(--success)`) standing out from the rest of the icon-only toolbar.
**Rule:** All toolbar buttons must use `.tb-btn` base style (transparent bg, no border). No filled/colored buttons in resting state. Color only for active states (e.g. `.routing` uses `--warning`).
**Applies to:** `viewer/index.html` toolbar CSS, any new toolbar button.

### K002: LCSC footprint fetch is async — thumbnails must handle timing
**Context:** Components with `lcsc "CXXXXXX"` attributes have NO pads in the initial snapshot. Pads only appear after async EasyEDA API fetch completes (`autoFetchLcscFootprints()`). If a thumbnail is generated before fetch, it shows an empty board.
**Rule:** Always regenerate thumbnails after LCSC fetch completes. Use `reloadAfterLcscFetch()` helper in the fetch `.then()` callback. Additionally, `onRefreshThumbnail` regenerates from current snapshot when PM opens — catches any timing edge cases.
**Applies to:** `viewer/src/main.ts` (all `autoFetchLcscFootprints` call sites), `viewer/src/project-manager.ts`.

### K003: `const`/`let` temporal dead zone — `typeof` doesn't help
**Context:** `typeof interactionState !== 'undefined'` does NOT protect against TDZ for `const`/`let` declarations. It still throws `ReferenceError`.
**Rule:** Use a separate `let interactionReady = false` flag set after initialization. Never rely on `typeof` for TDZ checking of `const`/`let` variables.
**Applies to:** `viewer/src/main.ts` `resize()` function.

### K004: Vite cache can serve stale transforms after edits
**Context:** Vite's `node_modules/.vite` cache sometimes holds stale transformed files, causing compile errors for code that no longer exists in source.
**Rule:** When encountering phantom compile errors ("await can only be used in async function" on a line with no await), delete `node_modules/.vite` and restart Vite.
**Applies to:** Any Vite dev server debugging.

### K005: WebSocket dev server vs standalone Vite
**Context:** `npx vite` runs frontend only (port 5173). `npx tsx server.ts` runs WS server (4322) + file watcher + Vite (port 4321). Features like workspace file listing require the WS server.
**Rule:** Features must work in both modes. Use graceful degradation — if WS is not connected, hide WS-dependent sections rather than showing errors. Project manager hides "Workspace" section when no WS, shows "Your projects" (localStorage-based) which works everywhere.
**Applies to:** `viewer/src/main.ts` WS callbacks, `viewer/src/project-manager.ts`.

### K006: Offscreen thumbnail render size matters
**Context:** Original 200×150 thumbnails had components too small to see (sub-pixel on small displays). Board takes most of the thumbnail area, leaving pads at 2-3 pixels.
**Rule:** Thumbnail render size is 400×300px. CSS displays at full card width with `aspect-ratio: 4/3`. This gives enough resolution for component pads to be clearly visible.
**Applies to:** `viewer/src/project-manager.ts` `generateThumbnail()`.

### K007: Port conflicts when running dev server
**Context:** `server.ts` spawns Vite as child process. If a previous Vite or server is still running, it fails with "Port XXXX already in use".
**Rule:** Kill existing processes on ports 4321, 4322, 5173 before starting dev server. Check with `netstat -tlnp | grep -E "4321|4322|5173"`.
**Applies to:** Development workflow.

### K008: WS connection timing vs showProjectManager
**Context:** `showProjectManager()` is called during `init()` before `connectWebSocket()` assigns `wsConnection`. Calling `wsConnection?.send()` in PM callbacks at init time sends nothing (wsConnection is null).
**Rule:** WS-dependent features (like file listing) should be triggered from `onConnect` callback (fires when WS actually connects), not from `showProjectManager()`. The `onConnect` callback auto-sends `list-files` request.
**Applies to:** `viewer/src/main.ts` WS + project manager integration.

### K009: Split-button pattern for Route
**Context:** Route has a split-button — main button triggers routing, caret opens dropdown with options (auto-route toggle, tuning sliders).
**Rule:** Keep the split-button group (`.tb-route-group`). Main button = action, caret = options. The caret dropdown `#route-menu-dropdown` contains auto-route checkbox + 4 tuning sliders (via cost, layer preference, roundness, density). Sliders debounce at 300ms.
**Applies to:** `viewer/index.html`, `viewer/src/main.ts`.
