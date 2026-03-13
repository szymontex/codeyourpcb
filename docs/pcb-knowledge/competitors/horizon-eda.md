# Horizon EDA / pcb (Diode) Architecture

Analysis of the `pcb` tool by Diode Computers (which incorporates Horizon EDA
concepts and extends them with a Starlark-based DSL). Architectural patterns
relevant to CodeYourPCB's diagnostics and rule system.

**Source:** `pcb` repository (Diode Computers, Inc.)
License: Proprietary (source was analyzed from a publicly available repository).
Analysis based on the codebase as of 2026-03.

**License note:** No code was copied. This analysis describes architectural
patterns observed from publicly available source code. Our implementation is
independently written in Rust.

---

## Architecture Overview

The `pcb` tool uses Starlark (a Python-like configuration language) as its
design entry format. The DRC/diagnostics system is built around a structured
diagnostics framework rather than traditional DRC test providers.

### Crate Structure

```
crates/
  pcb-zen-core/         — core language, diagnostics, passes
  pcb-kicad/            — KiCad import/export, DRC report parsing
  pcb-ui/               — terminal UI, progress, spinners
  pcb-fmt/              — code formatter
  pcb-docgen/           — documentation generator
  pcb-starlark-lsp/     — LSP server for IDE integration
```

### Diagnostics System

The central pattern is a **diagnostics framework** that unifies all error
reporting — language errors, DRC violations, design warnings, and lint
suggestions all flow through the same pipeline.

```rust
struct Diagnostic {
    severity: EvalSeverity,     // Error, Warning, Advice, Disabled
    body: String,               // multi-line description
    path: String,               // source file or board location
    span: Option<ResolvedSpan>, // exact source position
    source_error: Option<Arc<dyn Error>>,
    suppressed: bool,           // filtered out by suppress pass
}
```

Key properties:
- **Unified pipeline** — all diagnostics (not just DRC) use the same type
- **Severity levels** — Error (blocks output), Warning (informational),
  Advice (suggestion), Disabled (silenced)
- **Source spans** — diagnostics can point to exact positions in Starlark source
- **Suppressible** — users can suppress specific diagnostic kinds

---

## Diagnostics Passes

The diagnostics system uses a **pass-based pipeline** for filtering and
transforming diagnostics before display:

```rust
trait DiagnosticsPass {
    fn apply(&self, diagnostics: &mut Diagnostics);
}

// Built-in passes:
FilterHiddenPass    // removes Disabled-severity diagnostics
SuppressPass        // marks user-suppressed kinds as suppressed
```

This is interesting because it separates diagnostic *generation* from
*presentation*. A DRC check produces diagnostics, but the display layer can
filter, sort, and format them independently.

**Relevance to us:** Our current DRC returns `Vec<DrcViolation>` directly.
Adding a similar pass-based pipeline would let us support filtering by
severity, suppressing specific violation kinds, or adding custom post-processing.
Not needed now, but a clean extension point.

---

## Compact Diagnostic Rendering

The rendering system produces compact, terminal-friendly output:

```rust
struct CompactDiagnostic {
    kind_short: Option<String>,  // e.g., "clearance" (last segment of kind path)
    first_line: String,          // headline
    extra_lines: Vec<String>,    // additional context
}
```

Rendered as:
```
Error: [clearance] Copper-to-copper clearance violation (0.1mm < 0.15mm required)
  Trace on layer F.Cu near pad U1-12
  at board.pcb:42
```

Key features:
- **Kind path** — dotted path like `drc.clearance.copper` — the "short" version
  shows just the last segment (`copper`) for compact display
- **First line is the headline** — always meaningful on its own
- **Extra lines are context** — shown dimmed, optional detail
- **Location** — file path + span when available

**Relevance to us:** Our `DrcViolation::message` is a single string. The
first-line/extra-lines split is worth adopting for CLI output — show the
headline in normal text, show the detail when verbose mode is on.

---

## Error Categorization

Diagnostics use a `CategorizedDiagnostic` type that adds structured error
classification:

```rust
struct CategorizedDiagnostic {
    kind: String,     // e.g., "drc.clearance.copper_to_copper"
    // ... inherits from Diagnostic
}
```

