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
