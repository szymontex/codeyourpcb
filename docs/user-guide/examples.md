# Example Project Walkthroughs

This guide walks through the example projects included with CodeYourPCB. Each example demonstrates different features of the `.cypcb` DSL and teaches progressively more advanced concepts.

## 1. Blink Circuit (Beginner)

**File:** [`examples/blink.cypcb`](../../examples/blink.cypcb)

**Purpose:** A minimal LED circuit demonstrating the basics of component placement and net definitions. This is the "Hello World" of PCB design.

**What you'll learn:**
- Basic `.cypcb` file structure
- Component placement with position coordinates
- Net definitions connecting component pins
- Net constraints (current rating)

### Code Walkthrough

```cypcb
// Comments start with // (C-style)
version 1   // File format version (currently always 1)

// Board definition: size and layer count
board blink {
    size 50mm x 30mm    // Physical board dimensions (width x height)
    layers 2            // 2-layer board (Top and Bottom copper)
}

// Component placement: LED
component LED1 led "0805" {    // RefDes, type, footprint package
    value "RED"                // Component value/color
    at 25mm, 15mm              // Position on board (x, y from origin)
}

// Component placement: Current-limiting resistor
component R1 resistor "0402" {
    value "330"                // 330 ohm resistor
    at 15mm, 15mm
}

// Net definition: Power rail
// Constraints go in square brackets BEFORE the braces
net VCC [current 20mA] {       // Net named "VCC" with 20mA current limit
    R1.1                       // Pin 1 of R1
}

// Net definition: LED anode connection
net LED_ANODE {
    R1.2                       // Pin 2 of R1
    LED1.A                     // Anode pin of LED1
}

// Net definition: Ground rail
net GND {
    LED1.K                     // Cathode pin of LED1
}
```

### Circuit Description

The resulting PCB has three components arranged in a line:
- **R1** (resistor) at 15mm, 15mm on the left
- **LED1** (LED) at 25mm, 15mm on the right

**Electrical connections:**
- VCC connects to R1 pin 1
- R1 pin 2 connects to LED anode (LED1.A) via LED_ANODE net
- LED cathode (LED1.K) connects to GND

This forms a simple series circuit: VCC → R1 → LED1 → GND.

**Key concepts:**
- Pin references use dot notation: `ComponentName.PinNumber`
- Nets can have constraints like `[current 20mA]` for DRC
- Each net lists all pins connected to it

---

## 2. Power Indicator (Intermediate)

**File:** [`examples/power-indicator.cypcb`](../../examples/power-indicator.cypcb)

**Purpose:** A complete power indicator circuit with a header connector, LED, resistor, and decoupling capacitor. Demonstrates power distribution and good PCB design practices.

**What you'll learn:**
- Connector components (headers)
- Power net constraints (current and width)
- Decoupling capacitors
- More complex net topology

### Code Walkthrough

```cypcb
version 1

// Comments explaining circuit purpose
// - 5V power input via 2-pin header
// - LED with current limiting resistor (330R for ~10mA @ 5V)
// - Decoupling capacitor for noise filtering

board power_indicator {
    size 25mm x 20mm
    layers 2
}

// Power input - 2.54mm pitch header
component J1 connector "PIN-HDR-1x2" {   // Through-hole 2-pin header
    at 5mm, 10mm
}

// Current limiting resistor for LED
// 5V - 2V(LED) = 3V, 3V/330R = ~9mA
component R1 resistor "0805" {
    value "330R"                         // Explicit "R" suffix for ohms
    at 12mm, 14mm
}

// Power indicator LED (green)
component LED1 led "0805" {
    value "GREEN"                        // LED color
    at 18mm, 14mm
}

// Decoupling capacitor near power input
component C1 capacitor "0805" {
    value "100nF"                        // Nanofarads
    at 12mm, 6mm
}

// Power rail connections
// Net constraints go in square brackets BEFORE the braces
net VCC [current 100mA width 0.3mm] {    // Power net with current and trace width
    J1.1                                 // Pin 1 of header (VCC)
    R1.1                                 // Resistor power side
    C1.1                                 // Capacitor power side
}

// Ground rail
net GND {
    J1.2                                 // Pin 2 of header (GND)
    LED1.2                               // LED cathode
    C1.2                                 // Capacitor ground side
}

// LED circuit: R1 -> LED1
net LED_ANODE {
    R1.2                                 // Resistor output
    LED1.1                               // LED anode
}
```

### Circuit Description

The PCB layout has components arranged for optimal power distribution:
- **J1** (header) at 5mm, 10mm - Power input on left edge
- **C1** (capacitor) at 12mm, 6mm - Near power input for filtering
- **R1** (resistor) at 12mm, 14mm - Center of board
- **LED1** (LED) at 18mm, 14mm - Right side

