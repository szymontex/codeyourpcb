---
status: gap_closed
phase: 05-intelligence
source: 05-01-SUMMARY.md, 05-02-SUMMARY.md, 05-03-SUMMARY.md, 05-04-SUMMARY.md, 05-05-SUMMARY.md, 05-06-SUMMARY.md, 05-07-SUMMARY.md, 05-08-SUMMARY.md, 05-09-SUMMARY.md, 05-11-SUMMARY.md
started: 2026-01-22T12:00:00Z
updated: 2026-01-28T13:30:00Z
gap_closure_plan: 05-11-PLAN.md
---

## Current Test

[testing complete]

## Tests

### 1. Manual Trace DSL Syntax
expected: Parser accepts manual trace definition syntax without errors. Run `cargo run -p cypcb-cli -- check` on a file with trace block.
result: pass

### 2. Net Current Constraint Syntax
expected: Parser accepts `current 500mA` or `current 2A` in net blocks. Check command reports no errors.
result: issue
reported: "Syntax error: unexpected token: 'current 500mA' - parser rejects the current constraint syntax"
severity: major

### 3. DSN Export for FreeRouting
expected: Running `cargo run -p cypcb-cli -- route examples/blink.cypcb --dry-run` exports a .dsn file containing board boundary, components, nets, and padstacks.
result: pass

### 4. Viewer Shows Traces
expected: Open viewer with a board containing traces. Traces render as copper-colored polylines at their actual width. Top layer traces appear red, bottom blue.
result: pass

### 5. Viewer Shows Vias
expected: Vias render as copper-colored circles with drill holes visible in the center.
result: pass

### 6. Ratsnest Toggle
expected: Toggle Ratsnest checkbox in viewer toolbar. When enabled, unrouted net connections appear as gold dashed lines. When disabled, they disappear.
result: pass

### 7. Route Button in Viewer
expected: Click Route button in viewer toolbar. Progress overlay appears with pass count, routed/unrouted status, and elapsed time. Cancel button appears.
result: pass

### 8. Trace Width Calculator
expected: Create a net with `current 1A` constraint. The IPC-2221 formula should calculate ~0.25mm trace width for external layer. Verify via hover info in LSP (if working) or test output.
result: pass

## Summary

total: 8
passed: 7
issues: 1
pending: 0
skipped: 0

## Gaps

- truth: "Parser accepts `current 500mA` or `current 2A` in net blocks"
  status: closed
  reason: "User reported: Syntax error: unexpected token: 'current 500mA' - parser rejects the current constraint syntax"
  severity: minor
  test: 2
  root_cause: "NOT A BUG - SYNTAX MISMATCH. Constraints must be in square brackets BEFORE braces: `net VCC [current 500mA] { pins }` not inside body."
  artifacts:
    - path: "crates/cypcb-parser/grammar/grammar.js"
      lines: "150-171"
      issue: "Grammar designed with constraint block before braces, working as intended"
  resolution: "Plan 05-11 created comprehensive DSL syntax documentation (docs/SYNTAX.md) and updated example files to demonstrate correct constraint syntax"
  fixed_by: "05-11-PLAN.md"
  debug_session: ".planning/debug/parser-current-syntax.md"
