import { test, expect } from '@playwright/test';
import * as fs from 'fs';
import * as path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const BLINK_PATH = path.resolve(__dirname, '../../examples/blink.cypcb');

/** Load an example from `examples/` via the app's __loadBoard debug surface. */
async function loadExample(page: import('@playwright/test').Page, name: string): Promise<void> {
  const content = fs.readFileSync(path.resolve(__dirname, '../../examples', name), 'utf-8');
  await page.evaluate((src) => {
    const loader = (window as any).__loadBoard;
    if (loader) loader(src);
  }, content);
  await page.waitForTimeout(500);
}

/** Load blink.cypcb via the app's __loadBoard debug surface. */
async function loadBlink(page: import('@playwright/test').Page): Promise<void> {
  const content = fs.readFileSync(BLINK_PATH, 'utf-8');
  await page.evaluate((src) => {
    const loader = (window as any).__loadBoard;
    if (loader) loader(src);
  }, content);
  await page.waitForTimeout(500);
}

/** Activate 3D view and wait for renderer to be ready. */
async function activate3D(page: import('@playwright/test').Page): Promise<void> {
  await page.click('#view-3d-btn');
  await expect(page.locator('#view-3d-btn')).toHaveClass(/active/, { timeout: 5_000 });
  // Wait for renderer to finish building geometry
  await page.waitForFunction(
    () => (window as any).__renderer3d?.isActive === true,
    { timeout: 5_000 },
  );
  // Allow build methods to complete
  await page.waitForTimeout(300);
}

/** Read all geometry counters from the debug surface. */
async function getGeometryCounts(page: import('@playwright/test').Page) {
  return page.evaluate(() => {
    const r = (window as any).__renderer3d;
    return {
      componentCount: r?.componentCount,
      traceSegmentCount: r?.traceSegmentCount,
      padCount: r?.padCount,
      viaCount: r?.viaCount,
      padDrillCount: r?.padDrillCount,
      meshCount: r?.meshCount,
    };
  });
}

test.describe('3D View Toggle', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });
  });

  test('3D button activates Three.js renderer', async ({ page }) => {
    // Click 3D button
    await page.click('#view-3d-btn');

    // Wait for Three.js canvas to appear inside canvas-container
    // Three.js creates a <canvas> element inside the container
    await expect(page.locator('#view-3d-btn')).toHaveClass(/active/, { timeout: 5_000 });

    // Verify renderer3d debug surface reports active
    const isActive = await page.evaluate(() => {
      return (window as any).__renderer3d?.isActive === true;
    });
    expect(isActive).toBe(true);
  });

  test('pressing 3 key toggles 3D view', async ({ page }) => {
    // Dismiss project manager overlay so canvas is clickable
    const MINIMAL_BOARD = `version 1\nboard test {\n  size 50mm x 50mm\n  layers 2\n}`;
    await page.evaluate((src) => (window as any).__loadBoard(src), MINIMAL_BOARD);
    // Focus body to ensure key works
    await page.click('#pcb-canvas');
    await page.keyboard.press('3');

    await expect(page.locator('#view-3d-btn')).toHaveClass(/active/, { timeout: 5_000 });

    const isActive = await page.evaluate(() => {
      return (window as any).__renderer3d?.isActive === true;
    });
    expect(isActive).toBe(true);
  });

  test('toggling back to 2D removes 3D and restores canvas', async ({ page }) => {
    // Activate 3D
    await page.click('#view-3d-btn');
    await expect(page.locator('#view-3d-btn')).toHaveClass(/active/, { timeout: 5_000 });

    // 2D canvas should be hidden
    await expect(page.locator('#pcb-canvas')).toBeHidden();

    // Toggle back to 2D
    await page.click('#view-3d-btn');

    // Button should lose active class
    await expect(page.locator('#view-3d-btn')).not.toHaveClass(/active/);

    // 2D canvas should be visible again
    await expect(page.locator('#pcb-canvas')).toBeVisible();

    // renderer3d should be disposed
    const isActive = await page.evaluate(() => {
      return (window as any).__renderer3d?.isActive ?? false;
    });
    expect(isActive).toBe(false);
  });
});

