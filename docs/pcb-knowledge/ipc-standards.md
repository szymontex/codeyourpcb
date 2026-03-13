# IPC Standards Reference

Key IPC standards that inform the `cypcb-rules` constraint types and `cypcb-drc`
validation rules. These are the foundational PCB design standards published by
IPC (Association Connecting Electronics Industries).

> **Paywall note:** IPC standards are copyrighted and sold by IPC. The formulas
> and tables below come from widely-published summaries in industry literature
> (Cadence, Altium, Sierra Circuits, ProtoExpress, PCB Universe). They represent
> standard industry practice but are approximations — not verbatim IPC text.

---

## IPC-2221: Generic Standard on Printed Board Design

The umbrella standard covering clearance, trace width, thermal management, and
general PCB design rules. Our `DesignConstraints` struct maps directly to
IPC-2221 parameters.

Reference: <https://www.ipc.org/ipc-2221>

### Table 6-1: Minimum Electrical Clearance (Creepage)

Minimum clearance between conductors based on peak voltage (DC or AC peak) and
surface coating condition. Implemented in `cypcb-rules::clearance_table`.

| Peak Voltage (V) | Bare Board (mm) | Conformal Coated (mm) | Sea Level (mm) |
|------------------:|----------------:|----------------------:|--------------:|
| 0–15              | 0.10            | 0.05                  | 0.10          |
| 16–30             | 0.10            | 0.05                  | 0.10          |
| 31–50             | 0.60            | 0.13                  | 0.60          |
| 51–100            | 0.60            | 0.13                  | 0.60          |
| 101–150           | 0.60            | 0.40                  | 0.60          |
| 151–170           | 1.25            | 0.40                  | 1.25          |
| 171–250           | 1.25            | 0.40                  | 1.25          |
| 251–300           | 1.25            | 0.40                  | 1.25          |
| 301–500           | 2.50            | 0.80                  | 2.50          |
| 501–750           | 5.00 (est.)     | ≥ 0.80                | ≥ 2.50        |

*Higher voltages require board-specific analysis.*

Our `clearance_table::lookup()` function returns the minimum clearance for a
given voltage and `CoatingType`. It uses a stepped lookup — returns the clearance
for the lowest breakpoint ≥ the input voltage.

### Trace Width / Current Carrying Capacity

IPC-2221 Section 6.2 provides the standard formula for minimum trace width
given a required current capacity. This is the most widely-used PCB design
equation.

#### The IPC-2221 Trace Width Formula

The relationship between current, temperature rise, and cross-sectional area:

```
I = k × ΔT^0.44 × A^0.725
```

Where:
- **I** = current (Amperes)
- **k** = constant: 0.048 for internal layers, 0.024 for external layers
  (Note: some sources use k=0.048 external, k=0.024 internal — the values
  were swapped in IPC-2221A vs B. We use the IPC-2221B convention.)
- **ΔT** = temperature rise above ambient (°C)
- **A** = cross-sectional area of the trace (mil²)

To solve for area given current and temperature rise:

```
A = (I / (k × ΔT^0.44))^(1/0.725)
```

To convert cross-sectional area to trace width:

```
Width (mil) = A / (thickness × 1.378)
```

Where thickness is copper weight in oz/ft². Standard values:
- 0.5 oz = 0.0007" = 17.5 µm
- 1.0 oz = 0.0014" = 35 µm
- 2.0 oz = 0.0028" = 70 µm

#### Practical Reference Table (1 oz copper, external layer, 10°C rise)

| Current (A) | Min Width (mil) | Min Width (mm) |
|------------:|-----------------:|---------------:|
| 0.5         | 10               | 0.254          |
| 1.0         | 20               | 0.508          |
| 2.0         | 50               | 1.270          |
| 3.0         | 80               | 2.032          |
| 5.0         | 150              | 3.810          |
| 10.0        | 400              | 10.160         |

*These are conservative estimates. Use the formula for precise values.*

Our `DesignConstraints::max_current_per_width_x100` stores the maximum current
(in mA×100) per unit width, derived from this formula for the specific copper
weight and layer position.

