# Domain Pitfalls: Code-First PCB Design Tool

**Domain:** Code-first PCB/EDA design tool
**Researched:** 2026-01-21
**Confidence:** MEDIUM (mix of authoritative sources and community experience)

---

## Critical Pitfalls

Mistakes that cause rewrites, fundamental breakage, or project failure.

### Pitfall 1: DSL Syntax Lock-in

**What goes wrong:** Early DSL design decisions become permanent because users write code against them. Syntax mistakes discovered later cannot be fixed without breaking all existing designs.

**Why it happens:**
- Rushing to "something that works" without considering evolution
- Not understanding the PCB domain deeply enough before designing syntax
- Copying syntax from other DSLs without considering PCB-specific needs
- "More ways to do it" seems friendly but creates maintenance burden

**Consequences:**
- Permanent technical debt in the language itself
- Users must learn multiple syntax variants
- Documentation becomes cluttered with deprecated patterns
- LLMs trained on old syntax produce incompatible code

**Prevention:**
1. **Version the DSL from day one** - Include `version: 1` in every file
2. **Start minimal** - Fewer keywords are easier to evolve. "One way to do it + escape hatch" (expose raw values when DSL doesn't cover use case)
3. **Dogfood extensively** - Design real boards with the DSL before freezing syntax
4. **Reserve keywords** - Reserve likely-needed keywords even if not implemented
5. **Study prior art deeply** - KDL, S-expressions (KiCad), JITX, SKiDL syntax choices and their tradeoffs

**Detection (warning signs):**
- Developers asking "can we also support X syntax for the same thing?"
- Documentation showing multiple ways to express identical intent
- Users confused about which variant to use
- Discussions about "the right way" to express something

**Phase mapping:** Phase 1 (Foundation) - get grammar design right before any code depends on it

**Sources:**
- [Martin Fowler DSL Q&A](https://martinfowler.com/bliki/DslQandA.html) - "Having more than one way to do something is not a virtue, it's a curse"
- [DSL Evolution InfoQ](https://www.infoq.com/articles/dsl-evolution/) - Versioning strategies

---

### Pitfall 2: Floating-Point Geometry Errors

**What goes wrong:** Using floating-point numbers (f32/f64) for PCB coordinates leads to cumulative precision errors. Two traces that should connect don't. DRC reports false violations or misses real ones. Gerber output has micro-gaps.

**Why it happens:**
- Floating-point is the default in most languages
- Errors are small initially, only manifest at scale
- Different operations accumulate errors differently
- Operations far from origin have worse precision

**Consequences:**
- Non-deterministic builds (same source file produces slightly different outputs)
- Manufacturing defects from micro-gaps in Gerber output
- DRC inconsistencies (pass on one run, fail on another)
- Debug nightmare - issues appear/disappear based on operation order

**Prevention:**
1. **Use integer nanometers internally** - KiCad uses 32-bit signed integers with 1nm resolution (supports boards up to ~2.14m)
2. **Convert at boundaries only** - Parse mm/mils from user input to internal integers, convert back only for display/export
3. **Avoid coordinate system mismatches** - KiCad's pixel-style (Y-down) vs Gerber's Cartesian (Y-up) causes mirrored placements if not handled correctly
4. **Test with extreme coordinates** - Generate test cases at board corners, with many sequential operations
5. **Snap to grid after operations** - Prevents drift accumulation

**Detection:**
- Unit tests comparing geometry operations show non-zero deltas
- Same design produces different Gerber checksums on different runs
- DRC results are non-deterministic
- Visual inspection shows micro-gaps at high zoom

**Phase mapping:** Phase 1 (Foundation) - core data model must use integers from the start

**Sources:**
- [KiCad Coordinate System](https://forum.kicad.info/t/coordinate-system-grid-and-origins-in-the-pcb-editor/24535) - "Internal measurement resolution is 1 nanometer, stored as 32-bit integers"
- [Mitigating Floating Point Errors in Computational Geometry](https://medium.com/@moiserushanika2006/mitigating-floating-point-errors-in-computational-geometry-algorithms-a62525da45ef)

---

### Pitfall 3: Gerber Generation Edge Cases

**What goes wrong:** Gerber export works for simple cases but fails silently or produces unmanufacturable output for complex designs. Manufacturing house rejects files or produces wrong boards.

**Why it happens:**
- Testing only with simple boards
- Not understanding Gerber format nuances (RS-274X vs X2, aperture handling)
- Drill file coordinate system mismatches
- Large copper pours generating vector fills instead of contours

**Consequences:**
- Manufacturing delays (files rejected, require manual fixes)
- Wrong boards produced (expensive prototype wasted)
- User trust destroyed (tool "works" until it matters)
- Support burden from debugging manufacturing issues

**Prevention:**
1. **Use Gerber X2 (not 274D or even plain 274X)** - Modern standard with metadata
2. **Test against Gerber viewers** - gerbv, KiCad Gerber viewer, manufacturer's viewer
3. **Verify drill/Gerber alignment** - Units and zero-suppression must match between Excellon drill and Gerber files
4. **Generate flash pads, not vector pads** - Vector pads slow manufacturing, may cause errors
5. **Always include board outline** - "Most common mistake" per multiple manufacturers
6. **Use contour fills, not vector fills** - Large copper areas with 1-2mil vectors become too large for plotters
7. **Test with multiple manufacturers' DFM tools** - JLCPCB, PCBWay, OSH Park all have free checks

**Detection:**
- Gerber viewer shows visual anomalies (gaps, jagged edges)
- File sizes suspiciously large (vector fill issue)
- Manufacturer DFM tool reports errors
- Drill holes visually offset from pads in viewer

**Phase mapping:** Phase 2 (Core Features) - Gerber export must be battle-tested before release

**Sources:**
- [Common Gerber File Issues - Bittele](https://www.7pcb.com/blog/common-gerber-issues-how-to-fix-them)
- [Common Problems with Gerber Files - Sierra Circuits](https://www.protoexpress.com/blog/common-problems-associated-with-gerber-files/)
- [Gerber Files - Bay Area Circuits](https://bayareacircuits.com/common-problems-with-gerber-files-and-how-to-avoid-them/)

---

### Pitfall 4: Autorouter Non-Determinism

**What goes wrong:** Running the autorouter twice on the same design produces different results. Version control becomes meaningless. "Works on my machine" for routing.

**Why it happens:**
- Random seeds not controlled
- Floating-point accumulation in cost functions
- Order-dependent data structures (hash maps with non-deterministic iteration)
- External autorouter (FreeRouting) may have own non-determinism

**Consequences:**
- Git history becomes noise (every commit changes routes even if design unchanged)
- LLM-assisted editing becomes unreliable (can't predict routing outcome)
- Debugging impossible (can't reproduce the specific routing that failed DRC)
- Violates core value proposition ("same file = same output")

**Prevention:**
1. **Seed all randomness explicitly** - Accept seed as parameter, default to hash of design
2. **Use deterministic data structures** - BTreeMap not HashMap, sorted iteration
3. **Integer arithmetic for cost functions** - Avoid floating-point in optimization loops
4. **Cache and version routing results** - Store solved routes with content hash of inputs
5. **Document FreeRouting determinism** - If using external router, understand its guarantees (may need patches)
6. **Include routing in test suite** - Same inputs must produce byte-identical outputs

**Detection:**
- Running router twice produces different `.session` files
- Git shows routing changes when only components changed
- Users report "it routed fine yesterday but fails today"
- Route quality varies between runs

**Phase mapping:** Phase 3 (Intelligence) - autorouter integration must address this upfront

**Sources:**
- [Why PCB Autorouting Remains Broken](https://autocuro.com/blog/why-pcb-autorouting-remains-broken)
- [tscircuit autorouting repo](https://github.com/tscircuit/autorouting) - "CBS algorithm's predictability and determinism"

---

### Pitfall 5: DRC Performance Cliff

**What goes wrong:** DRC works fine for 50-component boards but takes minutes or hangs completely for 500-component boards. Real-time DRC becomes unusable, users disable it, ship broken designs.

**Why it happens:**
- Naive O(n^2) algorithms (check every object pair)
- No spatial indexing
- Checking entire board on every edit
- Copper pour recalculation on every change

**Consequences:**
- Users disable DRC, ship boards with violations
- IDE/editor becomes sluggish
- Large designs become impractical
- Competitive disadvantage vs tools with fast DRC

**Prevention:**
1. **Spatial indexing from day one** - R*-tree (rstar crate) for all geometry queries
2. **Incremental DRC** - Only check objects affected by the edit
3. **Zone-based checking** - Divide board into zones, parallelize
4. **Tiered DRC** - Fast subset for real-time, full check on save/export
5. **Profile with realistic boards** - 500+ components, dense routing, large copper pours
6. **GPU acceleration for geometry** - Modern DRC research uses GPU for intersection tests

**Detection:**
- DRC time grows non-linearly with component count
- Profiler shows geometry functions dominating
- Users complaining about lag after adding copper pours
- Memory usage spikes during DRC

**Phase mapping:** Phase 2 (Core Features) - basic DRC must scale; Phase 5 (Advanced) - GPU acceleration

**Sources:**
- [PDRC: GPU-Accelerated DRC](http://www.cse.cuhk.edu.hk/~byu/papers/C219-DAC2024-PDRC.pdf) - Bentley-Ottmann variants, R-tree optimization
- [EasyEDA DRC](https://docs.easyeda.com/en/PCB/Design-Rule-Check/) - Real-time DRC approach

---

### Pitfall 6: File Format Breaking Changes Without Migration

**What goes wrong:** File format changes break existing designs. Users can't open their old projects. Or worse, projects open but with subtle corruption.

**Why it happens:**
- "We'll add migration later" (you won't)
- Not versioning files from the start
- Changing semantics without changing syntax
- Incomplete migration (handles 80% of cases, corrupts 20%)

**Consequences:**
- Users lose work (or think they did)
- Trust destroyed instantly
- Support nightmare
- Fork pressure (users stick to old version)

**Prevention:**
1. **Version in every file** - `version: 1` on line 1, mandatory
2. **Never remove, only deprecate** - Old syntax continues to parse
3. **Write migration with every breaking change** - Automated upgrade path
4. **Round-trip tests** - Parse old format, save new format, verify no data loss
5. **Keep old version parsers** - Can always read old files
6. **Warn on save, not on load** - "Saving will upgrade to v2 format. Continue?"

**Detection:**
- Old test fixtures start failing mysteriously
- Users report "file won't open" after updating
- Diff shows unexpected changes in unchanged sections
- Migration test suite incomplete or missing

**Phase mapping:** Phase 1 (Foundation) - versioning must be in grammar from start

**Sources:**
- [KiCad File Compatibility](https://forum.kicad.info/t/backward-and-forward-compatibility/45234) - "Major releases almost always come with changes to file formats"
- [Go Backward Compatibility](https://go.dev/blog/compat) - Language evolution lessons

---

## v1.1 Integration Pitfalls

Critical mistakes when adding library management, Tauri desktop, web deployment, and Monaco editor to the existing v1.0 system.

### Pitfall 7: Library Namespace Conflicts Without Resolution Strategy

**What goes wrong:** Multiple library sources (KiCad, JLCPCB, custom) contain components with identical names but different implementations. User imports "R_0805" from three different sources, gets silent overwrites or unpredictable behavior. Designs reference the wrong footprint, leading to manufacturing failures caught only at prototype stage.

**Why it happens:** Each library ecosystem (KiCad, JLCPCB, SnapEDA) uses its own naming conventions. Developers assume "last write wins" or "merge on import" without considering that footprints with identical names may have different pad layouts, silkscreen, or 3D models. The "Footprint Release God" pattern from professional teams (one person approves all additions) doesn't translate to multi-source consumption.

**How to avoid:**
- Implement namespace-prefixed imports: `kicad::R_0805`, `jlcpcb::R_0805`, `custom::R_0805`
- Store library source metadata with each imported footprint
- Detect conflicts at import time and require explicit user resolution
- Create a conflict resolution UI showing side-by-side comparison of duplicate footprints
- Version control the library index with source metadata

**Warning signs:**
- Import logs show "replaced existing footprint" messages without user confirmation
- Users report "board looks different after re-importing libraries"
- Manufacturing errors where pad sizes don't match expected values
- No library source indicated in component metadata

**Phase to address:** v1.1 Phase 1 (Library Management Foundation) must implement namespace system. Phase 2 (Library Integration) must build conflict resolution UI.

**Sources:**
- [Managing PCB Footprint Libraries with Altium 365](https://resources.altium.com/p/managing-pcb-footprint-libraries-with-altium-365)
- [Centralized Component Libraries Best Practices](https://resources.altium.com/p/centralized-component-libraries-best-practices-hardware-teams)

---

### Pitfall 8: Desktop/Web Feature Parity Drift Without Abstraction Layer

**What goes wrong:** Desktop Tauri build gets native file system access, while web version can't read/write files. Developers implement file operations using Tauri's `fs` plugin directly in business logic. Web deployment breaks completely. Codebase diverges into two separate implementations with different bugs, making maintenance hell.

**Why it happens:** Tauri is "not meant for web platform" (GitHub #11347). `window.__TAURI_INTERNALS__` is undefined in web contexts, causing runtime errors. Developers take the path of least resistance: `if (window.__TAURI__) { /* desktop code */ } else { /* web fallback */ }` scattered throughout codebase. Research shows 800% increase in React code duplication since 2023.

**How to avoid:**
- Create platform abstraction layer at project start: `FileSystem`, `Dialog`, `Menu` interfaces
- Desktop implementation uses Tauri APIs (`@tauri-apps/plugin-fs`)
- Web implementation uses File System Access API (with fallback to input/download)
- Use Vite's `TAURI_ENV_PLATFORM` for build-time conditional compilation, not runtime checks
- Tree-shake unused platform code via Vite's dead code elimination
- Write integration tests for both platforms in CI

**Warning signs:**
- Direct imports of `@tauri-apps/api` in business logic (not abstraction layer)
- Runtime platform detection (`if (window.__TAURI__)`) instead of build-time
- Features working in desktop but "not implemented" stubs in web
- Complaints that "web version is always behind desktop"
- Bundle size includes both Tauri and web APIs

**Phase to address:** v1.1 Phase 3 (Tauri Desktop Foundation) must establish abstraction layer BEFORE implementing any Tauri-specific features. Phase 4 (Web Deployment) validates abstraction works.

**Sources:**
- [Tauri IPC Issue #11347](https://github.com/tauri-apps/tauri/issues/11347)
- [Code Duplication: Best Tools 2026](https://www.codeant.ai/blogs/stop-code-duplication-developers-guide)
- [Tauri Environment Variables](https://v2.tauri.app/reference/environment-variables/)

---

### Pitfall 9: Monaco Editor Bundle Size Explosion (4MB+ Initial Load)

**What goes wrong:** Developer imports Monaco Editor with default configuration. Initial bundle size jumps from ~500KB to 4.5MB. Web deployment becomes unusable on slow connections. Even with gzip, first meaningful paint takes 10+ seconds on 3G. Users abandon the app before the editor loads.

**Why it happens:** Monaco Editor includes support for 40+ languages by default, each with its own parser, syntax highlighter, and web worker. The `monaco-editor-webpack-plugin` was archived in November 2023, leaving developers without clear optimization path. Developers miss that "workers feature includes language web workers, and if not set you will have to provide them manually or accept a heavy performance penalty."

**How to avoid:**
- Use `vite-plugin-monaco-editor` with explicit language worker configuration
- For .cypcb files, only include: `['editorWorkerService']` (no language-specific workers needed)
- Implement custom syntax highlighting via Tree-sitter (already in codebase for LSP)
- Lazy load Monaco: `const Monaco = lazy(() => import('./MonacoEditor'))`
- Code split Monaco bundle separately: Vite automatically chunks at dynamic import boundaries
- Compress workers with Brotli (50% size reduction according to WASM optimization docs)
- Set performance budget in CI: fail build if Monaco chunk exceeds 500KB compressed
- Consider CDN loader for Monaco workers instead of bundling

**Warning signs:**
- Initial JavaScript bundle exceeds 2MB uncompressed
- Monaco included in main bundle instead of async chunk
- All language workers loading (CSS, HTML, JSON, TypeScript) when only custom language needed
- No dynamic imports for Monaco components
- Build time increases by 30+ seconds due to Monaco processing
- Network tab shows 40+ worker files loading on editor mount

**Phase to address:** v1.1 Phase 5 (Monaco Editor Integration) must implement lazy loading and worker optimization from day one. Phase 6 (Performance) validates bundle budgets.

**Sources:**
- [Monaco Editor Bundle Size Issue #3518](https://github.com/microsoft/monaco-editor/issues/3518)
- [vite-plugin-monaco-editor](https://github.com/vdesjs/vite-plugin-monaco-editor)
- [Understanding Monaco's web worker architecture](https://app.studyraid.com/en/read/15534/540352/understanding-monacos-web-worker-architecture)

---

### Pitfall 10: Dark Mode Theme Inconsistency Across Surfaces

**What goes wrong:** Developer implements dark mode for application UI using CSS custom properties. Monaco Editor, embedded PDFs, 3D previews, and external library components remain light-themed. Result is jarring "flashbang" effect when switching tabs. User-generated content (imported footprints with hardcoded colors) doesn't respect theme. Application feels unpolished and "stitched together."

**Why it happens:** Each subsystem has its own theming API: Monaco uses `setTheme()`, Canvas/wgpu needs shader recompilation, Three.js scene background, CSS custom properties. Developer implements theme toggle that only updates CSS vars. JavaScript executes after CSS, causing "flash of incorrect theme" (FOIT). Third-party embedded media (footprint previews) have varying backgrounds that clash with dark mode.

**How to avoid:**
- Create central theme manager that coordinates all subsystems:
  ```typescript
  ThemeManager.setTheme('dark', {
    updateCSS: () => document.documentElement.dataset.theme = 'dark',
    updateMonaco: () => monaco.editor.setTheme('vs-dark'),
    updateCanvas: () => renderer.setBackgroundColor(0x1e1e1e),
    updateThreeJS: () => scene.background = new THREE.Color(0x1e1e1e)
  })
  ```
- Save theme preference to localStorage before applying to prevent FOIT
- Inject theme CSS in `<head>` before body content loads
- Define color palette as design tokens shared across all systems
- Test all UI states in both themes (hover, active, disabled, error)
- For user-generated content, apply CSS filters or overlays to blend with theme
- Validate theme consistency with automated screenshot tests

**Warning signs:**
- Theme toggle only updates CSS variables, not Monaco/Canvas/Three.js
- White flash visible when page loads in dark mode
- User preferences not persisting across sessions
- Some UI components use hardcoded colors instead of theme tokens
- Third-party components (library preview cards) have light backgrounds in dark mode
- Accessibility contrast ratios fail in one theme but not the other

**Phase to address:** v1.1 Phase 2 (Dark Mode & UI Polish) must establish theme architecture. All subsequent phases must validate theme consistency for new features.

**Sources:**
- [Dark Mode: Users Think About It and Issues to Avoid](https://www.nngroup.com/articles/dark-mode-users-issues/)
- [A Complete Guide to Dark Mode on the Web](https://css-tricks.com/a-complete-guide-to-dark-mode-on-the-web/)

---

### Pitfall 11: File System API Mismatches Breaking Project Persistence

**What goes wrong:** Desktop Tauri build saves projects using native file system (`@tauri-apps/plugin-fs`). Users expect "Open Recent" menu, file watchers, and auto-save. Web version uses File System Access API (Chrome) or falls back to download/upload pattern (Firefox). Recent files list breaks, auto-save downloads 30 files, file watchers don't work. Users lose work because "Save" behavior is unpredictable across platforms.

**Why it happens:** Developer assumes Tauri's file system semantics apply everywhere. Tauri allows "path traversal prevention" but assumes full file system access to project directories. Web's File System Access API requires explicit user permission per directory, no persistent access across sessions (except with permission prompts). Firefox/Safari don't support File System Access API at all, requiring completely different code path.

**How to avoid:**
- Design file persistence API around most restricted platform (web), then enhance for desktop:
  ```typescript
  interface ProjectPersistence {
    // Works everywhere (download/upload pattern)
    exportProject(): Promise<Blob>
    importProject(file: File): Promise<void>

    // Enhanced for platforms with persistent access
    saveToFileSystem?(): Promise<void>
    enableAutoSave?(): Promise<void>
  }
  ```
- Web: Use IndexedDB for auto-save, export to download when user clicks "Save"
- Desktop: Use native file system with file watchers and auto-save
- Show platform-appropriate UI: "Download" button on web, "Save" on desktop
- Warn users on web that "unsaved changes" means "not downloaded"
- Test with all browser combinations: Chrome (FS Access API), Firefox (download), Safari

**Warning signs:**
- "Open Recent" menu exists in Tauri but not web
- Auto-save triggers downloads in browser
- No warning when closing browser tab with unsaved changes
- Code assumes persistent file handles work everywhere
- File watchers attempted in web context (always fails)
- Error handling missing for permission denials

**Phase to address:** v1.1 Phase 3 (Tauri Desktop Foundation) must NOT assume native file semantics are universal. Phase 4 (Web Deployment) must validate degraded-but-functional persistence.

**Sources:**
- [Tauri File System Plugin](https://v2.tauri.app/plugin/file-system/)
- [Tauri Discussion #6941: Detect desktop mode](https://github.com/tauri-apps/tauri/discussions/6941)

---

### Pitfall 12: Library Version Drift Without Dependency Locking

**What goes wrong:** Team imports KiCad library version 6.0.7. Three months later, new developer clones project, imports "latest" KiCad library (now 6.0.10). Footprints have subtle changes (pad sizes adjusted for manufacturability). DRC passes on old version, fails on new version. Manufacturing files generated from different library versions are incompatible. Git shows no diff because library isn't version-controlled.

**Why it happens:** Industry best practice is "work with local copy, push to centralized repo with version control" but this assumes single source of truth. CodeYourPCB wants to support multiple library sources (KiCad, JLCPCB, custom). Developer implements "import from URL" feature without capturing version/commit hash. Users assume "R_0805 is R_0805" universally when in reality standards evolve (IPC-7351C released in 2022 changed footprint calculations).

**How to avoid:**
- Lock library dependencies in project manifest (similar to package.json):
  ```json
  {
    "libraries": {
      "kicad-official": {
        "source": "https://gitlab.com/kicad/libraries/kicad-footprints",
        "version": "6.0.7",
        "commit": "abc123def456",
        "imported": "2026-01-15"
      }
    },
    "components": {
      "R1": { "footprint": "kicad-official::R_0805", "library_version": "6.0.7" }
    }
  }
  ```
- Version control the imported footprints directory, not just references
- Warn when component references library version different from project lock
- Provide `library update` command that shows diff before updating
- Document library provenance: source URL, import date, commit hash, checksums

**Warning signs:**
- Project file references footprints by name only, no version info
- Library import doesn't record source metadata
- Different developers get different DRC results on same design
- "Works on my machine" syndrome for PCB validation
- No way to reproduce historical builds (manufacturability regressions)

**Phase to address:** v1.1 Phase 1 (Library Management Foundation) must implement versioned library manifest. Phase 7 (Documentation) must document library update workflow.

**Sources:**
- [Best practice for version controlling PCB design](https://www.embeddedrelated.com/thread/9643/best-practice-for-version-controlling-of-a-pcb-design)
- [PCB Footprint Creation Guidelines](https://www.ultralibrarian.com/2024/02/13/pcb-footprint-creation-guidelines-avoid-redundant-library-demands-ulc/)

---

## Technical Debt Patterns

Mistakes that don't break things immediately but accumulate into larger problems.

### Pattern 1: Hardcoded Manufacturing Assumptions

**What goes wrong:** Tool assumes all manufacturers have same capabilities. Users can't specify their manufacturer's constraints. Designs pass DRC but are rejected by fab.

**Why it happens:**
- Using "typical" values from one manufacturer
- Not exposing constraint customization
- Assuming "smaller is always harder" (not always true)

**Prevention:**
- Make DRC rules data-driven from manufacturer capability files
- Provide presets for common manufacturers (JLCPCB, PCBWay, OSH Park)
- Allow custom constraint profiles
- Validate against manufacturer's stated capabilities, not guesses

**Phase mapping:** Phase 2 (Core Features) - DRC constraint system design

---

### Pattern 2: Units Confusion

**What goes wrong:** Mix of mm, mils, inches throughout codebase. Conversion errors in calculations. User specifies 10mm, gets 10mil trace.

**Why it happens:**
- Different PCB conventions (US uses mils, metric uses mm)
- Copy-pasting code without checking units
- No type safety on dimensional values

**Prevention:**
- Single internal unit (nanometers as integers)
- Type-safe dimensions: `struct Millimeters(i64)`, `struct Mils(i64)`
- Explicit conversion functions, no bare number arithmetic
- Display units configurable per-user, stored values always canonical

**Phase mapping:** Phase 1 (Foundation) - data model types

---

### Pattern 3: Monolithic Component Model

**What goes wrong:** Component = one giant struct with all possible fields. Simple resistor carries baggage for BGA-specific fields. Hard to extend for new component types.

**Why it happens:**
- Started with simple components, added fields as needed
- Fear of "too many types"
- Not using ECS or composition patterns

**Prevention:**
- ECS architecture for component model (brainstorm.md already plans this)
- Composition: `Position + Footprint + NetConnections + OptionalFields`
- Components are entities with attached component-data, not inheritance hierarchies

**Phase mapping:** Phase 1 (Foundation) - data model architecture

---

### Pattern 4: Schematic-Layout Desync

**What goes wrong:** For code-first tools, the "schematic" view and "layout" view show different information. Net connections don't match. Users trust the wrong view.

**Why it happens:**
- Schematic and layout are separate rendering paths
- No single source of truth enforced
- Round-trip sync is genuinely hard

**Prevention:**
- Single authoritative data model that both views query
- No separate "schematic netlist" and "layout netlist"
- Test: modify data model, verify both views update consistently
- Consider: schematic view is optional/generated, not authoritative

**Phase mapping:** Phase 4 (Full Experience) - when adding schematic view

**Sources:**
- [SKiDL Discussion on Schematic Generation](https://github.com/devbisme/skidl/discussions/129) - "JitX has a Sr Software engineer dedicated to just schematic generation so it's got to be a hard problem"

---

### Pattern 5: Platform Abstraction Shortcuts (v1.1)

**What goes wrong:** Skip platform abstraction layer, use `if (window.__TAURI__)` checks scattered throughout business logic. Desktop and web codebases diverge with 800% code duplication. Feature parity becomes impossible to maintain.

**Why it happens:** Abstraction feels like overhead early on. Runtime checks seem simpler than build-time configuration. Pressure to ship features quickly leads to copy-paste solutions for each platform.

**Prevention:**
- Never allow direct Tauri API imports in business logic (enforce with linter)
- Create abstraction layer before implementing first platform-specific feature
- Use build-time conditional compilation (Vite's `TAURI_ENV_PLATFORM`) not runtime checks
- Write integration tests that run on both platforms
- Code review checklist: "Does this code work on both web and desktop?"

**Phase mapping:** v1.1 Phase 3 (Tauri Desktop) - establish abstraction BEFORE feature work

---

### Pattern 6: Monaco Bundle Bloat Through Lazy Configuration (v1.1)

**What goes wrong:** Include Monaco with default configuration "to get it working." Bundle size jumps to 4MB. "We'll optimize later" becomes "users complain about slow load times." Web deployment becomes unusable for users on slow connections.

**Why it happens:** Monaco optimization requires understanding workers, language services, and code splitting. Default configuration includes 40+ languages. Developer focuses on functionality first, considers performance "later." Performance debt harder to pay down after features ship.

**Prevention:**
- Configure `vite-plugin-monaco-editor` with minimal workers from day one
- Set performance budget in CI before Monaco integration
- Lazy load Monaco only when user clicks "edit code"
- Use Tree-sitter for syntax highlighting (already in codebase)
- Measure bundle size impact before merging Monaco PR

**Phase mapping:** v1.1 Phase 5 (Monaco Integration) - optimization is not optional

---

## Performance Traps

Patterns that seem fine but kill performance at scale.

### Trap 1: Recalculating Copper Pours on Every Edit

**What goes wrong:** Moving a component triggers full copper pour recalculation. Board with multiple pours becomes unusable in real-time editing.

**Why it happens:**
- Pour geometry depends on component positions
- Naive implementation: any change = full recalc
- Polygon operations are expensive

**Prevention:**
- Lazy pour evaluation (mark dirty, recalc on demand)
- Zone-based dirty tracking (only recalc pours in affected zone)
- Background pour calculation with preview
- Cache pour geometry with invalidation

---

### Trap 2: String Comparisons for Net Matching

**What goes wrong:** Net connectivity checks use string comparison for net names. Performance degrades O(n * string_length) and is allocation-heavy.

**Why it happens:**
- Net names are strings in the source file
- Easy to compare `net1.name == net2.name`
- Works fine for small designs

**Prevention:**
- Intern net names to numeric IDs at parse time
- Compare IDs (integer comparison) not names
- Only convert back to strings for display

---

### Trap 3: Full Board Re-render on Viewport Change

**What goes wrong:** Panning or zooming renders entire board. Large boards drop to single-digit FPS during navigation.

**Why it happens:**
- Simple render loop: "for each object, draw"
- No culling, no level-of-detail
- Canvas/WebGL state thrashing

**Prevention:**
- Viewport culling (only render visible objects using spatial index)
- Level-of-detail rendering (simplify geometry when zoomed out)
- Batch similar draw calls
- Dirty rectangle tracking (only redraw changed regions)

---

### Trap 4: Loading All Footprint Thumbnails Eagerly (v1.1)

**What goes wrong:** Smooth experience with 10 components, gradually slower imports as library grows. Once library hits 500+ footprints, library browser becomes unusable with 10+ second load times.

**Why it happens:** Rendering footprint thumbnails is expensive (SVG parsing, canvas rendering). Loading all thumbnails upfront seems simple. Performance acceptable during initial development with small test library.

**Prevention:**
- Virtualize library browser (only render visible rows)
- Lazy-load thumbnails on scroll with Intersection Observer
- Pre-generate thumbnail sprites for common components
- Cache rendered thumbnails in IndexedDB

**Phase mapping:** v1.1 Phase 1 (Library Management) - virtualization from day one

**Sources:**
- [High-Performance Web Apps in 2026](https://letket.com/high-performance-web-apps-in-2026-webassembly-webgpu-and-edge-architectures/)

---

### Trap 5: Synchronous File Writes on Every Edit (v1.1)

**What goes wrong:** Instant feedback for small files, but files >10MB cause UI freezes. Rapid editing (10+ edits/second) triggers excessive I/O, degrading performance.

**Why it happens:** Auto-save feature implemented naively without debouncing. Each edit immediately writes to disk. Works fine during initial testing with small example projects.

**Prevention:**
- Debounce auto-save (5 second delay)
- Use optimistic UI (show changes immediately, persist asynchronously)
- Write to temp location first, atomic rename on success
- IndexedDB for web (async by default), native fs for desktop

**Phase mapping:** v1.1 Phase 4 (Web Deployment) - persistence strategy design

---

## UX Pitfalls

Design decisions that hurt user adoption.

### Pitfall 1: "Code-First" Means "No Visual Feedback"

**What goes wrong:** Tool requires writing code with no visual preview. Users can't see what they're building. Traditional EDA users bounce immediately.

**Why it happens:**
- "Code-first" interpreted as "code-only"
- Visual preview is hard, deferred
- Underestimating importance of visual feedback for spatial tasks

**Consequences:**
- Steep learning curve
- Users can't validate their understanding
- No immediate gratification
- Adoption limited to command-line enthusiasts

**Prevention:**
- Hot-reload visual preview is MVP, not "nice to have"
- "Code-first" means code is source of truth, not that code is only interface
- Preview updates on every file save (or faster with file watching)

**Phase mapping:** Phase 1 (Foundation) - file watching and basic renderer are core MVP

---

### Pitfall 2: Component Library Chicken-and-Egg

**What goes wrong:** Tool requires components, but has no library. Users must create every footprint from scratch. Barrier to first successful design is weeks of work.

**Why it happens:**
- "We'll build the library later"
- Underestimating library importance
- Not realizing users won't create their own

**Consequences:**
- Users can't build anything practical
- First experience is frustration
- Abandoned after "hello world" attempt

**Prevention:**
- Import KiCad footprint libraries from day one (PROJECT.md already plans this)
- Ship with curated set of common components (0805, QFP, SOT-23, etc.)
- Make footprint import/creation as easy as possible
- Provide procedural footprint generation for common patterns

**Phase mapping:** Phase 5 (Advanced) - but basic import earlier

---

### Pitfall 3: Error Messages Without Location

**What goes wrong:** "Invalid net connection" with no file/line information. Users can't find the problem in a 500-line design file.

**Why it happens:**
- Errors generated after parsing loses source location
- Not threading source spans through compilation
- Easier to just print the error message

**Prevention:**
- Every AST node carries source span
- Errors include file:line:column
- Provide code snippet context in error messages
- LSP diagnostics with precise ranges (Phase 4)

**Phase mapping:** Phase 1 (Foundation) - Tree-sitter preserves locations; maintain them through pipeline

---

### Pitfall 4: Learning HDL-Like Syntax as a PCB Designer

**What goes wrong:** Target users are PCB designers, not programmers. HDL-style syntax with functions, loops, and imports is unfamiliar. Users give up before understanding the paradigm.

**Why it happens:**
- Tool built by programmers for programmers
- Assuming PCB designers want to learn programming
- Not providing gradual on-ramp

**Consequences:**
- Adoption limited to software engineers doing hobby electronics
- Professional PCB designers stick with traditional tools
- Market constrained unnecessarily

**Prevention:**
- Simplest designs require minimal syntax (just component placement)
- Advanced features (loops, functions) are opt-in for power users
- Excellent error messages guide users to correct syntax
- Examples for every common task
- AI-assisted editing lowers barrier (user describes intent, AI writes syntax)

**Sources:**
- [HDL Learning Curve Challenges](https://www.sciencedirect.com/science/article/abs/pii/S0950584923000502) - "HDLs have a steep learning curve for beginners"
- [PCB HDL Adoption Challenges](https://ducky64.github.io/HATRA20_PCB_HDLs.pdf) - "While some learning curve is inevitable, flattening it as much as possible is necessary"

---

### Pitfall 5: Modal "Importing Library" Dialog Blocking UI (v1.1)

**What goes wrong:** User can't continue working while waiting for 50MB library download. Modal dialog blocks entire UI. User frustration as they're forced to watch progress bar instead of working.

**Why it happens:** Simple implementation uses modal dialog with progress bar. Synchronous download easier to implement than background task. Seems acceptable during testing with small libraries.

**Prevention:**
- Background import with notification system
- Allow continued editing during library download
- Show progress in status bar, not modal dialog
- Queue multiple library imports concurrently

**Phase mapping:** v1.1 Phase 1 (Library Management) - background tasks from start

**Sources:**
- [Dark Mode: Users Think About It and Issues](https://www.nngroup.com/articles/dark-mode-users-issues/) - Discusses interruption patterns

---

### Pitfall 6: Theme Toggle Without Preference Persistence (v1.1)

**What goes wrong:** User sets dark mode, refreshes page, back to light mode. Eye strain from unexpected light theme. User has to toggle dark mode on every visit.

**Why it happens:** Theme toggle updates UI state but doesn't persist to localStorage. Developer forgets that web apps don't automatically preserve state. No respect for `prefers-color-scheme` media query.

**Prevention:**
- Save theme preference to localStorage immediately on toggle
- Load theme preference before first paint (prevent FOIT)
- Respect `prefers-color-scheme` as default
- Apply theme synchronously in `<head>` before body loads

**Phase mapping:** v1.1 Phase 2 (Dark Mode) - persistence is day one requirement

**Sources:**
- [A Complete Guide to Dark Mode on the Web](https://css-tricks.com/a-complete-guide-to-dark-mode-on-the-web/)

---

### Pitfall 7: Desktop and Web Versions Behave Differently (v1.1)

**What goes wrong:** User expects "File → Open" on web but it doesn't exist. Desktop has "Open Recent" menu, web doesn't. Confusion about platform differences. Support burden from "this feature doesn't work" reports.

**Why it happens:** Platform capabilities differ (native file system vs browser security model). Developer implements features where possible, leaving gaps elsewhere. No unified design for cross-platform workflows.

**Prevention:**
- Keep workflows similar across platforms with appropriate labels: "Open" vs "Import from file"
- Show platform-appropriate UI but consistent mental model
- Document platform differences in help system
- Use same keyboard shortcuts where possible
- Graceful degradation, not "feature missing" error messages

**Phase mapping:** v1.1 Phase 4 (Web Deployment) - cross-platform UX design

---

### Pitfall 8: Auto-Save Download Spam on Web (v1.1)

**What goes wrong:** User makes 10 edits, gets 10 download prompts. Disables auto-save to stop spam. Loses work when browser crashes. Bad experience drives users away from web version.

**Why it happens:** Desktop auto-save pattern (write to file system) naively ported to web. Web's security model prevents silent file writes. Each auto-save triggers download dialog.

**Prevention:**
- Auto-save to IndexedDB silently (no download prompt)
- Show "export to file" action in menu (user-initiated)
- Warn on tab close if IndexedDB has unsaved changes
- Desktop gets real auto-save, web gets background persistence
- Clear messaging: "Auto-save keeps your work safe in browser storage"

**Phase mapping:** v1.1 Phase 4 (Web Deployment) - persistence UX design

**Sources:**
- [Tauri Discussion #6941: Detect desktop mode](https://github.com/tauri-apps/tauri/discussions/6941)

---

## "Looks Done But Isn't" Checklist

Features that appear complete but have subtle gaps.

### Gerber Export
- [ ] Board outline included (separate file or in drill file)
- [ ] Drill file units match Gerber units
- [ ] Drill file zero-suppression consistent
- [ ] Flash pads, not vector pads for SMD
- [ ] Contour fills, not vector fills for copper pours
- [ ] Aperture list is single, not multiple
- [ ] Tested with gerbv, KiCad viewer, manufacturer DFM
- [ ] Edge cuts geometry actually in file (KiCad bug reference)
- [ ] Coordinate system matches (Y-up vs Y-down handled)

### Autorouter Integration
- [ ] Results are deterministic (same input = same output)
- [ ] Routing respects all design rules
- [ ] Via placement follows design rules
- [ ] Differential pairs handled correctly
- [ ] Length matching constraints respected
- [ ] Error reporting is actionable (not just "routing failed")
- [ ] Partial routing success doesn't corrupt board

### DRC Implementation
- [ ] Scales to 1000+ components without hanging
- [ ] Clearance checking handles all object types (trace-trace, trace-pad, pad-pad, trace-zone)
- [ ] Net-aware (same-net objects don't violate clearance)
- [ ] Via annular ring checking
- [ ] Drill-to-copper clearance
- [ ] Silk-to-pad clearance
- [ ] Thermal relief verification for plane connections
- [ ] Min trace width per net class
- [ ] Differential pair spacing and skew
- [ ] Results are deterministic

### Component Library Import
- [ ] KiCad S-expression footprints parse correctly
- [ ] 3D models referenced (even if not rendered yet)
- [ ] Pin names preserved
- [ ] Pad types (SMD, TH, NPTH) handled
- [ ] Custom pad shapes supported
- [ ] Courtyard/silkscreen layers imported
- [ ] Units conversion correct (KiCad uses mm)

### DSL Parser
- [ ] Error recovery (partial parse of invalid file)
- [ ] Source locations preserved through to error messages
- [ ] Comments preserved for round-trip (if editing support planned)
- [ ] Unicode handling (component names, user strings)
- [ ] Large file performance (1000+ lines)
- [ ] Incremental parsing works

### File Format
- [ ] Version number in every file
- [ ] Forward migration path defined
- [ ] Round-trip test (parse-save-parse produces identical result)
- [ ] Handles missing optional fields (backward compat)
- [ ] Warns on unknown fields (forward compat)
- [ ] Special characters in strings escaped correctly

### v1.1 Integration Checklist

#### Library Management
- [ ] Namespace-prefixed imports prevent conflicts
- [ ] Library source metadata stored with footprints
- [ ] Conflict detection shows side-by-side comparison
- [ ] Version locking in project manifest
- [ ] Library update shows diff before applying

#### Platform Abstraction (Tauri/Web)
- [ ] No direct Tauri API imports in business logic
- [ ] FileSystem, Dialog, Menu interfaces abstract platform
- [ ] Build-time conditional compilation (not runtime checks)
- [ ] Integration tests run on both platforms
- [ ] Feature parity validated in CI

#### Monaco Integration
- [ ] Monaco chunk <500KB compressed
- [ ] Lazy loaded with dynamic import
- [ ] Only editorWorkerService included (no language workers)
- [ ] Tree-sitter used for syntax highlighting
- [ ] Performance budget enforced in CI

#### Dark Mode
- [ ] Theme manager coordinates CSS, Monaco, Canvas, Three.js
- [ ] No FOIT (flash of incorrect theme)
- [ ] Preference persists to localStorage
- [ ] Respects prefers-color-scheme
- [ ] All UI states tested in both themes

#### File Persistence
- [ ] Web: IndexedDB auto-save, export to download
- [ ] Desktop: Native file system with watchers
- [ ] Platform-appropriate UI (Download vs Save)
- [ ] Warning on tab close for unsaved web changes
- [ ] Graceful degradation documented

#### Performance
- [ ] Footprint thumbnails lazy-loaded with Intersection Observer
- [ ] Library browser virtualized (only visible rows)
- [ ] Auto-save debounced (5s delay)
- [ ] Bundle size budgets enforced

---

## Phase-Specific Warnings

| Phase | Likely Pitfall | Mitigation |
|-------|---------------|------------|
| Phase 1: Foundation | DSL syntax lock-in | Minimal grammar, version from start |
| Phase 1: Foundation | Floating-point in data model | Integer nanometers, convert at edges |
| Phase 2: Core Features | DRC performance cliff | Spatial indexing from day one |
| Phase 2: Core Features | Gerber edge cases | Test against multiple viewers/manufacturers |
| Phase 3: Intelligence | Autorouter non-determinism | Seed randomness, deterministic data structures |
| Phase 4: Full Experience | Schematic-layout desync | Single source of truth |
| Phase 5: Advanced | Component library chicken-egg | KiCad import as MVP priority |
| All Phases | File format breaking changes | Version in files, migration tests |

### v1.1 Phase-Specific Warnings

| Phase | Likely Pitfall | Mitigation |
|-------|---------------|------------|
| v1.1 Phase 1: Library Management | Namespace conflicts | Prefix imports, detect conflicts at import time |
| v1.1 Phase 1: Library Management | Version drift | Lock library versions in manifest |
| v1.1 Phase 1: Library Management | Eager thumbnail loading | Virtualize browser, lazy-load on scroll |
| v1.1 Phase 2: Dark Mode | Theme inconsistency | Central theme manager for all surfaces |
| v1.1 Phase 2: Dark Mode | FOIT on page load | Apply theme before body renders |
| v1.1 Phase 3: Tauri Desktop | Feature parity drift | Abstraction layer before Tauri features |
| v1.1 Phase 3: Tauri Desktop | Runtime platform checks | Build-time conditional compilation |
| v1.1 Phase 4: Web Deployment | File system mismatch | IndexedDB persistence, export to download |
| v1.1 Phase 4: Web Deployment | Auto-save download spam | Silent IndexedDB save, manual export |
| v1.1 Phase 5: Monaco Integration | Bundle size explosion | vite-plugin-monaco-editor, lazy load, minimal workers |
| v1.1 Phase 5: Monaco Integration | Worker misconfiguration | Test Network tab for 404s |

---

## Sources

### DSL Design
- [Martin Fowler DSL Q&A](https://martinfowler.com/bliki/DslQandA.html)
- [DSL Evolution InfoQ](https://www.infoq.com/articles/dsl-evolution/)
- [DSL Best Practices LinkedIn](https://www.linkedin.com/advice/0/how-do-you-evolve-dsl-java-without-breaking-backward-compatibility)
- [Tonsky DSL Design](https://tonsky.me/blog/dsl/)

### PCB/EDA Specific
- [KiCad Coordinate System](https://forum.kicad.info/t/coordinate-system-grid-and-origins-in-the-pcb-editor/24535)
- [KiCad File Compatibility](https://forum.kicad.info/t/backward-and-forward-compatibility/45234)
- [Common Gerber Issues - Bittele](https://www.7pcb.com/blog/common-gerber-issues-how-to-fix-them)
- [Common Gerber Problems - Sierra Circuits](https://www.protoexpress.com/blog/common-problems-associated-with-gerber-files/)
- [PCB Design Mistakes - Cadence](https://resources.pcb.cadence.com/blog/2025-common-pcb-design-mistakes)
- [Why PCB Autorouting Remains Broken](https://autocuro.com/blog/why-pcb-autorouting-remains-broken)

### Code-First PCB Tools
- [SKiDL Discussions](https://github.com/devbisme/skidl/discussions/)
- [JITX Documentation](https://docs.jitx.com/)
- [tscircuit](https://tscircuit.com/)
- [PCB HDL Research Paper](https://ducky64.github.io/HATRA20_PCB_HDLs.pdf)

### Numerical/Geometric
- [Mitigating Floating Point Errors - Medium](https://medium.com/@moiserushanika2006/mitigating-floating-point-errors-in-computational-geometry-algorithms-a62525da45ef)
- [Floating Point Precision - LinkedIn](https://www.linkedin.com/advice/1/how-can-you-prevent-floating-point-errors-computational-hcmoe)

### Performance
- [PDRC: GPU-Accelerated DRC - CUHK](http://www.cse.cuhk.edu.hk/~byu/papers/C219-DAC2024-PDRC.pdf)
- [Routing Algorithms - Wikipedia](https://en.wikipedia.org/wiki/Routing_(electronic_design_automation))

### v1.1 Integration Sources

**Tauri:**
- [Tauri 2.0 Stable Release](https://v2.tauri.app/blog/tauri-20/)
- [Tauri File System Plugin](https://v2.tauri.app/plugin/file-system/)
- [Tauri IPC Architecture](https://v2.tauri.app/concept/inter-process-communication/)
- [Tauri Environment Variables](https://v2.tauri.app/reference/environment-variables/)
- [GitHub Issue #11347: HTTP plugin fallback](https://github.com/tauri-apps/tauri/issues/11347)
- [GitHub Discussion #6941: Detect desktop mode](https://github.com/tauri-apps/tauri/discussions/6941)

**Monaco Editor:**
- [Monaco Bundle Size Issue #97](https://github.com/microsoft/monaco-editor-webpack-plugin/issues/97)
- [Monaco Issue #3518: Import adds ALL to bundle](https://github.com/microsoft/monaco-editor/issues/3518)
- [vite-plugin-monaco-editor](https://github.com/vdesjs/vite-plugin-monaco-editor)
- [Understanding Monaco's web worker architecture](https://app.studyraid.com/en/read/15534/540352/understanding-monacos-web-worker-architecture)
- [Configuring Monaco workers](https://app.studyraid.com/en/read/15534/540353/configuring-monaco-workers-for-optimal-performance)

**Library Management:**
- [Managing PCB Footprint Libraries - Altium 365](https://resources.altium.com/p/managing-pcb-footprint-libraries-with-altium-365)
- [Centralized Component Libraries Best Practices](https://resources.altium.com/p/centralized-component-libraries-best-practices-hardware-teams)
- [PCB Footprint Creation Guidelines - Ultra Librarian](https://www.ultralibrarian.com/2024/02/13/pcb-footprint-creation-guidelines-avoid-redundant-library-demands-ulc/)
- [Best practice for version controlling PCB design](https://www.embeddedrelated.com/thread/9643/best-practice-for-version-controlling-of-a-pcb-design)

**Web Performance:**
- [The State of WebAssembly 2025-2026](https://platform.uno/blog/the-state-of-webassembly-2025-2026/)
- [High-Performance Web Apps in 2026](https://letket.com/high-performance-web-apps-in-2026-webassembly-webgpu-and-edge-architectures/)
- [Optimizing WASM Binary Size](https://book.leptos.dev/deployment/binary_size.html)
- [Code-splitting and minimal edge latency](https://www.fastly.com/blog/code-splitting-and-minimal-edge-latency-the-perfect-match)

**Dark Mode:**
- [Dark Mode: Users Issues - NN/G](https://www.nngroup.com/articles/dark-mode-users-issues/)
- [Complete Guide to Dark Mode - CSS-Tricks](https://css-tricks.com/a-complete-guide-to-dark-mode-on-the-web/)
- [Dark Side of Dark Mode - Vareweb](https://vareweb.com/blog/the-dark-side-of-dark-mode-in-web-design/)

**Code Quality:**
- [Code Duplication Best Tools 2026](https://www.codeant.ai/blogs/stop-code-duplication-developers-guide)
- [Code Rot Vs Code Gen 2025-2026](https://fullstacktechies.com/code-rot-vs-code-gen-ai-react-strategy/)
- [DRY Principle in AI-Generated Code](https://www.faros.ai/blog/ai-generated-code-and-the-dry-principle)

---
*Pitfalls research for: CodeYourPCB (general domain + v1.1 integration challenges)*
*Updated: 2026-01-29 for v1.1 milestone*
