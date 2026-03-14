# T05: Clearance Checking

**Slice:** S03 — **Milestone:** M001

## Description

Implement clearance checking rule using the spatial index.

Purpose: DRC-01 requirement - detect copper features that are too close together for manufacturing. This is the core DRC rule that catches most design errors.

Output: ClearanceRule struct implementing DrcRule trait with two-phase spatial checking.

## Must-Haves

- [ ] "Clearance rule detects pads closer than min_clearance"
- [ ] "Same-net items are exempt from clearance checking"
- [ ] "Layer filtering prevents false positives on different layers"
- [ ] "Duplicate pair checking is avoided (A-B not checked again as B-A)"

## Files

- `crates/cypcb-drc/src/rules/clearance.rs`
- `crates/cypcb-drc/src/rules/mod.rs`
