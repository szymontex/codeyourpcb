# atopile Constraint Solver Architecture

Analysis of atopile's constraint system — a fundamentally different approach
to design rule validation compared to traditional DRC. Where KiCad and our
`cypcb-drc` check rules *after* layout (post-hoc), atopile validates
constraints *before* and *during* design resolution (pre-solve).

**Source:** atopile is open-source under MIT license.
Repository: <https://github.com/atopile/atopile>
Analysis based on atopile codebase as of 2026-03.

**License note:** atopile is MIT licensed. No code was copied. This analysis
describes architectural patterns observed from the public repository. Our
constraint handling is independently implemented in Rust.

---

## Architecture Overview

atopile is a code-first hardware description language. Unlike traditional
EDA tools where you draw a schematic and then run DRC, atopile lets you
*declare constraints* in code and the compiler *solves* them.

### The Key Difference: Solver vs. Checker

**Traditional DRC (KiCad, us):**
```
1. User designs board (placement, routing)
2. DRC checks design against rules
3. Violations reported
4. User fixes violations manually
5. Repeat until clean
```

**atopile constraint solver:**
```
1. User declares constraints in code
2. Compiler resolves constraints against component database
3. If constraints are satisfiable → proceed with valid design
4. If contradictory → compile error (before any layout happens)
```

This is a fundamentally different point in the design lifecycle — atopile
catches constraint violations at compile time, not at DRC time.

### What atopile Constrains

atopile's constraints are primarily about **component selection**, not
**physical layout**:

```python
# atopile constraint examples
resistor.resistance = 10kohm +/- 10%
capacitor.capacitance = 100nF +/- 20%
regulator.output_voltage = 3.3V +/- 5%

# The compiler finds real components from LCSC that match
assert resistor.resistance within 10kohm +/- 10%
```

This is out of scope for our PCB DRC — we handle physical layout rules
(clearance, trace width, drill size), not component selection.

---

## Default Override Pattern

The most relevant pattern from atopile for our work: **defaults that yield
to explicit constraints**.

### has_default_constraint Trait

```python
class has_default_constraint(Node):
    """
    Marks a parameter with a default constraint value.
    The default is only applied if no explicit constraint exists.
    """
```

The behavior:
1. Package author sets a default: `max_current.default = 1A`
2. If the user doesn't constrain `max_current`, the default applies
3. If the user writes `device.max_current = 500mA`, their constraint
   takes precedence — no contradiction

### Implementation

The `has_default_constraint` trait registers a design check that runs
*before* the constraint solver:

```python
# Pseudocode of the default application logic:
def apply_default(parameter):
    # Check if user set an explicit constraint
    existing_constraints = parameter.get_constraints(IsSubset, Is)
    
    if existing_constraints:
        # User constraint exists — skip default
        return
    
    # No explicit constraint — apply default
    parameter.add_constraint(IsSubset(default_value))
```

Key insight: **defaults are applied as a pre-solve pass**, not baked into
the constraint values. This means:
- Defaults can be inspected separately from user constraints
- The solver sees a uniform set of constraints (doesn't need to know which
  are defaults)
- No "contradiction by literal" errors from conflicting defaults and user values

**Relevance to us:** Our `PresetRuleSet::with_net_override()` follows this
same principle. The preset provides defaults, and per-net overrides take
precedence. The pattern is solid — we just implement it differently (explicit
override map vs. implicit default resolution).

---

## Constraint Types

atopile uses a type hierarchy for constraint values:

### Literal Types

- **Numbers** — `10kohm`, `3.3V`, `100nF` (with physical units)
- **Ranges** — `10kohm +/- 10%`, `1V to 5V`
- **Strings** — `"0402"`, `"SOIC-8"`
- **Booleans** — `true`, `false`
- **Enums** — component-specific enumerated values

### Constraint Operations

| Operation | Meaning | Example |
|-----------|---------|---------|
| `=` | Exact equality | `r.resistance = 10kohm` |
| `within` | Range membership | `assert r.resistance within 10kohm +/- 10%` |
| `>=`, `<=` | Bounds | `assert v.voltage >= 3.0V` |

### Design Checks

atopile has a design check system that runs validations at specific lifecycle
points:

