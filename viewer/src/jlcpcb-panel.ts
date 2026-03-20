/**
 * JLCPCB Search Panel — right-side overlay for searching JLCPCB/LCSC components.
 *
 * Handles: debounced search input → API call → result rendering → component selection callback.
 * Follows the overlay pattern from project-manager.ts (show/hide/toggle, callback interface).
 *
 * Debug surface: `window.__jlcpcbSearch` exposes lastQuery, resultCount, lastError for E2E.
 */

import { searchComponents, JLCPCBSearchError, proxyImageUrl, type JLCPCBComponent } from './jlcpcb';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface JLCPCBPanelCallbacks {
  /** Called when user clicks a search result */
  onComponentSelect: (component: JLCPCBComponent) => void;
  /** Called when user wants to insert component snippet into the editor */
  onInsertToEditor?: (component: JLCPCBComponent) => void;
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

  // Phase 1: Render all results immediately — no images yet
  const imageSlots: { wrap: HTMLElement; comp: JLCPCBComponent }[] = [];

  for (const comp of results) {
    const row = document.createElement('div');
    row.className = 'jlcpcb-result';

    // Make row draggable
    row.draggable = true;
    row.addEventListener('dragstart', (e) => {
      const snippet = buildComponentSnippet(comp);
      e.dataTransfer?.setData('text/plain', snippet);
      e.dataTransfer?.setData('application/x-cypcb-component', JSON.stringify({
        lcsc: comp.lcsc,
        mfr: comp.mfr,
        package: comp.package,
        snippet,
      }));
      if (e.dataTransfer) e.dataTransfer.effectAllowed = 'copy';
      row.classList.add('dragging');
    });
    row.addEventListener('dragend', () => row.classList.remove('dragging'));

    // Top section: image slot + info
    const topRow = document.createElement('div');
    topRow.className = 'jlcpcb-result-top';

    // Reserve image slot only if component has an imageUrl
    if (comp.imageUrl) {
      const imgWrap = document.createElement('div');
      imgWrap.className = 'jlcpcb-result-img-wrap';
      imgWrap.style.display = 'none'; // hidden until thumbnail loads
      topRow.appendChild(imgWrap);
      imageSlots.push({ wrap: imgWrap, comp });
    }

    const infoCol = document.createElement('div');
    infoCol.className = 'jlcpcb-result-info';

    // Header: LCSC# + badge + package
    const header = document.createElement('div');
    header.className = 'jlcpcb-result-header';

    const lcscEl = document.createElement('span');
    lcscEl.className = 'jlcpcb-result-lcsc';
    lcscEl.textContent = `C${comp.lcsc}`;
    header.appendChild(lcscEl);

    if (comp.isBasic) {
      const basicBadge = document.createElement('span');
      basicBadge.className = 'jlcpcb-result-basic';
      basicBadge.textContent = 'Basic';
      header.appendChild(basicBadge);
    }

    if (comp.package) {
      const pkgEl = document.createElement('span');
      pkgEl.className = 'jlcpcb-result-package';
      pkgEl.textContent = comp.package;
      header.appendChild(pkgEl);
    }
    infoCol.appendChild(header);

    // Manufacturer + MPN
    if (comp.mfr || comp.manufacturer) {
      const mfrEl = document.createElement('div');
      mfrEl.className = 'jlcpcb-result-mfr';
      const parts: string[] = [];
      if (comp.manufacturer) parts.push(comp.manufacturer);
      if (comp.mfr) parts.push(comp.mfr);
      mfrEl.textContent = parts.join(' · ');
      infoCol.appendChild(mfrEl);
    }

    // Description
    if (comp.description) {
      const descEl = document.createElement('div');
      descEl.className = 'jlcpcb-result-desc';
      descEl.textContent = comp.description;
      descEl.title = comp.description;
      infoCol.appendChild(descEl);
    }

    // Attributes summary
    const attrKeys = Object.keys(comp.attributes);
    if (attrKeys.length > 0) {
      const attrsEl = document.createElement('div');
      attrsEl.className = 'jlcpcb-result-attrs';
      const attrText = attrKeys
        .slice(0, 3)
        .map((k) => `${k}: ${comp.attributes[k]}`)
        .join(' · ');
      attrsEl.textContent = attrText;
      infoCol.appendChild(attrsEl);
    }

    topRow.appendChild(infoCol);
    row.appendChild(topRow);

    // Footer: price, stock, actions
    const footer = document.createElement('div');
    footer.className = 'jlcpcb-result-footer';

    const metaWrap = document.createElement('div');
    metaWrap.className = 'jlcpcb-result-meta';

    const priceEl = document.createElement('span');
    priceEl.className = 'jlcpcb-result-price';
    priceEl.textContent = `$${comp.price.toFixed(4)}`;
    metaWrap.appendChild(priceEl);

    const stockEl = document.createElement('span');
    stockEl.className = 'jlcpcb-result-stock';
    stockEl.textContent = `Stock: ${comp.stock.toLocaleString()}`;
    metaWrap.appendChild(stockEl);

    if (comp.datasheetUrl) {
      const dsLink = document.createElement('a');
      dsLink.className = 'jlcpcb-result-datasheet';
      dsLink.href = comp.datasheetUrl;
      dsLink.target = '_blank';
      dsLink.rel = 'noopener noreferrer';
      dsLink.textContent = 'Datasheet';
      dsLink.addEventListener('click', (e) => e.stopPropagation());
      metaWrap.appendChild(dsLink);
    }

    footer.appendChild(metaWrap);

    // Insert to editor button
    const insertBtn = document.createElement('button');
    insertBtn.className = 'jlcpcb-result-insert';
    insertBtn.textContent = '⎘ Insert';
    insertBtn.title = 'Insert component snippet into editor (or drag & drop)';
    insertBtn.addEventListener('click', (e) => {
      e.stopPropagation();
      callbacks?.onInsertToEditor?.(comp);
    });
    footer.appendChild(insertBtn);

    row.appendChild(footer);

    // Click handler — select component (3D model, etc.)
    row.addEventListener('click', () => {
      callbacks?.onComponentSelect(comp);
    });

    resultsContainer.appendChild(row);
  }

  // Phase 2: Load thumbnails lazily after results are rendered
  if (imageSlots.length > 0) {
    loadThumbnailsLazy(imageSlots);
  }
}

