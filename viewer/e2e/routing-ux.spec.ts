import { test, expect } from '@playwright/test';
import * as fs from 'fs';
import * as path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const FIXTURE_PATH = path.resolve(__dirname, 'fixtures/routing-test.cypcb');

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Load routing-test.cypcb via __loadBoard, wait for render settle. */
async function loadFixture(page: import('@playwright/test').Page): Promise<void> {
  const content = fs.readFileSync(FIXTURE_PATH, 'utf-8');
  await page.evaluate((src) => {
    (window as any).__loadBoard(src);
  }, content);
  await page.waitForTimeout(600);
}

/**
 * Get the screen-coordinate center of a pad by component refdes + pad number.
 * Uses the snapshot from __pcbEngine and the viewport worldToScreen math
 * (reimplemented in browser context to avoid importing TS modules).
 */
async function getPadScreenCoords(
  page: import('@playwright/test').Page,
  refdes: string,
  padNumber: string,
): Promise<{ x: number; y: number }> {
  const coords = await page.evaluate(
    ({ refdes, padNumber }) => {
      const engine = (window as any).__pcbEngine;
      if (!engine) throw new Error('__pcbEngine not exposed');
      const snap = engine.get_snapshot();
      if (!snap) throw new Error('No snapshot');

      // Find component
      const comp = snap.components.find((c: any) => c.refdes === refdes);
      if (!comp) throw new Error(`Component ${refdes} not found`);

      // Find pad
      const pad = comp.pads.find((p: any) => p.number === padNumber);
      if (!pad) throw new Error(`Pad ${padNumber} not found on ${refdes}`);

      // Compute pad world position (with rotation)
      const radians = (comp.rotation_mdeg / 1000) * (Math.PI / 180);
      const cos = Math.cos(radians);
      const sin = Math.sin(radians);
      const rx = pad.x_nm * cos - pad.y_nm * sin;
      const ry = pad.x_nm * sin + pad.y_nm * cos;
      const worldX = comp.x_nm + rx;
      const worldY = comp.y_nm + ry;

      // Read actual viewport from diagnostic surface
      const vp = (window as any).__viewport;
      if (!vp) throw new Error('__viewport not exposed');

      const canvas = document.getElementById('pcb-canvas') as HTMLCanvasElement;
      const rect = canvas.getBoundingClientRect();

      // worldToScreen (matching viewport.ts)
      const sx = (worldX - vp.centerX) * vp.scale + vp.width / 2;
      const sy = vp.height / 2 - (worldY - vp.centerY) * vp.scale;

      // Convert canvas-local coords to page coords
      return { x: rect.left + sx, y: rect.top + sy };
    },
    { refdes, padNumber },
  );
  return coords;
}

/** Read __routingState diagnostic surface. */
async function getRoutingState(page: import('@playwright/test').Page) {
  return page.evaluate(() => {
    const rs = (window as any).__routingState;
    if (!rs) return null;
    return {
      mode: rs.mode,
      netName: rs.netName,
      currentLayer: rs.currentLayer,
      angleSnapEnabled: rs.angleSnapEnabled,
      magneticSnapEnabled: rs.magneticSnapEnabled,
      snappedToPad: rs.snappedToPad,
      targetPadsCount: rs.targetPadsCount,
      committedSegments: rs.committedSegments,
    };
  });
}

/** Read __renderDiag.highlightedNet. */
async function getHighlightedNet(page: import('@playwright/test').Page): Promise<string | null> {
  return page.evaluate(() => (window as any).__renderDiag?.highlightedNet ?? null);
}

