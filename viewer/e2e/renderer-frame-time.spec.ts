import { test, expect } from '@playwright/test';

/**
 * How long a frame takes on a board with real copper on it.
 *
 * The renderer's frame budget had never been measured - the tracker carried
 * "render frame time on the largest example" as a queued item for weeks - so
 * nobody could say whether the viewer was fast, slow, or fast by accident.
 * `main.ts` times the `render()` call now and keeps the last hundred; this
 * drives the canvas hard enough to fill that buffer and reads it.
 *
 * The board is generated rather than loaded from `examples/`, because every
 * example there is small: the biggest is ninety lines. This one is 200 parts
 * and 100 traces.
 *
 * Measured 2026-08-08 in headless chromium on the build machine:
 *
 *   200 parts, 100 traces     median 0.70ms   p95 1.60ms
 *   1000 parts, 500 traces    median 2.50ms   p95 4.10ms
 *
 * One frame at 60Hz is 16.7ms, so the renderer has an order of magnitude in
 * hand even at five times this board. There is nothing to optimise here; what
 * this test is for is noticing the day that stops being true.
 */

/** A board with `parts` components and a trace joining every second pair. */
function heavyBoard(parts: number): string {
  const lines = [
    'version 1',
    '',
    'board stress {',
    '    size 200mm x 200mm',
    '    layers 2',
    '}',
    '',
  ];

  const perRow = 20;
  for (let i = 0; i < parts; i++) {
    const x = 5 + (i % perRow) * 9;
    const y = 5 + Math.floor(i / perRow) * 9;
    lines.push(`component R${i} resistor "0402" {`);
    lines.push('    value "10k"');
    lines.push(`    at ${x}mm, ${y}mm`);
    lines.push('}');
    lines.push('');
  }

  for (let i = 0; i + 1 < parts; i += 2) {
    lines.push(`net N${i} {`);
    lines.push(`    R${i}.2`);
    lines.push(`    R${i + 1}.1`);
    lines.push('}');
    lines.push('');
    lines.push(`trace N${i} {`);
    lines.push(`    from R${i}.2`);
    lines.push(`    to R${i + 1}.1`);
    lines.push('    layer Top');
    lines.push('    width 0.25mm');
    lines.push('}');
    lines.push('');
  }

  return lines.join('\n');
}

function median(values: number[]): number {
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.floor(sorted.length / 2)];
}

function percentile(values: number[], p: number): number {
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * p))];
}

test.describe('Renderer frame time', () => {
  test('a board with 200 parts and 100 traces draws inside a frame budget', async ({ page }) => {
    await page.goto('/');
    await page.waitForFunction(() => typeof (window as never as { __loadBoard?: unknown }).__loadBoard === 'function');

    await page.evaluate(
      source => (window as never as { __loadBoard: (s: string, k: string) => void }).__loadBoard(source, 'cypcb'),
      heavyBoard(200),
    );
    await page.waitForTimeout(1000);

    // Fit the whole board, so the frames measured are drawing all of it and
    // not a corner of it.
    await page.keyboard.press('f');
    await page.waitForTimeout(300);

    await page.evaluate(() => (window as never as { __renderTiming: { reset: () => void } }).__renderTiming.reset());

    // Force redraws: every wheel event zooms, which marks the canvas dirty.
    const canvas = page.locator('#pcb-canvas');
    await canvas.hover();
    for (let i = 0; i < 40; i++) {
      await page.mouse.wheel(0, i % 2 === 0 ? -120 : 120);
      await page.waitForTimeout(20);
    }

    const samples: number[] = await page.evaluate(() =>
      (window as never as { __renderTiming: { samples: () => number[] } }).__renderTiming.samples(),
    );

    expect(samples.length, 'the canvas never redrew, so nothing was measured').toBeGreaterThan(10);

    const mid = median(samples);
    const p95 = percentile(samples, 0.95);
    console.log(
      `[frame time] samples=${samples.length} median=${mid.toFixed(2)}ms p95=${p95.toFixed(2)}ms max=${Math.max(...samples).toFixed(2)}ms`,
    );

    // 16.7ms is one frame at 60Hz. The ceiling here is deliberately looser -
    // this runs in a headless browser on a shared build machine, and a test
    // that fails when the machine is busy teaches people to ignore it. What it
    // catches is a change that makes the renderer slower by an order of
    // magnitude, which is what has actually happened to renderers before.
    expect(mid, `median frame time was ${mid.toFixed(2)}ms`).toBeLessThan(50);
  });
});
