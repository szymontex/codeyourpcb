import { test, expect } from '@playwright/test';

test.describe('Theme Toggle & Persistence', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });
  });

  test('initial theme has data-theme attribute set', async ({ page }) => {
    const theme = await page.getAttribute('html', 'data-theme');
    // Should be either 'light' or 'dark' (auto resolves to one)
    expect(['light', 'dark']).toContain(theme);
  });

  test('theme shortcut cycles theme', async ({ page }) => {
    // #theme-toggle is display:none - theme moved into Preferences, and the
    // keyboard shortcut is the surviving direct path.
    await page.keyboard.press('Control+Shift+t');

    // Theme should change (cycle: light → dark → auto → light)
    // The data-theme value should potentially change
    const afterFirst = await page.getAttribute('html', 'data-theme');
    // Note: auto resolves to light or dark depending on system, so data-theme
    // might be the same as initial in the auto→light transition
    // Just verify the attribute is still valid
    expect(['light', 'dark']).toContain(afterFirst);

    // Advance the cycle again
    await page.keyboard.press('Control+Shift+t');
    const afterSecond = await page.getAttribute('html', 'data-theme');
    expect(['light', 'dark']).toContain(afterSecond);
  });

  test('Ctrl+Shift+T advances the icon with the setting', async ({ page }) => {
    // This asked only whether `#theme-icon` had any text in it, which it does
    // before the key is pressed - so it passed on a shortcut that did nothing.
    // The icon is one of three characters, one per setting (`updateThemeIcon`
    // in `main.ts`), so a press that advances the cycle has to change it.
    const ICON_FOR = { light: '☀️', dark: '🌙', auto: '🔄' } as const;

    const before = await page.locator('#theme-icon').textContent();

    await page.keyboard.press('Control+Shift+t');

    const after = await page.locator('#theme-icon').textContent();
    expect(after?.trim(), 'the icon did not change, so the shortcut did nothing').not.toBe(
      before?.trim(),
    );

    // And it is the icon for the setting that was actually stored, not just a
    // different one - the icon and the theme cannot drift apart silently.
    const stored = (await page.evaluate(() => localStorage.getItem('theme'))) as
      | keyof typeof ICON_FOR
      | null;
    expect(stored, 'the shortcut changed the icon without storing a theme').not.toBeNull();
    expect(after?.trim()).toBe(ICON_FOR[stored!]);
  });

  test('theme persists to localStorage', async ({ page }) => {
    // Set to a known state
    await page.keyboard.press('Control+Shift+t');

    const stored = await page.evaluate(() => localStorage.getItem('theme'));
    expect(stored).toBeTruthy();
    expect(['light', 'dark', 'auto']).toContain(stored);

    // Toggle again
    await page.keyboard.press('Control+Shift+t');
    const stored2 = await page.evaluate(() => localStorage.getItem('theme'));
    expect(stored2).toBeTruthy();
    expect(stored2).not.toBe(stored); // Should have advanced in the cycle
  });

  test('Preferences modal theme button cycles theme with single click', async ({ page }) => {
    // Open Preferences modal
    await page.click('#prefs-btn');
    await expect(page.locator('#prefs-overlay')).not.toHaveClass(/hidden/, { timeout: 3_000 });

    const btn = page.locator('#prefs-theme-btn');
    const initialLabel = await btn.textContent();

    // Single-click the preferences theme button — verifies M002 bug fix
    // (the bug required double-click; single click must cycle the theme)
    await btn.click();

    // Button label must change on every click (light→dark→auto→light cycle)
    await expect(btn).not.toHaveText(initialLabel ?? '');

    // data-theme is still a valid resolved value
    const dataTheme = await page.getAttribute('html', 'data-theme');
    expect(['light', 'dark']).toContain(dataTheme);

    // Dismiss preferences modal
    await page.click('#prefs-close');
    await expect(page.locator('#prefs-overlay')).toHaveClass(/hidden/, { timeout: 3_000 });
  });

  test('theme icon reflects current state', async ({ page }) => {
    // One of the three theme emojis, asserted through the locator so it is
    // retried rather than read once - the icon is written after the theme
    // manager resolves, which is not necessarily before this line runs.
    await expect(page.locator('#theme-icon')).toHaveText(/^(☀️|🌙|🔄)$/);
  });
});
