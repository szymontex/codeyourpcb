# T03: Menu Data Model & Platform Facade

**Slice:** S08 — **Milestone:** M001

## Description

Add the Menu abstraction and Platform facade that provides a single entry point to all platform services.

Purpose: Menu completes the four required abstractions (PLAT-04). Platform facade gives application code one import to access all platform services, preventing business logic from importing platform-specific types. This is the integration layer.

Output: Menu types for declarative menu building, Platform struct aggregating all services.

## Must-Haves

- [ ] "Menu trait defines application menu structure declaratively"
- [ ] "Platform struct provides single entry point to all platform services"
- [ ] "Application code uses Platform struct, never touches platform-specific types directly"
- [ ] "Full crate compiles for both native and wasm32 targets with all modules"

## Files

- `crates/cypcb-platform/src/menu.rs`
- `crates/cypcb-platform/src/lib.rs`
- `crates/cypcb-platform/src/platform.rs`