/** Batch size for lazy thumbnail loading */
const THUMB_BATCH_SIZE = 5;

/**
 * Load thumbnails in small batches so results appear instantly,
 * then images pop in progressively. Each loaded image reveals its wrapper;
 * failed loads leave the wrapper hidden (no placeholder).
 */
function loadThumbnailsLazy(
  slots: { wrap: HTMLElement; comp: JLCPCBComponent }[],
): void {
  let index = 0;

  function loadBatch(): void {
    const batch = slots.slice(index, index + THUMB_BATCH_SIZE);
    if (batch.length === 0) return;
    index += THUMB_BATCH_SIZE;

    for (const { wrap, comp } of batch) {
      const img = document.createElement('img');
      img.className = 'jlcpcb-result-img';
      img.referrerPolicy = 'no-referrer';
      img.alt = `C${comp.lcsc}`;
      img.onload = () => {
        wrap.style.display = '';
        // Wire hover preview only after thumbnail loaded
        const largeUrl = comp.imageUrlLarge || comp.imageUrl;
        wrap.addEventListener('mouseenter', () => {
          showImagePreview(proxyImageUrl(largeUrl, true), wrap);
        });
        wrap.addEventListener('mouseleave', () => {
          hideImagePreview();
        });
      };
      img.onerror = () => {
        // Failed — leave wrapper hidden, no placeholder
        wrap.remove();
      };
      wrap.appendChild(img);
      img.src = proxyImageUrl(comp.imageUrl);
    }

    // Schedule next batch after a short delay
    if (index < slots.length) {
      setTimeout(loadBatch, 100);
    }
  }

  // Start first batch on next frame so DOM renders first
  requestAnimationFrame(() => loadBatch());
}

