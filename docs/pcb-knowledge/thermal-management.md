# Thermal Management

PCB thermal design guidelines covering current derating, thermal relief,
via stitching for heat transfer, and power plane design. These practices
inform `DesignConstraints` thermal fields and power signal routing.

---

## IPC-2221 Current Derating

The fundamental relationship between trace width, copper weight, and
current-carrying capacity. Based on IPC-2221B Section 6.2.

### The Formula

```
I = k × ΔT^0.44 × A^0.725
```

Where:
- **I** = current (A)
- **k** = 0.048 (external layers) or 0.024 (internal layers)
- **ΔT** = temperature rise above ambient (°C)
- **A** = cross-sectional area (mil²)

Cross-sectional area: `A = W × T × 1.378`
- W = trace width in mils
- T = copper thickness in oz/ft²
- 1.378 = conversion factor (1 oz ≈ 1.378 mil thickness)

### Current Capacity Tables

#### External Layer, 1 oz Copper (35 µm)

| Trace Width | 10°C Rise | 20°C Rise | 30°C Rise | 45°C Rise |
|------------:|----------:|----------:|----------:|----------:|
| 0.25mm (10 mil) | 0.5A | 0.7A | 0.9A | 1.1A |
| 0.50mm (20 mil) | 1.0A | 1.4A | 1.7A | 2.0A |
| 1.00mm (40 mil) | 1.7A | 2.3A | 2.8A | 3.4A |
| 1.50mm (60 mil) | 2.3A | 3.2A | 3.8A | 4.7A |
| 2.00mm (80 mil) | 2.8A | 3.9A | 4.7A | 5.7A |
| 3.00mm (120 mil) | 3.9A | 5.4A | 6.5A | 7.9A |
| 5.00mm (200 mil) | 5.8A | 8.0A | 9.7A | 11.8A |

#### Internal Layer, 1 oz Copper (35 µm)

Internal layers have roughly **half** the capacity of external layers due to
reduced convective cooling (surrounded by dielectric instead of air).

| Trace Width | 10°C Rise | 20°C Rise | 30°C Rise | 45°C Rise |
|------------:|----------:|----------:|----------:|----------:|
| 0.25mm (10 mil) | 0.3A | 0.4A | 0.5A | 0.6A |
| 0.50mm (20 mil) | 0.5A | 0.7A | 0.9A | 1.1A |
| 1.00mm (40 mil) | 0.9A | 1.2A | 1.5A | 1.8A |
| 1.50mm (60 mil) | 1.2A | 1.7A | 2.0A | 2.5A |
| 2.00mm (80 mil) | 1.5A | 2.1A | 2.5A | 3.0A |
| 3.00mm (120 mil) | 2.1A | 2.9A | 3.4A | 4.2A |

#### 2 oz Copper (70 µm) — External Layer

| Trace Width | 10°C Rise | 20°C Rise | 30°C Rise |
|------------:|----------:|----------:|----------:|
| 0.25mm (10 mil) | 0.8A | 1.1A | 1.4A |
| 0.50mm (20 mil) | 1.6A | 2.2A | 2.7A |
| 1.00mm (40 mil) | 2.7A | 3.7A | 4.5A |
| 2.00mm (80 mil) | 4.5A | 6.2A | 7.5A |

### Derating Considerations

- **Altitude**: above 3050m (10,000 ft), reduce capacity by 10%
- **Adjacent hot components**: reduce by 15-25% if near high-power ICs
- **Vias in the path**: each via adds ~0.5mΩ resistance — usually negligible
  for moderate currents, but significant for > 5A paths with many vias
- **Safety margin**: design for 50% of the calculated maximum (2:1 derating)

Our `DesignConstraints::max_current_per_width_x100` stores the maximum current
per mm of trace width (in mA×100). Default: 100,000 = 1000 mA/mm (1 oz, outer,
10°C rise) — a conservative value.

---

## Thermal Relief

Connection pattern between a trace/pad and a copper pour (ground/power plane).
Without thermal relief, the large copper area acts as a heat sink, making
soldering difficult.

### Thermal Relief Design

```
        ┌───────────────────┐
        │   copper pour     │
        │                   │
        │   ╱── spoke ──╲   │
        │  │    (PAD)    │  │
        │   ╲── spoke ──╱   │
        │       gap         │
        └───────────────────┘
```

The pad connects to the pour through narrow "spokes" with gaps between them,
instead of a full solid connection.

### Parameters

| Parameter | Constraint Field | Typical Value | Purpose |
|-----------|-----------------|---------------|---------|
| Gap width | `thermal_relief_gap` | 0.254mm (10 mil) | Isolates pad heat from pour |
| Spoke width | `thermal_relief_spoke_width` | 0.254mm (10 mil) | Limits heat flow to pour |
| Number of spokes | `thermal_relief_spokes` | 4 | 2 or 4 spokes typical |

### Spoke Configurations

| Spokes | Pattern | Use Case |
|-------:|---------|----------|
| 2 | ── PAD ── | Minimal connection, easiest soldering |
| 4 | + PAD + | Standard — good balance of heat/connection |
| Full | Solid | High-current pads where soldering is not a concern (wave solder, reflow with proper profile) |

### When to Use Thermal Relief

- **SMD pads** on copper pour: **always** use thermal relief (hand soldering becomes
  nearly impossible without it)
- **Through-hole pads** on ground plane: use thermal relief unless the pad carries > 3A
- **Via pads** on ground plane: thermal relief recommended (especially for
  rework/repair)
