import { test, expect, type Page, type Route } from '@playwright/test';

// ---------------------------------------------------------------------------
// Mock data — shapes match real API responses from S06-RESEARCH.md
// ---------------------------------------------------------------------------

const MOCK_SEARCH_RESULTS = {
  components: [
    {
      lcsc: 17414,
      mfr: '0805W8F1002T5E',
      package: '0805',
      is_basic: true,
      stock: 15457503,
      price: 0.001642857,
      extra: JSON.stringify({
        manufacturer: { name: 'UNI-ROYAL' },
        attributes: { Resistance: '10kΩ', Tolerance: '±1%' },
        datasheet: { pdf: 'https://example.com/datasheet.pdf' },
      }),
    },
    {
      lcsc: 25752,
      mfr: 'RC0805JR-0710KL',
      package: '0805',
      is_basic: false,
      stock: 4200000,
      price: 0.002,
      extra: JSON.stringify({
        manufacturer: { name: 'YAGEO' },
        attributes: { Resistance: '10kΩ', Power: '0.125W' },
        datasheet: { pdf: 'https://example.com/yageo.pdf' },
      }),
    },
    {
      lcsc: 84376,
      mfr: 'ERJ-6ENF1002V',
      package: '0805',
      is_basic: false,
      stock: 890000,
      price: 0.0034,
      extra: JSON.stringify({
        manufacturer: { name: 'Panasonic' },
        attributes: { Resistance: '10kΩ' },
        datasheet: { pdf: '' },
      }),
    },
  ],
};

const MOCK_EMPTY_RESULTS = { components: [] };

const MOCK_COMPONENT_DATA = {
  result: {
    packageDetail: {
      dataStr: {
        shape: [
          'PAD~1~1~0.5~0.4~0.3~0.1~1~~1~0',
          'SVGNODE~{"c_etype":"outline3D","uuid":"c7acac53bcbc44d68fbab8f60a747688","z":0}',
        ],
      },
    },
  },
};

// Minimal EasyEDA OBJ — a cube with 2 materials
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

const MINIMAL_BOARD = `version 1\nboard test {\n  size 50mm x 50mm\n  layers 2\n}`;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Set up route interception for all JLCPCB/EasyEDA external APIs. */
async function interceptAPIs(
  page: Page,
  options?: {
    searchResponse?: object | null;
    searchStatus?: number;
    componentResponse?: object;
    objText?: string;
    onSearchRequest?: () => void;
  },
) {
  const {
    searchResponse = MOCK_SEARCH_RESULTS,
    searchStatus = 200,
    componentResponse = MOCK_COMPONENT_DATA,
    objText = MOCK_OBJ_TEXT,
    onSearchRequest,
  } = options ?? {};

  // Intercept jlcsearch API.
  //
  // The app fans out over every jlcsearch category - 34 of them - and merges
  // what comes back, so answer with the fixture once, for resistors, and give
  // the rest an empty list the way the real API does. Returning the same body
  // for every category multiplies the fixture by 34 and the search hits its
  // result limit instead.
  //
  // The Access-Control-Allow-Origin header matters: jlcsearch is cross-origin,
  // and without it the browser drops the fulfilled response, so the app's fetch
  // lands in its catch block and caches an empty category.
  await page.route('**/jlcsearch.tscircuit.com/**', async (route: Route) => {
    onSearchRequest?.();
    if (searchResponse === null) {
      await route.abort();
      return;
    }
    const isResistors = route.request().url().includes('/resistors/');
    await route.fulfill({
      status: searchStatus,
      contentType: 'application/json',
      headers: { 'Access-Control-Allow-Origin': '*' },
      body: JSON.stringify(isResistors ? searchResponse : { components: [] }),
    });
  });

  // Intercept the EasyEDA component API. In dev the app talks to the Vite proxy
  // path, not to easyeda.com - matching only the upstream host let every one of
  // these requests leave the machine for real during the E2E run.
  await page.route('**/easyeda-api/api/products/*/components*', async (route: Route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      headers: { 'Access-Control-Allow-Origin': '*' },
      body: JSON.stringify(componentResponse),
    });
  });
  await page.route('**/easyeda-modules/3dmodel/**', async (route: Route) => {
    await route.fulfill({
      status: 200,
      contentType: 'text/plain',
      headers: { 'Access-Control-Allow-Origin': '*' },
      body: objText,
    });
  });
  await page.route('**/easyeda.com/api/products/*/components*', async (route: Route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      headers: { 'Access-Control-Allow-Origin': '*' },
      body: JSON.stringify(componentResponse),
    });
  });

  // Intercept EasyEDA 3D model CDN
  await page.route('**/modules.easyeda.com/3dmodel/**', async (route: Route) => {
    await route.fulfill({
      status: 200,
      contentType: 'text/plain',
      headers: { 'Access-Control-Allow-Origin': '*' },
      body: objText,
    });
  });
}

/** Load a minimal board to dismiss project manager. */
async function loadBoard(page: Page) {
  await page.evaluate((src) => (window as any).__loadBoard(src), MINIMAL_BOARD);
  await page.waitForTimeout(300);
}

