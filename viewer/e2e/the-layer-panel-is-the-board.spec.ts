import { test, expect } from '@playwright/test';

/**
 * The layer panel lists the layers the board declares.
 *
 * Reported from use on 2026-09-10: a four-layer board opens and the panel
 * offers `Top` and `Bottom`. The panel is built from the board rather than
 * from a fixed pair - `syncLayerPicker` calls
 * `copperLayerNames(boardLayerCount())`, and `boardLayerCount()` reads
 * `snapshot.board.layer_count` - so this asks the browser what the panel
 * actually shows.
 *
 * The engine's half is answered next door:
 * `cargo test -p cypcb-render --test a_four_layer_board_says_it_has_four`
 * loads `examples/four-layer.cypcb` and finds `layer_count` 4 with copper on
 * `Inner1` and `Inner2`. So if this fails, the count is lost between the
 * snapshot and the panel, and the case says which of the two is wrong rather
 * than leaving somebody to guess.
 */
const FOUR_LAYER = `version 1

board four {
    size 30mm x 20mm
    layers 4
}

component J1 connector "PAD1" {
    at 5mm, 10mm
}

component J2 connector "PAD1" {
    at 25mm, 10mm
}

net SIG {
    J1.1
    J2.1
}

trace SIG {
    layer Inner1
    width 0.25mm
    path 6mm,10mm -> 15mm,10mm
}

trace SIG {
    layer Inner2
    width 0.25mm
    path 15mm,10mm -> 24mm,10mm
}
`;

test.describe('the layer panel', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });
    await page.evaluate(
      (src) => (window as never as { __loadBoard: (s: string) => void }).__loadBoard(src),
      FOUR_LAYER,
    );
    await page.waitForTimeout(1500);
  });

  test('a four-layer board gets four rows, in the order it is pressed', async ({ page }) => {
    const rows = page.locator('#lp-copper .lp-row');
    await expect(rows).toHaveCount(4);

    const names = await rows.evaluateAll((elements) =>
      elements.map((element) => (element as HTMLElement).dataset.layer),
    );
    expect(names).toEqual(['Top', 'Inner1', 'Inner2', 'Bottom']);
  });

  test('a two-layer board is still two rows', async ({ page }) => {
    // The control: a panel that always draws four would pass the case above
    // and be just as wrong.
    await page.evaluate(
      (src) => (window as never as { __loadBoard: (s: string) => void }).__loadBoard(src),
      FOUR_LAYER.replace('layers 4', 'layers 2')
        .replace('layer Inner1', 'layer Top')
        .replace('layer Inner2', 'layer Bottom'),
    );
    await page.waitForTimeout(1200);

    await expect(page.locator('#lp-copper .lp-row')).toHaveCount(2);
  });
});
