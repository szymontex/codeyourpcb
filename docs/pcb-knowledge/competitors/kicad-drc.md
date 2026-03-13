# KiCad DRC Architecture

Analysis of KiCad's Design Rule Check system — architectural patterns and
design decisions relevant to CodeYourPCB's `cypcb-drc` crate.

**Source:** KiCad is open-source under GPL-3.0.
Repository: <https://gitlab.com/kicad/code/kicad>
Analysis based on KiCad 8.x (2024-2025) architecture.

**License note:** KiCad is GPL-3.0. No code was copied. This analysis
describes publicly observable architectural patterns for the purpose of
independent reimplementation using different approaches. Our `cypcb-drc`
crate is independently written in Rust with no KiCad code.

---

## Architecture Overview

KiCad's DRC is implemented in C++ within the `pcbnew` module. The system
follows a **test-based architecture** where each DRC check is an independent
test class.

### Key Components

```
pcbnew/drc/
  ├── drc_engine.cpp          — orchestrator, runs all tests
  ├── drc_test_provider.h     — base class for all DRC tests
  ├── drc_rule.h              — rule definitions with conditions
  ├── drc_item.h              — violation markers
  ├── drc_test_provider_clearance.cpp
  ├── drc_test_provider_copper.cpp
  ├── drc_test_provider_edge_clearance.cpp
  ├── drc_test_provider_hole_size.cpp
  ├── drc_test_provider_annular_ring.cpp
  ├── drc_test_provider_silk_clearance.cpp
  ├── drc_test_provider_connectivity.cpp
  └── ... (~20 test providers)
```

### Test Provider Pattern

Each test provider is a class implementing `DRC_TEST_PROVIDER`:

- `GetName()` — human-readable test name
- `GetDescription()` — what the test checks
- `Run()` — execute the test, report violations
- Tests are stateless — they receive the board, run, and produce markers

**Our analog:** `DrcRule` trait with `check(&self, world, constraints) -> Vec<DrcViolation>`.
Same pattern — stateless rules that produce structured violations.

### DRC Engine

The engine:
1. Loads all test providers
2. Loads the rule set (net classes, custom rules, conditional rules)
3. Iterates through all providers, calling `Run()` on each
4. Collects violations (DRC markers)
5. Can run in batch mode (CLI) or interactive mode (GUI with markers)

**Our analog:** `check_all()` function in `cypcb-drc` that iterates through
all registered rules.

---

## Net Class System

KiCad organizes nets into **net classes** with per-class constraints:

```
(net_class "Default"
  (clearance 0.2)
  (trace_width 0.25)
  (via_dia 0.8)
  (via_drill 0.4)
)

(net_class "HighSpeed"
  (clearance 0.15)
  (trace_width 0.15)
  (via_dia 0.6)
  (via_drill 0.3)
  (diff_pair_width 0.1)
  (diff_pair_gap 0.15)
)
```

Key properties:
- **Default class** applies to all unassigned nets
- Nets are assigned to classes explicitly or by pattern matching
- Each class defines its own clearance, trace width, via geometry
- DRC uses the *most restrictive* constraint when two nets from different
  classes interact (e.g., clearance between a HighSpeed and Default net uses
  the larger clearance of the two)

**Our analog:** `SignalClass` enum with `SignalClassConstraints`. We use a
fixed taxonomy (Digital, HighSpeed, Analog, Power, Differential) rather than
user-defined classes. This is simpler but less flexible — a reasonable
tradeoff for our autorouter-first design.

---

## Conditional Rules

KiCad 6+ added **custom DRC rules** with conditions:

```
(rule "USB Clearance"
  (condition "A.NetClass == 'USB' && B.NetClass == 'USB'")
  (constraint clearance (min 0.15mm))
)

(rule "BGA Region"
  (condition "A.insideArea('BGA_Zone')")
  (constraint track_width (min 0.1mm))
)
```

Properties:
- Rules have a condition expression (evaluated per pair of items)
- Conditions can reference net class, component, area, layer, and net properties
- Multiple rules can apply to the same items — most restrictive wins
- Rules can be ordered by priority