**Electrical topology:**
1. Power enters via J1 (5V on pin 1, GND on pin 2)
2. VCC net distributes to both R1 and C1
3. Decoupling capacitor C1 filters noise between VCC and GND
4. Current flows through R1 → LED1 → GND

**Best practices demonstrated:**
- Decoupling capacitor placed close to power input
- Net constraints specify current capacity (100mA) and trace width (0.3mm)
- Comments explain electrical calculations (LED current calculation)

---

## 3. Simple Power Supply (Advanced)

**File:** [`examples/simple-psu.cypcb`](../../examples/simple-psu.cypcb)

**Purpose:** A complete 5V linear regulator circuit (LDO). Demonstrates voltage regulation with input/output filtering.

**What you'll learn:**
- IC components (integrated circuits)
- Multiple power domains (VIN vs VOUT)
- Input and output filtering
- Pin number assignments for ICs

**Note:** This example contains Polish comments. The DSL syntax is the same regardless of comment language.

### Code Walkthrough

```cypcb
// Najprostszy zasilacz 5V (Simplest 5V power supply)
// Wejście: 7-12V DC (Input: 7-12V DC)
// Wyjście: 5V stabilizowane (Output: 5V regulated)
version 1

board simple_psu {
    size 30mm x 20mm
    layers 2
}

// Złącze wejściowe - 2-pin header (Input connector)
component J1 connector "PIN-HDR-1x2" {
    at 5mm, 10mm
}

// Kondensator wejściowy (filtr) (Input capacitor - filter)
component C1 capacitor "0805" {
    value "100nF"
    at 12mm, 14mm
}

// Regulator napięcia LDO 5V (SOT-23) (LDO 5V voltage regulator)
// np. MCP1700, XC6206, lub podobny (e.g., MCP1700, XC6206, or similar)
component U1 ic "SOT-23" {                   // IC type, SOT-23 package
    value "LDO-5V"                           // Part type designation
    at 18mm, 10mm
}

// Kondensator wyjściowy (stabilizacja) (Output capacitor - stabilization)
component C2 capacitor "0805" {
    value "100nF"
    at 24mm, 14mm
}

// Złącze wyjściowe - 2-pin header (Output connector)
component J2 connector "PIN-HDR-1x2" {
    at 27mm, 10mm
}

// Sieć VIN - wejście nieregulowane (VIN net - unregulated input)
net VIN {
    J1.1                                     // Input header pin 1
    C1.1                                     // Input filter cap
    U1.3                                     // MCP1700: pin 3 = VIN
}

// Sieć VOUT - wyjście 5V (VOUT net - 5V output)
net VOUT {
    U1.2                                     // MCP1700: pin 2 = VOUT
    C2.1                                     // Output filter cap
    J2.1                                     // Output header pin 1
}

// Masa wspólna (Common ground)
net GND {
    J1.2                                     // Input ground
    C1.2                                     // Input cap ground
    U1.1                                     // MCP1700: pin 1 = GND
    C2.2                                     // Output cap ground
    J2.2                                     // Output ground
}
```

### Circuit Description

The PCB implements a linear voltage regulator circuit:
- **J1** (input header) at 5mm, 10mm - Accepts 7-12V DC
- **C1** (input cap) at 12mm, 14mm - Filters input voltage
- **U1** (LDO regulator) at 18mm, 10mm - Steps down to 5V
- **C2** (output cap) at 24mm, 14mm - Stabilizes output
- **J2** (output header) at 27mm, 10mm - Provides regulated 5V

**Signal flow:**
1. Unregulated voltage (7-12V) enters via J1
2. C1 filters high-frequency noise on VIN
3. U1 regulates voltage down to 5V
4. C2 stabilizes VOUT and suppresses oscillation
5. Clean 5V exits via J2

**IC pin assignments:**
- U1.1 = GND (common ground)
- U1.2 = VOUT (5V regulated output)
- U1.3 = VIN (7-12V unregulated input)

**Key concepts:**
- IC components use numeric pin references (U1.1, U1.2, U1.3)
- Pin numbers map to physical IC package pinout
- Separate nets for different voltage domains (VIN vs VOUT vs GND)
- Input and output capacitors are standard practice for LDO stability

---

## 4. Routing Test (Routing Demonstration)

**File:** [`examples/routing-test.cypcb`](../../examples/routing-test.cypcb)

**Purpose:** A test case for routing algorithms. Demonstrates numeric pin names and basic net topology for auto-routing validation.

**What you'll learn:**
- Numeric pin names for generic components
- Nets with shared connections (GND connects multiple components)
- Minimal working example for routing tools

### Code Walkthrough

