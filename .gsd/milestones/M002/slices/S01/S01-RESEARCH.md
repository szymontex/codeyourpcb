# S01: PCB Knowledge Base & Design Rules — Research

**Date:** 2026-03-13

## Summary

The existing codebase has a solid DRC foundation (`cypcb-drc`) with 5 rules (clearance, drill size, trace width, connectivity, keepout) and 4 manufacturer presets (JLCPCB 2/4-layer, PCBWay, Prototype). The `cypcb-calc` crate already implements IPC-2221 trace width calculations. However, the current `DesignRules` struct only covers 7 fields — a professional EDA tool needs 30+ rule parameters to match KiCad/Altium. No `cypcb-rules` crate or `docs/pcb-knowledge/` directory exists yet.

The main work is (1) building a comprehensive PCB knowledge base as structured documentation, (2) creating a `cypcb-rules` crate that significantly extends `DesignRules` with signal integrity classification, thermal management, impedance control, and per-net rule overrides, and (3) adding OSHPark and additional manufacturer presets. The existing DRC architecture (`DrcRule` trait, spatial index, violation system) is well-designed and should be extended rather than replaced.

Key finding: the current JLCPCB preset values are slightly conservative. JLCPCB actually supports 5mil trace/space on 2-layer (not 6mil) at standard pricing, and 0.15mm minimum drill holes. The presets should be updated with tiered capabilities (standard vs. advanced) and our research values verified against current manufacturer specs.

## Recommendation

**Approach: Extend existing `cypcb-drc` + new `cypcb-rules` crate + structured knowledge docs**

1. **`docs/pcb-knowledge/`** — Structured markdown knowledge base organized by topic (IPC standards, manufacturer rules, signal integrity, thermal, trace geometry). This is the human-readable reference that informs implementation.

2. **`crates/cypcb-rules/`** — New crate that depends on `cypcb-core` and provides:
   - `RoutingRuleSet` trait with typed rule queries
   - Extended `DesignConstraints` struct (30+ fields covering signal classes, impedance, thermal, differential pairs, via fanout, etc.)
   - Signal classification system (`SignalClass::Digital`, `Analog`, `Power`, `HighSpeed`, `Differential`)
   - Per-net and per-class rule overrides
   - Manufacturer presets (JLCPCB standard/advanced 2/4/6-layer, PCBWay, OSHPark 2/4-layer, generic IPC tiers)
   - Stackup definitions with dielectric constants and copper weights
   - IPC-2221 clearance tables encoded as lookup functions

3. **Extend `cypcb-drc`** — Update existing `DesignRules` to wrap or delegate to the new `cypcb-rules` types for backward compatibility. Add new DRC rules for edge clearance and annular ring checking.

4. **`docs/pcb-knowledge/competitors/`** — Analysis of KiCad, LibrePCB, and Horizon EDA DRC internals with license-compliant notes.

This approach keeps backward compatibility with existing code that uses `DesignRules`, provides the rich constraint model S02's autorouter needs, and creates a knowledge base that future slices reference.

## Don't Hand-Roll

| Problem | Existing Solution | Why Use It |
|---------|------------------|------------|
| Spatial queries for DRC | `rstar` R*-tree (already in `cypcb-world`) | O(log n) clearance checks, proven in production |
| IPC-2221 trace width calc | `cypcb-calc::TraceWidthCalculator` (already exists) | Correct formula with warnings, tested |
| Unit conversions | `cypcb-core::Nm`, `Unit`, `Dimension` (already exists) | Nanometer-based integer math avoids float errors |
| ECS components | `bevy_ecs` (already in `cypcb-world`) | DRC rules query world via ECS, pattern established |
| Impedance calculation | IPC-2141 formulas (implement from standard) | Well-documented formulas; no crate needed |

## Existing Code and Patterns

