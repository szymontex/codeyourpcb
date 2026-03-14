# T03: Manufacturer Presets

**Slice:** S03 — **Milestone:** M001

## Description

Implement manufacturer preset structs for JLCPCB and PCBWay design rules.

Purpose: Provide type-safe design rule configurations that DRC rules will check against. Users can select a manufacturer preset to validate their design against real fabrication constraints.

Output: DesignRules struct with constructors for JLCPCB 2-layer, JLCPCB 4-layer, PCBWay standard, and prototype presets.

## Must-Haves

- [ ] "DesignRules struct contains all manufacturer constraints"
- [ ] "JLCPCB 2-layer preset matches documented specs"
- [ ] "PCBWay standard preset matches documented specs"
- [ ] "Presets can be loaded via preset name"

## Files

- `crates/cypcb-drc/src/presets/mod.rs`
- `crates/cypcb-drc/src/presets/jlcpcb.rs`
- `crates/cypcb-drc/src/presets/pcbway.rs`
- `crates/cypcb-drc/src/lib.rs`
- `crates/cypcb-drc/src/rules/mod.rs`