```python
class implements_design_check(Node):
    """Design check trait — validators that run at specific points."""
    
    # Check points:
    PRE_SOLVE = "pre_solve"           # Before constraint solving
    POST_SOLVE = "post_solve"         # After constraint solving
    POST_INSTANTIATION = "post_inst"  # After component instantiation
```

This is more nuanced than our "run all DRC rules" approach — atopile can
validate constraints at different stages of the design process.

---

## Graph-Based Type System

atopile uses a graph (node/edge) model for representing hardware:

```python
class Node:
    """Base type for all hardware objects."""
    # Children, traits, parameters are all graph edges
    
class Component(Node):
    """Physical component with parameters."""
    
class Module(Node):
    """Reusable hardware module (collection of components)."""
    
class Interface(Node):
    """Typed connection interface (I2C, SPI, USB, Power)."""
```

Key properties:
- **Inheritance** — modules inherit parameters from base types
- **Type checking** — connections between incompatible interfaces are errors
- **Parameterization** — modules are templates with constraint parameters

This is a richer type system than our flat `Component` + `Net` model.
atopile's approach enables reusable hardware modules with type-safe
interfaces — closer to software engineering patterns.

**Relevance to us:** Our DSL doesn't have inheritance or typed interfaces.
These are powerful features for reusable designs, but they add significant
language complexity. We could adopt typed interfaces in the future without
changing our DRC/constraint architecture.

---

## MCP Server Integration

atopile provides a Model Context Protocol (MCP) server:

```bash
ato mcp  # starts MCP server
```

This allows AI agents to:
- Query project structure
- Modify constraints
- Run builds
- Inspect errors

**Relevance to us:** An MCP server for CodeYourPCB would enable AI-assisted
PCB design — the agent could adjust routing rules, run DRC, and iterate on
layout. This is architecturally independent of our DRC system but worth
noting as a direction.

---

## Comparison: atopile vs. Our Approach

| Aspect | atopile | CodeYourPCB |
|--------|---------|-------------|
| Constraint timing | Pre-solve (compile time) | Post-hoc (after layout) |
| Constraint scope | Component selection + parameters | Physical layout rules |
| Rule language | Embedded in ato DSL | Rust trait objects |
| Defaults | Implicit override pattern | Explicit preset + per-net overrides |
| Type system | Graph-based with inheritance | Flat structs with enum classification |
| Solver | SAT-like constraint satisfaction | Rule iteration with violation collection |
| Output | Valid component selection | Violation list with locations |
| Physical DRC | Delegated to KiCad | Built-in (`cypcb-drc`) |
| Standalone | No (requires KiCad for layout/DRC) | Yes (full self-contained pipeline) |

### Key Insight

atopile and our DRC are complementary, not competing:
- atopile ensures the *right components* are selected for the *right constraints*
- Our DRC ensures the *physical layout* meets *manufacturing and signal integrity rules*

atopile doesn't have its own physical DRC — it delegates to KiCad. We don't
have component selection — we take a BOM as input. The two approaches solve
different problems.

---

## Architectural Patterns Worth Noting

1. **Pre-solve validation** — catching constraint violations before expensive
   operations (layout, routing). We could add pre-route validation that checks
   whether the constraint set is even satisfiable before starting the autorouter.

2. **Default override pattern** — clean separation between library defaults
   and user customization. Already adopted in our `PresetRuleSet`.

3. **Staged design checks** — different validations at different lifecycle
   points (pre-solve, post-solve, post-instantiation). More nuanced than our
   single DRC pass. Worth considering for multi-stage validation.

4. **Physical units in the type system** — `10kohm`, `3.3V` as typed values,
   not strings. Our `Nm` type provides this for dimensions, but we don't have
   typed electrical values. The `cypcb-core` crate could be extended.

5. **Package registry + community modules** — reusable hardware definitions
   with constraint inheritance. A future ecosystem feature, independent of DRC.

---

## Sources

- atopile repository: <https://github.com/atopile/atopile>
- atopile documentation: <https://docs.atopile.io>
- atopile package registry: <https://packages.atopile.io>
- Competitor analysis: `/workspace/competitors/atopile-vs-us.md`
- faebryk library (atopile's Python core): `src/faebryk/library/`

**License:** atopile is MIT licensed. This analysis describes architectural
patterns for educational purposes. No atopile code was used in CodeYourPCB.
