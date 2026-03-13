# Trace Geometry Best Practices

Guidelines for trace routing, via placement, differential pairs, and copper
management. These practices inform the autorouter's routing strategies and
the DRC's geometry checks.

---

## Via Fanout Patterns

Via fanout is the routing pattern used to escape signals from pads (especially
BGA) to inner layers or routing channels.

### Dog-Bone Fanout

The most common BGA breakout pattern:

```
  [PAD]
    |
    | (short trace)
    |
   (o) VIA
```

- Trace exits pad, via placed adjacent
- Suitable for BGAs with ≥ 0.8mm pitch
- For 0.5mm pitch BGA: requires microvia (≤ 0.15mm drill)
- Via placement: offset by `pad_radius + min_clearance + via_radius`

### Via-in-Pad

Via drilled directly in the pad, filled and plated over:

- Eliminates dog-bone trace, saves space
- Requires via fill + cap plating (advanced process)
- Preferred for thermal pads (ground slug on QFNs)
- Cost premium at most manufacturers

### Fanout Routing Order

For BGA/QFP with many pins:

1. **Power/ground first** — connect to planes via direct vias
2. **Outer ring** — route to outer channels without vias if possible
3. **Inner rings** — dog-bone fanout to inner layers
4. **Center pins** — may require via-in-pad or multiple routing layers

---

## Differential Pair Routing

Critical for high-speed signals (USB, HDMI, Ethernet, DDR clocks, LVDS).

### Key Rules

| Rule | Guideline | Reason |
|------|-----------|--------|
| Gap consistency | Maintain constant gap within ±10% | Gap variation changes impedance |
| Symmetry | Both traces same width, same layer | Asymmetry causes mode conversion |
| Length matching | Match within `length_match_tolerance` | Skew causes timing errors |
| Reference plane | Route over continuous ground plane | Gaps in reference plane destroy impedance |
| Layer changes | Both traces change layer at same point | Via stub mismatch causes reflections |
| Coupling length | Keep coupled for ≥80% of total length | Loose sections are uncontrolled impedance |

### Gap and Width Selection

For a target differential impedance Z_diff:

1. Start with single-ended impedance Z₀ = Z_diff / 2 (rough approximation)
2. Calculate trace width for Z₀ using the microstrip/stripline formula
3. Adjust gap — tighter gap reduces Z_diff (more coupling)
4. Iterate or use manufacturer's impedance calculator

Common starting points (FR-4, 1oz copper):

| Target Z_diff | Trace Width | Gap | Layer |
|--------------:|------------:|----:|-------|
| 90 Ω (USB)    | 0.15mm      | 0.15mm | External microstrip |
| 100 Ω (Ethernet) | 0.13mm   | 0.18mm | External microstrip |
| 100 Ω (DDR clk) | 0.10mm   | 0.15mm | Internal stripline |

*These are starting estimates. Final values depend on actual stackup.*

### Bends

- **Avoid 90° corners** — use 45° mitered bends or arcs
- Both traces of the pair must bend together (same radius)
- Inner trace is shorter at a bend — add length compensation on the inner trace
  or use a slight meander after the bend

### Breakout from Components

```
  [IC PAD P] ─────┐
                   ├── coupled pair ──→
  [IC PAD N] ─────┘
```

- Match the "uncoupled" length from pad to where coupling begins
- Keep uncoupled segments as short as possible (< 5mm ideal)

---

## Length Matching

Required for parallel buses (DDR data, address) and differential pairs.

### Techniques

**Trombone meander** (accordion pattern):
```
  ───┐ ┌───┐ ┌───
     │ │   │ │
     └─┘   └─┘
```

- Spacing between meander legs ≥ 3× trace width (avoid coupling)
- Meander amplitude: keep ≤ 1.5mm to avoid resonance at high frequencies
- Place meanders close to the source (near driver IC)

**Serpentine routing** — similar but with rounded corners:
- Preferred for very high-speed signals (less impedance discontinuity)
- More space-consuming

### Matching Groups

| Bus | Matching Requirement | Typical Tolerance |
|-----|---------------------|-------------------|
| DDR3 Data (DQ) | Match within byte lane | ±25 mil (0.635mm) |
| DDR3 Address/Command | Match to clock | ±50 mil (1.27mm) |
| DDR4 Data (DQ) | Match within byte lane | ±10 mil (0.254mm) |
| USB 2.0 D+/D- | Match pair | ±150 mil (3.81mm) |
| USB 3.x TX/RX | Match pair | ±5 mil (0.127mm) |
| PCIe TX/RX | Match pair | ±5 mil (0.127mm) |
| LVDS pair | Match pair | ±50 mil (1.27mm) |

