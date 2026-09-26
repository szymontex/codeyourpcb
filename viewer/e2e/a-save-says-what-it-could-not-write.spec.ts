import { test, expect, type Page } from '@playwright/test';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

/**
 * A save says what it could not write, and a board with a tail is refused.
 *
 * A KiCad board saved as a `.cypcb` is what its owner will open from now on,
 * so whatever the language cannot say yet has to reach them at the save and
 * not sit unread in the file. And a `.kicad_pcb` is one `(kicad_pcb ...)`:
 * the reader used to stop at its closing parenthesis and drop whatever
 * followed, copper included, without a word.
 */

const NAME = 'led_blink.kicad_pcb';
const KICAD = fs.readFileSync(path.resolve(__dirname, '../../tests/fixtures/benchmark/led_blink.kicad_pcb'), 'utf-8');

/** `led_blink` with one via whose ring is not twice its drill. */
const WITH_OWN_RING = KICAD.trimEnd().replace(/\)$/, '  (via (at 120 115) (size 0.45) (drill 0.2) (layers "F.Cu" "B.Cu") (net 1))\n)\n');

type Written = { to?: string; picker?: string; text?: string };

async function fakeFileSystem(page: Page): Promise<void> {
  await page.addInitScript(() => {
    const w = window as never as {
      __writes: Written[];
      __nextOpen: { name: string; text: string };
      showOpenFilePicker: unknown;
      showSaveFilePicker: unknown;
    };
    w.__writes = [];
    const handle = (name: string, text: string) => ({
      kind: 'file',
      name,
      getFile: async () => new File([text], name),
      createWritable: async () => ({
        write: async (data: string) => {
          w.__writes.push({ to: name, text: String(data) });
        },
        close: async () => {},
      }),
    });
    w.showOpenFilePicker = async () => [handle(w.__nextOpen.name, w.__nextOpen.text)];
    w.showSaveFilePicker = async (options: { suggestedName?: string }) => {
      w.__writes.push({ picker: options?.suggestedName });
      return handle(options?.suggestedName ?? 'new', '');
    };
  });
}

async function open(page: Page, text: string): Promise<void> {
  await page.goto('/');
  await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });
  await page.evaluate((text) => {
    (window as never as { __nextOpen: unknown }).__nextOpen = { name: 'led_blink.kicad_pcb', text };
  }, text);
  await page.evaluate(() => (document.querySelector('#open-btn') as HTMLElement).click());
  await page.locator('#pm-open-btn').click();
}

async function save(page: Page): Promise<void> {
  await page.locator('#pcb-canvas').focus().catch(() => {});
  await page.keyboard.press('Control+s');
  await expect.poll(() => page.evaluate(() => (window as never as { __writes: Written[] }).__writes.length)).toBeGreaterThan(1);
}

test.describe('a save says what it could not write', () => {
  test.beforeEach(async ({ page }) => {
    await fakeFileSystem(page);
  });

  test('a via with a ring of its own is named in the status after the save', async ({ page }) => {
    await open(page, WITH_OWN_RING);
    await expect(page.locator('#status-text')).toContainText(`Loaded ${NAME}`, { timeout: 10_000 });
    const vias = await page.evaluate(() => ((window as any).__pcbEngine.get_snapshot().vias ?? []).length);
    expect(vias, 'the control: the via reached the board').toBe(1);
    await save(page);
    await expect(page.locator('#status-text')).toContainText(
      `${NAME} untouched - not written: 1 via(s) written with a ring of twice the drill instead of their own`,
    );
  });

  test('a board the language says in full saves with nothing named', async ({ page }) => {
    await open(page, KICAD);
    await expect(page.locator('#status-text')).toContainText(`Loaded ${NAME}`, { timeout: 10_000 });
    await save(page);
    await expect(page.locator('#status-text')).toContainText(`${NAME} untouched`);
    await expect(page.locator('#status-text')).not.toContainText('not written');
  });

  test('a KiCad board with text after it is refused with where the text starts', async ({ page }) => {
    const board = KICAD.trimEnd();
    const line = board.split('\n').length + 2;
    await open(page, `${board}\n\nversion 1\n`);
    await expect(page.locator('#status-text')).toContainText(`Loaded ${NAME} (1 warnings)`, { timeout: 10_000 });
    const said = await page.evaluate(() =>
      JSON.parse((window as any).__pcbEngine.get_diagnostics_json()).map((d: { message: string }) => d.message));
    expect(said.join('\n')).toContain(`line ${line}, column 1`);
    const loaded = await page.evaluate(() => (window as any).__pcbEngine.get_snapshot().components.length);
    expect(loaded, 'the half that parsed was not shown as the board').toBe(0);
  });
});
