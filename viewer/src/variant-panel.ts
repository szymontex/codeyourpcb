/**
 * Variant panel UI — shows ranked routing variants after Route button generates them.
 * Supports hover preview and click-to-apply.
 */

/** Shape of a single variant result from the WASM engine */
export interface VariantData {
  name: string;
  score: {
    total_length: number;
    via_count: number;
    drc_violations: number;
    smoothness: number;
    crossings: number;
    layer_balance: number;
    composite: number;
  };
  routes: Array<{
    net_name: string;
    layer: string;
    width: number;
    segments: Array<{ start: [number, number]; end: [number, number] }>;
  }>;
  vias: Array<{
    x: number;
    y: number;
    drill: number;
    net_name: string;
  }>;
}

/** Debug surface shape exposed on window.__variantPanel */
export interface VariantPanelDebug {
  visible: boolean;
  variantCount: number;
  activeIndex: number;
  hoveredIndex: number;
  variants: Array<{ name: string; composite: number }>;
}

/** Callbacks from variant panel interactions */
export interface VariantPanelCallbacks {
  onHover: (index: number | null) => void;
  onClick: (index: number) => void;
}

let panelEl: HTMLElement | null = null;
let listEl: HTMLElement | null = null;
let variants: VariantData[] = [];
let activeIndex = 0;
let hoveredIndex = -1;
let callbacks: VariantPanelCallbacks | null = null;

/**
 * Initialize the variant panel — call once at startup.
 * Attaches to the DOM elements.
 */
export function initVariantPanel(cbs: VariantPanelCallbacks): void {
  panelEl = document.getElementById('variant-panel');
  listEl = document.getElementById('variant-list');
  callbacks = cbs;
  updateDebugSurface();
}

/**
 * Format a composite score for display.
 * Shows 1 decimal place; lower is better.
 */
export function formatScore(composite: number): string {
  return composite.toFixed(1);
}

/**
 * Show the variant panel with ranked results.
 * The best variant (index 0) is auto-selected as active.
 */
export function showVariants(variantResults: VariantData[], bestIndex: number = 0): void {
  variants = variantResults;
  activeIndex = bestIndex;
  hoveredIndex = -1;

  if (!panelEl || !listEl) return;

  // Clear existing rows
  listEl.textContent = '';

  const localList = listEl; // capture for closures

  // Create rows for each variant
  variants.forEach((v, i) => {
    const row = document.createElement('div');
    row.className = 'variant-row' + (i === activeIndex ? ' active' : '');
    row.dataset.index = String(i);

    const nameEl = document.createElement('span');
    nameEl.className = 'variant-name';
    nameEl.textContent = v.name;

    const scoreEl = document.createElement('span');
    scoreEl.className = 'variant-score';
    scoreEl.textContent = formatScore(v.score.composite);

    const metricsEl = document.createElement('span');
    metricsEl.className = 'variant-metrics';
    metricsEl.textContent = `${v.score.via_count}v · ${v.routes.length}r`;

    row.appendChild(nameEl);
    row.appendChild(scoreEl);
    row.appendChild(metricsEl);

    // Hover handlers
    row.addEventListener('mouseenter', () => {
      hoveredIndex = i;
      if (i !== activeIndex) {
        callbacks?.onHover(i);
      }
      updateDebugSurface();
    });

    row.addEventListener('mouseleave', () => {
      hoveredIndex = -1;
      callbacks?.onHover(null);
      updateDebugSurface();
    });

    // Click to apply
    row.addEventListener('click', () => {
      activeIndex = i;
      callbacks?.onClick(i);
      // Update row styles
      localList.querySelectorAll('.variant-row').forEach((r, ri) => {
        r.classList.toggle('active', ri === i);
      });
      updateDebugSurface();
    });

    localList.appendChild(row);
  });

  panelEl.classList.remove('hidden');
  updateDebugSurface();
}

/**
 * Hide the variant panel and clear state.
 */
export function hideVariants(): void {
  variants = [];
  activeIndex = 0;
  hoveredIndex = -1;

  if (panelEl) {
    panelEl.classList.add('hidden');
  }
  if (listEl) {
    listEl.textContent = '';
  }
  updateDebugSurface();
}

/**
 * Check if the variant panel is currently visible.
 */
export function isVariantPanelVisible(): boolean {
  return panelEl != null && !panelEl.classList.contains('hidden');
}

/**
 * Get the currently stored variants.
 */
export function getVariants(): VariantData[] {
  return variants;
}

/**
 * Get the active variant index.
 */
export function getActiveIndex(): number {
  return activeIndex;
}

/**
 * Update the window.__variantPanel debug surface.
 */
function updateDebugSurface(): void {
  const debug: VariantPanelDebug = {
    visible: isVariantPanelVisible(),
    variantCount: variants.length,
    activeIndex,
    hoveredIndex,
    variants: variants.map(v => ({
      name: v.name,
      composite: v.score.composite,
    })),
  };
  (window as any).__variantPanel = debug;
}
