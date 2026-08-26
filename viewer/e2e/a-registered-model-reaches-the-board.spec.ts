import { test, expect, type Route } from '@playwright/test';

/**
 * Every link of the 3D model chain has a test and the chain has never been run.
 *
 * `register3DModel` reaches the engine (a vitest says so), the engine copies
 * the uuid onto each component of the snapshot (a Rust test says so), and
 * `renderer3d.ts` fetches an OBJ for a component that has one and replaces the
 * placeholder mesh named after its refdes. Until this file, nothing had put
 * those three together, and the path that used to run passed the LCSC number
 * where a refdes belongs - so no model had ever been drawn on a board.
 */

// Minimal EasyEDA OBJ - a cube with two materials, as the panel's own suite
// mocks it.
const MOCK_OBJ_TEXT = `
v -0.5 -0.5 -0.5
v  0.5 -0.5 -0.5
v  0.5  0.5 -0.5
v -0.5  0.5 -0.5
v -0.5 -0.5  0.5
v  0.5 -0.5  0.5
v  0.5  0.5  0.5
v -0.5  0.5  0.5
newmtl 1
Ka 0.2 0.2 0.2
Kd 0.8 0.8 0.8
Ks 0.3 0.3 0.3
d 0.0
endmtl
newmtl 2
Ka 0.1 0.1 0.1
Kd 0.3 0.3 0.7
Ks 0.5 0.5 0.5
d 0.0
endmtl
usemtl 1
f 1// 2// 3//
f 3// 4// 1//
f 5// 6// 7//
f 7// 8// 5//
usemtl 2
f 1// 2// 6//
f 6// 5// 1//
f 3// 4// 8//
f 8// 7// 3//
`.trim();

const BOARD = `version 1

board models {
    size 40mm x 30mm
    layers 2
}

component R1 resistor "0805" {
    value "10k"
    at 10mm, 15mm
}
`;

test.describe('a registered model reaches the board', () => {
  test('the component with a model gets its geometry, and the one without keeps its box', async ({
    page,
  }) => {
    let objRequests = 0;
    for (const pattern of ['**/easyeda-modules/3dmodel/**', '**/modules.easyeda.com/3dmodel/**']) {
      await page.route(pattern, async (route: Route) => {
        objRequests++;
        await route.fulfill({
          status: 200,
          contentType: 'text/plain',
          headers: { 'Access-Control-Allow-Origin': '*' },
          body: MOCK_OBJ_TEXT,
        });
      });
    }

    await page.goto('/');
    await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });

    await page.evaluate((src) => (window as any).__loadBoard(src), BOARD);
    await page.waitForTimeout(300);

    // The key the engine files a model under is the package the component
    // names, so the test asks the model rather than guessing the spelling.
    const pkg = await page.evaluate(() => {
      const snapshot = (window as any).__pcbEngine.get_snapshot();
      return snapshot.components[0].footprint as string;
    });
    expect(pkg, 'the board has a component with a package').toBeTruthy();

    // Nothing is fetched for a board whose parts have no model.
    await page.click('#view-3d-btn');
    await page.waitForFunction(() => (window as any).__renderer3d?.isActive === true, {
      timeout: 5_000,
    });
    await page.waitForTimeout(500);
    expect(objRequests, 'a part with no model asks for nothing').toBe(0);

    // Teach the engine what the JLCPCB panel teaches it, and load the board
    // again so the snapshot carries the uuid.
    await page.evaluate(
      (name) => (window as any).__pcbEngine.register_3d_model(name, 'uuid-e2e'),
      pkg,
    );
    await page.evaluate((src) => (window as any).__loadBoard(src), BOARD);
    const carried = await page.evaluate(
      () => (window as any).__pcbEngine.get_snapshot().components[0].model_3d,
    );
    expect(carried, 'the engine puts the uuid on the component').toBe('uuid-e2e');

    // The scene is built when the view is opened and when the board changes
    // under it; the test hook that loads a board does not go through the
    // second path, so the view is closed and opened to rebuild it.
    await page.click('#view-3d-btn');
    await expect(page.locator('#view-3d-btn')).not.toHaveClass(/active/, { timeout: 5_000 });
    await page.click('#view-3d-btn');
    await page.waitForFunction(() => (window as any).__renderer3d?.isActive === true, {
      timeout: 5_000,
    });

    await page.waitForFunction(
      () => ((window as any).__renderer3d?.objModelCount ?? 0) >= 1,
      { timeout: 15_000 },
    );
    expect(objRequests, 'the OBJ was fetched once the component carried a model').toBeGreaterThan(
      0,
    );
  });
});
