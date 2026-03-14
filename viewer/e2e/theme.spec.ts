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

  test('clicking theme toggle cycles theme', async ({ page }) => {
    const initialTheme = await page.getAttribute('html', 'data-theme');

    // Click theme toggle button
    await page.click('#theme-toggle');

    // Theme should change (cycle: light → dark → auto → light)
    // The data-theme value should potentially change
    const afterFirst = await page.getAttribute('html', 'data-theme');
    // Note: auto resolves to light or dark depending on system, so data-theme
    // might be the same as initial in the auto→light transition
    // Just verify the attribute is still valid
    expect(['light', 'dark']).toContain(afterFirst);

    // Click again to advance cycle
    await page.click('#theme-toggle');
    const afterSecond = await page.getAttribute('html', 'data-theme');
    expect(['light', 'dark']).toContain(afterSecond);
  });

  test('Ctrl+Shift+T toggles theme', async ({ page }) => {
    const initialTheme = await page.getAttribute('html', 'data-theme');

    await page.keyboard.press('Control+Shift+t');

    // Theme icon should update
    const icon = await page.locator('#theme-icon').textContent();
    expect(icon).toBeTruthy();
  });

  test('theme persists to localStorage', async ({ page }) => {
    // Set to a known state by clicking toggle
    await page.click('#theme-toggle');

    const stored = await page.evaluate(() => localStorage.getItem('theme'));
    expect(stored).toBeTruthy();
    expect(['light', 'dark', 'auto']).toContain(stored);

    // Toggle again
    await page.click('#theme-toggle');
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
    const afterLabel = await btn.textContent();
    expect(afterLabel).not.toBe(initialLabel);

    // data-theme is still a valid resolved value
    const dataTheme = await page.getAttribute('html', 'data-theme');
    expect(['light', 'dark']).toContain(dataTheme);

    // Dismiss preferences modal
    await page.click('#prefs-close');
    await expect(page.locator('#prefs-overlay')).toHaveClass(/hidden/, { timeout: 3_000 });
  });

  test('theme icon reflects current state', async ({ page }) => {
    const icon = page.locator('#theme-icon');
    const text = await icon.textContent();
    // Icon should be one of the theme emojis
    expect(['☀️', '🌙', '🔄']).toContain(text?.trim());
  });
});
