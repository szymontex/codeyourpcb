# T06: Drill, Trace & Connectivity Rules

**Slice:** S03 — **Milestone:** M001

## Description

Implement drill size, unconnected pin, and trace width DRC rules.

Purpose: DRC-02 (minimum trace width), DRC-03 (minimum drill size) and DRC-04 (unconnected pin detection) requirements. These catch manufacturability issues and incomplete designs.

Output: MinDrillSizeRule, UnconnectedPinRule, and MinTraceWidthRule (placeholder) structs implementing DrcRule trait.

## Must-Haves

- [ ] "Drill size rule detects holes smaller than min_drill_size"
- [ ] "Unconnected pin rule detects pads with no net assignment"
- [ ] "Trace width rule placeholder exists (deferred - no traces yet)"
- [ ] "Both active rules use ECS queries efficiently"

## Files

- `crates/cypcb-drc/src/rules/drill_size.rs`
- `crates/cypcb-drc/src/rules/connectivity.rs`
- `crates/cypcb-drc/src/rules/trace_width.rs`
- `crates/cypcb-drc/src/rules/mod.rs`
- `crates/cypcb-drc/src/lib.rs`
