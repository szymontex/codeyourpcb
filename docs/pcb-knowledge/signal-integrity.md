# Signal Integrity

Signal classification taxonomy, per-class routing rules, and impedance targets.
This document describes the domain knowledge behind `cypcb-rules::SignalClass`
and `SignalClassConstraints`.

---

## Signal Classification Taxonomy

Every net in a PCB design is assigned one of five signal classes. The class
determines routing constraints, clearance requirements, and special handling.

### SignalClass::Digital

**Standard digital signals** — GPIO, chip select, reset, enable, LED drivers,
slow SPI/I2C (< 10 MHz), UART.

| Parameter | Value | Rationale |
|-----------|------:|-----------|
| Min trace width | 0.15mm (6 mil) | Comfortable for manufacturing, no impedance requirement |
| Min clearance | 0.15mm (6 mil) | Standard manufacturing clearance |
| Preferred layers | Any | No layer preference |
| Impedance control | No | Signal speeds don't require it |
| Length matching | No | Timing margins are generous |
| Guard traces | No | Not noise-sensitive |
| Max stub length | None | No stub restrictions |

**Routing guidelines:**
- Route on any available layer
- Use standard trace width (6 mil minimum, 8–10 mil preferred)
- 90° corners are acceptable for slow digital (but 45° is still preferred)
- No length matching needed unless part of a parallel bus

### SignalClass::HighSpeed

**High-speed digital signals** — USB data lines (single-ended side), HDMI, PCIe,
DDR clocks and strobes, SPI > 25 MHz, SDIO, high-speed ADC clocks.

| Parameter | Value | Rationale |
|-----------|------:|-----------|
| Min trace width | 0.127mm (5 mil) | Controlled impedance requires specific width |
| Min clearance | 0.20mm (8 mil) | Extra clearance reduces crosstalk |
| Preferred layers | L0, L2 (outer + inner-1) | Route adjacent to ground reference plane |
| Impedance control | **Yes** | Signal integrity depends on controlled Z₀ |
| Length matching | **Yes** | Timing-critical signals |
| Guard traces | No | Clearance is sufficient |
| Max stub length | 0.5mm | Stubs cause reflections at high frequencies |

**Routing guidelines:**
- Route over continuous ground plane — never cross a split in the reference plane
- Use 45° bends only (no 90° corners)
- Keep vias to a minimum (`max_vias_per_high_speed_net` = 4 default)
- Add teardrops at via connections
- Length match within tolerance (`length_match_tolerance`)
- Stub length < 0.5mm to avoid quarter-wave resonance
- Keep parallel coupling with other high-speed nets to minimum (3× clearance between parallel runs)

### SignalClass::Analog

**Analog signals** — ADC/DAC inputs/outputs, sensor signals (thermistors,
strain gauges, current sense), audio signals, reference voltages.

| Parameter | Value | Rationale |
|-----------|------:|-----------|
| Min trace width | 0.20mm (8 mil) | Wider traces for lower resistance, less noise pickup |
| Min clearance | 0.30mm (12 mil) | Isolation from noisy digital signals |
| Preferred layers | Any | But isolate from digital — see guidelines |
| Impedance control | No | Low-frequency signals don't need it |
| Length matching | No | Not timing-critical |
| Guard traces | **Yes** (0.5mm clearance) | Guard traces shield from crosstalk |
| Max stub length | None | Not relevant at analog frequencies |

**Routing guidelines:**
- **Separate from digital** — route analog on a different area of the board or
  different layer from high-speed digital
- Use guard traces (grounded traces flanking the analog signal) for sensitive
  inputs (high-impedance ADC inputs, current sense lines)
- Avoid routing analog signals near switching regulators, clock generators,
  or high-speed digital buses
- Use a dedicated analog ground region if the design has mixed-signal ICs
  (connect analog and digital grounds at a single point near the ADC/DAC)
- Wider traces reduce resistance and thermal noise
- Short traces reduce antenna pickup — keep analog runs as short as possible

### SignalClass::Power

**Power distribution** — VCC rails, voltage regulators, battery connections,
GND connections, motor drivers, LED power.

| Parameter | Value | Rationale |
|-----------|------:|-----------|
| Min trace width | 0.50mm (20 mil) | Current-carrying capacity |
| Min clearance | 0.20mm (8 mil) | Standard clearance |
| Preferred layers | Any | Power planes preferred for high current |
| Impedance control | No | DC/low-frequency distribution |
| Length matching | No | Not timing-critical |
| Guard traces | No | Not noise-sensitive |
| Max stub length | None | Not relevant |

**Routing guidelines:**
- **Size for current** — use the IPC-2221 trace width formula (see ipc-standards.md)
- For > 1A: consider copper pours or dedicated power planes instead of traces
- Use thermal relief when connecting to copper pours (see thermal-management.md)
- Add decoupling capacitors close to IC power pins (100nF ceramic, placed within 5mm)
- Route power entry → bulk cap → regulator → decoupling → IC pin
- Star topology for multiple voltage domains (avoid daisy-chaining)
- Voltage drop: `V_drop = I × R`, where `R = ρ × L / (W × T)`:
  - ρ (copper) = 1.72 × 10⁻⁸ Ω·m
  - L = trace length, W = trace width, T = copper thickness

### SignalClass::Differential

**Differential pair signals** — USB D+/D−, Ethernet TX±/RX±, LVDS, SATA,
DDR differential clocks, CAN bus, RS-485.

