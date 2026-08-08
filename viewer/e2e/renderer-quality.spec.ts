import { test, expect } from '@playwright/test';
import * as fs from 'fs';
import * as path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const BLINK_PATH = path.resolve(__dirname, '../../examples/blink.cypcb');

/**
 * Helper: load blink.cypcb via the app's __loadBoard surface.
 * This triggers load_source + pullSnapshot + fitBoard + re-render.
 */
async function loadBlink(page: import('@playwright/test').Page): Promise<void> {
  const content = fs.readFileSync(BLINK_PATH, 'utf-8');
  await page.evaluate((src) => {
    const loader = (window as any).__loadBoard;
    if (loader) loader(src);
  }, content);
  // Wait for render cycle to complete
  await page.waitForTimeout(500);
}

/** Helper: load any example from `examples/` the same way. */
async function loadExample(page: import('@playwright/test').Page, name: string): Promise<void> {
  const content = fs.readFileSync(path.resolve(__dirname, '../../examples', name), 'utf-8');
  await page.evaluate((src) => {
    const loader = (window as any).__loadBoard;
    if (loader) loader(src);
  }, content);
  await page.waitForTimeout(500);
}

/**
 * Helper: zoom the canvas via wheel events.
 * Negative deltaY = zoom in, positive = zoom out.
 */
async function zoomCanvas(
  page: import('@playwright/test').Page,
  steps: number,
  deltaY: number,
): Promise<void> {
  const box = await page.locator('#pcb-canvas').boundingBox();
  if (!box) throw new Error('Canvas not found for zoom');
  // Move mouse to center of canvas first
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  for (let i = 0; i < steps; i++) {
    await page.mouse.wheel(0, deltaY);
    await page.waitForTimeout(30);
  }
  // Let the last render frame settle
  await page.waitForTimeout(200);
}