Our `DesignConstraints::length_match_tolerance` stores the per-preset default.
`SignalClass::HighSpeed` and `SignalClass::Differential` enforce tighter tolerances.

---

## Teardrops

Teardrop-shaped fillets at the junction of traces and pads/vias:

```
     ╱─────  trace
    ╱
  (PAD)
    ╲
     ╲─────  trace
```

### Benefits

- Reduces stress concentration at the pad-trace junction
- Improves manufacturability (less chance of trace lifting during reflow)
- Reduces impedance discontinuity at via transitions
- Recommended for all vias on boards ≥ 4 layers

### Parameters

| Parameter | Typical Value | Notes |
|-----------|--------------|-------|
| Teardrop length | 2× trace width | From pad edge |
| Teardrop width at pad | Pad diameter | Blends into pad |
| Shape | Curved (preferred) or linear | Curved is gentler impedance transition |

### When to Use

- **Always** on via pads (especially high-speed nets)
- **Recommended** on SMD pads
- **Optional** on through-hole pads (usually large enough already)

---

## Copper Balancing

Uneven copper distribution across layers causes board warpage during manufacturing.

### Guidelines

| Rule | Detail |
|------|--------|
| Symmetric stackup | Mirror copper pattern top-to-bottom |
| Fill empty areas | Use copper pour (ground fill) on sparse layers |
| Balance inner layers | If Layer 2 is mostly ground, Layer 3 should be mostly power |
| Avoid large voids | Fill unused areas with hatched ground pour |
| Thermal relief | Use thermal relief on pour connections (see thermal-management.md) |

### Copper Pour Rules

- Pour-to-trace clearance: use `min_copper_pour_clearance` from constraints
- Pour-to-edge clearance: use `min_edge_clearance`
- Connect pour to ground net (or leave floating — but floating copper
  can cause antenna effects at high frequencies)
- Use cross-hatch fill for flexible sections (rigid-flex boards)

---

## Trace Tapering

Gradually widening traces when transitioning between routing and pads:

```
  thin trace ──╱  wide pad area
               ╲  thin trace
```

### When to Taper

- **Power traces** transitioning from routing width to pad connection
- **High-current paths** connecting to wider copper areas
- **BGA breakout** where inner traces are thinner than outer routing

### Guidelines

- Taper angle ≤ 45° (gradual transition)
- Taper length ≥ 2× width difference
- Avoid acute angles (acid traps) — our `min_acid_trap` constraint catches these

---

## Acid Traps

Acute angles (< 90°) in copper create "acid traps" during etching, where
etchant pools and over-etches the copper.

```
  ╲  ← acid trap (acute angle)
   ╲
    ╲───── trace
```

### Prevention

- Minimum angle between trace segments: 90° (our `min_acid_trap` constraint)
- Use 45° routing (octagonal) instead of arbitrary angles
- Fillet internal corners of copper pours
- The DRC flags violations when trace angles create gaps smaller than `min_acid_trap`

---

## Routing Layer Strategy

### 2-Layer Boards

- Route horizontal on top, vertical on bottom (or vice versa)
- Use copper pour on the bottom as pseudo-ground plane
- Minimize vias — each via is a bottleneck on a 2-layer board

### 4-Layer Boards (Sig-GND-PWR-Sig)

- Route high-speed signals on outer layers (reference ground plane on L2)
- Use inner layers (L3 power plane) for power distribution
- Short vertical connections between top signals and bottom signals via through-hole vias
- Inner signal routing only when outer layers are full

### 6-Layer Boards (Sig-GND-Sig-Sig-PWR-Sig)

- Route high-speed on L1 (reference L2 ground)
- Route sensitive analog on L3 (reference L2 ground from above)
- Route digital on L4 (reference L5 power)
- Route remaining signals on L6 (reference L5 power)
- Inner signal layers L3/L4 share the core — watch for crosstalk

---

## Sources

- IPC-2221B routing guidelines
- Altium "High Speed Design Guide" — <https://resources.altium.com/>
- Henry Ott, "Electromagnetic Compatibility Engineering" — PCB layout chapters
- Eric Bogatin, "Signal and Power Integrity" — differential pair routing
- Rick Hartley, "PCB Routing for Signal Integrity" — return path integrity
