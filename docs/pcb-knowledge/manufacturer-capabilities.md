# Manufacturer Capabilities

Comparison of PCB manufacturers supported by `cypcb-rules` presets. Every value
is sourced from the manufacturer's published capability page with retrieval date.

This table drives the `RulesPreset` and `Preset` (DRC) configurations. When a
user selects a manufacturer preset, all constraints come from these values.

---

## Manufacturer Comparison Table

### Basic Geometry

| Parameter              | JLCPCB Std 2L | JLCPCB Std 4L | JLCPCB Adv 2L | JLCPCB Adv 4L | PCBWay       | OSHPark 2L   | OSHPark 4L   |
|------------------------|:-------------:|:-------------:|:-------------:|:-------------:|:------------:|:------------:|:------------:|
| Min Trace Width        | 0.127mm (5mil)| 0.100mm (4mil)| 0.090mm (3.5mil)| 0.090mm (3.5mil)| 0.150mm (6mil)| 0.150mm (6mil)| 0.127mm (5mil)|
| Min Clearance          | 0.127mm (5mil)| 0.100mm (4mil)| 0.090mm (3.5mil)| 0.090mm (3.5mil)| 0.150mm (6mil)| 0.150mm (6mil)| 0.127mm (5mil)|
| Min Drill Size         | 0.300mm       | 0.200mm       | 0.150mm       | 0.150mm       | 0.200mm      | 0.254mm (10mil)| 0.254mm (10mil)|
| Min Via Drill          | 0.300mm       | 0.200mm       | 0.150mm       | 0.150mm       | 0.200mm      | 0.254mm (10mil)| 0.254mm (10mil)|
| Min Annular Ring       | 0.150mm (6mil)| 0.125mm (5mil)| 0.100mm (4mil)| 0.100mm (4mil)| 0.150mm (6mil)| 0.127mm (5mil)| 0.100mm (4mil)|
| Min Silk Width         | 0.150mm       | 0.150mm       | 0.100mm       | 0.100mm       | 0.220mm      | 0.127mm (5mil)| 0.127mm (5mil)|
| Min Edge Clearance     | 0.300mm       | 0.250mm       | 0.200mm       | 0.200mm       | 0.300mm      | 0.381mm (15mil)| 0.381mm (15mil)|

### Manufacturing Capabilities

| Parameter              | JLCPCB Std 2L | JLCPCB Std 4L | JLCPCB Adv 2L | JLCPCB Adv 4L | PCBWay       | OSHPark 2L   | OSHPark 4L   |
|------------------------|:-------------:|:-------------:|:-------------:|:-------------:|:------------:|:------------:|:------------:|
| Max Copper Layers      | 2             | 4             | 2             | 4             | 14           | 2            | 4            |
| Copper Weight          | 1.0 oz        | 1.0 oz        | 1.0 oz        | 1.0 oz        | 1.0 oz       | 1.0 oz       | 1.0 oz       |
| Board Thickness        | 1.6mm         | 1.6mm         | 1.6mm         | 1.6mm         | 1.6mm        | 1.6mm        | 1.6mm        |
| Blind Vias             | ❌            | ❌            | ❌            | ✅            | ✅ (+cost)   | ❌           | ❌           |
| Buried Vias            | ❌            | ❌            | ❌            | ❌            | ✅ (+cost)   | ❌           | ❌           |
| Castellated Holes      | ❌            | ❌            | ✅            | ✅            | ✅           | ❌           | ❌           |
| Impedance Control      | ❌            | ❌            | ✅ (est.)     | ✅ (est.)     | ✅ (+cost)   | ❌           | ❌           |

### Pricing & Lead Time

| Parameter              | JLCPCB Std    | JLCPCB Adv    | PCBWay        | OSHPark       |
|------------------------|:-------------:|:-------------:|:-------------:|:-------------:|
| Approximate 2L Price   | ~$2 (5pcs)    | ~$10 (5pcs)   | ~$5 (10pcs)   | ~$5/sq.in     |
| Lead Time              | 1-3 days      | 3-5 days      | 3-5 days      | ~12 days      |
| Assembly Available     | ✅ (SMT)      | ✅ (SMT+THT)  | ✅ (SMT+THT)  | ❌            |
| Country                | China         | China         | China         | USA           |

