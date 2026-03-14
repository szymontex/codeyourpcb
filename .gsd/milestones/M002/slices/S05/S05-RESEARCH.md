# S05 Research: DSL v2 — Modules, Units & Constraints

**Researched:** 2026-03-13

## Requirements Mapping

This slice owns or supports:
- No explicit requirements currently mapped, but it directly enables reusable circuit design (modules/imports), physical-units-first workflow (typed values like `10kohm`, `3.3V`), and constraint-driven design (assert statements). These are foundational for DOC-01, EDIT-02, EDIT-03, and the overall DSL evolution toward atopile parity.

## Scope

**Goal:** Extend the `.cypcb` grammar and parser to support modules, typed interfaces, physical units, and constraint assertions — while all existing v1 `.cypcb` files continue to parse identically.

**Concrete deliverables:**
1. Extended Tree-sitter grammar (`grammar.js`) with new rules
2. New AST node types in `cypcb-parser/src/ast.rs`
3. Extended parser (`cypcb-parser/src/parser.rs`) to convert new CST nodes
4. New error variants in `cypcb-parser/src/errors.rs`
5. Updated Monaco tokenizer (`viewer/src/editor/cypcb-language.ts`)
6. Updated LSP completions/hover for new keywords
7. Backward compatibility test suite (all 10 existing `.cypcb` examples pass unchanged)
8. Forward compatibility examples (new v2 `.cypcb` files exercising all features)

## Existing Code Analysis

### Current Grammar (v1)

The Tree-sitter grammar in `crates/cypcb-parser/grammar/grammar.js` supports:
- `version N` — version declaration
- `board name { size, layers, stackup }` — board definition
- `component REFDES type "footprint" { value, at, rotate, pin=net }` — components
- `net NAME [constraints] { pin_refs }` — nets with optional width/clearance/current constraints
- `footprint NAME { description, pad, courtyard }` — custom footprints
- `zone/keepout NAME { bounds, layer, net }` — zones
- `trace NET { from, to, via, layer, width, locked }` — manual traces
- Units: `mm`, `mil`, `in`, `nm` for dimensions; `mA`, `A` for current
- Comments: `//` line, `/* */` block

**Key observations:**
- Grammar uses `word: $ => $.identifier` for keyword optimization — new keywords are automatically handled
- All definitions are top-level (no nesting) — modules will need a block containing definitions
- The `identifier` regex is `[a-zA-Z_][a-zA-Z0-9_]*` — no dots or colons, so qualified names like `std.I2C` need new rules
- Dimension currently only supports length units — new physical units (ohms, volts, farads) need extending
- Net constraints are bracketed `[width 0.3mm]` — constraint assertions need a different syntax

### Current AST

`crates/cypcb-parser/src/ast.rs` has ~600 lines of typed AST nodes. Key types:
- `SourceFile { version, definitions, span }` — root, holds `Vec<Definition>`
- `Definition` enum: Board, Component, Net, Footprint, Zone, Trace
- `Dimension { value: f64, unit: Unit, span }` — only supports length units
- `NetConstraints { width, clearance, current }` — hardcoded constraint fields

**Impact:** Adding modules means `Definition` gets a new variant. Import needs a new top-level construct. Physical units need a `PhysicalValue` type broader than `Dimension`. Constraints need expression-level AST nodes.

### Current Parser

`crates/cypcb-parser/src/parser.rs` (~1700 lines) converts Tree-sitter CST to AST using a visitor pattern. Each `convert_*` method handles one grammar rule. Error recovery collects errors and continues.

**Pattern to follow:** Add `convert_module_definition()`, `convert_import_statement()`, `convert_assert_statement()`, `convert_interface_definition()` following the existing structure.

### Current Units System

`crates/cypcb-core/src/units.rs` defines `Unit { Mm, Mil, Inch, Nm }` with `to_nm()` / `from_nm()` conversions. All internal values are `Nm` (nanometer integers, i64).

