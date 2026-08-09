# CodeYourPCB DSL Syntax Reference

This document describes the syntax of the CodeYourPCB domain-specific language (DSL).

## Table of Contents

1. [Version Declaration](#version-declaration)
2. [Board Definition](#board-definition)
3. [Board Outline](#board-outline)
4. [Component Definition](#component-definition)
5. [Net Definition](#net-definition)
6. [Zone Definition](#zone-definition)
7. [Trace Definition](#trace-definition)
8. [Custom Footprint Definition](#custom-footprint-definition)
9. [Modules and Interfaces](#modules-and-interfaces)
10. [Assertions](#assertions)
11. [Imports](#imports)
12. [Comments](#comments)
13. [Units](#units)

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

## Board Outline

A board is the rectangle its `size` describes unless it says otherwise. An
outline is the real edge, as a ring of points - which is how a board gets a
cutout, a slot or a chamfer:

```
outline {
    point 0mm, 0mm
    point 40mm, 0mm
    point 40mm, 15mm
    point 20mm, 15mm
    point 20mm, 30mm
    point 0mm, 30mm
}
```

The ring closes itself: the last point joins the first. The outline is what the
edge-cuts layer is drawn from on export, and what the checker measures edge
clearance against.

## Component Definition

Place components on the board:

```
component <refdes> <type> "<footprint>" {
    value "<value>"
    at <x>, <y>
    rotate <angle>
}
```

**Example:**
```
component R1 resistor "0402" {
    value "330"
    at 15mm, 10mm
    rotate 90
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

### Naming a component's nets in place

A connection can be written at either end. The `net` block lists the pins it
joins; `pin.<N> = <NET>` says the same thing from the component:

```
component R1 resistor "0402" {
    value "10k"
    at 5mm, 5mm
    pin.1 = VCC
    pin.2 = OUT
}
```

Both forms make one net. A net named by a block and by an assignment is the
same net, and the block's constraints stand - an assignment says who is
connected, never how wide the copper is:

```
net SIG [width 0.5mm] {
    R1.2
}
```

An earlier version of this guide said the parser had never accepted
`pin.<N> = <NET>`. It parsed it all along; what it did not do was put the pin
on the net, so a design written this way came out with no nets and a report
full of unconnected pins. It works now.

**Footprint Examples:**
- SMD resistors/capacitors: "0402", "0603", "0805", "1206"
- ICs: "SOIC8", "SOIC14", "SOT23", "TQFP32"
- Through-hole: "DIP-8", "PIN-HDR-1x2"
- Mounting holes: "MOUNT-M2", "MOUNT-M2.5", "MOUNT-M3", "MOUNT-M4"

See `examples/` directory for more component examples.

### Mounting Holes

A hole the board is screwed down by is a component like any other, placed with
a footprint that has a drill and no copper:

```
component H1 generic "MOUNT-M3" {
    value "M3"
    at 5mm, 5mm
}
```

The four sizes are named for the screw and drilled to its clearance hole -
2.2mm for M2, 2.7 for M2.5, 3.2 for M3, 4.3 for M4. The number is the drill,
not the screw: an M3 screw passes through a 3.2mm hole.

Because the hole carries no copper, the whole chain treats it as what it is:

- it goes in the **NPTH drill file**, so the fabricator does not plate it - a
  plated hole comes back narrower than the screw and connected to any copper
  it passes,
- **no copper layer** flashes a pad there,
- the **router** treats it as solid, on every layer,
- it appears in **neither the bill of materials nor the placement file**.
  Nobody buys a hole and no machine places one.

This is not the same as `keepout mounting_hole` further down, which reserves a
region and drills nothing.

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

### Net Classes

A rule stated once for a group of nets, instead of repeated on each:

```
netclass Power [width 0.5mm clearance 0.3mm] {
    VCC
    GND
}
```

Precedence is explicit: a class is applied first, and a net that states
something of its own overwrites **only the field it states**. So with the class
above, `net VCC [width 0.8mm]` is 0.8mm wide and still carries the class's
0.3mm clearance.

### Differential Pairs

Two nets that carry one signal between them:

```
diffpair USB {
    USB_DP
    USB_DM
}
```

The receiver reads the difference between the two, so copper one half runs and
the other does not arrives late. The checker measures that skew against the
fab's length-match tolerance - 0.5mm on JLCPCB's standard process, 0.127mm on
IPC Class 3 - and reports the pair with both lengths when it is exceeded:

```
diff-pair-skew: diffpair 'USB': USB_DP runs 30.000mm and USB_DM runs 40.000mm
```

A pair naming a net the board does not have is reported too, because a typo
there would otherwise turn the check off without a word.

Not checked yet: the gap between the two halves. That is the other half of a
differential-pair rule and needs the router to place them alongside each other
first.

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

Defines a copper pour area. The zone is filled against the copper already on
its layer - foreign copper is cut out with the fab's clearance, and a pad on
the pour's own net keeps a thermal gap bridged by spokes. The filled copper is
what the Gerber carries and what the viewer draws.
```
zone <name> {
    bounds <x1>, <y1> to <x2>, <y2>
    layer <layer>
    net <netname>
}
```

**Layers:**
- `top` or `Top`: Top copper layer
- `bottom` or `Bottom`: Bottom copper layer
- `all` or `All`: All layers

Either spelling is accepted here and in a `trace` block. They used to differ -
a zone took `top` and a trace took `Top` - so one capital letter decided
whether a line was correct or a syntax error depending on which block it sat
in.

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
- `via`: Waypoint coordinates for routing (optional, can repeat). Takes an
  optional drill and an optional layer pair:
  `via 12mm, 10mm drill 0.3mm layers Top to Inner1`. Without the pair a via
  goes through the board. A pair that stops at an inner layer is a blind or
  buried via: it gets its own Excellon file, the checker only measures it
  against holes made in the same drill pass, and the 3D view draws it to that
  depth.
- `layer`: Copper layer (`Top`, `Bottom`, `Inner1`-`Inner4`, or the same names in lower case)
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

## Modules and Interfaces

A module is a circuit block: components, the nets between them, and the pins it
exposes to whoever places it.

```
module <name> {
    [implements <interface>]
    pin <name>
    ...

    component ... { ... }
    net ... { ... }
}
```

**Example:**
```
module Divider {
    pin IN
    pin OUT

    component RTOP resistor "0402" {
        value 10kohm
        at 0mm, 0mm
    }

    component RBOT resistor "0402" {
        value 10kohm
        at 0mm, 2mm
    }

    net IN {
        RTOP.1
    }

    net OUT {
        RTOP.2
        RBOT.1
    }
}
```

Place one with `use`. Its components arrive under the instance name as a
prefix - `SENSE_RTOP`, not a second `RTOP` - and each pin is wired to a net the
design names:

```
use Divider as SENSE at 20mm, 10mm rotate 90 {
    IN = VIN
    OUT = MID
}
```

Every pin the module declares has to be given a net. Leave one out and the
checker names it.

### Interfaces

An interface is a contract: a named set of pins. A module signs it with
`implements`, and the checker holds the module to it.

```
interface I2C {
    pin SDA
    pin SCL
}

module TemperatureSensor {
    implements I2C
    pin SDA
    pin SCL

    component U1 ic "SOIC-8" {
        value "TMP102"
        at 0mm, 0mm
    }
}
```

Write `implements` once per interface, the way `pin` is written once per pin.
Two things are errors, both reported with the line that caused them:

- claiming an interface nobody defined
- claiming one and not exposing all of its pins - `implements I2C` without an
  `SDA` gives `module 'X' implements 'I2C' without pin SDA`

An interface nobody implements is fine; it is a definition waiting for a
module.

## Assertions

What a design claims about itself. The checker evaluates these with the design
rules and reports any that do not hold:

```
assert board.width <= 100mm
assert board.layers >= 2
assert VBUS.width >= 0.5mm
assert VBUS.current >= 1A
```

Readable today: `board.width`, `board.height`, `board.layers`, and a net's
`width`, `clearance` and `current`. Operands are typed - a length is refused a
comparison with a resistance rather than being quietly coerced into one.

**An assertion the checker cannot evaluate is reported, not skipped.** Asking
about something it cannot read comes back as "not something the checker can
read yet", because a statement that quietly does nothing leaves the board
looking checked.

## Imports

A file can use what another file defines:

```
import "lib/blocks.cypcb"
import Divider, LedDriver from "lib/blocks.cypcb"
```

Plain `import` takes everything reusable; the second form takes only what it
names, and says so if a name is not there.

- **What comes across:** modules, footprints and interfaces. Not a board, not
  components, not nets - importing a file must not place parts on the design.
- **Where the path points:** relative to the file doing the importing. There is
  no project root to configure and no search path to get wrong.
- **What a named import brings with it:** whatever the named definitions need -
  the footprints their components use, the interfaces they implement, the
  modules they instantiate. A library you cannot use without knowing what is
  inside it is not a library.
- A file that imports itself, directly or through others, is reported rather
  than followed.

In the browser there is no filesystem to read, so the page fetches the files
and hands them to the engine: a template's library is served beside it, and a
design opened from the dev server's directory is read from there. A file opened
through the browser's file picker has no directory to fetch from, and the
import is reported as one that could not be followed.

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
- Use `cypcb route --variants` to auto-route traces. It routes the board
  several ways and keeps the one that scores best, because no single setting
  wins on every board - measured across the benchmark suite, the winner
  differs per board every time.
- Export Gerber files with `cypcb export`

For more information, see the main project README.