**Our approach:** We don't have conditional rules yet. Our `PresetRuleSet`
supports per-net overrides via `with_net_override()`, which handles the common
case. Full conditional rules with expression evaluation would be a future
enhancement if users need spatial or component-based rule variations.

---

## DRC Markers / Violation Reporting

KiCad's DRC produces **DRC markers** — visual indicators placed on the board:

```cpp
struct DRC_ITEM {
    int         m_errorCode;      // numeric error type
    wxString    m_errorMessage;   // human-readable description
    BOARD_ITEM* m_mainItem;       // primary item involved
    BOARD_ITEM* m_auxItem;        // secondary item (e.g., the other trace)
    VECTOR2I    m_pos;            // marker position
    SEVERITY    m_severity;       // error, warning, exclusion
};
```

Key features:
- Each violation references up to two board items (pair-based checks)
- Position is the geometric point of the violation
- Severity levels: error (fails), warning (informational), exclusion (user-waived)
- Markers are rendered as arrows/icons on the board view
- Violations can be exported as JSON (see DRC report format)

**Our analog:** `DrcViolation` with `kind`, `location` (Point), `entity_id`,
`related_entity_id`, and `message`. Same concept — structured violation with
location and entity references.

### JSON DRC Report

KiCad can export DRC results as JSON:

```json
{
  "violations": [
    {
      "type": "clearance_violation",
      "severity": "error",
      "description": "Clearance violation (0.15mm < 0.2mm required)",
      "items": [
        { "description": "Track on F.Cu", "pos": {"x": 100.0, "y": 50.0} },
        { "description": "Track on F.Cu", "pos": {"x": 100.1, "y": 50.0} }
      ]
    }
  ]
}
```

The `pcb` competitor project (Diode) has a KiCad DRC report parser
(`pcb-kicad/src/drc.rs`) that deserializes this format — confirming it's a
stable interchange format.

---

## Violation Types

KiCad defines ~40 distinct violation types. The ones most relevant to us:

| KiCad Error Code | Description | Our Equivalent |
|------------------|-------------|----------------|
| DRCE_CLEARANCE_VIOLATION | Copper-to-copper clearance | `ViolationKind::Clearance` |
| DRCE_TRACK_WIDTH | Trace too narrow | `ViolationKind::TraceWidth` |
| DRCE_VIA_DIAMETER | Via too small | `ViolationKind::DrillSize` |
| DRCE_ANNULAR_WIDTH | Annular ring too small | `ViolationKind::AnnularRing` |
| DRCE_COPPER_EDGE_CLEARANCE | Too close to board edge | `ViolationKind::EdgeClearance` |
| DRCE_HOLE_CLEARANCE | Hole-to-hole clearance | (not yet implemented) |
| DRCE_UNCONNECTED_ITEMS | Ratsnest incomplete | `ViolationKind::Connectivity` |
| DRCE_SHORTING_ITEMS | Different nets touching | (handled by clearance check) |
| DRCE_SILK_CLEARANCE | Silk overlap/clearance | (not yet implemented) |

---

## Architectural Patterns Worth Noting

1. **Independent test providers** — each DRC check is self-contained, easy to
   add/remove, testable in isolation. We follow this same pattern.

2. **Pair-based checking** — clearance checks compare *pairs* of items, using
   spatial indexing for efficiency. We currently do O(n²) pair checking — spatial
   indexing is a future optimization.

3. **Net class inheritance** — "most restrictive wins" when two net classes
   interact. Our `SignalClassConstraints` follows the same principle.

4. **Severity levels** — not all violations are equal. KiCad's error/warning/
   exclusion model is more nuanced than our current all-errors approach.

5. **Incremental DRC** — KiCad can re-check only items that changed since the
   last full run. Our batch-only approach is simpler but doesn't support
   interactive editing feedback.

---

## Sources

- KiCad source: <https://gitlab.com/kicad/code/kicad/-/tree/master/pcbnew/drc>
- KiCad DRC documentation: <https://docs.kicad.org/8.0/en/pcbnew/pcbnew.html#design-rule-check>
- KiCad custom rules: <https://docs.kicad.org/8.0/en/pcbnew/pcbnew.html#custom-design-rules>

**License:** KiCad is GPL-3.0. This analysis describes architectural patterns
for educational purposes. No KiCad code was used in CodeYourPCB.
