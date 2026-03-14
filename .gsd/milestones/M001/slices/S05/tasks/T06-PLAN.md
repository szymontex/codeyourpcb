# T06: FreeRouting SES Import & CLI

**Slice:** S05 — **Milestone:** M001

## Description

Implement SES import and FreeRouting CLI integration to complete the autorouting pipeline.

Purpose: After DSN export (05-04), we need to import FreeRouting's output and manage the external process. INT-01 requirement (second half).
Output: Complete FreeRouting integration with import and CLI wrapper

## Must-Haves

- [ ] "SES files from FreeRouting parse to RouteSegments"
- [ ] "FreeRouting CLI runs with timeout"
- [ ] "Routing can be cancelled"
- [ ] "Partial results returned if routing incomplete"

## Files

- `crates/cypcb-router/src/ses.rs`
- `crates/cypcb-router/src/freerouting.rs`
- `crates/cypcb-router/src/lib.rs`
