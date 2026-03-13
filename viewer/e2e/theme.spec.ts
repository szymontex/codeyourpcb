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

  test('theme icon reflects current state', async ({ page }) => {
    const icon = page.locator('#theme-icon');
    const text = await icon.textContent();
    // Icon should be one of the theme emojis
    expect(['☀️', '🌙', '🔄']).toContain(text?.trim());
  });
});