### IPC-2221 Board Classes

Three reliability classes define progressively stricter constraints:

| Parameter              | Class 1 (General)   | Class 2 (Dedicated)  | Class 3 (High Rel)  |
|------------------------|--------------------:|---------------------:|--------------------:|
| Purpose                | Consumer electronics | Industrial/telecom   | Mil/med/life-safety |
| Min trace width        | 0.20 mm (8 mil)     | 0.15 mm (6 mil)      | 0.10 mm (4 mil)     |
| Min clearance          | 0.20 mm (8 mil)     | 0.15 mm (6 mil)      | 0.10 mm (4 mil)     |
| Min annular ring       | 0.15 mm             | 0.13 mm              | 0.10 mm             |
| Min drill size         | 0.30 mm             | 0.25 mm              | 0.20 mm             |
| Min hole-to-hole       | 0.25 mm             | 0.25 mm              | 0.20 mm             |
| Copper weight (min)    | 1 oz                | 1 oz                 | 1 oz                |
| Solder mask bridge     | 0.10 mm             | 0.08 mm              | 0.05 mm             |

These are implemented as `RulesPreset::IpcClass1`, `IpcClass2`, `IpcClass3` in
`cypcb-rules::presets::ipc`.

---

## IPC-2141: Design Guide for High-Speed Controlled Impedance

Covers impedance control for high-speed signal routing. The key formulas we
use for impedance estimation.

Reference: Based on IPC-2141 and widely-published microstrip/stripline models.

### Microstrip Impedance (External Layer)

A trace on an external layer with a ground plane below:

```
Z₀ = (87 / √(εr + 1.41)) × ln(5.98 × h / (0.8 × w + t))
```

Where:
- **Z₀** = characteristic impedance (Ω)
- **εr** = dielectric constant of substrate (FR-4 ≈ 4.2–4.5)
- **h** = dielectric thickness between trace and ground plane (mm)
- **w** = trace width (mm)
- **t** = copper thickness (mm)

Valid for `w/h` ratios between 0.1 and 3.0. Accuracy: ±5% for typical FR-4.

### Stripline Impedance (Internal Layer)

A trace between two ground planes:

```
Z₀ = (60 / √εr) × ln(4 × b / (0.67 × π × (0.8 × w + t)))
```

Where:
- **b** = distance between ground planes (mm)
- Other variables as above

Accuracy: ±5% for `w/b` < 0.35.

### Edge-Coupled Differential Pair (Microstrip)

```
Z_diff = 2 × Z₀ × (1 - 0.48 × exp(-0.96 × s / h))
```

Where:
- **Z_diff** = differential impedance (Ω)
- **Z₀** = single-ended impedance of each trace
- **s** = gap between the two traces (mm)
- **h** = dielectric height (mm)

### Edge-Coupled Differential Pair (Stripline)

```
Z_diff = 2 × Z₀ × (1 - 0.347 × exp(-2.9 × s / b))
```

### Common Impedance Targets

| Interface      | Type         | Target Impedance | Tolerance |
|----------------|--------------|-----------------:|----------:|
| USB 2.0        | Differential | 90 Ω             | ±10%      |
| USB 3.x        | Differential | 90 Ω             | ±10%      |
| HDMI           | Differential | 100 Ω            | ±10%      |
| Ethernet       | Differential | 100 Ω            | ±10%      |
| DDR3/4 Data    | Single-ended | 50 Ω             | ±10%      |
| DDR3/4 Clock   | Differential | 100 Ω            | ±10%      |
| PCIe           | Differential | 85 Ω             | ±15%      |
| LVDS           | Differential | 100 Ω            | ±10%      |
| SATA           | Differential | 100 Ω            | ±10%      |
| General GPIO   | Single-ended | 50 Ω             | ±20%      |

Our `DesignConstraints::default_impedance_ohms_x100` stores the default target
impedance (×100 for integer precision). `SignalClass::HighSpeed` and
`SignalClass::Differential` constraints enforce impedance control.

### Accuracy Limitations

