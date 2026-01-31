# Getting Started with CodeYourPCB

Welcome to CodeYourPCB - a code-first PCB design tool that makes your circuit designs git-friendly, AI-editable, and deterministic.

## What is CodeYourPCB?

CodeYourPCB is a revolutionary approach to PCB design where you describe your circuit board using a simple domain-specific language (DSL) instead of point-and-click GUI tools. This approach offers several advantages:

- **Git-friendly**: Your entire design is text, so you can use version control effectively
- **AI-editable**: ChatGPT, Claude, and other AI assistants can help you write and modify designs
- **Deterministic**: Same source file always produces the same board layout
- **Reviewable**: Pull requests show exactly what changed in your circuit
- **Scriptable**: Generate variations, run automated checks, integrate with CI/CD

## Your First .cypcb File

Let's create a simple LED blink circuit. Create a file named `blink.cypcb`:

```
// Simple LED blink circuit
version 1

board blink {
    size 50mm x 30mm
    layers 2
}

component LED1 led "0805" {
    value "RED"
    at 25mm, 15mm
}

component R1 resistor "0402" {
    value "330"
    at 15mm, 15mm
}

// Net constraints go in square brackets BEFORE the braces
net VCC [current 20mA] {
    R1.1
}

net LED_ANODE {
    R1.2
    LED1.A
}

net GND {
    LED1.K
}
```

### Understanding the Structure

Every `.cypcb` file follows this basic structure:

1. **Version declaration** - Always start with `version 1`
2. **Board definition** - Physical dimensions and layer count
3. **Components** - Parts placed on the board
4. **Nets** - Electrical connections between component pins

## Core Concepts Explained

### 1. Version Declaration

```
version 1
```

This must be the first non-comment line in your file. It specifies which version of the DSL syntax you're using, enabling future syntax evolution without breaking existing designs.

### 2. Board Definition

```
board blink {
    size 50mm x 30mm
    layers 2
}
```

The board definition sets up your PCB's physical properties:
- **name**: Identifies your board (used in export filenames)
- **size**: Width x height with explicit units (mm, mil, or in)
- **layers**: Number of copper layers (2, 4, 6, etc.)

### 3. Component Definition

```
component LED1 led "0805" {
    value "RED"
    at 25mm, 15mm
}
```

Components are the building blocks of your circuit:
- **refdes**: Reference designator (LED1, R1, U1, etc.)
- **type**: Component category (resistor, capacitor, led, ic, connector, etc.)
- **footprint**: Physical package in quotes ("0805", "SOIC8", "DIP-8")
- **value**: Component value or part number
- **at**: Position on the board with units
- **rotate**: Optional rotation angle in degrees (default: 0)

#### Component Types

Available component types:
- `resistor`, `capacitor`, `inductor` - Passive components
- `led`, `diode`, `transistor` - Semiconductors
- `ic` - Integrated circuits
- `connector` - Headers, terminals, edge connectors
- `crystal`, `generic` - Other components

#### Common Footprints

**SMD resistors and capacitors:**
- `"0402"` - 1.0mm x 0.5mm
- `"0603"` - 1.6mm x 0.8mm
- `"0805"` - 2.0mm x 1.25mm (most common for hand soldering)
- `"1206"` - 3.2mm x 1.6mm

**Integrated circuits:**
- `"SOIC8"`, `"SOIC14"` - Small outline IC packages
- `"SOT23"` - Small outline transistor (3-pin)
- `"TQFP32"`, `"TQFP64"` - Thin quad flat pack

**Through-hole:**
- `"DIP-8"`, `"DIP-14"` - Dual inline package
- `"PIN-HDR-1x2"` - 2.54mm pitch header

### 4. Net Definition

Nets define electrical connections between component pins:

```
net VCC [current 20mA] {
    R1.1
}
```

**Basic syntax:**
```
net <name> {
    <component>.<pin>
    <component>.<pin>
    ...
}
```

**Pin references:**
- Numeric: `R1.1`, `R1.2`, `C1.1`, `C1.2`
- Named: `LED1.A` (anode), `LED1.K` (cathode)
- IC pins: `U1.VCC`, `U1.GND`, `U1.1`, `U1.2`

#### Net Constraints

**IMPORTANT:** Constraints must be in square brackets BEFORE the braces:

```
net VCC [current 100mA width 0.3mm clearance 0.2mm] {
    R1.1
    C1.1
}
```

Available constraints:
- `current <value><unit>` - Expected current (20mA, 500mA, 2A)
- `width <dimension>` - Minimum trace width (0.3mm, 0.5mm)
- `clearance <dimension>` - Minimum spacing to other nets (0.2mm, 1mm)

These constraints inform the design rule checker (DRC) and autorouter.

### 5. Inline Pin Assignments

You can assign nets directly in component definitions:

```
component R1 resistor "0402" {
    value "330"
    at 15mm, 15mm
    pin.1 = VCC
    pin.2 = LED_ANODE
}
```

This is equivalent to defining separate nets but more compact for point-to-point connections.

## Complete Example Walkthrough

Let's build a power indicator circuit step by step:

