# Benchmark Fixtures

KiCad 8 PCB benchmark files for autorouter validation. Each file uses `(version 20240108)` format with the `footprint` keyword (KiCad 7/8 style).

## Fixtures

### led_blink.kicad_pcb — Simple

- **Source:** Synthetic (hand-crafted for benchmark suite)
- **License:** Same as project (MIT-equivalent)
- **Original project:** N/A — realistic LED blink circuit
- **Complexity tier:** Simple
- **Components:** 7 (1 connector, 1 switch, 2 capacitors, 2 resistors, 1 LED)
- **Nets:** 7 (VCC, GND, LED_ANODE, BATT_POS, SW_OUT, R_LED, BYPASS)
- **Layers:** 2 (F.Cu, B.Cu)
- **Board size:** 40×30 mm
- **Traces:** 3 segments, 2 vias

### stm32_breakout.kicad_pcb — Medium

- **Source:** Synthetic (hand-crafted for benchmark suite)
- **License:** Same as project (MIT-equivalent)
- **Original project:** N/A — realistic STM32F103C8T6 breakout board
- **Complexity tier:** Medium
- **Components:** 29 (1 MCU, 1 regulator, 1 USB-C, 1 ESD protection, 1 crystal, 1 switch, 2 GPIO headers, 1 SWD header, 11 capacitors, 7 resistors, 2 LEDs)
- **Nets:** 40 (power rails, GPIO PA0-PA7/PB0-PB7/PC13-PC15, USB, SWD, I2C, SPI, UART, ADC)
- **Layers:** 2 (F.Cu, B.Cu)
- **Board size:** 75×65 mm
- **Traces:** 8 segments, 4 vias

### multi_ic.kicad_pcb — Complex

- **Source:** Synthetic (hand-crafted for benchmark suite)
- **License:** Same as project (MIT-equivalent)
- **Original project:** N/A — realistic STM32F407 + Ethernet PHY + SPI Flash + CAN board
- **Complexity tier:** Complex
- **Components:** 52 (1 MCU LQFP-100, 1 Ethernet PHY QFN-24, 1 SPI Flash SOIC-8, 2 voltage regulators, 1 CAN transceiver, 1 USB ESD, 2 crystals, 1 Ethernet magnetics, 5 connectors, 1 ferrite bead, 1 reset switch, 5 LEDs, 12 resistors, 18 capacitors)
- **Nets:** 94 (3 power rails, 48 GPIO, USB, Ethernet RMII, SPI Flash, CAN, I2C, UART, SWD, ADC, oscillator)
- **Layers:** 4 (F.Cu, In1.Cu, In2.Cu, B.Cu)
- **Board size:** 100×80 mm
- **Traces:** 15 segments, 10 vias

## Usage

These files are parsed by `cypcb-kicad::pcb_parser::parse_kicad_pcb()` and used as benchmark inputs for:
- Parser correctness validation (`benchmark_parse.rs` integration tests)
- Autorouter performance benchmarking (S03/S07)
- Ratsnest extraction compatibility testing

## Format Notes

All files use KiCad 8 format (`version 20240108`) with:
- `footprint` keyword (not legacy `module`)
- `property` keyword for Reference/Value (not legacy `fp_text`)
- Realistic footprint library names (e.g., `Resistor_SMD:R_0402_1005Metric`)
- Proper layer assignments and net declarations