The `kind` is a dotted string path that enables:
- **Filtering** — suppress all `drc.clearance.*` warnings
- **Grouping** — count violations by category in summary tables
- **Machine parsing** — stable identifiers for CI/CD integration

**Relevance to us:** Our `ViolationKind` enum serves a similar purpose but
is less flexible. A dotted-path kind string would support user-defined
rule kinds without enum changes. Tradeoff: enum is type-safe and exhaustive,
string is flexible and extensible. We chose enum — right for now.

---

## Summary Table Rendering

After printing individual diagnostics, the system shows a summary table:

```
╭────────────┬───────┬──────────┬────────╮
│ Kind       │ Error │ Warning  │ Advice │
├────────────┼───────┼──────────┼────────┤
│ clearance  │     3 │        0 │      0 │
│ width      │     1 │        0 │      0 │
│ drill      │     0 │        2 │      0 │
╰────────────┴───────┴──────────┴────────╯
```

Uses the `comfy_table` crate for formatted terminal tables.

**Relevance to us:** A summary table grouped by `ViolationKind` would be a
useful CLI output format. We could add this without changing the core DRC
types — it's a presentation concern.

---

## KiCad DRC Report Parser

The `pcb-kicad` crate includes a parser for KiCad's JSON DRC report format:

```rust
struct DrcReport {
    violations: Vec<DrcViolation>,
    unconnected_items: Vec<DrcViolation>,
    schematic_parity: Vec<DrcViolation>,
}

struct DrcViolation {
    violation_type: String,    // "clearance_violation", etc.
    severity: String,          // "error", "warning"
    description: String,
    items: Vec<DrcItem>,       // involved board items with positions
    excluded: bool,            // user-waived
}
```

This is notable because it:
- Separates violations into categories (copper DRC, connectivity, schematic parity)
- Supports excluded/waived violations
- Uses string-typed violation kinds (not enum) for KiCad compatibility

**Relevance to us:** If we ever need to import/compare with KiCad DRC results,
this format is well-defined. Our `ViolationKind` enum variants roughly map to
KiCad's `violation_type` strings.

---

## Starlark Configuration Language

The `pcb` tool uses Starlark (a deterministic, Pythonic language by Google)
for board design description:

```python
# Starlark board definition
component("U1", "ESP32-S3-WROOM-1")
component("R1", "0402_100k")

net("VCC", U1.VCC, R1.pin1)
net("GND", U1.GND, R1.pin2)

# DRC-relevant configuration
drc_rules(
    min_clearance = 0.15,
    min_trace_width = 0.15,
)
```

Advantages:
- Deterministic evaluation (no side effects, hermetic)
- Familiar Python-like syntax
- Can be statically analyzed (LSP support)
- Integrated documentation generation

**Our approach:** We use a custom DSL (`.cypcb` files) rather than Starlark.
Our DSL is more domain-specific — purpose-built for PCB semantics rather than
embedding PCB concepts in a general-purpose language. Tradeoff: less familiar
syntax, but tighter integration with our type system.

---

## Architectural Patterns Worth Noting

1. **Unified diagnostics pipeline** — one type for all diagnostic messages,
   not separate error types per subsystem. Reduces code duplication, enables
   shared filtering/rendering.

2. **Pass-based filtering** — separates diagnostic generation from presentation.
   Clean extension point for adding severity-based filtering, CI integration,
   IDE reporting.

3. **Compact rendering with kind paths** — dotted-path classification enables
   both human-readable display and machine filtering.

4. **Summary tables** — aggregated view of violation counts by category and
   severity. Useful for CI/CD pass/fail decisions.

5. **Severity as first-class** — not all problems are equal. Advice-level
   diagnostics encourage best practices without blocking builds.

---

## Sources

- `pcb` repository: publicly available source code by Diode Computers, Inc.
- Starlark specification: <https://github.com/bazelbuild/starlark>

**License:** The `pcb` tool's source was analyzed from a publicly available
repository. No code was copied into CodeYourPCB. Our implementation is
independently designed and written.