test.describe('3D Geometry Verification', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });
  });

  test('3D view renders component geometry after loading board', async ({ page }) => {
    // Load a real board with components
    await loadBlink(page);

    // Toggle to 3D
    await activate3D(page);

    const counts = await getGeometryCounts(page);

    // blink.cypcb has multiple components — must see them in 3D
    expect(counts.componentCount).toBeGreaterThan(0);
    // Total mesh count includes board, components, pads, traces, vias
    expect(counts.meshCount).toBeGreaterThan(1);
  });

  test('a through-hole board gets its holes drilled in 3D', async ({ page }) => {
    // The 3D view built drilled cylinders for vias and nothing else, so a
    // board of through-hole parts came out solid - copper on both faces with
    // no hole between them. blink has a 2-pin header, so it has holes.
    await loadBlink(page);
    await activate3D(page);

    const counts = await getGeometryCounts(page);

    expect(typeof counts.padDrillCount).toBe('number');
    expect(
      counts.padDrillCount,
      'blink carries through-hole pads and none of them was drilled',
    ).toBeGreaterThan(0);
    expect(
      counts.padDrillCount,
      'more holes than pads would mean something is drilled twice',
    ).toBeLessThanOrEqual(counts.padCount);

    // The mesh reaches the scene rather than only the counter.
    const named = await page.evaluate(() => {
      const scene = (window as any).__renderer3d;
      return scene?.padDrillCount ?? -1;
    });
    expect(named).toBe(counts.padDrillCount);
  });

  test('the 3D scene holds the board the engine holds', async ({ page }) => {
    // This asserted `>= 0` on four counters, which is true of a scene that
    // built nothing at all - the exact failure a 3D view has. Every counter is
    // compared against the same board the 2D side draws instead, so a scene
    // that silently drops components, pads or copper fails here.
    //
    // The board is `uat-routing-locked` rather than blink, because blink has
    // no traces: comparing a segment count against it is 0 against 0, which is
    // the same kind of assertion this test is being fixed for.
    await loadExample(page, 'uat-routing-locked.cypcb');
    await activate3D(page);

    const counts = await getGeometryCounts(page);
    const board = await page.evaluate(() => {
      const snap = (window as any).__pcbEngine?.get_snapshot?.();
      return {
        components: snap?.components?.length ?? -1,
        vias: snap?.vias?.length ?? -1,
        segments: (snap?.traces ?? []).reduce(
          (n: number, t: any) => n + (t.segments?.length ?? 0),
          0,
        ),
      };
    });

    expect(board.components, 'the engine holds no board, so there is nothing to compare').toBeGreaterThan(0);
    expect(board.segments, 'the fixture lost its copper, so the segment count proves nothing').toBeGreaterThan(0);

    expect(counts.componentCount).toBe(board.components);
    expect(counts.traceSegmentCount).toBe(board.segments);
    expect(counts.viaCount).toBe(board.vias);

    // Pads are not in the snapshot as a count - they come from each part's
    // footprint - so what can be said is that every part brought copper with
    // it. A blink board of resistors, an LED and a header has more pads than
    // parts, and a scene with fewer built some parts without any.
    expect(counts.padCount).toBeGreaterThan(board.components);
  });

  test('3D toggle preserves geometry on re-toggle', async ({ page }) => {
    await loadBlink(page);

    // First toggle — capture counts
    await activate3D(page);
    const first = await getGeometryCounts(page);

    // Sanity: must have geometry to compare
    expect(first.componentCount).toBeGreaterThan(0);

    // Toggle back to 2D
    await page.click('#view-3d-btn');
    await expect(page.locator('#view-3d-btn')).not.toHaveClass(/active/);

    // Toggle to 3D again
    await activate3D(page);
    const second = await getGeometryCounts(page);

    // Re-init must reconstruct the same geometry
    expect(second.componentCount).toBe(first.componentCount);
    expect(second.meshCount).toBe(first.meshCount);
    expect(second.padCount).toBe(first.padCount);
    expect(second.viaCount).toBe(first.viaCount);
    expect(second.traceSegmentCount).toBe(first.traceSegmentCount);
  });
});