| Parameter | Value | Rationale |
|-----------|------:|-----------|
| Min trace width | 0.127mm (5 mil) | Controlled impedance |
| Min clearance | 0.20mm (8 mil) | Extra clearance from other nets |
| Preferred layers | L0 (outer) | Microstrip is preferred for diff pairs |
| Impedance control | **Yes** | Differential impedance must be controlled |
| Length matching | **Yes** | Intra-pair skew causes common-mode conversion |
| Diff pair routing | **Yes** | Must be routed as a coupled pair |
| Max stub length | 0.3mm | Tighter than single-ended high-speed |

**Routing guidelines:**
- Route both traces of the pair together — same layer, same width, constant gap
- **Never** split a differential pair across layers (both change layers at the same via point)
- Keep the gap constant within ±10% of the target
- Length match within pair to minimize skew
- Minimize uncoupled sections (pad breakout) — keep < 5mm
- Route over continuous ground plane
- Avoid routing between the traces of a differential pair (nothing crosses between P and N)
- See trace-geometry.md for differential pair bend rules and breakout patterns

---

## Impedance Targets by Interface

Common interface impedance requirements, stored as
`DesignConstraints::default_impedance_ohms_x100`:

| Interface | Type | Target Z | Tolerance | Trace Config |
|-----------|------|----------|-----------|--------------|
| USB 2.0 | Differential | 90 Ω | ±10% | Coupled pair, microstrip |
| USB 3.0/3.1/3.2 | Differential | 90 Ω | ±10% | Coupled pair, stripline OK |
| USB4 / Thunderbolt | Differential | 85 Ω | ±10% | Coupled pair, stripline |
| HDMI 1.x/2.0 | Differential | 100 Ω | ±10% | Coupled pair, length matched |
| Ethernet (10/100/1G) | Differential | 100 Ω | ±10% | Coupled pair |
| DDR3 Data | Single-ended | 50 Ω | ±10% | Matched within byte lane |
| DDR3 Clock | Differential | 100 Ω | ±10% | Coupled pair |
| DDR4 Data | Single-ended | 50 Ω | ±10% | Tighter matching than DDR3 |
| DDR4 Clock | Differential | 100 Ω | ±10% | Coupled pair |
| PCIe Gen1-4 | Differential | 85 Ω | ±15% | AC-coupled, length matched |
| PCIe Gen5/6 | Differential | 85 Ω | ±10% | Tighter tolerance |
| LVDS | Differential | 100 Ω | ±10% | Coupled pair |
| SATA | Differential | 100 Ω | ±10% | AC-coupled |
| CAN Bus | Differential | 120 Ω | ±10% | Bus terminated at endpoints |
| RS-485 | Differential | 120 Ω | ±10% | Bus terminated at endpoints |
| General GPIO | Single-ended | 50 Ω | ±20% | Loose tolerance OK |

### Notes on Impedance

- Impedance is controlled by trace width, copper thickness, dielectric height,
  and dielectric constant — see ipc-standards.md for formulas
- Our default impedance (`default_impedance_ohms_x100 = 5000` = 50 Ω) is a
  reasonable single-ended default for general routing
- Manufacturers offering impedance control will adjust the stackup to hit the
  target — they provide the controlled stackup, we provide the impedance target
- For differential pairs, the important parameter is Z_diff (differential
  impedance), not the single-ended Z₀ of each trace

---

## Return Path Integrity

The most overlooked signal integrity concept. Every signal has a return current
that flows through the nearest reference plane.

### Key Rules

1. **Never route a signal across a split in its reference plane.** The return
   current must detour around the split, creating a large loop = antenna.

2. **When changing layers, add a ground via near the signal via.** This gives
   the return current a path to transition between reference planes.

3. **High-speed signals should reference a continuous ground plane** (not a power
   plane with splits for different voltage domains).

4. **Avoid slots in ground planes** under high-speed signals (mounting holes,
   connector cutouts can create unintentional slots).

### Practical Checks

- Visual: highlight the ground plane beneath each high-speed signal — look for breaks
- Via transitions: each high-speed via should have a ground via within 2× the
  dielectric height (typically < 0.5mm away)
- Differential pairs: the return current for a diff pair is mostly between the
  two traces, so reference plane continuity is slightly less critical (but still
  important for common-mode rejection)

---

## Crosstalk

Unwanted coupling between adjacent traces.

### Types

- **Capacitive (forward) crosstalk** — electric field coupling, dominates for microstrip
- **Inductive (backward) crosstalk** — magnetic field coupling, dominates for stripline

### Mitigation Rules

| Rule | Guideline |
|------|-----------|
| 3W rule | Space traces ≥ 3× trace width apart (center-to-center) for < 70% coupling |
| 5W rule | Space traces ≥ 5× trace width apart for < 5% coupling (recommended for sensitive signals) |
| Guard traces | Grounded trace between aggressor and victim, via-stitched every 1/10 wavelength |
| Layer separation | Route sensitive signals on different layers, orthogonally when possible |
| Minimize parallel run | Keep parallel segments short — coupling increases with length |
| Our constraints | `min_clearance` provides basic separation; `guard_trace_clearance` adds isolation |

---

## Sources

- Howard Johnson & Martin Graham, "High-Speed Digital Design" — the foundational text
- Eric Bogatin, "Signal and Power Integrity — Simplified"
- Henry Ott, "Electromagnetic Compatibility Engineering"
- Rick Hartley, "Return Current Path" presentations (DesignCon)
- USB Implementers Forum, USB 2.0/3.x specification — impedance requirements
- JEDEC DDR3/DDR4 design guidelines — trace matching requirements
- PCI-SIG PCIe CEM specification — channel impedance and loss budgets