- `crates/cypcb-drc/src/lib.rs` — DRC engine with `run_drc()`, `DrcResult`, timing. 5 rule implementations. Pattern: stateless rule structs implement `DrcRule` trait, config comes from `DesignRules`. Extend this, don't replace.
- `crates/cypcb-drc/src/presets/mod.rs` — `DesignRules` struct (7 fields), `Preset` enum with `from_name()` for DSL parsing. The enum + name-based lookup pattern is good — expand it for new manufacturers.
- `crates/cypcb-drc/src/rules/clearance.rs` — Spatial-index-based clearance checking with canonical pair dedup. Has TODO for same-net exemption. Good pattern for new rules.
- `crates/cypcb-drc/src/violation.rs` — `DrcViolation` with kind, location, entities, source span, message. Well-structured for UI display. Add new `ViolationKind` variants as needed.
- `crates/cypcb-calc/src/trace_width.rs` — IPC-2221 implementation with `TraceWidthParams` builder, internal/external constants, warning system. Extend with impedance calc.
- `crates/cypcb-core/src/units.rs` — `Unit` enum (Mm/Mil/Inch/Nm), `Dimension` struct. Pattern for adding electrical units (ohms, farads, volts) in S05.
- `crates/cypcb-world/src/spatial.rs` — `SpatialIndex` wrapping R*-tree with layer-filtered queries. The autorouter (S02) will rely heavily on this.
- `crates/cypcb-router/src/types.rs` — `RouteSegment`, `ViaPlacement`, `RoutingResult`, `RoutingMetrics` with quality scoring. The `cypcb-rules` crate must produce constraints these types can consume.
- `crates/cypcb-world/src/components/physical.rs` — `Layer` enum with `TopCopper`/`BottomCopper`/`Inner(u8)` + silk/mask/paste/outline. Sufficient for multi-layer routing.

## Constraints