```
version 1

// Simple power indicator circuit
// - 5V power input via 2-pin header
// - LED with current limiting resistor (330R for ~10mA @ 5V)
// - Decoupling capacitor for noise filtering

board power_indicator {
    size 25mm x 20mm
    layers 2
}
```

**Step 1:** Start with version and board definition. We need a small board (25mm x 20mm) with 2 layers.

```
// Power input - 2.54mm pitch header
component J1 connector "PIN-HDR-1x2" {
    at 5mm, 10mm
}

// Current limiting resistor for LED
// 5V - 2V(LED) = 3V, 3V/330R = ~9mA
component R1 resistor "0805" {
    value "330R"
    at 12mm, 14mm
}

// Power indicator LED (green)
component LED1 led "0805" {
    value "GREEN"
    at 18mm, 14mm
}

// Decoupling capacitor near power input
component C1 capacitor "0805" {
    value "100nF"
    at 12mm, 6mm
}
```

**Step 2:** Place components with calculated positions. Note the decoupling capacitor C1 is positioned near the power input J1 for effective noise filtering.

```
// Power rail connections
// Net constraints go in square brackets BEFORE the braces
net VCC [current 100mA width 0.3mm] {
    J1.1
    R1.1
    C1.1
}

net GND {
    J1.2
    LED1.2
    C1.2
}

// LED circuit: R1 -> LED1
net LED_ANODE {
    R1.2
    LED1.1
}
```

**Step 3:** Define nets connecting the components. VCC has constraints because it carries power. The LED_ANODE net connects the resistor to the LED.

## Common Patterns

### Power Rails with Decoupling

Always add decoupling capacitors near power inputs:

```
component C1 capacitor "0805" {
    value "100nF"
    at 12mm, 6mm
}

net VCC [current 500mA width 0.4mm] {
    J1.1      // Power input
    C1.1      // Decoupling capacitor
    U1.VCC    // IC power pin
}

net GND {
    J1.2
    C1.2
    U1.GND
}
```

### Ground Planes

For 2-layer boards, define a ground net connecting all GND pins. The manufacturer can create a ground pour:

```
net GND {
    J1.2
    C1.2
    C2.2
    U1.GND
    LED1.K
}
```

### High-Current Traces

For power supplies or motor drivers, specify wider traces:

```
net MOTOR_POWER [current 2A width 1mm clearance 0.5mm] {
    J1.1
    Q1.D
    M1.PLUS
}
```

## Viewing Your Design

After creating your `.cypcb` file, you can view it in two ways:

### Web Viewer (No Installation)

1. Open the CodeYourPCB web app in your browser
2. Click "Open File" and select your `.cypcb` file
3. The board renders in 3D with component outlines
4. Use mouse/trackpad to pan, zoom, and rotate

**Controls:**
- Left mouse drag: Rotate view
- Right mouse drag: Pan view
- Scroll wheel: Zoom
- Two-finger drag (trackpad): Pan
- Two-finger pinch (trackpad): Zoom

### Desktop App

1. Install CodeYourPCB desktop app for your platform
2. File → Open or drag-and-drop your `.cypcb` file
3. Native menus and file dialogs for seamless workflow
4. Supports auto-routing with FreeRouting integration

See [platform-differences.md](platform-differences.md) for desktop vs web feature comparison.

## Units

All dimensions require explicit units to avoid ambiguity:

- `mm` - Millimeters (most common, PCB industry standard)
- `mil` - Thousandths of an inch (1 mil = 0.0254mm)
- `in` - Inches
- `nm` - Nanometers (internal precision)

**Examples:**
```
size 50mm x 30mm
at 1.5in, 20mil
width 0.254mm
```

You can mix units in the same file - the parser handles conversion automatically.

## Comments

Use comments to document your design decisions:

```
// Line comment - everything after // is ignored

/*
 * Block comment
 * Spans multiple lines
 */

// Explain component choices
component R1 resistor "0805" {
    value "330"  // Chosen for ~10mA LED current at 5V
    at 15mm, 15mm
}
```

## Next Steps

Now that you understand the basics:

1. **Study complete examples** in the `examples/` directory:
   - `examples/blink.cypcb` - Simple LED circuit
   - `examples/power-indicator.cypcb` - Power indicator with decoupling
   - `examples/simple-psu.cypcb` - 5V regulator circuit

2. **Learn the complete syntax** in [SYNTAX.md](../SYNTAX.md):
   - Trace definitions for manual routing
   - Zone definitions for keepouts and copper pours
   - Custom footprint definitions
   - All available constraints and properties

3. **Organize your projects** - See [project-structure.md](project-structure.md) for file organization best practices

4. **Manage libraries** - See [library-management.md](library-management.md) for importing KiCad footprints and searching components

5. **Understand platform differences** - See [platform-differences.md](platform-differences.md) if you're choosing between desktop and web versions

## Getting Help

- Check [SYNTAX.md](../SYNTAX.md) for complete syntax reference
- Review example files in `examples/` directory
- Validate designs with the CLI: `cypcb check my_board.cypcb`
- Ask AI assistants like ChatGPT or Claude to help write/modify designs

Welcome to code-first PCB design!
