/**
 * JLCPCB Search Panel — right-side overlay for searching JLCPCB/LCSC components.
 *
 * Handles: debounced search input → API call → result rendering → component selection callback.
 * Follows the overlay pattern from project-manager.ts (show/hide/toggle, callback interface).
 *
 * Debug surface: `window.__jlcpcbSearch` exposes lastQuery, resultCount, lastError for E2E.
 */

import { searchComponents, JLCPCBSearchError, type JLCPCBComponent } from './jlcpcb';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface JLCPCBPanelCallbacks {
  /** Called when user clicks a search result */
  onComponentSelect: (component: JLCPCBComponent) => void;
}

// ---------------------------------------------------------------------------
// Module state
// ---------------------------------------------------------------------------

let panel: HTMLElement | null = null;
let searchInput: HTMLInputElement | null = null;
let resultsContainer: HTMLElement | null = null;
let statusEl: HTMLElement | null = null;
let searchBtn: HTMLElement | null = null;
let callbacks: JLCPCBPanelCallbacks | null = null;
let visible = false;
let debounceTimer: number | null = null;

// Debug surface state
let lastQuery = '';
let resultCount = 0;
let lastError: string | null = null;

const DEBOUNCE_MS = 300;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/**
 * Wire DOM event handlers on the search panel.
 * Call once after app init.
 */
export function initSearchPanel(cb: JLCPCBPanelCallbacks): void {
  callbacks = cb;

  panel = document.getElementById('jlcpcb-search-panel');
  searchInput = document.getElementById('jlcpcb-search-input') as HTMLInputElement | null;
  resultsContainer = document.getElementById('jlcpcb-search-results');
  statusEl = document.getElementById('jlcpcb-search-status');
  searchBtn = document.getElementById('jlcpcb-search-btn');

  if (!panel || !searchInput || !resultsContainer || !statusEl) {
    console.warn('[JLCPCB] Search panel elements not found');
    return;
  }

  // Close button
  const closeBtn = document.getElementById('jlcpcb-search-close');
  if (closeBtn) {
    closeBtn.addEventListener('click', hideSearchPanel);
  }

  // Search input — debounced
  searchInput.addEventListener('input', () => {
    if (debounceTimer !== null) {
      clearTimeout(debounceTimer);
    }
    debounceTimer = window.setTimeout(() => {
      debounceTimer = null;
      const query = searchInput!.value.trim();
      if (query.length > 0) {
        executeSearch(query);
      } else {
        clearResults();
      }
    }, DEBOUNCE_MS);
  });

  exposeDebugSurface();
}

/**
 * Show the search panel and focus the input.
 */
export function showSearchPanel(): void {
  if (!panel) return;
  panel.classList.remove('hidden');
  visible = true;
  searchBtn?.classList.add('active');
  // Focus input after display
  setTimeout(() => searchInput?.focus(), 50);
  exposeDebugSurface();
}

/**
 * Hide the search panel.
 */
export function hideSearchPanel(): void {
  if (!panel) return;
  panel.classList.add('hidden');
  visible = false;
  searchBtn?.classList.remove('active');
  exposeDebugSurface();
}

/**
 * Toggle the search panel open/closed.
 */
export function toggleSearchPanel(): void {
  if (visible) {
    hideSearchPanel();
  } else {
    showSearchPanel();
  }
}

/**
 * Returns whether the panel is currently visible.
 */
export function isSearchPanelVisible(): boolean {
  return visible;
}

// ---------------------------------------------------------------------------
// Search logic
// ---------------------------------------------------------------------------

