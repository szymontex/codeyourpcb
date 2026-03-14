# T05: BOM & Pick-and-Place Export

**Slice:** S04 — **Milestone:** M001

## Description

Implement BOM (Bill of Materials) and CPL (Component Placement List/Pick-and-Place) file export.

Purpose: BOM tells assemblers what parts to order. CPL tells pick-and-place machines where to place them. Both are essential for PCBA (PCB assembly).

Output: BOM export in CSV/JSON formats, CPL export in CSV format matching JLCPCB requirements.

## Must-Haves

- [ ] "BOM groups identical components and counts quantity"
- [ ] "BOM contains Designator, Footprint, Value columns"
- [ ] "Pick-and-place file contains X/Y coordinates in mm"
- [ ] "Rotation angle exports in degrees"

## Files

- `crates/cypcb-export/src/bom/mod.rs`
- `crates/cypcb-export/src/bom/csv.rs`
- `crates/cypcb-export/src/bom/json.rs`
- `crates/cypcb-export/src/cpl/mod.rs`
- `crates/cypcb-export/src/cpl/csv.rs`
- `crates/cypcb-export/src/lib.rs`