*Pricing is approximate, for small prototype quantities. Subject to change.*

---

## Detailed Manufacturer Notes

### JLCPCB

Source: <https://jlcpcb.com/capabilities/pcb-capabilities>
Additional: <https://www.schemalyzer.com/en/blog/manufacturing/jlcpcb/jlcpcb-design-rules>
Capabilities verified: 2026-03-13

**Standard Process:**
- Most cost-effective option for prototypes
- 5mil trace/space on 2-layer (tighter than the commonly cited 6mil)
- 4-layer standard supports 4mil trace/space at standard pricing
- No impedance control, blind/buried vias, or castellated holes
- HASL finish by default, ENIG available at extra cost
- Panelization supported (V-cut and tab routing)

**Advanced Process:**
- 3.5mil trace/space — suitable for BGA breakout
- 0.15mm via drill — enables microvia-like density without true HDI
- Blind vias available on 4-layer advanced
- Castellated holes for module designs
- Impedance control available (±10% tolerance, manufacturer specifies stackup)

**Preset mapping in code:**
- `RulesPreset::Jlcpcb2Layer` → `jlcpcb::standard_2layer()`
- `RulesPreset::Jlcpcb4Layer` → `jlcpcb::standard_4layer()`
- `RulesPreset::JlcpcbAdvanced2Layer` → `jlcpcb::advanced_2layer()`
- `RulesPreset::JlcpcbAdvanced4Layer` → `jlcpcb::advanced_4layer()`

### PCBWay

Source: <https://www.pcbway.com/capabilities.html>
Capabilities verified: 2026-03-13

- 6mil trace/space as recommended minimum (some sources cite 5mil capability)
- 0.2mm minimum drill — smaller than JLCPCB standard
- Supports up to 14 copper layers
- Blind and buried vias available at extra cost
- Castellated holes supported
- Wide range of surface finishes (HASL, ENIG, OSP, immersion tin/silver)
- Flex and rigid-flex PCB capability
- Impedance control available with ±10% tolerance

**Preset mapping:** `RulesPreset::Pcbway` → `pcbway::standard()`

### OSHPark

Source: <https://docs.oshpark.com/design-tools/>
Additional: <https://docs.oshpark.com/services/>
Capabilities verified: 2026-03-13

- US-based manufacturer — higher quality ENIG finish standard, no extra cost
- Purple soldermask is the signature aesthetic
- 6mil trace/space on 2-layer, 5mil on 4-layer
- Larger minimum drill (10mil / 0.254mm) — all through-hole, no blind/buried
- 15mil edge clearance — more conservative than Chinese manufacturers
- After Dark service: 2-layer on FR408 with tighter tolerances
- SuperSwift service: ~5 day turnaround (vs. ~12 day standard)
- No assembly service — boards only
- Per-square-inch pricing model (vs. per-board at JLCPCB/PCBWay)

**Preset mapping:**
- `RulesPreset::Oshpark2Layer` → `oshpark::two_layer()`
- `RulesPreset::Oshpark4Layer` → `oshpark::four_layer()`

---

## How to Add a New Manufacturer

1. Research the manufacturer's capability page — document the URL and access date
2. Create a preset file in `crates/cypcb-rules/src/presets/` following the existing pattern
3. Add a `RulesPreset` enum variant in `presets/mod.rs`
4. Add a `Preset` variant in `crates/cypcb-drc/src/preset.rs` (if DRC preset needed)
5. Update this document with the new manufacturer's values
6. Write tests verifying key constraint values match the documented capabilities

---

## Sources

| Manufacturer | URL | Verified |
|-------------|-----|----------|
| JLCPCB | <https://jlcpcb.com/capabilities/pcb-capabilities> | 2026-03-13 |
| JLCPCB (alt) | <https://www.schemalyzer.com/en/blog/manufacturing/jlcpcb/jlcpcb-design-rules> | 2026-03-13 |
| PCBWay | <https://www.pcbway.com/capabilities.html> | 2026-03-13 |
| OSHPark | <https://docs.oshpark.com/design-tools/> | 2026-03-13 |
| OSHPark services | <https://docs.oshpark.com/services/> | 2026-03-13 |