**Challenge:** Physical units like ohms, volts, farads are fundamentally different from length units. They don't convert to nanometers. Options:
1. New `PhysicalUnit` enum alongside `Unit` (separate type for electrical values)
2. Extend `Unit` to include all physical units (breaks the `to_nm` contract)
3. Keep `Unit` for lengths, add `ElectricalValue` as a separate concept

Option 1 is cleanest — it preserves the existing Nm precision for lengths while allowing typed electrical values.

### Downstream Consumers

Files that consume the parser AST and would need updates:
- `crates/cypcb-world/src/sync.rs` — AST→ECS sync (must handle new Definition variants gracefully)
- `crates/cypcb-lsp/src/completion.rs` — completions for new keywords
- `crates/cypcb-lsp/src/hover.rs` — hover info for new constructs
- `crates/cypcb-lsp/src/diagnostics.rs` — validation of new syntax
- `crates/cypcb-render/src/lib.rs` — WASM bridge (uses parser in native mode)
- `viewer/src/editor/cypcb-language.ts` — Monaco syntax highlighting

**Important:** `sync_ast_to_world()` dispatches on `Definition` variants with a match. New variants will cause a compile error until handled. This is actually good — the compiler enforces completeness.

### Autorouter Constraint Interface

`crates/cypcb-rules/src/routing_rules.rs` defines `RoutingRuleSet` trait with:
- `constraints_for_net(net_id)` → `&DesignConstraints`
- `constraints_for_class(class)` → `SignalClassConstraints`
- `via_cost()`, `layer_change_cost()`, `clearance_between()`

DSL constraints should eventually drive these. The boundary is: DSL constraints → parsed into AST → resolved during sync → stored as per-net overrides in the routing rule set. S05 handles the first two stages; wiring to the autorouter is S06/S07.

## Competitor Analysis

### atopile DSL

Atopile uses an ANTLR grammar (Python-style indentation, colons, `new` keyword):
```
module ESP32_MINIMAL:
    micro = new ESP32_C3_MINI_1
    ldo_3V3 = new TI_TLV75901
    power_3v3 = new ElectricPower
    power_3v3 ~ micro.power
    ldo_3V3.v_out = 3.3V +/- 3%
    assert resistor.resistance within 10kohm +/- 10%
```

Key features:
- `module`, `component`, `interface` as block types
- `import` / `from "path" import Name` for imports
- `new` keyword for instantiation (optionally with arrays: `new Resistor[3]`)
- `~` for wire connections, `~>` / `<~` for directed connections
- `assert X within Y +/- Z%` for constraint assertions
- Physical units: `3.3V`, `10kohm`, `100nF`, `50ohm` — just NAME tokens parsed as units
- `for x in iterable: block` for iteration
- Python-style indentation for blocks

**Our differentiator:** We use braces (not indentation), which is more familiar to C/Rust/JS developers and avoids indentation-sensitivity headaches. We should keep our brace-based syntax while adopting the best semantic features.

### diodeinc/pcb (Starlark-based)

Uses `.zen` files with a Starlark (Python-like) DSL:
```
Resistor = Module("@stdlib/generics/Resistor.zen")
Resistor(name="R1", value="1kohm", package="0402", P1=vcc, P2=led_anode)
```

Key features:
- Full programming language (Starlark = Python subset)
- `interface()` for typed bus definitions (I2C, SPI, USB, etc.)
- `Net()`, `Power()`, `Ground()` as typed net constructors
- Comprehensive stdlib with 40+ interface types

**Insight:** Their interface stdlib (`interfaces.zen`) is a goldmine of PCB bus type definitions. We should define equivalent typed interfaces but in our brace syntax.

## Proposed DSL v2 Design

### Version Declaration
```cypcb
version 2
```
Parser checks version and enables v2 features. Version 1 files continue working as-is.

### Import System
```cypcb
import "std/interfaces.cypcb"           // import all from file
import I2C from "std/interfaces.cypcb"  // import specific name
import I2C, SPI from "std/interfaces.cypcb"  // multiple names
```