async function executeSearch(query: string): Promise<void> {
  if (!resultsContainer || !statusEl) return;

  lastQuery = query;
  lastError = null;

  // Show loading state
  showStatus(`<span class="jlcpcb-spinner"></span>Searching "${query}"...`, false);
  resultsContainer.textContent = '';
  exposeDebugSurface();

  try {
    const results = await searchComponents(query);
    resultCount = results.length;

    console.log(`[JLCPCB] Search: "${query}" → ${resultCount} results`);

    if (results.length === 0) {
      showStatus('No results found', false);
    } else {
      hideStatus();
      renderResults(results);
    }
  } catch (error) {
    const msg = error instanceof Error ? error.message : String(error);
    lastError = msg;
    resultCount = 0;
    console.error(`[JLCPCB] Search error: ${msg}`);
    const userMsg = error instanceof JLCPCBSearchError
      ? `Search failed — server returned ${error.status}`
      : `Search failed — check connection`;
    showStatus(userMsg, true);
  }

  exposeDebugSurface();
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

function renderResults(results: JLCPCBComponent[]): void {
  if (!resultsContainer) return;
  resultsContainer.textContent = '';

  for (const comp of results) {
    const row = document.createElement('div');
    row.className = 'jlcpcb-result';

    // Header: LCSC# + package
    const header = document.createElement('div');
    header.className = 'jlcpcb-result-header';

    const lcscEl = document.createElement('span');
    lcscEl.className = 'jlcpcb-result-lcsc';
    lcscEl.textContent = `C${comp.lcsc}`;
    header.appendChild(lcscEl);

    if (comp.package) {
      const pkgEl = document.createElement('span');
      pkgEl.className = 'jlcpcb-result-package';
      pkgEl.textContent = comp.package;
      header.appendChild(pkgEl);
    }
    row.appendChild(header);

    // Manufacturer + MPN
    if (comp.mfr || comp.manufacturer) {
      const mfrEl = document.createElement('div');
      mfrEl.className = 'jlcpcb-result-mfr';
      const parts: string[] = [];
      if (comp.manufacturer) parts.push(comp.manufacturer);
      if (comp.mfr) parts.push(comp.mfr);
      mfrEl.textContent = parts.join(' · ');
      row.appendChild(mfrEl);
    }

    // Attributes summary (resistance, capacitance, etc.)
    const attrKeys = Object.keys(comp.attributes);
    if (attrKeys.length > 0) {
      const attrsEl = document.createElement('div');
      attrsEl.className = 'jlcpcb-result-attrs';
      const attrText = attrKeys
        .slice(0, 3)
        .map((k) => `${k}: ${comp.attributes[k]}`)
        .join(' · ');
      attrsEl.textContent = attrText;
      row.appendChild(attrsEl);
    }

    // Footer: price, stock, datasheet
    const footer = document.createElement('div');
    footer.className = 'jlcpcb-result-footer';

    const priceEl = document.createElement('span');
    priceEl.className = 'jlcpcb-result-price';
    priceEl.textContent = `$${comp.price.toFixed(4)}`;
    footer.appendChild(priceEl);

    const stockEl = document.createElement('span');
    stockEl.className = 'jlcpcb-result-stock';
    stockEl.textContent = `Stock: ${comp.stock.toLocaleString()}`;
    footer.appendChild(stockEl);

    if (comp.datasheetUrl) {
      const dsLink = document.createElement('a');
      dsLink.className = 'jlcpcb-result-datasheet';
      dsLink.href = comp.datasheetUrl;
      dsLink.target = '_blank';
      dsLink.rel = 'noopener noreferrer';
      dsLink.textContent = 'Datasheet';
      // Prevent click from bubbling to the row handler
      dsLink.addEventListener('click', (e) => e.stopPropagation());
      footer.appendChild(dsLink);
    }

    row.appendChild(footer);

    // Click handler — select component
    row.addEventListener('click', () => {
      callbacks?.onComponentSelect(comp);
    });

    resultsContainer.appendChild(row);
  }
}

function clearResults(): void {
  if (resultsContainer) resultsContainer.textContent = '';
  hideStatus();
  resultCount = 0;
  lastQuery = '';
  lastError = null;
  exposeDebugSurface();
}

// ---------------------------------------------------------------------------
// Status helpers
// ---------------------------------------------------------------------------

function showStatus(message: string, isError: boolean): void {
  if (!statusEl) return;
  statusEl.innerHTML = message;
  statusEl.classList.remove('hidden', 'error');
  if (isError) statusEl.classList.add('error');
}

function hideStatus(): void {
  if (!statusEl) return;
  statusEl.classList.add('hidden');
  statusEl.classList.remove('error');
}

// ---------------------------------------------------------------------------
// Debug surface
// ---------------------------------------------------------------------------

function exposeDebugSurface(): void {
  if (typeof window === 'undefined') return;

  (window as any).__jlcpcbSearch = {
    get lastQuery() { return lastQuery; },
    get resultCount() { return resultCount; },
    get lastError() { return lastError; },
    get visible() { return visible; },
  };
}
