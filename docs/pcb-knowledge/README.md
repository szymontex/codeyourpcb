# PCB Knowledge Base

Reference documentation for the CodeYourPCB design rules engine (`cypcb-rules`),
DRC checker (`cypcb-drc`), and autorouter. These docs capture the standards,
manufacturer specifications, and design best practices that inform our
constraint types and validation rules.

This knowledge base serves three audiences:

1. **Contributors** implementing new rules, presets, or signal class logic
2. **Users** who want to understand *why* a constraint has a particular value
3. **The AI agent** making design decisions during auto-routing and DRC

## Contents

### Standards & Theory

| Document | Description |
|----------|-------------|
| [IPC Standards](ipc-standards.md) | IPC-2221 clearance/trace width, IPC-2141 impedance, IPC-2222 stackup — formulas, tables, accuracy notes |
| [Signal Integrity](signal-integrity.md) | Signal classification taxonomy, per-class routing rules, impedance targets for common interfaces |
| [Thermal Management](thermal-management.md) | Current derating, thermal relief, via stitching, power plane design |
| [Trace Geometry](trace-geometry.md) | Via fanout, differential pairs, length matching, teardrops, copper balancing |

### Manufacturer Capabilities

| Document | Description |
|----------|-------------|
| [Manufacturer Capabilities](manufacturer-capabilities.md) | Full comparison table of JLCPCB, PCBWay, OSHPark — every parameter sourced with URL and date |

### Competitor Analysis

| Document | Description |
|----------|-------------|
| [Competitor Overview](competitors/README.md) | Summary of analyzed competitors and adopted patterns |
| [KiCad DRC](competitors/kicad-drc.md) | DRC architecture — individual tests, net classes, conditional rules, markers |
| [Horizon EDA / pcb (Diode)](competitors/horizon-eda.md) | Diagnostics system, Starlark-based rules, severity classification |
| [atopile](competitors/atopile-constraints.md) | Constraint solver approach, default overrides, pre-solve vs. post-hoc checking |

## Relationship to Code

These docs describe the *domain knowledge* behind the implementation in:

- `crates/cypcb-rules/` — `DesignConstraints`, `SignalClass`, `Stackup`, `RoutingRuleSet`, manufacturer presets, IPC clearance tables
- `crates/cypcb-drc/` — DRC rules (`ClearanceRule`, `TraceWidthRule`, `EdgeClearanceRule`, `AnnularRingRule`, etc.) and `Preset` configurations

When a doc references a specific constraint field or enum variant, that's the
actual Rust type — they map 1:1.

## Accuracy & Limitations

- **IPC standards are paywalled.** Our formulas and tables come from widely-published
  summaries (Cadence, Altium, Sierra Circuits, ProtoExpress). They match industry
  practice but may differ from the authoritative IPC text in edge cases.
- **Manufacturer specs change.** Source URLs and retrieval dates are documented.
  If a value seems wrong, check the source URL — manufacturers update capabilities.
- **Competitor analysis covers architectural patterns only.** No code was copied.
  Each analysis doc includes license attribution.
