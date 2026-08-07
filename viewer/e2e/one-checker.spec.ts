import { test, expect } from '@playwright/test';

/**
 * The browser gets its design rules from the engine, and only from the engine.
 *
 * `silk-clearance` and `trace-current` used to be written twice: as Rust rules
 * the engine runs, and again as TypeScript in `wasm.ts` whose results the
 * adapter appended. A board that tripped either was reported twice, under two
 * different names, and the copies had drifted - the Rust silk rule learned
 * about printed designators and about clipping the legend off copper, and the
 * TypeScript one knew about neither.
 *
 * Deleting the TypeScript copies is only safe if the Rust ones actually reach
 * the screen. This loads a board that trips each rule and asks the running app
 * what it found.
 */

/** Two parts close enough that one part's legend lands on the other's copper. */
const A_BOARD_WITH_A_SILK_FAULT = `
board silk {
    size 20mm x 20mm
    layers 2
}

component R1 resistor "0805" {
    value "10k"
    at 10mm, 10mm
}

component R2 resistor "0805" {
    value "10k"
    at 11.5mm, 10mm
}
`;

/** A net that states a current its trace is far too thin to carry. */
const A_BOARD_WITH_A_CURRENT_FAULT = `
board thin {
    size 20mm x 20mm
    layers 2
}

component R1 resistor "0805" {
    value "10k"
    at 5mm, 10mm
}

component R2 resistor "0805" {
    value "10k"
    at 15mm, 10mm
}

net VCC [current 3A] {
    R1.1
    R2.1
}

trace VCC {
    layer Top
    width 0.2mm
    path 5mm,10mm -> 15mm,10mm
}
`;

async function violationsFor(page: import('@playwright/test').Page, source: string) {
  await page.goto('/');
  await page.waitForFunction(() => typeof (window as never as { __loadBoard?: unknown }).__loadBoard !== 'undefined');
  await page.evaluate((src) => (window as never as { __loadBoard: (s: string) => void }).__loadBoard(src), source);
  await page.waitForTimeout(500);
  return page.evaluate(() => {
    const engine = (window as never as { __pcbEngine?: { get_snapshot(): { violations?: { kind: string; message: string }[] } } }).__pcbEngine;
    return engine ? (engine.get_snapshot().violations ?? []) : null;
  });
}

test.describe('one checker', () => {
  test('the engine reports a silkscreen fault to the browser', async ({ page }) => {
    const violations = await violationsFor(page, A_BOARD_WITH_A_SILK_FAULT);
    test.skip(violations === null, 'the app exposes no engine handle in this build');

    const silk = violations!.filter((v) => v.kind === 'silk-clearance');
    expect(silk.length).toBeGreaterThan(0);
    // The Rust rule's wording, not the TypeScript one's `silk <-> pad`.
    expect(silk[0].message).toContain('silkscreen over');
  });

  test('the engine reports a trace too thin for its current', async ({ page }) => {
    const violations = await violationsFor(page, A_BOARD_WITH_A_CURRENT_FAULT);
    test.skip(violations === null, 'the app exposes no engine handle in this build');

    expect(violations!.filter((v) => v.kind === 'trace-current').length).toBeGreaterThan(0);

    // The TypeScript copy emitted `trace-width-current` under its own name.
    // Nothing emits it now, and nothing should: two names for one defect is
    // what this change removed.
    expect(violations!.filter((v) => v.kind === 'trace-width-current')).toHaveLength(0);
  });
});
