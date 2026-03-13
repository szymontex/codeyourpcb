# Competitor DRC Architecture Analysis

Architectural pattern analysis of three PCB design tools with DRC/constraint
systems relevant to CodeYourPCB. The goal is to learn from their design
patterns — what problems they solved, how they structured their rule systems,
and which ideas are worth adopting.

**No code was copied.** These analyses describe architectural patterns and
design decisions observed from publicly available source code and documentation.

---

## Analyzed Competitors

| Tool | Focus | What We Learned |
|------|-------|----------------|
| [KiCad](kicad-drc.md) | Mature open-source EDA suite | DRC violation taxonomy, net class system, conditional rules, marker UX |
| [Horizon EDA / pcb (Diode)](horizon-eda.md) | Starlark-based PCB DSL | Diagnostics severity model, error categorization, compact rendering |
| [atopile](atopile-constraints.md) | Code-first hardware description | Constraint solver approach, default override pattern, pre-solve validation |

## Patterns We Adopted

### From KiCad

- **Violation taxonomy** — distinct violation kinds (clearance, width, drill, etc.)
  rather than a single "violation" type. Our `ViolationKind` enum mirrors this.
- **Net class concept** — grouping nets by signal type with per-class constraints.
  Our `SignalClass` serves a similar purpose with a fixed taxonomy.

### From pcb (Diode/Horizon EDA)

- **Severity classification** — error vs. warning vs. advice levels for diagnostics.
  Our DRC currently treats all violations as errors, but the severity model
  is a natural extension.
- **Compact diagnostic rendering** — structured first-line + extra-lines pattern
  for readable output. Our `DrcViolation` messages follow a similar structure.

### From atopile

- **Constraint satisfaction vs. post-hoc checking** — atopile validates constraints
  *before* layout (during compilation), while we check *after* placement. Both are
  needed: pre-solve for early feedback, post-hoc for physical verification.
- **Default override pattern** — defaults apply only if no explicit constraint
  exists. Our `PresetRuleSet::with_net_override()` implements this same concept.

## What We Deliberately Didn't Adopt

- **KiCad's exclusion/waiver system** — too complex for our current needs.
  Users can adjust constraints instead of waiving violations.
- **Starlark DSL for rules** — adds language complexity. Our Rust trait-based
  rules are simpler and type-safe.
- **atopile's component selection solver** — out of scope. We handle PCB
  layout rules, not component selection.
