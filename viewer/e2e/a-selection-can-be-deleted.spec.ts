import { test, expect } from '@playwright/test';

/**
 * Select every trace on the board, then delete them.
 *
 * The editor held a single `selectedTraceId` until `7520c8f`, so `Ctrl+A` did
 * not exist and `Delete` could only ever remove one trace. The model that
 * replaced it is covered by unit tests; this is the half those cannot reach -
 * that the two keys, on a real board, in a real browser, actually take the
 * copper away.
 *
 * It needs a board with copper on it, and no example this project ships has
 * any: `grep -c '^trace ' examples/*.cypcb` returns 0 for every one of them.
 * Hence the fixture below rather than a file.
 */
const ROUTED = `version 1

board sel {
    size 40mm x 20mm
    layers 2
}

component R1 resistor "0402" {
    value "10k"
    at 8mm, 10mm
}

component R2 resistor "0402" {
    value "10k"
    at 32mm, 10mm
}

net SIG {
    R1.2
    R2.1
}

trace SIG {
    layer Top
    width 0.25mm
    path 9mm,10mm -> 20mm,10mm
}

trace SIG {
    layer Top
    width 0.25mm
    path 20mm,10mm -> 31mm,10mm
}

trace SIG {
    layer Bottom
    width 0.25mm
    path 9mm,6mm -> 31mm,6mm
}
`;

test.describe('selecting every trace and deleting it', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });
    await page.evaluate((src) => (window as never as { __loadBoard: (s: string) => void }).__loadBoard(src), ROUTED);
    await page.waitForTimeout(1500);
  });

  test('Ctrl+A takes the copper and Delete removes it', async ({ page }) => {
    await page.keyboard.press('Control+a');
    await expect(page.locator('#status-text')).toContainText('trace(s)', { timeout: 5_000 });

    // The count has to be the copper actually on the board, not a number the
    // status line invented - the old code printed one it did not hold.
    const selected = await page.locator('#status-text').textContent();
    expect(selected).toMatch(/Selected 3 trace\(s\)/);

    await page.keyboard.press('Delete');
    await expect(page.locator('#status-text')).toContainText('deleted', { timeout: 5_000 });
    expect(await page.locator('#status-text').textContent()).toMatch(/3 traces deleted/);

    // And they are gone rather than merely deselected: a second select-all
    // over the same board finds nothing left to take.
    await page.waitForTimeout(800);
    await page.keyboard.press('Control+a');
    await expect(page.locator('#status-text')).toContainText('Nothing to select', { timeout: 5_000 });
  });

  /**
   * A hidden layer is not selected, which is what stops somebody clearing the
   * front of a board and quietly taking the back with it.
   */
  test('select-all leaves the layers that are turned off alone', async ({ page }) => {
    // Turn the bottom copper off from the layers panel.
    await page.locator('#lp-copper .lp-row[data-layer="Bottom"] .lp-eye').click();
    await page.waitForTimeout(400);

    await page.keyboard.press('Control+a');
    await expect(page.locator('#status-text')).toContainText('trace(s)', { timeout: 5_000 });
    expect(await page.locator('#status-text').textContent()).toMatch(/Selected 2 trace\(s\)/);
  });
});
