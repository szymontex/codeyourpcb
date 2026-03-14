# T03: Board Outline & Silkscreen Export

**Slice:** S04 — **Milestone:** M001

## Description

Implement board outline and silkscreen Gerber export.

Purpose: Board outline defines the physical board shape for routing/cutting. Silkscreen provides component labels and assembly markings. Both are essential manufacturing files.

Output: Functions to export board outline and silkscreen Gerber files.

## Must-Haves

- [ ] "Board outline Gerber contains closed path defining board edge"
- [ ] "Silkscreen layer contains component designators"
- [ ] "Outline uses Profile file function per X2 spec"

## Files

- `crates/cypcb-export/src/gerber/outline.rs`
- `crates/cypcb-export/src/gerber/silk.rs`
- `crates/cypcb-export/src/gerber/mod.rs`