/** Activate 3D view and wait for renderer. */
async function activate3D(page: Page) {
  await page.click('#view-3d-btn');
  await expect(page.locator('#view-3d-btn')).toHaveClass(/active/, { timeout: 5_000 });
  await page.waitForFunction(
    () => (window as any).__renderer3d?.isActive === true,
    { timeout: 5_000 },
  );
  await page.waitForTimeout(300);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

test.describe('JLCPCB Search Panel', () => {
  test.beforeEach(async ({ page }) => {
    await interceptAPIs(page);
    await page.goto('/');
    await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });
    await loadBoard(page);
  });

  test('search panel opens and closes via toolbar button', async ({ page }) => {
    const panel = page.locator('#jlcpcb-search-panel');
    const btn = page.locator('#jlcpcb-search-btn');

    // Initially hidden
    await expect(panel).toHaveClass(/hidden/);

    // Click 🔍 → panel visible
    await btn.click();
    await expect(panel).not.toHaveClass(/hidden/);
    await expect(btn).toHaveClass(/active/);

    // Verify debug surface
    const visible = await page.evaluate(() => (window as any).__jlcpcbSearch?.visible);
    expect(visible).toBe(true);

    // Click again → panel hidden
    await btn.click();
    await expect(panel).toHaveClass(/hidden/);
    await expect(btn).not.toHaveClass(/active/);
  });

  test('search returns results with metadata', async ({ page }) => {
    // Open panel
    await page.click('#jlcpcb-search-btn');
    await expect(page.locator('#jlcpcb-search-panel')).not.toHaveClass(/hidden/);

    // Type search query
    await page.fill('#jlcpcb-search-input', '0805 10k');

    // Wait for results to render (debounce + API mock)
    await expect(page.locator('.jlcpcb-result')).toHaveCount(3, { timeout: 5_000 });

    // Verify LCSC numbers are visible. Order is a ranking decision and all three
    // fixtures are 0805 10k resistors, so assert the set, not the sequence.
    const lcscCodes = await page.locator('.jlcpcb-result-lcsc').allTextContents();
    expect(lcscCodes.join(' ')).toContain('C17414');
    expect(lcscCodes.join(' ')).toContain('C25752');
    expect(lcscCodes.join(' ')).toContain('C84376');

    // Verify manufacturer and package are displayed for every hit
    await expect(page.locator('.jlcpcb-result-mfr')).toHaveCount(3);
    for (const pkg of await page.locator('.jlcpcb-result-package').allTextContents()) {
      expect(pkg).toContain('0805');
    }

    // Verify price and stock in footer
    await expect(page.locator('.jlcpcb-result-price').first()).toContainText('$');
    await expect(page.locator('.jlcpcb-result-stock').first()).toContainText('Stock:');

    // Verify debug surface
    const debug = await page.evaluate(() => (window as any).__jlcpcbSearch);
    expect(debug.lastQuery).toBe('0805 10k');
    expect(debug.resultCount).toBe(3);
    expect(debug.lastError).toBeNull();
  });

  test('empty search shows no-results message', async ({ page }) => {
    // Override search route to return empty results
    await page.unrouteAll({ behavior: 'ignoreErrors' });
    await interceptAPIs(page, { searchResponse: MOCK_EMPTY_RESULTS });

    await page.click('#jlcpcb-search-btn');
    await page.fill('#jlcpcb-search-input', 'nonexistent part xyz');

    // Wait for status message
    const status = page.locator('#jlcpcb-search-status');
    await expect(status).not.toHaveClass(/hidden/, { timeout: 5_000 });
    await expect(status).toContainText('No results', { timeout: 5_000 });

    // No result rows
    await expect(page.locator('.jlcpcb-result')).toHaveCount(0);

    // Debug surface confirms
    const debug = await page.evaluate(() => (window as any).__jlcpcbSearch);
    expect(debug.resultCount).toBe(0);
  });

  test('API error shows user-visible message', async ({ page }) => {
    // Abort the search route to simulate network failure.
    // searchComponents() catches all errors and returns [] — never throws.
    // The panel then shows "No results found" for empty arrays.
    await page.unrouteAll({ behavior: 'ignoreErrors' });
    await interceptAPIs(page, { searchResponse: null }); // null = route.abort()

    await page.click('#jlcpcb-search-btn');
    await page.fill('#jlcpcb-search-input', 'error test');

    // Panel shows "No results" status message (not error class — the catch
    // branch in executeSearch is unreachable via searchComponents since it
    // never throws, but the empty-results path is the user-visible error signal)
    const status = page.locator('#jlcpcb-search-status');
    await expect(status).not.toHaveClass(/hidden/, { timeout: 5_000 });
    await expect(status).toContainText('No results', { timeout: 5_000 });

    // No result rows rendered
    await expect(page.locator('.jlcpcb-result')).toHaveCount(0);

    // Debug surface confirms zero results
    const debug = await page.evaluate(() => (window as any).__jlcpcbSearch);
    expect(debug.resultCount).toBe(0);
  });

  test('search is debounced — rapid input fetches each category once', async ({ page }) => {
    const requestedUrls: string[] = [];

    // Re-register routes with a request recorder
    await page.unrouteAll({ behavior: 'ignoreErrors' });
    await interceptAPIs(page, {
      onSearchRequest: undefined,
    });
    page.on('request', (req) => {
      if (req.url().includes('jlcsearch.tscircuit.com')) requestedUrls.push(req.url());
    });

    await page.click('#jlcpcb-search-btn');

    // Type rapidly, character by character
    const input = page.locator('#jlcpcb-search-input');
    await input.pressSequentially('10k resistor', { delay: 30 });

    // Wait for debounce + response to settle
    await expect(page.locator('.jlcpcb-result')).toHaveCount(3, { timeout: 5_000 });

    // A search sweeps every jlcsearch category once and caches the answers, so
    // twelve keystrokes must not fetch anything twice. Comparing against the
    // set of distinct URLs says that without pinning the category count.
    expect(requestedUrls.length).toBeGreaterThan(0);
    expect(requestedUrls).toHaveLength(new Set(requestedUrls).size);
  });
});