- `DesignRules` is used by both `run_drc()` and the DSL `preset` directive — changes must be backward compatible
- `Preset::from_name()` is called from the parser — new presets need string aliases
- DRC runs in both native and WASM targets — no `Instant` in WASM (already handled with cfg)
- `cypcb-rules` must not depend on `cypcb-world` or `cypcb-drc` (it's a leaf dependency) — only `cypcb-core`
- The autorouter (S02) will be the primary consumer of routing rules — design the `RoutingRuleSet` trait with A* pathfinding cost functions in mind
- No code from KiCad/LibrePCB/Horizon can be copied — only design patterns and numerical values (IPC standards are public domain, manufacturer specs are public)
- All manufacturer specifications must be verified against official capability pages with source URLs in comments

## Common Pitfalls

- **Stale manufacturer specs** — JLCPCB/PCBWay update capabilities regularly. Our current JLCPCB 2-layer preset uses 6mil (0.15mm) but JLCPCB actually supports 5mil (0.127mm) at standard pricing. Document spec dates and sources in code comments.
- **Same-net clearance exemption** — The clearance rule has a TODO for same-net exemption. Without it, pads on the same net that are close together generate false violations. Must be addressed when expanding DRC.
- **Internal vs. external layer rules** — IPC-2221 has different constants for internal layers (k=0.024 vs k=0.048). Many rules differ by layer position. The rules crate must support per-layer constraints.
- **Impedance depends on stackup** — Controlled impedance requires dielectric thickness, Er, and copper weight — these vary by manufacturer and layer count. Can't hard-code impedance values without stackup context.
- **Over-engineering signal classes** — For S01, define the classification taxonomy but don't implement SI simulation. The autorouter (S02) only needs class→constraint mapping, not full IBIS/SPICE analysis.
- **Annular ring vs. pad size** — Annular ring = (pad_diameter - drill_diameter) / 2. Must validate this relationship, not just check absolute sizes independently.

## Open Risks

- **IPC standard accuracy** — IPC-2221 is behind a paywall. Our implementations are based on widely-published summaries and formulas. Edge cases (high altitude, conformal coating) may be approximate.
- **Manufacturer spec drift** — JLCPCB and PCBWay change capabilities without notice. Presets could become inaccurate over time. Mitigate with source URLs and date stamps.
- **cypcb-rules API stability** — This is a new crate consumed by S02's autorouter. If the trait design is wrong, S02 will force a rework. Mitigate by designing the API with autorouter cost-function needs in mind from the start.
- **Scope creep into simulation** — Signal integrity rules could expand into full SI simulation (IBIS, S-parameters). Keep S01 focused on classification and constraint tables, not simulation.
- **KiCad/LibrePCB repo cloning** — The roadmap says to clone these repos to `/workspace/competitors/`. These are large repos (KiCad is 1GB+). Only clone the relevant DRC/router subdirectories or analyze via GitHub search.

## Skills Discovered

| Technology | Skill | Status |
|------------|-------|--------|
| EDA / PCB design | `l3wi/claude-eda@eda-pcb` (55 installs) | available — PCB-specific EDA patterns |
| EDA / Schematics | `l3wi/claude-eda@eda-schematics` (44 installs) | available — not needed for S01 |
| KiCad file format | `o2scale/electronics-agent-kit@kicad-file-format` (26 installs) | available — useful for competitor analysis |
| tscircuit | `tscircuit/skill@tscircuit` (156 installs) | available — TypeScript PCB tooling, tangentially relevant |

**Recommendation:** Consider `l3wi/claude-eda@eda-pcb` for PCB-specific design patterns. The `kicad-file-format` skill could help with competitor analysis but isn't critical since we're analyzing patterns, not parsing files.

## Key Research Findings

### IPC Standards That Matter

| Standard | Covers | Relevance to S01 |
|----------|--------|-------------------|
| IPC-2221 | Generic PCB design (clearance, trace width, thermal) | **Core** — already partially implemented in `cypcb-calc` |
| IPC-7351 | Land pattern design (footprint dimensions) | Medium — footprint library is separate |
| IPC-2581 | Design data transfer (ODB++ replacement) | Low — export format, not design rules |
| IPC-2141 | Impedance control for high-speed design | **Important** — needed for signal class constraints |
| IPC-2222 | Rigid board design (stackup, via aspect ratio) | Medium — stackup definitions |

### Manufacturer Capabilities Summary

| Parameter | JLCPCB 2L | JLCPCB 4L | PCBWay | OSHPark 2L | OSHPark 4L |
|-----------|-----------|-----------|--------|------------|------------|
| Min trace width | 5mil (0.127mm) | 4mil (0.1mm) | 6mil (0.15mm) | 6mil (0.15mm) | 5mil (0.127mm) |
| Min clearance | 5mil (0.127mm) | 4mil (0.1mm) | 6mil (0.15mm) | 6mil (0.15mm) | 5mil (0.127mm) |
| Min drill | 0.15mm | 0.2mm | 0.2mm | 0.254mm (10mil) | 0.254mm (10mil) |
| Min via drill | 0.3mm | 0.2mm | 0.2mm | 0.254mm (10mil) | 0.254mm (10mil) |
| Min annular ring | 0.15mm | 0.125mm | 0.15mm | 0.127mm (5mil) | 0.1mm (4mil) |
| Min silk width | 0.15mm | 0.15mm | 0.22mm | 0.127mm (5mil) | 0.127mm (5mil) |
| Min edge clearance | 0.3mm | 0.25mm | 0.3mm | 0.381mm (15mil) | 0.381mm (15mil) |
| Copper weight | 1oz/2oz | 1oz outer/0.5oz inner | 1oz/2oz | 1oz | 1oz outer/0.5oz inner |
| Blind/buried vias | No | No | Yes (extra cost) | No | No |

### Signal Classification for Autorouter

The autorouter (S02) needs signal classes to apply differentiated routing rules:

| Class | Trace Width | Clearance | Layer Preference | Special Rules |
|-------|------------|-----------|-----------------|---------------|
| Digital (standard) | ≥6mil | ≥6mil | Any copper | None |
| Digital (high-speed) | Impedance-controlled | ≥3W spacing | Adjacent to ground plane | Length matching, no stubs |
| Analog | ≥10mil | ≥20mil from digital | Isolated, with guard traces | Minimize vias |
| Power | IPC-2221 based on current | ≥8mil | Planes preferred | Thermal relief on pads |
| Differential pair | Impedance-controlled, matched | Tight coupling | Adjacent to ground plane | Length matching ±5mil |

### Competitor DRC Architecture (Patterns Only)

- **KiCad** — DRC organized as individual tests (clearance, connectivity, copper sliver, courtyard overlap, etc.) each returning a list of markers. Net classes provide per-net overrides. Design rules can be conditional (e.g., "clearance between net A and net B = 0.3mm").
- **Horizon EDA** — Rules defined per-net-class with JSON configuration. Supports per-rule-per-net overrides. Interesting: "rule_match" concept for conditional rules.
- **atopile** — Constraint solver approach (symbolic constraint propagation) rather than post-hoc DRC checking. Rules are part of the design intent, not after-the-fact validation.

## Sources

- IPC-2221 trace width/clearance formulas validated against standard (source: Cadence summary, ProtoExpress reference)
- JLCPCB capabilities: 5mil standard on 2-layer, 4mil on 4-layer, 3.5mil on 6+ (source: [JLCPCB capabilities page](https://jlcpcb.com/capabilities/pcb-capabilities), [Schemalyzer analysis](https://www.schemalyzer.com/en/blog/manufacturing/jlcpcb/jlcpcb-design-rules))
- OSHPark: 6mil on 2-layer, 5mil on 4/6-layer, 10mil min drill on 2L, 8mil on 6L (source: [OSHPark design rules](https://docs.oshpark.com/design-tools/))
- PCBWay: 6mil recommended, 3mil achievable, 0.2mm min drill (source: [PCBWay capabilities](https://www.pcbway.com/capabilities.html))
- Signal integrity classification and best practices compiled from AllPCB, ProtoExpress, EMA-EDA references
- Competitor router patterns analyzed from `/workspace/competitors/pcb/` (DeepPCB cloud router, KiCad integration) and `/workspace/competitors/atopile/` (constraint solver in `faebryk.core.solver`)
