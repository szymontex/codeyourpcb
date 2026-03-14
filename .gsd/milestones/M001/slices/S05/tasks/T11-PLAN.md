# T11: DSL Syntax Documentation

**Slice:** S05 — **Milestone:** M001

## Description

Document the correct net constraint syntax to close UAT gap

Purpose: Users reported "Syntax error: unexpected token: 'current 500mA'" because they placed constraints inside net braces instead of in square brackets before braces. The grammar is working as designed - this is a documentation gap, not a code bug.

Output: DSL syntax reference doc + updated example files demonstrating correct usage

## Must-Haves

- [ ] "Documentation explains net constraint syntax with square brackets"
- [ ] "At least one example file demonstrates current constraint usage"
- [ ] "Syntax is clear enough for users to write correct constraints"

## Files

- `docs/SYNTAX.md`
- `examples/power-indicator.cypcb`
- `examples/blink.cypcb`
