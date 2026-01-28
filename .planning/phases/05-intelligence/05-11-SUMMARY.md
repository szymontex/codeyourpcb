---
phase: 05-intelligence
plan: 11
subsystem: documentation
completed: 2026-01-28
duration: 100s

requires:
  - 05-01 (Trace & Via ECS + DSL extensions with current constraint)
  - Tree-sitter grammar (net constraint block syntax)

provides:
  - DSL syntax reference documentation
  - Example files demonstrating correct net constraint syntax
  - Clear explanation of square bracket placement for constraints

affects:
  - User documentation experience
  - UAT gap closure (syntax error misunderstanding)
  - Future DSL learning curve

tech-stack:
  added: []
  patterns:
    - Documentation-first gap closure
    - Example-driven learning

key-files:
  created:
    - docs/SYNTAX.md (418 lines - comprehensive DSL reference)
  modified:
    - examples/power-indicator.cypcb (added current and width constraints)
    - examples/blink.cypcb (added current constraint)

decisions:
  - id: DOC-01
    title: Place net constraint examples early in documentation
    choice: Document net constraints as subsection of net definition
    rationale: Users encounter nets early, need immediate clarity on constraint syntax
    alternatives:
      - Separate constraints section (harder to discover)
      - Only in examples (no reference text)

  - id: DOC-02
    title: Show both correct and incorrect syntax
    choice: Include side-by-side CORRECT vs INCORRECT examples
    rationale: Users already attempted incorrect syntax, explicit contrast prevents confusion
    alternatives:
      - Only show correct syntax (doesn't address existing user misconception)

  - id: DOC-03
    title: Add constraints to existing examples
    choice: Update blink.cypcb and power-indicator.cypcb with constraints
    rationale: Users already reference these files, zero friction learning path
    alternatives:
      - Create new constraint-focused example (users might not discover it)

tags: [documentation, DSL, syntax, net-constraints, gap-closure, user-experience]
---

# Phase 05 Plan 11: DSL Syntax Documentation Summary

**One-liner:** Comprehensive DSL syntax reference with clear net constraint bracket placement examples to close UAT gap

## Overview

This plan closed a critical documentation gap identified during UAT. Users attempted to place net constraints inside braces (`net VCC { current 500mA }`) but the grammar requires square brackets before braces (`net VCC [current 500mA] { }`). The parser was working correctly - this was purely a documentation issue.

Created a comprehensive DSL syntax reference and updated example files to demonstrate the correct constraint syntax, preventing future user confusion.

## What Was Built

### 1. DSL Syntax Reference (docs/SYNTAX.md)

Created a 418-line comprehensive reference document covering all DSL constructs:

**Major Sections:**
- Version declaration
- Board definition (size, layers, stackup)
- Component definition (all types, properties, inline net assignment)
- **Net definition with detailed constraint syntax explanation**
- Zone definition (keepouts and copper pours)
- Trace definition (manual routing)
- Custom footprint definition
- Comments and units

**Net Constraint Documentation:**
- Clear section titled "Net with Constraints"
- Side-by-side CORRECT vs INCORRECT examples
- Emphasis on square bracket placement: `net VCC [current 500mA] { pins }`
- Documentation of all three constraint types: `current`, `width`, `clearance`
- Multiple constraint example: `net VCC [current 500mA width 0.4mm clearance 0.3mm]`
- Common mistakes section highlighting the bracket issue

**Key Feature:**
Explicit contrast showing the exact user error:
```
CORRECT - Constraints in square brackets before braces:
net VCC [current 500mA] {
    R1.1
}

INCORRECT - Constraints cannot go inside braces:
net VCC {
    R1.1
    current 500mA  // ERROR: unexpected token
}
```

### 2. Updated Example Files

**power-indicator.cypcb:**
- Added `[current 100mA width 0.3mm]` constraint to VCC net
- Demonstrates multiple constraints in single bracket block
- Includes explanatory comment: "Net constraints go in square brackets BEFORE the braces"
- Validates successfully with constraints

**blink.cypcb:**
- Added `[current 20mA]` constraint to VCC net
- Simple single-constraint example for basic learning
- Includes same explanatory comment
- Validates successfully

Both files serve as working references users can copy from.

## Technical Decisions

### DOC-01: Place net constraint examples early in documentation

**Decision:** Document net constraints as a subsection of the net definition section, not in a separate advanced topics area.

**Rationale:** Users encounter net definitions early in their DSL learning journey. If they need to add a constraint, the syntax explanation must be immediately visible in the same section, not buried elsewhere.

**Implementation:** Created "Net with Constraints" as a subsection directly under "Net Definition" with prominent visibility.

### DOC-02: Show both correct and incorrect syntax

**Decision:** Include explicit side-by-side CORRECT vs INCORRECT examples with the exact error users encountered.

**Rationale:** Users have already attempted the incorrect syntax and received confusing error messages. Simply showing the correct way doesn't address their mental model. Explicitly contrasting the two prevents the same mistake.

**Impact:** Documentation directly addresses the reported UAT issue: "Syntax error: unexpected token: 'current 500mA'"

### DOC-03: Add constraints to existing examples

**Decision:** Update existing example files (blink.cypcb, power-indicator.cypcb) rather than creating a new constraint-focused example.

**Rationale:**
- Users already reference blink.cypcb as the canonical simple example
- Power-indicator.cypcb is a logical place for current constraints (actual power circuit)
- Zero friction: users don't need to discover a new file
- Keeps the example count manageable

## Implementation Notes

### Grammar Syntax (for reference)

The Tree-sitter grammar correctly implements the constraint syntax:

```javascript
net_definition: seq(
  'net',
  field('name', $.identifier),
  optional($.net_constraint_block),  // Optional [ ] block
  '{',
  optional($.pin_ref_list),
  '}'
)

net_constraint_block: seq('[', repeat($.net_constraint), ']')
net_constraint: choice($.width_constraint, $.clearance_constraint, $.current_constraint)
```

This was implemented in Phase 05-01. The documentation gap was the only issue.

### Validation Success

All example files validate successfully:
```bash
$ cargo run -p cypcb-cli -- check examples/power-indicator.cypcb
OK: examples/power-indicator.cypcb validated successfully

$ cargo run -p cypcb-cli -- check examples/blink.cypcb
OK: examples/blink.cypcb validated successfully
```

Both files parse without errors, confirming the syntax is correct.

## Files Changed

### Created Files

**docs/SYNTAX.md** (418 lines)
- Table of contents with all DSL constructs
- Comprehensive net constraint documentation
- Common mistakes section
- Examples for every construct
- References to example files

### Modified Files

**examples/power-indicator.cypcb**
- Added `[current 100mA width 0.3mm]` to VCC net (line 38)
- Added explanatory comment about bracket syntax

**examples/blink.cypcb**
- Added `[current 20mA]` to VCC net (line 19)
- Added explanatory comment about bracket syntax

## Deviations from Plan

None - plan executed exactly as written.

All three tasks completed successfully:
1. ✓ Created DSL syntax reference with net constraint emphasis
2. ✓ Updated power-indicator.cypcb with constraint examples
3. ✓ Updated blink.cypcb with constraint example

All verification criteria met:
- ✓ docs/SYNTAX.md exists with comprehensive documentation
- ✓ Net constraint syntax clearly explained with bracket placement warning
- ✓ power-indicator.cypcb validates successfully
- ✓ blink.cypcb validates successfully
- ✓ Examples demonstrate current and width constraint types

## Testing

### Validation Testing

All example files validate successfully without errors:

```bash
cargo run -p cypcb-cli -- check examples/power-indicator.cypcb
# OK: examples/power-indicator.cypcb validated successfully

cargo run -p cypcb-cli -- check examples/blink.cypcb
# OK: examples/blink.cypcb validated successfully
```

### Documentation Verification

Confirmed documentation contains:
- ✓ 5 instances of `[current` examples
- ✓ Clear "INCORRECT" section with warning
- ✓ All major DSL constructs documented
- ✓ References to example files
- ✓ Common mistakes section

## UAT Gap Closure

This plan directly addresses the UAT issue from Phase 05:

**Original Issue:**
> "Syntax error: unexpected token: 'current 500mA'"
> User attempted: `net VCC { current 500mA }`

**Root Cause:**
Grammar is correct. User placed constraints inside braces instead of in square brackets before braces.

**Resolution:**
- Created comprehensive DSL reference documenting correct syntax
- Added side-by-side correct vs incorrect examples
- Updated example files with working constraint demonstrations
- Clear explanatory comments in example code

**Result:**
Users can now:
1. Reference docs/SYNTAX.md to understand constraint syntax
2. Copy from working examples (blink.cypcb, power-indicator.cypcb)
3. Understand why their original syntax failed
4. Write correct net constraints with confidence

## Next Phase Readiness

Documentation foundation now supports:
- **Phase 05-12:** Language server can reference this syntax in hover/completion
- **Phase 06:** Desktop UI can link to this documentation
- **User Onboarding:** Clear syntax reference reduces learning curve

No blockers introduced. All files validate successfully.

## Impact Assessment

**Positive:**
- Closes UAT gap #1 (net constraint syntax confusion)
- Provides reusable syntax reference for all future users
- Example files now demonstrate more DSL features
- Documentation can be referenced by LSP hover/completion

**Risks:**
- None. Documentation-only change with no code modifications.

**Maintenance:**
- docs/SYNTAX.md must be updated if grammar changes
- Example files should stay synchronized with documentation

## Lessons Learned

1. **Documentation gaps are as critical as bugs:** User attempted valid-looking syntax but received cryptic error. Documentation prevents this entirely.

2. **Example-driven learning is powerful:** Updating existing example files (blink.cypcb) ensures users encounter correct syntax in their natural learning path.

3. **Explicit contrast prevents confusion:** Showing both CORRECT and INCORRECT syntax directly addresses user misconceptions.

4. **Gap closure plan pattern works:** This was a gap_closure plan (gap_closure: true in frontmatter), and the focused scope enabled fast turnaround.

## Commit History

1. **d636143** - docs(05-11): create comprehensive DSL syntax reference
   - Created docs/SYNTAX.md with full DSL documentation
   - Emphasized net constraint square bracket syntax
   - Added CORRECT vs INCORRECT examples

2. **87be472** - feat(05-11): add net constraint example to power-indicator
   - Added `[current 100mA width 0.3mm]` to VCC net
   - Demonstrates multiple constraints in practice
   - File validates successfully

3. **a3746ae** - feat(05-11): add current constraint to blink.cypcb example
   - Added `[current 20mA]` to VCC net
   - Simple single-constraint example
   - File validates successfully

All commits use phase-plan prefix (05-11) for traceability.

## Verification

All success criteria met:

- ✅ DSL syntax reference document created at docs/SYNTAX.md
- ✅ Net constraint syntax documented with clear examples showing square bracket placement
- ✅ Example files updated to demonstrate correct constraint usage
- ✅ All example files pass parser validation
- ✅ Users can reference documentation to understand why `net VCC { current 500mA }` fails

Plan complete in 100 seconds (1m 40s).
