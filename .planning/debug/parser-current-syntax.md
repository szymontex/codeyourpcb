---
status: diagnosed
trigger: "Diagnose why the parser rejects `current 500mA` syntax in net blocks"
created: 2026-01-22T00:00:00Z
updated: 2026-01-22T00:02:00Z
symptoms_prefilled: true
goal: find_root_cause_only
---

## Current Focus

hypothesis: CONFIRMED - Syntax mismatch between user expectation and grammar design
test: Verified with both syntaxes
expecting: N/A - Root cause found
next_action: Report findings

## Symptoms

expected: Parser accepts `current 500mA` syntax inside net blocks
actual: Parser returns "Syntax error: unexpected token: 'current 500mA'"
errors: "Syntax error: unexpected token: 'current 500mA'"
reproduction: Add `current 500mA` inside a net definition block
started: Unknown - 05-01-SUMMARY claims it was implemented

## Eliminated

## Evidence

- timestamp: 2026-01-22T00:01:00Z
  checked: grammar.js lines 150-171 (net_definition and net_constraint_block)
  found: |
    Net constraints use square bracket syntax BEFORE the braces:
    - net_definition: seq('net', name, optional(net_constraint_block), '{', pin_ref_list, '}')
    - net_constraint_block: seq('[', repeat(net_constraint), ']')
    - current_constraint rule EXISTS at lines 186-189

    Expected syntax: net VCC [current 500mA] { ... }
    User's syntax:   net VCC { ... current 500mA }
  implication: User is placing constraint inside braces; grammar only accepts constraints in square brackets before braces

- timestamp: 2026-01-22T00:02:00Z
  checked: Ran parser test with both syntaxes
  found: |
    User syntax (inside braces): FAILS with "unexpected token: 'current 500mA'"
    Correct syntax (square brackets): PASSES
  implication: Confirms grammar design, not a bug - documentation/syntax mismatch

## Resolution

root_cause: |
  NOT A BUG - SYNTAX MISMATCH

  The grammar is implemented correctly. The `current` constraint IS supported,
  but it must appear in square brackets BEFORE the net body braces, not inside them.

  Grammar design (grammar.js lines 150-158):
    net_definition: seq('net', name, optional(net_constraint_block), '{', pin_ref_list, '}')

  CORRECT SYNTAX:
    net VCC [current 500mA] {
        R1.1
        C1.1
    }

  INCORRECT (user's attempt):
    net VCC {
        R1.1
        C1.1
        current 500mA   // ERROR - constraints go in [ ] before braces
    }

fix: N/A - This is a documentation issue, not a code bug
verification: N/A
files_changed: []