/** Get trace count from snapshot. */
async function getTraceCount(page: import('@playwright/test').Page): Promise<number> {
  return page.evaluate(() => {
    const engine = (window as any).__pcbEngine;
    if (!engine) return 0;
    const snap = engine.get_snapshot();
    return snap?.traces?.length ?? 0;
  });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

test.describe('Routing UX — E2E', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });
  });

  test('fixture loads with expected components and nets', async ({ page }) => {
    await loadFixture(page);

    const info = await page.evaluate(() => {
      const engine = (window as any).__pcbEngine;
      const snap = engine.get_snapshot();
      return {
        componentCount: snap.components.length,
        netCount: snap.nets.length,
        refdesces: snap.components.map((c: any) => c.refdes).sort(),
        netNames: snap.nets.map((n: any) => n.name).sort(),
      };
    });

    expect(info.componentCount).toBe(3);
    expect(info.netCount).toBeGreaterThanOrEqual(3);
    expect(info.refdesces).toEqual(['LED1', 'R1', 'R2']);
    expect(info.netNames).toContain('POWER');
    expect(info.netNames).toContain('SIGNAL');
    expect(info.netNames).toContain('GROUND');
  });

  test('start route on pad → routing mode active + correct net + highlight set', async ({ page }) => {
    await loadFixture(page);

    // Verify idle state
    const before = await getRoutingState(page);
    expect(before?.mode).toBe('idle');

    const beforeHighlight = await getHighlightedNet(page);
    expect(beforeHighlight).toBeNull();

    // Click pad R1.1 (on POWER net) to start routing
    const r1p1 = await getPadScreenCoords(page, 'R1', '1');
    await page.mouse.click(r1p1.x, r1p1.y);
    await page.waitForTimeout(300);

    const after = await getRoutingState(page);
    expect(after?.mode).toBe('routing');
    expect(after?.netName).toBe('POWER');
    expect(after?.targetPadsCount).toBeGreaterThan(0);

    const highlight = await getHighlightedNet(page);
    expect(highlight).toBe('POWER');
  });

  test('complete route pad-to-pad → trace added + highlight cleared + idle mode', async ({ page }) => {
    await loadFixture(page);

    const tracesBefore = await getTraceCount(page);

    // Start route from R1.1 (POWER net)
    const r1p1 = await getPadScreenCoords(page, 'R1', '1');
    await page.mouse.click(r1p1.x, r1p1.y);
    await page.waitForTimeout(300);

    // Verify routing started
    const routing = await getRoutingState(page);
    expect(routing?.mode).toBe('routing');
    expect(routing?.netName).toBe('POWER');

    // Click target pad R2.1 (also on POWER net) to complete route
    const r2p1 = await getPadScreenCoords(page, 'R2', '1');

    // Move mouse to target first to trigger magnetic snap preview
    await page.mouse.move(r2p1.x, r2p1.y);
    await page.waitForTimeout(200);

    await page.mouse.click(r2p1.x, r2p1.y);
    await page.waitForTimeout(400);

    // Verify route completed
    const afterState = await getRoutingState(page);
    expect(afterState?.mode).toBe('idle');

    const afterHighlight = await getHighlightedNet(page);
    expect(afterHighlight).toBeNull();

    const tracesAfter = await getTraceCount(page);
    expect(tracesAfter).toBe(tracesBefore + 1);
  });

  test('cancel route with Escape → idle mode + highlight cleared + no trace added', async ({ page }) => {
    await loadFixture(page);

    const tracesBefore = await getTraceCount(page);

    // Start route from R1.2 (SIGNAL net)
    const r1p2 = await getPadScreenCoords(page, 'R1', '2');
    await page.mouse.click(r1p2.x, r1p2.y);
    await page.waitForTimeout(300);

    // Verify routing started
    const routing = await getRoutingState(page);
    expect(routing?.mode).toBe('routing');
    expect(routing?.netName).toBe('SIGNAL');

    const highlight = await getHighlightedNet(page);
    expect(highlight).toBe('SIGNAL');

    // Cancel with Escape
    await page.keyboard.press('Escape');
    await page.waitForTimeout(300);

    // Verify cancelled
    const afterState = await getRoutingState(page);
    expect(afterState?.mode).toBe('idle');

    const afterHighlight = await getHighlightedNet(page);
    expect(afterHighlight).toBeNull();

    const tracesAfter = await getTraceCount(page);
    expect(tracesAfter).toBe(tracesBefore);
  });

  test('angle toggle with A key → angleSnapEnabled flips', async ({ page }) => {
    await loadFixture(page);

    // Start route to enter routing mode (A key only works during routing)
    const r1p1 = await getPadScreenCoords(page, 'R1', '1');
    await page.mouse.click(r1p1.x, r1p1.y);
    await page.waitForTimeout(300);

    const before = await getRoutingState(page);
    expect(before?.mode).toBe('routing');
    expect(before?.angleSnapEnabled).toBe(true); // KiCad: 45 degree snap starts on

    // Press A to toggle angle snap off
    await page.keyboard.press('a');
    await page.waitForTimeout(100);

    const afterOn = await getRoutingState(page);
    expect(afterOn?.angleSnapEnabled).toBe(false);

    // Press A again to toggle back on
    await page.keyboard.press('a');
    await page.waitForTimeout(100);

    const afterOff = await getRoutingState(page);
    expect(afterOff?.angleSnapEnabled).toBe(true);
  });

  test('layer flip with F key → currentLayer toggles', async ({ page }) => {
    await loadFixture(page);

    // Start route
    const r1p1 = await getPadScreenCoords(page, 'R1', '1');
    await page.mouse.click(r1p1.x, r1p1.y);
    await page.waitForTimeout(300);

    const before = await getRoutingState(page);
    expect(before?.mode).toBe('routing');
    const initialLayer = before?.currentLayer;

    // Press F to flip layer
    await page.keyboard.press('f');
    await page.waitForTimeout(100);

    const afterFlip = await getRoutingState(page);
    expect(afterFlip?.currentLayer).not.toBe(initialLayer);

    // Press F again — should return to original
    await page.keyboard.press('f');
    await page.waitForTimeout(100);

    const afterFlipBack = await getRoutingState(page);
    expect(afterFlipBack?.currentLayer).toBe(initialLayer);
  });
});