- **Thermal pads** (exposed pad on QFN/DFN): use **direct connection** (no relief) —
  the entire purpose is heat transfer
- **Power pads** carrying > 5A: consider direct connection with wider spokes

---

## Via Stitching for Thermal Transfer

Using vias to transfer heat from one layer to another — typically from a
component pad to an internal ground plane or a bottom-side heatsink.

### Thermal Via Array

For components with thermal pads (QFN, DFN, LGA packages):

```
  ┌──────────────────┐
  │  ╔══╗  ╔══╗      │
  │  ║VIA║ ║VIA║     │  Component thermal pad (top)
  │  ╚══╝  ╚══╝      │
  │  ╔══╗  ╔══╗      │
  │  ║VIA║ ║VIA║     │
  │  ╚══╝  ╚══╝      │
  └──────────────────┘
         │
    through vias
         │
  ┌──────────────────┐
  │  copper pour      │  Bottom copper (heatsink area)
  └──────────────────┘
```

### Via Array Guidelines

| Parameter | Recommendation | Reason |
|-----------|---------------|--------|
| Via diameter | 0.3mm drill, 0.6mm pad | Standard through-hole via |
| Via spacing | 1.0–1.2mm pitch | Balance density with manufacturing |
| Fill vias | Yes, if budget allows | Prevents solder wicking during reflow |
| Via count | Fill the thermal pad area | More vias = lower thermal resistance |
| Connect to | Ground plane or bottom pour | Spread heat across board |

### Thermal Resistance

Approximate thermal resistance of a single via (0.3mm drill, 1.6mm board):

```
R_via ≈ 70°C/W (unfilled)
R_via ≈ 35°C/W (filled with copper)
```

An array of 9 vias (3×3 grid):
```
R_array ≈ R_via / N ≈ 70/9 ≈ 8°C/W (unfilled)
```

For a 1W component with 9 thermal vias: ΔT ≈ 8°C above ambient at the bottom.

### Via Stitching for Ground Planes

Beyond thermal management, via stitching ground planes provides:
- Lower ground impedance at high frequencies
- Better EMI shielding (ground fence around board edge)
- Reduced resonance in ground plane cavities

Spacing: λ/20 at the highest frequency of concern
- 1 GHz → λ ≈ 300mm → stitch every 15mm
- 2.4 GHz (WiFi) → λ ≈ 125mm → stitch every 6mm

---

## Thermal Pad Patterns

### QFN/DFN Exposed Pad

The exposed pad (thermal pad) on the bottom of QFN packages requires special
PCB pad design:

1. **Solder paste**: use a grid pattern (e.g., 5×5 squares covering ~50-60% of pad area)
   — too much paste causes the component to float during reflow
2. **Vias in pad**: fill with copper or solder, cap with plating
3. **Connection**: direct connection to ground plane (no thermal relief on the thermal pad)
4. **Pad size**: match or slightly exceed the component's exposed pad dimension

### Power Transistor (D2PAK, TO-263, etc.)

- Large copper area on the tab pad layer
- Extend copper pour ≥ 5mm beyond pad in all directions
- Thermal vias under the tab pad (0.3mm drill, 1.0mm pitch)
- Bottom side: matching copper pour for heatsink attachment or convective cooling

---

## Power Plane Design

### Plane Splits

When a board has multiple voltage domains (3.3V, 1.8V, 1.2V), the power plane
may be split into regions:

```
┌─────────────┬─────────────┐
│             │             │
│   3.3V      │    1.8V     │
│   region    │   region    │
│             │             │
└─────────────┴─────────────┘
```

### Split Plane Guidelines

- **Never route high-speed signals across a plane split** — the return current
  path is disrupted
- Keep split boundaries away from sensitive components
- Add decoupling capacitors at the boundary between voltage domains
- Consider using a solid ground plane and discrete power routing instead of
  split power planes (simpler, fewer SI issues)

### Decoupling Strategy

| Component | Capacitor | Placement | Purpose |
|-----------|-----------|-----------|---------|
| Every IC | 100nF ceramic | Within 5mm of power pin | High-frequency noise |
| Every IC | 1–10µF ceramic | Within 10mm of power pin | Mid-frequency bypass |
| Power entry | 10–100µF electrolytic | Near connector | Bulk energy storage |
| Voltage regulator | Per datasheet | As close as possible | Stability |
| High-speed IC | 10nF + 100nF + 1µF | Multiple, closest first | Broadband filtering |

### Copper Pour Best Practices

| Rule | Value | Constraint Field |
|------|-------|-----------------|
| Pour-to-trace clearance | ≥ min_copper_pour_clearance | `min_copper_pour_clearance` |
| Pour-to-board-edge | ≥ min_edge_clearance | `min_edge_clearance` |
| Thermal relief on pads | Use gap and spoke settings | `thermal_relief_gap`, `thermal_relief_spoke_width` |
| Dead copper removal | Remove isolated copper islands | Manual review or DRC check |
| Pour connect to net | Always connect to a net (usually GND) | — |

---

## Sources

- IPC-2221B Section 6.2 — current carrying capacity
- IPC-7093 "Design and Assembly Process Implementation for Bottom Termination Components"
- TI Application Note SLMA002 — "PowerPAD Thermally Enhanced Package"
- Analog Devices AN-772 — "A Design and Manufacturing Guide for the Lead Frame Chip Scale Package"
- Saturn PCB Toolkit — trace current calculator: <https://saturnpcb.com/pcb_toolkit/>
- Robert Feranec, "Thermal Via Design" — <https://www.fedevel.com/>
