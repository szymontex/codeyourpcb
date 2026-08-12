import { test, expect } from '@playwright/test';

/**
 * An open finding, and what it turns out to be.
 *
 * Recorded from a session with the owner's board: "the Workspace section of
 * the project manager renders 23 files into a box measuring zero by zero" -
 * the cards present in the DOM and the section with no extent, so none of them
 * reachable. The next action written down was to read the section's CSS
 * against the `display:none` it starts with, on the theory that it was being
 * un-hidden without being given a display mode its grid children need.
 *
 * That theory is wrong, and this file is the measurement that says so.
 * `.pm-section` sets no `display` at all, so `style.display = ''` - which is
 * what `updateProjectFiles` does - falls back to a div's default `block`.
 *
 * What does produce the reported symptom is an ancestor:
 * `#project-manager.hidden { display: none; }`. Every descendant of a hidden
 * element measures zero on both axes while keeping its children, which is
 * exactly "23 cards in a 0x0 box". The last test below reproduces that
 * directly, so the next person to measure a zero here checks the overlay
 * before reading the section's own CSS.
 */

const WORKSPACE_SECTION = '#pm-projects-section';

test.describe('the Workspace section has a size when it is shown', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.evaluate(() => localStorage.clear());
    await page.reload();
    await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });
    await expect(page.locator('#project-manager')).toBeVisible();
  });

  test('starts hidden, which is a size of zero and is meant to be', async ({ page }) => {
    // The section ships with `style="display:none"` and is un-hidden only when
    // the dev server answers with a file list. Zero here is the design, and
    // saying so is what stops it being read as the fault.
    const box = await page.locator(WORKSPACE_SECTION).boundingBox();
    expect(box).toBeNull();
  });

  test('has real extent once it is un-hidden with a file in it', async ({ page }) => {
    // Un-hidden the way `updateProjectFiles` does it, with one card of the
    // class the real code creates. If the reported fault were in this
    // section's own CSS, this is where it would show.
    const size = await page.evaluate((selector) => {
      const section = document.querySelector(selector) as HTMLElement;
      section.style.display = '';
      const list = document.getElementById('pm-project-list')!;
      const item = document.createElement('div');
      item.className = 'pm-project-item';
      item.textContent = 'board.cypcb';
      list.appendChild(item);
      const box = section.getBoundingClientRect();
      const listBox = list.getBoundingClientRect();
      return {
        section: { w: box.width, h: box.height },
        list: { w: listBox.width, h: listBox.height },
        display: getComputedStyle(section).display,
        listDisplay: getComputedStyle(list).display,
      };
    }, WORKSPACE_SECTION);

    expect(size.display).toBe('block');
    expect(size.listDisplay).toBe('grid');
    expect(size.section.w).toBeGreaterThan(100);
    expect(size.section.h).toBeGreaterThan(20);
    expect(size.list.w).toBeGreaterThan(100);
    expect(size.list.h).toBeGreaterThan(10);
  });

  test('measures zero from inside a hidden overlay, cards and all', async ({ page }) => {
    // The artefact, reproduced. The section is shown and holds cards; the
    // overlay above it is hidden. Both boxes read zero while the children are
    // still there - which is the reported symptom, produced by something other
    // than the section.
    const measured = await page.evaluate((selector) => {
      const section = document.querySelector(selector) as HTMLElement;
      section.style.display = '';
      const list = document.getElementById('pm-project-list')!;
      for (let i = 0; i < 23; i += 1) {
        const item = document.createElement('div');
        item.className = 'pm-project-item';
        item.textContent = `board-${i}.cypcb`;
        list.appendChild(item);
      }
      document.getElementById('project-manager')!.classList.add('hidden');
      const box = section.getBoundingClientRect();
      return { w: box.width, h: box.height, cards: list.children.length };
    }, WORKSPACE_SECTION);

    expect(measured.cards).toBe(23);
    expect(measured.w).toBe(0);
    expect(measured.h).toBe(0);
  });
});