test.describe('Renderer Quality — Professional Visuals', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });
  });

  test('canvas renders with non-zero dimensions', async ({ page }) => {
    const canvas = page.locator('#pcb-canvas');
    await expect(canvas).toBeVisible();
    const box = await canvas.boundingBox();
    expect(box).toBeTruthy();
    expect(box!.width).toBeGreaterThan(100);
    expect(box!.height).toBeGreaterThan(100);
  });

  test('diagnostic surface is exposed with correct shape', async ({ page }) => {
    await page.waitForTimeout(300);
    const diag = await page.evaluate(() => (window as any).__renderDiag);
    expect(diag).toBeTruthy();
    expect(diag).toHaveProperty('lodTier');
    expect(diag).toHaveProperty('padNetMapSize');
    expect(diag).toHaveProperty('lastFrameMs');
    expect(diag).toHaveProperty('textElementsDrawn');
    expect(diag).toHaveProperty('highlightedNet');
    expect(typeof diag.lodTier).toBe('number');
    expect(typeof diag.lastFrameMs).toBe('number');
  });

  test('padNetMap populated after loading blink.cypcb', async ({ page }) => {
    await loadBlink(page);
    const diag = await page.evaluate(() => (window as any).__renderDiag);
    expect(diag).toBeTruthy();
    expect(diag.padNetMapSize).toBeGreaterThan(0);
  });

  test('component count matches snapshot expectations', async ({ page }) => {
    await loadBlink(page);
    // Wait for snapshot to propagate through engine
    await page.waitForTimeout(500);
    const componentCount = await page.evaluate(() => {
      const engine = (window as any).__pcbEngine;
      if (!engine) return 0;
      const snap = engine.get_snapshot();
      return snap?.components?.length ?? 0;
    });
    // blink.cypcb has components (U1, R1, R2, C1, C2, D1, J1)
    expect(componentCount).toBeGreaterThanOrEqual(5);
  });

  test('LOD tier changes with zoom — close/detail at zoom-in', async ({ page }) => {
    await loadBlink(page);

    // Fit board first so we start from a known state
    await page.click('#pcb-canvas');
    await page.keyboard.press('f');
    await page.waitForTimeout(300);

    const diagBefore = await page.evaluate(() => (window as any).__renderDiag);
    const tierBefore = diagBefore?.lodTier ?? -1;

    // Zoom in heavily (15 wheel steps × -200 delta)
    await zoomCanvas(page, 20, -200);

    const diagAfter = await page.evaluate(() => (window as any).__renderDiag);
    const tierAfter = diagAfter?.lodTier ?? -1;

    // After heavy zoom-in, tier should be Close (2) or Detail (3)
    expect(tierAfter).toBeGreaterThanOrEqual(2);
    expect(tierAfter).toBeGreaterThanOrEqual(tierBefore);
  });

  test('text elements drawn at close zoom, none at far zoom', async ({ page }) => {
    await loadBlink(page);

    // Fit board then zoom in to close level
    await page.click('#pcb-canvas');
    await page.keyboard.press('f');
    await page.waitForTimeout(300);
    await zoomCanvas(page, 20, -200);

    const diagClose = await page.evaluate(() => (window as any).__renderDiag);
    // At close zoom, text elements (refdes, pad numbers, net labels) should be drawn
    expect(diagClose.textElementsDrawn).toBeGreaterThan(0);

    // Now zoom way out to Far tier
    await zoomCanvas(page, 40, 300);

    const diagFar = await page.evaluate(() => (window as any).__renderDiag);
    // At far zoom, lodTier should be Far (0) and no text drawn
    expect(diagFar.lodTier).toBe(0); // LodTier.Far
    expect(diagFar.textElementsDrawn).toBe(0);
  });

  test('net highlight activates when clicking a trace', async ({ page }) => {
    // `blink.cypcb` has nine components, seven nets and **no traces** - it is
    // an unrouted board. This test loaded it, guarded its whole body behind
    // `if (traceCount > 0)`, and has therefore never executed a single line of
    // what it is named after. `uat-routing-locked` has copper on it.
    await loadExample(page, 'uat-routing-locked.cypcb');

    // Fit board to see all traces (press F on document — no canvas click to avoid hitting a pad)
    await page.keyboard.press('f');
    await page.waitForTimeout(300);

    // Ensure no routing is active (clear any accidental pad hit from prior interactions)
    await page.keyboard.press('Escape');
    await page.waitForTimeout(100);

    // Verify no net is highlighted initially
    const diagBefore = await page.evaluate(() => (window as any).__renderDiag);
    expect(diagBefore.highlightedNet).toBeNull();

    // Where a trace actually is on screen. The previous version of this test
    // read a segment midpoint out of the snapshot, threw it away, clicked the
    // middle of the canvas and then only asserted anything `if` something had
    // been highlighted - so it passed whether or not net highlighting worked at
    // all. `main.ts` publishes the viewport for exactly this, and the transform
    // is the one in `viewport.ts`.
    const target = await page.evaluate(() => {
      const snap = (window as any).__pcbEngine?.get_snapshot?.();
      const trace = snap?.traces?.find((t: any) => t.segments?.length > 0);
      if (!trace) return null;

      const seg = trace.segments[0];
      const worldX = (seg.start_x + seg.end_x) / 2;
      const worldY = (seg.start_y + seg.end_y) / 2;

      const vp = (window as any).__viewport;
      const rect = (document.getElementById('pcb-canvas') as HTMLCanvasElement).getBoundingClientRect();
      return {
        netName: trace.net_name,
        traceId: trace.id,
        // worldToScreen, plus the canvas' own offset in the page
        x: rect.x + (worldX - vp.centerX) * vp.scale + vp.width / 2,
        y: rect.y + vp.height / 2 - (worldY - vp.centerY) * vp.scale,
      };
    });

    expect(target, 'the board has no trace with segments, so there is nothing to click').not.toBeNull();

    await page.mouse.click(target!.x, target!.y);
    await page.waitForTimeout(300);

    const diagAfter = await page.evaluate(() => (window as any).__renderDiag);
    expect(
      diagAfter.highlightedNet,
      `clicked the midpoint of a segment of net ${target!.netName} and nothing was highlighted`,
    ).toBe(target!.netName);

    // The same click selects the trace it hit - one click, both effects.
    const selected = await page.evaluate(() => (window as any).__renderState?.selectedTraceId);
    expect(selected).toBe(target!.traceId);

    // And Escape gives the board back, which is the other half of the feature.
    await page.keyboard.press('Escape');
    await page.waitForTimeout(200);
    const diagCleared = await page.evaluate(() => (window as any).__renderDiag);
    expect(diagCleared.highlightedNet).toBeNull();
  });

  test('performance sanity — frame renders under 32ms at close zoom', async ({ page }) => {
    await loadBlink(page);

    // Fit board then zoom in
    await page.click('#pcb-canvas');
    await page.keyboard.press('f');
    await page.waitForTimeout(300);
    await zoomCanvas(page, 15, -200);

    // Force a re-render by toggling a layer (open View menu first)
    await page.click('#view-menu-btn');
    const topCb = page.locator('#layer-top');
    await topCb.uncheck();
    await page.waitForTimeout(100);
    await topCb.check();
    await page.waitForTimeout(200);
    await page.click('#view-menu-btn'); // close menu

    const diag = await page.evaluate(() => (window as any).__renderDiag);
    // 32ms = 2× headroom over 16ms budget for headless rendering overhead
    expect(diag.lastFrameMs).toBeLessThan(32);
  });
});