Grammar addition: `import_statement` as new top-level construct alongside `_definition`.

### Module System
```cypcb
module PowerSupply {
    // Contains component, net, constraint definitions
    component U1 ic "SOT-23" { value "LDO-3V3" }
    component C1 capacitor "0402" { value "100nF" }

    // Exposed interface pins
    pin VIN
    pin VOUT
    pin GND

    net input { VIN, U1.1, C1.1 }
    net output { VOUT, U1.2 }
    net ground { GND, U1.3, C1.2 }
}
```

Then used:
```cypcb
component PSU1 PowerSupply {
    at 10mm, 15mm
}
```

### Interface System
```cypcb
interface I2C {
    pin SDA
    pin SCL
}

interface Power {
    pin VCC
    pin GND
}
```

Interfaces are structural types — they define a set of named pins that can be connected as a group.

### Physical Units
```cypcb
component R1 resistor "0402" {
    value 10kohm         // was: value "10k"
    at 10mm, 8mm
}
component C1 capacitor "0402" {
    value 100nF          // was: value "100nF" (string)
}
```

New physical unit categories (parsed as `PhysicalValue`):
- **Resistance:** `ohm`, `kohm`, `Mohm` (Ω, kΩ, MΩ)
- **Capacitance:** `pF`, `nF`, `uF`, `mF` (picofarads → millifarads)
- **Inductance:** `nH`, `uH`, `mH`, `H` (nanohenries → henries)
- **Voltage:** `mV`, `V`, `kV`
- **Current:** `uA`, `mA`, `A` (already partially supported)
- **Frequency:** `Hz`, `kHz`, `MHz`, `GHz`
- **Power:** `mW`, `W`

The grammar already accepts `number + unit_text` for dimensions. We extend the `unit` rule to include these, and add a new `physical_value` AST node that carries a normalized base value.

### Constraint Assertions
```cypcb
assert R1.value within 10kohm +/- 10%
assert net VCC width >= 0.3mm
assert net GND clearance >= 0.2mm
assert board.layers == 4
```

This adds `assert_statement` as a new top-level construct with comparison operators and tolerance syntax.

### Tolerance Syntax
```cypcb
value 10kohm +/- 5%      // percentage tolerance
value 3.3V +/- 0.1V      // absolute tolerance
value 100nF to 220nF      // range
```

## Backward Compatibility Strategy

**Critical constraint:** All 10 existing `.cypcb` files (423 lines total) must parse identically.

**Strategy:**
1. All new constructs use new keywords (`module`, `interface`, `import`, `assert`, `pin` at module level) that don't conflict with existing grammar
2. `version 1` files use existing grammar path; `version 2` files enable new features
3. Physical units are additive — existing `dimension` rule already accepts optional units; new units just add to the `unit` choice
4. The `value` property currently takes a string literal (`value "10k"`). v2 adds `physical_value` as an alternative, but strings remain valid
5. Tree-sitter's error recovery means partially-valid v2 syntax in a v1-context degrades gracefully
6. Existing `_definition` choice is extended (not replaced) with new variants

**Risk:** The word `pin` is already used in `pin_identifier` (net assignment syntax `pin.1 = NET`). A `pin` declaration at module scope needs disambiguation. Atopile solves this with separate `pin` keyword and `signal` keyword. We can use context: `pin NAME` (declaration) vs `pin.N` (reference).

**Risk:** `import` could collide with a future identifier. Since `word: $ => $.identifier` is set, Tree-sitter treats `import` as a keyword when it matches the grammar rule, and as an identifier otherwise. This is safe.

## Implementation Plan Sketch

