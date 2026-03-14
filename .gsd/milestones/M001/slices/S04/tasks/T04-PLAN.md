# T04: Excellon Drill Export

**Slice:** S04 — **Milestone:** M001

## Description

Implement Excellon drill file export for through-hole pads and vias.

Purpose: Excellon drill files tell the manufacturer where to drill holes. Essential for through-hole components, mounting holes, and vias. Misaligned drills = unusable boards.

Output: Excellon drill file export with proper header, tool definitions, and coordinates.

## Must-Haves

- [ ] "Excellon file contains tool definitions with correct drill sizes"
- [ ] "Drill hits export at correct coordinates"
- [ ] "File header specifies metric units"

## Files

- `crates/cypcb-export/src/excellon/mod.rs`
- `crates/cypcb-export/src/excellon/writer.rs`
- `crates/cypcb-export/src/excellon/tools.rs`
- `crates/cypcb-export/src/lib.rs`