test.describe('JLCPCB 3D Model Loading', () => {
  test('component click triggers 3D model fetch pipeline when 3D view active', async ({ page }) => {
    let easyedaHit = false;
    let modulesHit = false;

    // Set up routes with tracking
    await page.route('**/jlcsearch.tscircuit.com/**', async (route: Route) => {
      // Same as interceptAPIs: answer once for resistors, empty elsewhere.
      const isResistors = route.request().url().includes('/resistors/');
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        headers: { 'Access-Control-Allow-Origin': '*' },
        body: JSON.stringify(isResistors ? MOCK_SEARCH_RESULTS : { components: [] }),
      });
    });

    await page.route('**/easyeda-api/api/products/*/components*', async (route: Route) => {
      easyedaHit = true;
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        headers: { 'Access-Control-Allow-Origin': '*' },
        body: JSON.stringify(MOCK_COMPONENT_DATA),
      });
    });

    await page.route('**/easyeda-modules/3dmodel/**', async (route: Route) => {
      modulesHit = true;
      await route.fulfill({
        status: 200,
        contentType: 'text/plain',
        headers: { 'Access-Control-Allow-Origin': '*' },
        body: MOCK_OBJ_TEXT,
      });
    });

    await page.route('**/easyeda.com/api/products/*/components*', async (route: Route) => {
      easyedaHit = true;
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
      headers: { 'Access-Control-Allow-Origin': '*' },
        body: JSON.stringify(MOCK_COMPONENT_DATA),
      });
    });

    await page.route('**/modules.easyeda.com/3dmodel/**', async (route: Route) => {
      modulesHit = true;
      await route.fulfill({
        status: 200,
        contentType: 'text/plain',
      headers: { 'Access-Control-Allow-Origin': '*' },
        body: MOCK_OBJ_TEXT,
      });
    });

    await page.goto('/');
    await expect(page.locator('#status-text')).toContainText('Ready', { timeout: 15_000 });
    await loadBoard(page);

    // Activate 3D view first
    await activate3D(page);

    // Open search panel and search
    await page.click('#jlcpcb-search-btn');
    await page.fill('#jlcpcb-search-input', '0805 10k');
    await expect(page.locator('.jlcpcb-result')).toHaveCount(3, { timeout: 5_000 });

    // Click first result — triggers onComponentSelect → fetch3DModel pipeline
    await page.locator('.jlcpcb-result').first().click();

    // Wait for the full fetch pipeline to complete (EasyEDA API → OBJ fetch)
    // The pipeline runs: component API → extract UUID → fetch OBJ → attempt loadComponentFromOBJ.
    // loadComponentFromOBJ will log an error because no placeholder mesh exists in the
    // minimal board (no components with matching refdes), but the fetch pipeline itself
    // should complete — verifiable via route hit tracking.
    await page.waitForFunction(
      () => {
        // The console.log for "[JLCPCB] Fetching 3D model" or "[3D] OBJ loaded" indicates
        // the pipeline ran. We also check __jlcpcbSearch to confirm selection happened.
        const s = (window as any).__jlcpcbSearch;
        return s && s.lastQuery === '0805 10k' && s.resultCount > 0;
      },
      { timeout: 5_000 },
    );

    // Give the async fetch chain time to complete
    await page.waitForTimeout(2000);

    // Verify the full 3D model fetch pipeline was triggered:
    // 1. EasyEDA component API was hit (to get 3D UUID)
    // 2. EasyEDA 3D model CDN was hit (to get OBJ text)
    expect(easyedaHit).toBe(true);
    expect(modulesHit).toBe(true);

    // Verify 3D renderer is still active (pipeline didn't crash it)
    const isActive = await page.evaluate(() => (window as any).__renderer3d?.isActive);
    expect(isActive).toBe(true);
  });
});