### Phase 1: Grammar + AST (core parser changes)
1. Extend `grammar.js` with: `import_statement`, `module_definition`, `interface_definition`, `assert_statement`, `pin_declaration`, `physical_value`, tolerance syntax
2. Run `tree-sitter generate` to regenerate `parser.c`
3. Add new AST types: `ImportDef`, `ModuleDef`, `InterfaceDef`, `AssertDef`, `PinDeclaration`, `PhysicalValue`, `Tolerance`
4. Add `Definition::Module`, `Definition::Interface`, `Definition::Import`, `Definition::Assert` variants
5. Implement `convert_*` methods in parser.rs
6. Add new error variants

### Phase 2: Physical units
1. Add `PhysicalUnit` enum to `cypcb-core/src/units.rs` (or new file)
2. Extend grammar `unit` rule with electrical/frequency units
3. Add `PhysicalValue` AST node (value + unit + optional tolerance)
4. Allow `value` property to accept `physical_value` OR `string`

### Phase 3: Downstream updates
1. Add `_ => {}` arms or explicit handling for new Definition variants in `sync.rs`
2. Update Monaco tokenizer with new keywords
3. Update LSP completions for `module`, `interface`, `import`, `assert`, `pin`
4. Update LSP hover info

### Phase 4: Verification
1. All 10 existing `.cypcb` examples parse identically (snapshot test)
2. New v2 examples parse correctly
3. All existing tests pass
4. New tests cover every v2 grammar construct
5. Round-trip: parse → AST → serialize → verify structure

## Key Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| Grammar conflicts from new keywords | High | Test v1 examples immediately after grammar changes. Tree-sitter's `word` optimization handles keyword/identifier disambiguation. |
| `pin` keyword ambiguity | Medium | Use context: `pin NAME` (declaration in module) vs `pin.N` (reference in nets). May need to rename module pin declarations to `expose` or `port` if conflicts arise. |
| Physical unit parsing conflicts with dimension units | Medium | Physical units are a superset of dimension units. The `dimension` rule stays for positions; a new `physical_value` rule handles component values. |
| Backward compat breakage | High | Dedicated test: parse all 10 existing files, compare AST output byte-for-byte with v1 parser output. Run this test first after every grammar change. |
| WASM parser feature flags | Medium | `cypcb-parser` has `tree-sitter-parser` feature. New AST types should be unconditionally available (they're just data types). Only the Tree-sitter conversion code needs the feature flag. |
| Downstream compile breaks | Low | Adding enum variants to `Definition` will cause compile errors in match statements — this is intentional and ensures all consumers are updated. |

## Technical Constraints

- Tree-sitter grammar changes require running `tree-sitter generate` (needs Node.js + tree-sitter CLI)
- The generated `parser.c` is committed to the repo (no build-time generation)
- Grammar must remain LR(1) parseable — no ambiguities that Tree-sitter can't resolve
- WASM builds disable `tree-sitter-parser` feature — AST types must be available without it
- All dimension fields in the core use `Nm` type (integer nanometers) — electrical units cannot reuse this

## Skill Discovery

**Technologies this slice depends on:**
- **Tree-sitter**: Grammar extension, parser generation. No relevant agent skill found; library docs available via Context7.
- **Rust (parser/AST)**: Standard Rust patterns. Installed skills: `coding-guidelines` (Rust style). Relevant but not critical for this work.
- **Monaco Editor**: Syntax highlighting updates. No relevant agent skill found.

No skills recommended for installation.

## Open Questions

1. **Module instantiation syntax**: Should modules be instantiated like components (`component PSU1 PowerSupply { ... }`) or with a `new` keyword (`psu = new PowerSupply { at 10mm, 15mm }`)? The component syntax is backward-compatible; `new` is more explicit.

2. **Import resolution**: Should imports resolve relative to the file, relative to a project root, or from a registry? Start with file-relative, add project root later.

3. **Constraint evaluation**: S05 parses constraint assertions into AST but doesn't evaluate them. Evaluation (connecting to DRC/autorouter) deferred to S06/S07. Is parse-only sufficient for this slice?

4. **Physical value in `value` property**: Currently `value "10k"` is a string. Should v2 support both `value "10k"` and `value 10kohm`? Yes — strings for backward compat, physical values for new code.