function clearResults(): void {
  if (resultsContainer) resultsContainer.textContent = '';
  hideStatus();
  hideImagePreview();
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
// Image preview popup
// ---------------------------------------------------------------------------

let previewEl: HTMLElement | null = null;
let previewImg: HTMLImageElement | null = null;

function showImagePreview(src: string, anchor: HTMLElement): void {
  if (!previewEl) {
    previewEl = document.createElement('div');
    previewEl.className = 'jlcpcb-img-preview';
    previewImg = document.createElement('img');
    previewImg.referrerPolicy = 'no-referrer';
    previewEl.appendChild(previewImg);
    document.body.appendChild(previewEl);
  }

  previewImg!.src = src;

  // Position: to the left of the panel, aligned with the thumbnail
  const rect = anchor.getBoundingClientRect();
  const panelEl = document.getElementById('jlcpcb-search-panel');
  const panelLeft = panelEl ? panelEl.getBoundingClientRect().left : rect.left;

  // Place preview to the left of the panel
  const previewSize = 300;
  let left = panelLeft - previewSize - 8;
  let top = rect.top - (previewSize / 2) + (rect.height / 2);

  // Clamp to viewport
  if (left < 4) left = 4;
  if (top < 4) top = 4;
  if (top + previewSize > window.innerHeight - 4) {
    top = window.innerHeight - previewSize - 4;
  }

  previewEl.style.left = left + 'px';
  previewEl.style.top = top + 'px';
  previewEl.classList.add('visible');
}

function hideImagePreview(): void {
  if (previewEl) {
    previewEl.classList.remove('visible');
  }
}

// ---------------------------------------------------------------------------
// Component snippet builder
// ---------------------------------------------------------------------------

/**
 * Build a .cypcb component snippet from JLCPCB search result.
 * Infers component type from package/attributes, uses LCSC# as reference.
 * When existingRefDes is provided, auto-numbers the new component
 * (e.g. if R1, R2 exist → generates R3).
 */
export function buildComponentSnippet(
  comp: JLCPCBComponent,
  existingRefDes: string[] = [],
): string {
  const lcscStr = `C${comp.lcsc}`;
  const prefix = inferRefDesPrefix(comp);
  const refDes = nextRefDes(prefix, existingRefDes);
  const compType = inferComponentType(comp);
  const pkg = comp.package || 'unknown';

  const lines: string[] = [];
  lines.push(`// ${comp.manufacturer ? comp.manufacturer + ' ' : ''}${comp.mfr} (LCSC: ${lcscStr})`);
  if (comp.description) {
    lines.push(`// ${comp.description}`);
  }
  lines.push(`component ${refDes} ${compType} "${pkg}" {`);

  // Add value from primary attribute
  const value = inferValue(comp);
  if (value) {
    lines.push(`    value "${value}"`);
  }

  lines.push(`    // lcsc "${lcscStr}"`);
  lines.push(`    at 10mm, 10mm`);
  lines.push(`}`);

  return lines.join('\n');
}

/** Infer a reference designator prefix (R, C, U, etc.) from component data */
function inferRefDesPrefix(comp: JLCPCBComponent): string {
  const attrs = comp.attributes;
  const mfr = (comp.mfr + ' ' + comp.manufacturer).toLowerCase();
  const desc = comp.description.toLowerCase();

  if (attrs['Capacitance'] || desc.includes('capacitor')) return 'C';
  if (attrs['Resistance'] || desc.includes('resistor')) return 'R';
  if (attrs['Inductance'] || desc.includes('inductor')) return 'L';
  if (desc.includes('led') || desc.includes('diode')) return 'D';
  if (desc.includes('transistor') || desc.includes('mosfet')) return 'Q';
  if (desc.includes('connector') || desc.includes('header') || desc.includes('socket')) return 'J';
  if (desc.includes('crystal') || desc.includes('oscillator')) return 'Y';
  if (desc.includes('fuse')) return 'F';
  if (mfr.includes('stm32') || mfr.includes('esp32') || desc.includes('mcu') || desc.includes('microcontroller')) return 'U';
  if (desc.includes('regulator') || desc.includes('converter')) return 'U';
  return 'U';
}

/**
 * Generate the next available refdes for a given prefix.
 * Scans existingRefDes for the highest number with that prefix and increments.
 * e.g. prefix='R', existing=['R1','R2','C1'] → 'R3'
 */
function nextRefDes(prefix: string, existing: string[]): string {
  let maxNum = 0;
  const re = new RegExp(`^${prefix}(\\d+)$`);
  for (const ref of existing) {
    const m = ref.match(re);
    if (m) {
      const n = parseInt(m[1], 10);
      if (n > maxNum) maxNum = n;
    }
  }
  return `${prefix}${maxNum + 1}`;
}

/** Infer component type keyword for .cypcb syntax */
function inferComponentType(comp: JLCPCBComponent): string {
  const attrs = comp.attributes;
  const desc = comp.description.toLowerCase();

  if (attrs['Capacitance'] || desc.includes('capacitor')) return 'capacitor';
  if (attrs['Resistance'] || desc.includes('resistor')) return 'resistor';
  if (attrs['Inductance'] || desc.includes('inductor')) return 'inductor';
  if (desc.includes('led')) return 'led';
  if (desc.includes('diode')) return 'diode';
  if (desc.includes('connector') || desc.includes('header') || desc.includes('socket')) return 'connector';
  if (desc.includes('crystal') || desc.includes('oscillator')) return 'crystal';
  return 'ic';
}

/** Extract a meaningful value string from component attributes */
function inferValue(comp: JLCPCBComponent): string {
  const attrs = comp.attributes;
  if (attrs['Capacitance']) return attrs['Capacitance'];
  if (attrs['Resistance']) return attrs['Resistance'];
  if (attrs['Inductance']) return attrs['Inductance'];
  if (attrs['Voltage Rated']) return attrs['Voltage Rated'];
  if (comp.mfr) return comp.mfr;
  return '';
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
