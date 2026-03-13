import { test, expect } from '@playwright/test';

test.describe('Performance Verification', () => {
  test('web load time under 3000ms (domContentLoaded)', async ({ page }) => {
    // Navigate and wait for app ready
    await page.goto('/');
    await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });

    // Measure domContentLoaded from navigation timing API
    const loadTime = await page.evaluate(() => {
      const nav = performance.getEntriesByType('navigation')[0] as PerformanceNavigationTiming;
      if (nav) {
        return nav.domContentLoadedEventEnd - nav.startTime;
      }
      // Fallback to legacy timing
      const t = performance.timing;
      return t.domContentLoadedEventEnd - t.navigationStart;
    });

    console.log(`[perf] domContentLoaded: ${loadTime.toFixed(0)}ms`);
    expect(loadTime).toBeLessThan(3000);
  });

  test('3D renderer achieves ≥30 FPS in headless', async ({ page }) => {
    // Load app and wait for ready
    await page.goto('/');
    await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });

    // Activate 3D view
    await page.click('#view-3d-btn');
    await expect(page.locator('#view-3d-btn')).toHaveClass(/active/, { timeout: 5_000 });

    // Verify renderer is active
    const isActive = await page.evaluate(() => {
      return (window as any).__renderer3d?.isActive === true;
    });
    expect(isActive).toBe(true);

    // Wait for FPS counter to stabilize — it updates every 1 second
    // Give it 3 seconds of rendering to accumulate a stable reading
    await page.waitForTimeout(3_500);

    const fps = await page.evaluate(() => {
      return (window as any).__renderer3d?.fps ?? 0;
    });

    console.log(`[perf] 3D FPS: ${fps}`);
    // Headless WebGL may not hit 60fps, so 30fps is the headless threshold
    expect(fps).toBeGreaterThanOrEqual(30);
  });
});