```cypcb
// Routing test with numeric pin names
version 1

board routing_test {
    size 40mm x 25mm                         // Wider board for routing space
    layers 2
}

// Two resistors in series configuration
component R1 resistor "0402" {
    value "10k"
    at 10mm, 12mm                            // Left side
}

component R2 resistor "0402" {
    value "10k"
    at 25mm, 12mm                            // Right side
}

// Decoupling capacitor
component C1 capacitor "0402" {
    value "100nF"
    at 17mm, 8mm                             // Center, below resistors
}

// Power net
net VCC {
    R1.1                                     // R1 pin 1
    C1.1                                     // C1 pin 1
}

// Ground net (shared by all components)
net GND {
    R1.2                                     // R1 pin 2
    R2.1                                     // R2 pin 1
    C1.2                                     // C1 pin 2
}

// Signal net
net SIGNAL {
    R2.2                                     // R2 pin 2 (unconnected elsewhere)
}
```

### Circuit Description

The PCB has three 0402-sized components with a simple routing challenge:
- **R1** at 10mm, 12mm (left)
- **C1** at 17mm, 8mm (center-bottom)
- **R2** at 25mm, 12mm (right)

**Net topology:**
- VCC connects R1.1 and C1.1 (2 pins, requires one trace)
- GND connects R1.2, R2.1, and C1.2 (3 pins, requires two traces or a pour)
- SIGNAL connects only R2.2 (single pin, no routing needed)

**Routing challenges:**
- GND net forms a T-junction (R1.2 ← → C1.2 ← → R2.1)
- Components are spaced to test trace clearance
- Small 0402 footprints test fine-pitch routing

This example is used to verify FreeRouting integration works correctly.

---

## 5. DRC Test (Design Rule Checking)

**File:** [`examples/drc-test.cypcb`](../../examples/drc-test.cypcb)

**Purpose:** Intentionally violates design rules to test DRC engine. Demonstrates what NOT to do in real designs.

**What you'll learn:**
- Clearance violations (components too close)
- Unconnected pins (missing net assignments)
- How DRC errors appear in the viewer

### Code Walkthrough

```cypcb
version 1

board drc_test {
    size 30mm x 30mm
    layers 2
}

// VIOLATION 1: Clearance violation
// Two resistors placed too close together (should trigger clearance violation)
// 0402 pads extend 0.8mm from component center, so at 0.5mm spacing the pads overlap
component R1 resistor "0402" {
    value "10k"
    at 10mm, 15mm
}

component R2 resistor "0402" {
    value "10k"
    at 10.5mm, 15mm                          // Only 0.5mm apart! (VIOLATION)
}

// Through-hole component with drills - test drill rule
component J1 connector "PIN-HDR-1x2" {
    at 20mm, 15mm
}

// VIOLATION 2: Unconnected pins
// Unconnected component (no net assignments) - test connectivity rule
// Both pins are unconnected
component C1 capacitor "0805" {
    value "100nF"
    at 15mm, 10mm                            // Not connected to any net!
}

// Properly connected component for comparison - no violations expected
component R3 resistor "0805" {
    value "1k"
    at 25mm, 15mm
}

// R3 is fully connected to nets (correct)
net VCC {
    R3.1
}

net GND {
    R3.2
}
```

### Expected DRC Violations

When you load this file in the viewer, you should see:

1. **Clearance violation:** R1 and R2 are only 0.5mm apart. With 0402 footprints (0.8mm pad extension), the pads physically overlap. DRC should flag this as a clearance error.

2. **Unconnected pin violations:** Component C1 has two pins (C1.1 and C1.2), but neither appears in any net definition. DRC should report two "unconnected pin" errors.

3. **Unconnected pins (partial):** Components R1, R2, and J1 have pins not assigned to any net, which should also trigger violations.

**Correct design:** Only R3 is properly connected (both pins assigned to nets VCC and GND).

### Using This Example

Open `examples/drc-test.cypcb` in the viewer:
1. Parser will load the file successfully (syntax is valid)
2. DRC engine will analyze the board
3. Violations panel will show clearance and connectivity errors
4. Violating components will be highlighted in red on the canvas

This example helps verify the DRC engine is working correctly.

---

## Summary

These examples demonstrate the full range of `.cypcb` features:

| Example | Difficulty | Key Features |
|---------|-----------|--------------|
| blink.cypcb | Beginner | Basic components, nets, pin references |
| power-indicator.cypcb | Intermediate | Power distribution, net constraints, decoupling |
| simple-psu.cypcb | Advanced | ICs, multiple power domains, filtering |
| routing-test.cypcb | Intermediate | Numeric pins, routing topology |
| drc-test.cypcb | Advanced | DRC violations (educational) |

**Next steps:**
- Try modifying these examples (change component positions, values, or nets)
- Create your own circuits based on these patterns
- Use the LSP auto-completion (Ctrl+Space) to discover available keywords
- Hover over keywords for inline documentation

For more details on the DSL syntax, see the [Language Reference](../reference/language.md).
