import { test, expect } from '@playwright/test';

/**
 * A rigid-flex board is drawn as three boxes, and the middle one is thinner.
 *
 * The unit test holds the arithmetic; this holds the browser to it, because
 * the figures it needs come out of the engine's snapshot rather than out of a
 * fixture: the stack's total thickness and the stiffener bonded on to stop
 * part of the board flexing.
 */

const RIGID_FLEX = `version 1

board ribbon {
    size 60mm x 16mm
    layers 2

    stackup {
        coverlay "cover top" 0.025mm material "PI"
        copper "F.Cu" 0.5oz
        core "flex core" 0.05mm material "PI" dk 3.4
        copper "B.Cu" 0.5oz
        coverlay "cover bottom" 0.025mm material "PI"
        stiffener 0.2mm material "FR4"
    }
}

flex bend {
    bounds 22mm, 0mm to 38mm, 16mm
    layer all
}
`;

const PLAIN = `version 1

board slab {
    size 60mm x 16mm
    layers 2
}
`;

test('the board is three boxes where it bends, and one where it does not', async ({ page }) => {
  await page.goto('/');
  await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });

  await page.evaluate((src) => (window as any).__loadBoard(src), PLAIN);
  await page.waitForTimeout(300);
  await page.click('#view-3d-btn');
  await page.waitForFunction(() => (window as any).__renderer3d?.isActive === true, {
    timeout: 5_000,
  });
  await page.waitForTimeout(300);
  expect(
    await page.evaluate(() => (window as any).__renderer3d?.substrateSlabCount),
    'an ordinary board is one slab',
  ).toBe(1);
  expect(
    await page.evaluate(() => (window as any).__renderer3d?.maskStepMm),
    'and its solder mask is flat',
  ).toBeCloseTo(0, 6);

  await page.evaluate((src) => (window as any).__loadBoard(src), RIGID_FLEX);
  await page.waitForTimeout(300);
  await page.click('#view-3d-btn');
  await expect(page.locator('#view-3d-btn')).not.toHaveClass(/active/, { timeout: 5_000 });
  await page.click('#view-3d-btn');
  await page.waitForFunction(() => (window as any).__renderer3d?.isActive === true, {
    timeout: 5_000,
  });
  await page.waitForTimeout(300);

  expect(
    await page.evaluate(() => (window as any).__renderer3d?.substrateSlabCount),
    'the ribbon splits the board into rigid, bend, rigid',
  ).toBe(3);

  // 0.335mm of board against 0.135mm of ribbon: each face 0.1mm nearer the
  // middle. The mask is drawn cell by cell, so the step is in its own
  // geometry rather than only in the number the renderer worked out.
  expect(
    await page.evaluate(() => (window as any).__renderer3d?.bendDropMm),
    'the faces drop by half what the stiffener adds',
  ).toBeCloseTo(0.1, 3);
  expect(
    await page.evaluate(() => (window as any).__renderer3d?.maskStepMm),
    'and the mask itself steps by the same amount',
  ).toBeCloseTo(0.1, 3);
});
