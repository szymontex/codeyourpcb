# CodeYourPCB DSL Syntax Reference

This document describes the syntax of the CodeYourPCB domain-specific language (DSL).

## Table of Contents

1. [Version Declaration](#version-declaration)
2. [Board Definition](#board-definition)
3. [Component Definition](#component-definition)
4. [Net Definition](#net-definition)
5. [Zone Definition](#zone-definition)
6. [Trace Definition](#trace-definition)
7. [Custom Footprint Definition](#custom-footprint-definition)
8. [Comments](#comments)
9. [Units](#units)

## Version Declaration

Every `.cypcb` file should start with a version declaration:

```
version 1
```

This specifies the DSL version and enables future syntax evolution.

## Board Definition

Define the physical PCB properties:

```
board <name> {
    size <width> x <height>
    layers <count>
}
```

**Example:**
```
board my_circuit {
    size 50mm x 30mm
    layers 2
}
```

**Properties:**
- `size`: Board dimensions (width x height) with units
- `layers`: Number of copper layers (2, 4, 6, etc.)

## Component Definition

Place components on the board:

```
component <refdes> <type> "<footprint>" {
    value "<value>"
    at <x>, <y>
    rotate <angle>
    pin.<number> = <net>
}
```

**Example:**
```
component R1 resistor "0402" {
    value "330"
    at 15mm, 10mm
    rotate 90
    pin.1 = VCC
    pin.2 = LED_ANODE
}
```

**Component Types:**
- `resistor`, `capacitor`, `inductor`
- `ic`, `led`, `diode`, `transistor`
- `connector`, `crystal`, `generic`

**Properties:**
- `value`: Component value as string (e.g., "330", "100nF", "ATmega328P")
- `at`: Position in board coordinates (x, y)
- `rotate`: Rotation angle in degrees (optional, defaults to 0)
- `pin.<N> = <NET>`: Inline net assignment for specific pins (optional)

**Footprint Examples:**
- SMD resistors/capacitors: "0402", "0603", "0805", "1206"
- ICs: "SOIC8", "SOIC14", "SOT23", "TQFP32"
- Through-hole: "DIP-8", "PIN-HDR-1x2"

See `examples/` directory for more component examples.

## Net Definition

Define electrical connections between component pins.

### Basic Net Syntax

```
net <name> {
    <component>.<pin>
    <component>.<pin>
    ...
}
```

**Example:**
```
net GND {
    R1.2
    C1.2
    LED1.K
}
```

### Net with Constraints

**IMPORTANT:** Net constraints must be placed in **square brackets BEFORE the braces**.

```
net <name> [<constraint1> <constraint2> ...] {
    <component>.<pin>
    ...
}
```

**CORRECT - Constraints in square brackets before braces:**
```
net VCC [current 100mA width 0.3mm] {
    R1.1
    C1.1
    J1.1
}
```

**INCORRECT - Constraints cannot go inside braces:**
```
net VCC {
    R1.1
    C1.1
    current 100mA  // ERROR: unexpected token
}
```

### Available Constraints

**Current Constraint:**
Specifies the expected current flow through the net.
```
current <value><unit>
```
Units: `mA` (milliamps) or `A` (amps)

Examples:
```
net VCC [current 500mA] { ... }
net MOTOR_POWER [current 2A] { ... }
```

**Width Constraint:**
Specifies the minimum trace width for this net.
```
width <dimension>
```

Example:
```
net VCC [width 0.5mm] { ... }
```

**Clearance Constraint:**
Specifies the minimum clearance to other nets.
```
clearance <dimension>
```

Example:
```
net HIGH_VOLTAGE [clearance 1mm] { ... }
```

**Multiple Constraints:**
You can combine multiple constraints in the same square brackets:
```
net VCC [current 500mA width 0.4mm clearance 0.3mm] {
    R1.1
    C1.1
}
```

**Pin References:**
Pin identifiers can be numbers or names:
- `R1.1`, `R1.2` (numeric pins)
- `LED1.A`, `LED1.K` (named pins: anode, cathode)
- `U1.VCC`, `U1.GND` (IC named pins)

See `examples/power-indicator.cypcb` for net constraint examples.

## Zone Definition

Define keepout areas or copper pour zones:

### Keepout Zone

Prevents component placement in a specific area:
```
keepout <name> {
    bounds <x1>, <y1> to <x2>, <y2>
    layer <layer>
}
```

**Example:**
```
keepout mounting_hole {
    bounds 5mm, 5mm to 8mm, 8mm
    layer all
}
```

### Copper Pour Zone

Defines a copper pour area (future):
```
zone <name> {
    bounds <x1>, <y1> to <x2>, <y2>
    layer <layer>
    net <netname>
}
```

**Layers:**
- `top`: Top copper layer
- `bottom`: Bottom copper layer
- `all`: All layers

## Trace Definition

Manually define routed traces:

```
trace <net> {
    from <component>.<pin>
    to <component>.<pin>
    via <x>, <y>
    layer <layer>
    width <dimension>
    locked
}
```

**Example:**
```
trace VCC {
    from R1.1
    to C1.1
    via 12mm, 10mm
    layer Top
    width 0.3mm
    locked
}
```

**Properties:**
- `from`: Starting pin reference
- `to`: Ending pin reference
- `via`: Waypoint coordinates for routing (optional, can repeat)
- `layer`: Copper layer (Top, Bottom, Inner1-4)
- `width`: Trace width (optional, defaults to DRC minimum)
- `locked`: Prevents autorouter from modifying this trace

Locked traces are preserved during auto-routing and exported as fixed wires.

## Custom Footprint Definition

Define custom footprints inline:

```
footprint <name> {
    description "<text>"
    courtyard <width> x <height>
    pad <number> <shape> at <x>, <y> size <w> x <h> [drill <d>]
    ...
}
```

**Example:**
```
footprint MY_CONNECTOR {
    description "Custom 3-pin connector"
    courtyard 5mm x 3mm

    pad 1 rect at -2mm, 0mm size 1mm x 1.5mm drill 0.8mm
    pad 2 circle at 0mm, 0mm size 1mm x 1mm drill 0.8mm
    pad 3 rect at 2mm, 0mm size 1mm x 1.5mm drill 0.8mm
}
```

**Pad Shapes:**
- `rect`: Rectangular pad
- `circle`: Circular pad
- `roundrect`: Rounded rectangle
- `oblong`: Oval/stadium shape

**Drill:**
- If `drill` is specified, pad is through-hole (THT)
- Without `drill`, pad is surface-mount (SMD)

## Comments

**Line comments:**
```
// This is a line comment
```

**Block comments:**
```
/*
 * This is a block comment
 * spanning multiple lines
 */
```

## Units

All dimensions require explicit units:

- `mm` - millimeters (most common)
- `mil` - thousandths of an inch
- `in` - inches
- `nm` - nanometers (internal precision)

**Examples:**
```
size 50mm x 30mm
at 1.5in, 20mil
width 0.254mm
```

Negative dimensions are supported for pad offsets:
```
pad 1 rect at -1mm, 0mm size 0.5mm x 0.8mm
```

## Example Files

Complete working examples can be found in the `examples/` directory:

- `examples/blink.cypcb` - Simple LED blink circuit
- `examples/power-indicator.cypcb` - Power indicator with current constraints
- `examples/drc-test.cypcb` - DRC rule demonstrations
- `examples/routing-test.cypcb` - Manual trace definitions

## Common Mistakes

### 1. Net Constraints Inside Braces

**Wrong:**
```
net VCC {
    R1.1
    current 500mA  // ERROR!
}
```

**Correct:**
```
net VCC [current 500mA] {
    R1.1
}
```

### 2. Missing Units

**Wrong:**
```
at 15, 10  // Missing units
```

**Correct:**
```
at 15mm, 10mm
```

### 3. Unquoted Footprint Names

**Wrong:**
```
component R1 resistor 0402 { ... }  // Missing quotes
```

**Correct:**
```
component R1 resistor "0402" { ... }
```

## Validation

Validate your `.cypcb` files with the CLI:

```bash
cypcb check my_board.cypcb
```

This checks for:
- Syntax errors
- Unknown footprints
- Duplicate component references
- Undefined net references
- DRC violations

## Next Steps

- Review example files in `examples/`
- Run `cypcb check` on your designs
- Use `cypcb route` to auto-route traces
- Export Gerber files with `cypcb export`

For more information, see the main project README.
