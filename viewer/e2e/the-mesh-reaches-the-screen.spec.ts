import { test, expect } from '@playwright/test';

/**
 * A hatched plane reaches the screen as a mesh.
 *
 * The filler cuts the mesh where the copper is decided, so the Gerber, the
 * checker and the browser all read one answer - and a Rust test holds that
 * arithmetic. This holds the browser to it: the screen is the one place a mesh
 * is actually looked at, and everything checked in this vector so far has been
 * checked on the other side of the wasm boundary.
 *
 * What is read is the copper the canvas is handed - `get_snapshot().pours` -
 * rather than pixels: the renderer draws each rectangle it is given, and a
 * pour that arrives as one rectangle is a solid plane however it is painted.
 */

const HATCHED = `version 1

board panel {
    size 20mm x 20mm
    layers 2
}

net GND {
}

zone GND {
    bounds 2mm, 2mm to 12mm, 12mm
    layer top
    net GND
    hatch 0.3mm pitch 1mm
}
`;

const SOLID = HATCHED.replace('    hatch 0.3mm pitch 1mm\n', '');

/** Every rectangle of copper the pours hand the canvas, in nanometres. */
async function pourRects(page: import('@playwright/test').Page, source: string) {
  await page.evaluate((src) => (window as any).__loadBoard(src), source);
  await page.waitForTimeout(300);
  return page.evaluate(() => {
    const snapshot = (window as any).__pcbEngine.get_snapshot();
    const rects: number[][] = [];
    for (const pour of snapshot.pours ?? []) {
      for (const piece of pour.rects ?? []) rects.push(piece as number[]);
    }
    return rects;
  });
}

test('a hatched plane arrives as lines and a solid one as a sheet', async ({ page }) => {
  await page.goto('/');
  await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });

  const solid = await pourRects(page, SOLID);
  expect(solid.length, 'a solid plane is one rectangle').toBe(1);

  const meshed = await pourRects(page, HATCHED);
  expect(meshed.length, 'a hatched plane is the lines of its mesh').toBeGreaterThan(10);

  // Every line is 0.3mm across one way or the other - the width the design
  // asked for. A mesh drawn at some other width is copper nobody ordered.
  for (const [minX, minY, maxX, maxY] of meshed) {
    const width = maxX - minX;
    const height = maxY - minY;
    expect(
      width === 300_000 || height === 300_000,
      `a line 0.3mm across one way: ${width}nm by ${height}nm`,
    ).toBe(true);
  }

  // And the mesh is copper rather than a redrawn sheet: the lines and the gaps
  // between them add up to less board than the solid plane covers.
  const area = (rects: number[][]) =>
    rects.reduce((total, [minX, minY, maxX, maxY]) => total + (maxX - minX) * (maxY - minY), 0);
  expect(area(meshed), 'a mesh is less copper than a sheet').toBeLessThan(area(solid));
});
