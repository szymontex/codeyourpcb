---
estimated_steps: 4
estimated_files: 4
---

# T02: Version naming, M002 bug verification, and milestone DOD signoff

**Slice:** S07 — Polish, Bugs & Verification
**Milestone:** M003

## Description

Final task of M003. Updates version strings to `0.1.0-beta` across all 3 locations, adds the missing E2E test for the Preferences modal theme button (M002 single-click bug fix), confirms errors.spec.ts stability, and runs the full 8-stage quality gate as the definitive milestone DOD verification.

## Steps

1. Update version to `0.1.0-beta` in:
   - `viewer/package.json` (`"version": "0.1.0-beta"`)
   - `Cargo.toml` workspace version (`version = "0.1.0-beta"`)
   - `src-tauri/tauri.conf.json` (`"version": "0.1.0-beta"`)

2. Add E2E test in `viewer/e2e/theme.spec.ts` for the Preferences modal theme button:
   - Open Preferences modal (click `#prefs-btn`)
   - Read current `data-theme` from `<html>`
   - Click `#prefs-theme-btn` once
   - Assert `data-theme` has changed (single-click cycles theme — verifies M002 bug fix)
   - Dismiss preferences modal

3. Run `npx playwright test e2e/errors.spec.ts` 5 times in sequence to confirm the "handles invalid input" test is stable. If any run fails, diagnose and fix.

4. Run `bash scripts/quality-gate.sh` — all 8 stages must pass. Cross-check each milestone DOD item:
   - 2D board view visual quality (S01 E2E tests)
   - 3D rendering (S02 E2E tests)
   - Routing flow (S03 E2E tests)
   - Preferences + units (S04 E2E tests)
   - Project manager (S05 E2E tests)
   - JLCPCB search (S06 E2E tests)
   - Toolbar clean / View menu (S04 structure tests)
   - M002 bugs fixed (theme.spec.ts + existing tests)
   - E2E suite coverage (93+ tests)
   - Quality gate green (all 8 stages)

## Must-Haves

- [ ] Version `0.1.0-beta` in all 3 files
- [ ] Prefs-theme E2E test passes
- [ ] errors.spec.ts stable across 5 runs
- [ ] All 8 quality gate stages green
- [ ] All 10 milestone DOD items verified

## Verification

- `bash scripts/quality-gate.sh` — 8/8 stages pass
- `npx playwright test e2e/theme.spec.ts` — all pass (including new prefs-theme test)
- `grep -r '0.1.0-beta' viewer/package.json Cargo.toml src-tauri/tauri.conf.json` — matches in all 3 files
- `npx playwright test e2e/errors.spec.ts` — stable across 5 consecutive runs

## Inputs

- T01 completed — quality gate stages 4 and 8 now pass
- `viewer/e2e/theme.spec.ts` — existing theme tests to extend
- `viewer/index.html` line 946 — `#prefs-theme-btn` exists in Preferences modal
- M003 roadmap DOD checklist — 10 items to verify

## Observability Impact

- Version string: `grep -r '0.1.0-beta' viewer/package.json Cargo.toml src-tauri/tauri.conf.json` — confirms all 3 locations updated. Future agents can check version alignment with this one-liner.
- Prefs-theme test: `npx playwright test e2e/theme.spec.ts --grep "Preferences modal"` — verifies the M002 single-click bug stays fixed. Fails if the button requires double-click again.
- errors.spec.ts stability: if this test was flaky, the fix is documented in T02-SUMMARY. Future flakiness should be investigated against the same root cause.
- Quality gate: `bash scripts/quality-gate.sh` — the definitive DOD check. All 8 stages green means M003 is shippable.

## Expected Output

- `viewer/package.json` — version `0.1.0-beta`
- `Cargo.toml` — version `0.1.0-beta`
- `src-tauri/tauri.conf.json` — version `0.1.0-beta`
- `viewer/e2e/theme.spec.ts` — new test for prefs-theme single-click
- Quality gate fully green — milestone DOD satisfied