These are simplified approximations. For production boards requiring
impedance control, manufacturers use field solvers (e.g., Polar SI,
HyperLynx) with actual material data. Our formulas provide reasonable
starting points for autorouter trace width selection, not final verification.

---

## IPC-2222: Sectional Standard on Rigid Organic Printed Board Design

Covers rigid board stackup design, via geometry, and mechanical constraints.

Reference: Based on IPC-2222 guidelines and manufacturer best practices.

### Via Aspect Ratio

The aspect ratio limits the minimum drill size relative to board thickness:

```
Aspect Ratio = Board Thickness / Drill Diameter
```

| Board Type        | Max Aspect Ratio | Notes                                  |
|-------------------|-----------------:|----------------------------------------|
| Standard 2-layer  | 10:1             | Most manufacturers handle easily       |
| Standard 4-layer  | 8:1              | Conservative for reliable plating      |
| Standard 6-layer  | 8:1              | Depends on total thickness             |
| HDI / microvia    | 1:1 typical      | Laser-drilled, ≤ 0.15mm               |
| Advanced          | 12:1–15:1        | Premium process, higher cost           |

Our `DesignConstraints::max_drill_aspect_ratio` stores this limit. The DRC
can verify: `board_thickness / drill_size ≤ max_aspect_ratio`.

### Standard Stackup Configurations

#### 2-Layer

```
Layer 1: Signal + Power (Top)
         ├── Prepreg: 1.0mm FR-4 (εr ≈ 4.3)
Layer 2: Signal + Ground (Bottom)
```

Total thickness: ~1.6mm with copper

#### 4-Layer (Signal-Ground-Power-Signal)

```
Layer 1: Signal (Top)
         ├── Prepreg: 0.20mm (εr ≈ 4.3)
Layer 2: Ground
         ├── Core: 0.80mm (εr ≈ 4.5)
Layer 3: Power
         ├── Prepreg: 0.20mm (εr ≈ 4.3)
Layer 4: Signal (Bottom)
```

Total thickness: ~1.6mm. This is the most common 4-layer configuration and
gives good impedance control (signal layers reference adjacent ground/power planes).

#### 6-Layer

```
Layer 1: Signal (Top)
         ├── Prepreg: 0.10mm
Layer 2: Ground
         ├── Core: 0.36mm
Layer 3: Signal (Inner 1)
         ├── Prepreg: 0.36mm
Layer 4: Signal (Inner 2)
         ├── Core: 0.36mm
Layer 5: Power
         ├── Prepreg: 0.10mm
Layer 6: Signal (Bottom)
```

These configurations are implemented as factory methods in `cypcb-rules::stackup`:
`Stackup::two_layer()`, `Stackup::four_layer()`, `Stackup::six_layer()`.

### Annular Ring Requirements

The annular ring is the copper ring remaining after drilling through a pad:

```
Annular Ring = (Pad Diameter - Drill Diameter) / 2
```

Minimums by class:

| Class   | Min Annular Ring | Notes                        |
|---------|-----------------:|------------------------------|
| Class 1 | 0.15 mm          | Consumer, least critical     |
| Class 2 | 0.13 mm          | Industrial standard          |
| Class 3 | 0.10 mm          | Military / medical           |
| HDI     | 0.05 mm          | Microvia, laser-drilled      |

Implemented as `DesignConstraints::min_annular_ring` and checked by
`cypcb-drc::AnnularRingRule`.

---

## Sources

- IPC-2221B "Generic Standard on Printed Board Design" — <https://www.ipc.org/ipc-2221>
- Cadence IPC trace width summary — <https://resources.pcb.cadence.com/blog/2020-ipc-trace-width-standard>
- Sierra Circuits trace width calculator — <https://www.protoexpress.com/tools/pcb-trace-width-calculator/>
- Altium impedance formulas — <https://resources.altium.com/p/using-altiums-impedance-calculator>
- PCB Universe IPC clearance reference — <https://www.pcbuniverse.com/pcbu-tech-tips.php?a=4>
- Saturn PCB Design trace current calculator — <https://saturnpcb.com/pcb_toolkit/>
