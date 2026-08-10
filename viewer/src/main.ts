/**
 * Main entry point for the CodeYourPCB viewer application
 * Integrates WASM engine, rendering, and user interaction
 */

import './theme/colors.css';
import { themeManager } from './theme/theme-manager';
import { loadWasm, isWasmLoaded, type PcbEngine } from './wasm';
import type { BoardSnapshot, ViolationInfo } from './types';
import type { SourceDiagnostic } from './editor/lsp-bridge';
import { createViewport, fitBoard, screenToWorld } from './viewport';
import { render, type RenderState } from './renderer';
import { createDefaultRenderConfig, buildPadNetMap } from './render-config';
import { setupInteraction, type InteractionState } from './interaction';
import { createRoutingState, type RoutingState } from './routing';
import { UndoStack, AddTraceCommand, RemoveTraceCommand, RotateComponentCommand, ResizeBoardCommand, EditTraceCommand, installDebugSurface } from './undo';
import { createLayerVisibility, innerVisibleFromUrlLayers } from './layers';
import { collectImportedFiles, importedPaths, readerForBaseUrl } from './imports';
import { createFilePicker, setupDropZone, readFileAsText } from './file-picker';
import { openFile, saveFile } from './file-access';
import { isDesktop, initDesktop } from './desktop';
import { decodeViewState } from './url-state';
import { getSettings, getPreference, setPreference, subscribe as subscribeSettings } from './settings';
import type { AppSettings, LayerColors, AutorouteParams } from './settings';
import { formatDimension, parseUserDimension } from './units';
import type { DisplayUnit } from './units';
import { initProjectManager, showProjectManager, hideProjectManager, addRecentFile, updateProjectFiles } from './project-manager';
import { initSearchPanel, hideSearchPanel, toggleSearchPanel, isSearchPanelVisible, buildComponentSnippet } from './jlcpcb-panel';
import { fetch3DModel, fetchComponentFootprint } from './jlcpcb';
import { registerDynamicFootprint, register3DModel, hasDynamicFootprint } from './wasm';
import { initVariantPanel, hideVariants, isVariantPanelVisible, type VariantData } from './variant-panel';
import type { VariantPreviewData } from './renderer';
import { mergeTracesIntoDsl, syncTracesToEditor } from './trace-persist';
import { describeViolationKind } from './violation-kinds';

// WebSocket server URL for hot reload + FreeRouting.
// Only used when `npm run start` (dev server with file watcher) is running.
function getWsUrl(): string {
  const proto = window.location.protocol === 'https:' ? 'wss' : 'ws';
  const host = window.location.hostname;
  const port = 4322;
  return `${proto}://${host}:${port}`;
}
const WS_URL = getWsUrl();

/**
 * WebSocket message types from the dev server
 */
interface WsMessage {
  type: string;
  file?: string;
  content?: string;
  timestamp?: number;
  output?: string;
  error?: string;
  sesContent?: string | null;
  routesContent?: string | null;
  pass?: number;
  routed?: number;
  unrouted?: number;
  files?: Array<{ path: string; name: string }>;
  path?: string;
}

/**
 * WebSocket connection interface for two-way communication with dev server
 */
interface WsConnection {
  send(message: object): void;
  isConnected(): boolean;
  /**
   * Ask the dev server for a file beside the design, for an `import`.
   *
   * Resolves with the text, or null when the server refuses or has no such
   * file - it answers `file-content` either way, so a missing library is a
   * null here rather than a promise nobody settles.
   */
  readFile(path: string): Promise<string | null>;
}

/**
 * Callbacks for various WebSocket events
 */
interface WsCallbacks {
  onReload: (content: string, file: string) => void;
  onConnect?: () => void;
  onRouteStart?: () => void;
  onRouteProgress?: (output: string) => void;
  onRouteComplete?: (sesContent: string | null, routesContent: string | null) => void;
  onRouteError?: (error: string) => void;
  onFileList?: (files: Array<{ path: string; name: string }>) => void;
}

/**
 * Connect to the WebSocket server for hot reload and routing.
 * Automatically reconnects on disconnect.
 */
function connectWebSocket(callbacks: WsCallbacks): WsConnection {
  let ws: WebSocket | null = null;
  let connected = false;
  let retries = 0;
  /** `read-file` requests waiting for their answer, by the path they asked for. */
  const pendingReads = new Map<string, (text: string | null) => void>();
  const MAX_RETRIES = 2; // Try 3 times total (initial + 2 retries), then give up silently

  function connect(): void {
    try {
      ws = new WebSocket(WS_URL);
    } catch {
      // WebSocket constructor can throw on invalid URLs
      return;
    }

    ws.onopen = () => {
      console.log('[WS] Connected to dev server');
      connected = true;
      retries = 0;
      callbacks.onConnect?.();
    };

    ws.onmessage = (event) => {
      try {
        const msg: WsMessage = JSON.parse(event.data);

        switch (msg.type) {
          case 'init':
          case 'reload':
            if (msg.content && msg.file) {
              callbacks.onReload(msg.content, msg.file);
            }
            break;
          case 'route-start':
            callbacks.onRouteStart?.();
            break;
          case 'route-progress':
            callbacks.onRouteProgress?.(msg.output || '');
            break;
          case 'route-complete':
            callbacks.onRouteComplete?.(msg.sesContent || null, msg.routesContent || null);
            break;
          case 'route-error':
            callbacks.onRouteError?.(msg.error || 'Unknown routing error');
            break;
          case 'file-list':
            callbacks.onFileList?.(msg.files || []);
            break;
          case 'file-content': {
            const settle = pendingReads.get(msg.path || '');
            if (settle) {
              pendingReads.delete(msg.path || '');
              settle(msg.content ?? null);
            }
            break;
          }
        }
      } catch (err) {
        console.error('[WS] Message parse error:', err);
      }
    };

    ws.onclose = () => {
      connected = false;
      if (retries < MAX_RETRIES) {
        retries++;
        setTimeout(connect, 2000);
      }
      // After MAX_RETRIES, stop silently — dev server not running is normal for `npx vite`
    };

    ws.onerror = () => {
      // Error is handled by onclose
    };
  }

  connect();

  return {
    send(message: object): void {
      if (ws && connected) {
        ws.send(JSON.stringify(message));
      } else {
        console.warn('[WS] Cannot send, not connected');
      }
    },
    isConnected(): boolean {
      return connected;
    },
    readFile(path: string): Promise<string | null> {
      if (!ws || !connected) return Promise.resolve(null);
      // One request per path in flight: the design and its libraries are
      // walked breadth-first and a path is asked for once.
      const waiting = pendingReads.get(path);
      if (waiting) return Promise.resolve(null);

      return new Promise((settle) => {
        const timer = setTimeout(() => {
          if (pendingReads.delete(path)) settle(null);
        }, 5_000);
        pendingReads.set(path, (text) => {
          clearTimeout(timer);
          settle(text);
        });
        ws!.send(JSON.stringify({ type: 'read-file', path }));
      });
    },
  };
}

// Note: Test data removed. Use examples/routing-test.cypcb and examples/routing-test.ses via file picker.

/**
 * Initialize the PCB viewer application
 */
async function init(): Promise<void> {
  const statusText = document.getElementById('status-text')!;
  const errorBadge = document.getElementById('error-badge')!;
  const errorCountEl = document.getElementById('error-count')!;
  const errorPanel = document.getElementById('error-panel')!;
  const errorList = document.getElementById('error-list')!;
  const errorPanelClose = document.getElementById('error-panel-close')!;
  const canvas = document.getElementById('pcb-canvas') as HTMLCanvasElement;
  const container = document.getElementById('canvas-container')!;
  const coordsEl = document.getElementById('coords')!;
  const topLayerCb = document.getElementById('layer-top') as HTMLInputElement;
  const bottomLayerCb = document.getElementById('layer-bottom') as HTMLInputElement;
  // A four-layer board's middle: drawn since the renderer learned about it,
  // and until this switch existed there was no way for a person to turn it off.
  const innerLayerCb = document.getElementById('layer-inner') as HTMLInputElement;
  const ratsnestCb = document.getElementById('layer-ratsnest') as HTMLInputElement;
  const colorByNetCb = document.getElementById('view-color-by-net') as HTMLInputElement;
  const gridVisibleCb = document.getElementById('view-grid-visible') as HTMLInputElement;
  const netLabelsCb = document.getElementById('view-net-labels') as HTMLInputElement;
  const viewMenuBtn = document.getElementById('view-menu-btn') as HTMLButtonElement;
  const viewMenuDropdown = document.getElementById('view-menu-dropdown')!;
  const prefsBtn = document.getElementById('prefs-btn') as HTMLButtonElement;
  const prefsOverlay = document.getElementById('prefs-overlay')!;
  const prefsClose = document.getElementById('prefs-close') as HTMLButtonElement;
  const helpBtn = document.getElementById('help-btn') as HTMLButtonElement;
  const helpOverlay = document.getElementById('help-overlay')!;
  const helpClose = document.getElementById('help-close') as HTMLButtonElement;
  const undoBtn = document.getElementById('undo-btn') as HTMLButtonElement;
  const redoBtn = document.getElementById('redo-btn') as HTMLButtonElement;
  const routeBtn = document.getElementById('route-btn') as HTMLButtonElement;
  const routeMenuBtn = document.getElementById('route-menu-btn') as HTMLButtonElement;
  const cancelRouteBtn = document.getElementById('cancel-route-btn') as HTMLButtonElement;
  const routingStatus = document.getElementById('routing-status')!;
  const routingProgress = document.getElementById('routing-progress')!;
  const openBtn = document.getElementById('open-btn') as HTMLButtonElement;
  const themeToggle = document.getElementById('theme-toggle') as HTMLButtonElement;
  const themeIcon = document.getElementById('theme-icon')!;
  const editorToggleBtn = document.getElementById('editor-toggle') as HTMLButtonElement;
  const fitBtn = document.getElementById('fit-btn') as HTMLButtonElement;
  const view3dBtn = document.getElementById('view-3d-btn') as HTMLButtonElement;
  const editorContainer = document.getElementById('editor-container')!

  const ctx = canvas.getContext('2d')!;

  // Routing state
  let isRouting = false;
  let currentFilePath: string | null = null;

  // File handle for save-in-place (File System Access API)
  let currentFileHandle: FileSystemFileHandle | null = null;

  /**
   * Update error badge with violation count
   */
  function updateErrorBadge(violations: ViolationInfo[]): void {
    if (violations.length > 0) {
      errorCountEl.textContent = String(violations.length);
      errorBadge.classList.remove('hidden');
      // Update panel header count too
      const panelCount = document.getElementById('error-panel-count');
      if (panelCount) panelCount.textContent = String(violations.length);
    } else {
      errorBadge.classList.add('hidden');
      errorPanel.classList.add('hidden');
    }
  }

  // State
  let snapshot: BoardSnapshot | null = null;
  let viewport = createViewport(canvas.width, canvas.height);
  let layers = createLayerVisibility();
  let selectedRefdes: string | null = null;
  let dirty = true;
  let tracesUnsaved = false; // True when routed traces haven't been saved to file
  let debugOverlayStage: number = -1;
  let debugData: any = null;
  let lastLoadedSource: string | null = null;
  /**
   * Which reader the open board came from.
   *
   * The editor hands its content to `load_source` on every change, and
   * `load_source` clears the world before parsing. With a `.kicad_pcb` in the
   * editor - which is what opening one put there - the first character typed
   * emptied the board.
   */
  let loadedKind: 'cypcb' | 'kicad_pcb' = 'cypcb';
  let showRatsnest = getPreference('ratsnestVisible');
  let gridVisible = getPreference('gridVisible');
  let showNetLabels = getPreference('netLabelsVisible');
  // Net colours say what is connected to what; layer colours say which side of
  // the board a trace is on. This was a `const true` in three places, so the
  // layer colours `getTraceColor` defines were unreachable for traces and the
  // screen could not answer "which layer is this on" at all - the one question
  // every other PCB tool answers by colour.
  let colorByNet = getPreference('traceColorByNet');
  let selectedTraceId: number | null = null;
  let hoveredTraceId: number | null = null;
  let labelPosition: { x: number; y: number } | null = null;
  let routingState: RoutingState = createRoutingState();
  let highlightedNet: string | null = null;
  const renderConfig = createDefaultRenderConfig();
  let padNetMap = new Map<string, string>();

  // Variant preview state
  let variantPreview: VariantPreviewData | null = null;
  let storedVariants: VariantData[] = [];

  /**
   * Pull fresh snapshot from engine and rebuild derived state (padNetMap).
   * Returns the snapshot for convenient non-null access at call sites.
   */
  function pullSnapshot(): BoardSnapshot {
    const s = engine.get_snapshot();
    snapshot = s;
    padNetMap = s.nets ? buildPadNetMap(s.nets) : new Map();
    return s;
  }

  /** Update document title with unsaved indicator. */
  function updateTitle(): void {
    const name = currentFilePath || 'Untitled';
    document.title = tracesUnsaved ? `\u2022 ${name} — CodeYourPCB` : `${name} — CodeYourPCB`;
  }

  /** Mark traces as modified (unsaved). */
  function markTracesUnsaved(): void {
    if (!tracesUnsaved) {
      tracesUnsaved = true;
      updateTitle();
    }
  }

  /**
   * Live-sync routed traces to the Monaco editor (if open).
   * Uses targeted edit to replace only the trace section — no cursor jump.
   */
  function syncEditorTraces(): void {
    if (!editorReady || !editorInstance) return;
    const exportedTraces = engine.export_traces_as_dsl();
    // Wrap suppressSync as ref object so syncTracesToEditor can toggle it
    const ref = { get value() { return suppressSync; }, set value(v: boolean) { suppressSync = v; } };
    syncTracesToEditor(editorInstance, exportedTraces, ref);
  }

  // Warn before closing with unsaved trace data
  window.addEventListener('beforeunload', (e) => {
    if (tracesUnsaved) {
      e.preventDefault();
      e.returnValue = '';
    }
  });

  /**
   * Fetch the real footprint for every part the design names.
   *
   * `lcsc "C7593"` is a property of the language, so the part number comes off
   * the board model. It used to come from a regular expression over the raw
   * source - a second reader of the language, which could not see a part in a
   * module, missed any part number that was not `C` followed by digits, and
   * gave up on a component block containing a nested brace.
   *
   * After fetching, the caller re-parses so components get their real pads.
   */
  async function autoFetchLcscFootprints(_source: string): Promise<boolean> {
    const parts = (engine.get_snapshot().components ?? [])
      .filter((component): component is typeof component & { lcsc: string } =>
        typeof component.lcsc === 'string' && component.lcsc.length > 0,
      );
    if (parts.length === 0) return false;

    const toFetch: { pkg: string; lcscId: number }[] = [];
    const seen = new Set<string>();
    for (const component of parts) {
      const lcscId = parseInt(component.lcsc.replace(/^C/i, ''), 10);
      if (isNaN(lcscId) || hasDynamicFootprint(component.footprint)) continue;
      if (seen.has(component.footprint)) continue;
      seen.add(component.footprint);
      toFetch.push({ pkg: component.footprint, lcscId });
    }

    // If everything is already cached, still signal re-parse needed
    if (toFetch.length === 0) return true;

    console.log(`[LCSC] Auto-fetching ${toFetch.length} footprint(s)...`);
    let fetched = 0;

    await Promise.all(toFetch.map(async ({ pkg, lcscId }) => {
      try {
        const fp = await fetchComponentFootprint(lcscId);
        if (fp) {
          registerDynamicFootprint(pkg, fp.pads, fp.silk);
          if (fp.modelUuid) {
            register3DModel(pkg, fp.modelUuid);
          }
          fetched++;
        }
      } catch (e) {
        console.warn(`[LCSC] Failed to fetch footprint for C${lcscId}:`, e);
      }
    }));

    console.log(`[LCSC] Fetched ${fetched}/${toFetch.length} footprints. Registered packages:`, toFetch.map(t => t.pkg));
    return fetched > 0;
  }

  // 3D view state
  let is3DActive = false;
  let renderer3d: import('./renderer3d').Renderer3D | null = null;

  // Flag to track if interactionState is initialized (const in TDZ can't be typeof-checked)
  let interactionReady = false;

  // Resize handler
  function resize(): void {
    canvas.width = container.clientWidth;
    canvas.height = container.clientHeight;
    viewport = {
      ...viewport,
      width: canvas.width,
      height: canvas.height,
    };
    // Sync to interaction state if it exists (not yet initialized on first call)
    if (interactionReady) {
      interactionState.viewport = viewport;
    }
    dirty = true;
  }
  resize();
  window.addEventListener('resize', resize);

  // Also watch container size changes (e.g. when editor panel opens/closes)
  if (typeof ResizeObserver !== 'undefined') {
    new ResizeObserver(() => resize()).observe(container);
  }

  // --- View menu ---
  viewMenuBtn.addEventListener('click', (e) => {
    e.stopPropagation();
    viewMenuDropdown.classList.toggle('hidden');
  });

  // Close View dropdown on click outside or Escape
  document.addEventListener('click', (e) => {
    if (!viewMenuDropdown.classList.contains('hidden') &&
        !viewMenuDropdown.contains(e.target as Node) &&
        e.target !== viewMenuBtn) {
      viewMenuDropdown.classList.add('hidden');
    }
  });
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && !viewMenuDropdown.classList.contains('hidden')) {
      viewMenuDropdown.classList.add('hidden');
    }
  });

  // Layer checkbox handlers — update both 2D and 3D views
  topLayerCb.addEventListener('change', () => {
    layers = { ...layers, topCopper: topLayerCb.checked };
    dirty = true;
    if (is3DActive && renderer3d) {
      renderer3d.updateLayerVisibility(layers);
    }
  });
  bottomLayerCb.addEventListener('change', () => {
    layers = { ...layers, bottomCopper: bottomLayerCb.checked };
    dirty = true;
    if (is3DActive && renderer3d) {
      renderer3d.updateLayerVisibility(layers);
    }
  });
  innerLayerCb?.addEventListener('change', () => {
    layers = { ...layers, innerCopper: innerLayerCb.checked };
    dirty = true;
    if (is3DActive && renderer3d) {
      renderer3d.updateLayerVisibility(layers);
    }
  });
  ratsnestCb.addEventListener('change', () => {
    showRatsnest = ratsnestCb.checked;
    setPreference('ratsnestVisible', ratsnestCb.checked);
    dirty = true;
  });
  gridVisibleCb.addEventListener('change', () => {
    gridVisible = gridVisibleCb.checked;
    setPreference('gridVisible', gridVisibleCb.checked);
    dirty = true;
  });
  netLabelsCb.addEventListener('change', () => {
    showNetLabels = netLabelsCb.checked;
    setPreference('netLabelsVisible', netLabelsCb.checked);
    dirty = true;
  });
  colorByNetCb.addEventListener('change', () => {
    colorByNet = colorByNetCb.checked;
    setPreference('traceColorByNet', colorByNetCb.checked);
    dirty = true;
  });

  // Initialize View menu checkboxes from settings
  gridVisibleCb.checked = gridVisible;
  netLabelsCb.checked = showNetLabels;
  ratsnestCb.checked = getPreference('ratsnestVisible');
  colorByNetCb.checked = colorByNet;

  // Undo and redo, in one place.
  //
  // The body was written three times - the toolbar buttons, the keyboard
  // handler, and nowhere for the desktop Edit menu, which is why clicking
  // Edit > Undo did nothing. Two of the copies had already drifted: the
  // toolbar forgot `interactionState.selectedTraceId`, so a trace selected
  // for dragging stayed selected in the interaction layer after the entity
  // it named was gone.
  const performUndo = () => {
    undoStack.undo();
    selectedTraceId = null;
    interactionState.selectedTraceId = null;
    labelPosition = null;
    dirty = true;
  };
  const performRedo = () => {
    undoStack.redo();
    selectedTraceId = null;
    interactionState.selectedTraceId = null;
    labelPosition = null;
    dirty = true;
  };

  // Undo/Redo toolbar buttons
  undoBtn.addEventListener('click', performUndo);
  redoBtn.addEventListener('click', performRedo);

  // The desktop build's Edit menu reaches the same two functions.
  window.addEventListener('desktop:edit', (event: Event) => {
    const detail = (event as CustomEvent).detail as { action?: string };
    if (detail?.action === 'undo') {
      performUndo();
    } else if (detail?.action === 'redo') {
      performRedo();
    }
  });

  // Load WASM
  statusText.textContent = 'Loading WASM...';
  let engine: PcbEngine;
  try {
    engine = await loadWasm();
  } catch (err) {
    console.error('WASM load failed:', err);
    statusText.textContent = `WASM Error: ${err}`;
    return;
  }

  const usingWasm = isWasmLoaded();

  // Expose engine for E2E / debugging
  (window as any).__pcbEngine = engine;

  // Undo/Redo stack
  const undoStack = new UndoStack();
  installDebugSurface(undoStack);

  // Subscribe to theme changes to trigger canvas re-render + 3D background sync
  themeManager.subscribe(() => {
    dirty = true;
    if (is3DActive && renderer3d) {
      const style = getComputedStyle(document.documentElement);
      const bgColor = style.getPropertyValue('--bg-canvas').trim() || '#1a1a2e';
      renderer3d.setBackground(bgColor);
    }
  });

  // Theme toggle
  function updateThemeIcon(): void {
    const theme = themeManager.getTheme();
    switch (theme) {
      case 'light':
        themeIcon.textContent = '☀️';
        themeToggle.title = 'Theme: Light (click to switch)';
        break;
      case 'dark':
        themeIcon.textContent = '🌙';
        themeToggle.title = 'Theme: Dark (click to switch)';
        break;
      case 'auto':
        themeIcon.textContent = '🔄';
        themeToggle.title = 'Theme: Auto (click to switch)';
        break;
    }
  }

  themeToggle.addEventListener('click', () => {
    const current = themeManager.getTheme();
    // Simple toggle: if currently dark (or auto resolving dark), switch to light; otherwise dark
    // Auto is only settable from Preferences panel
    const resolved = current === 'auto'
      ? (window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light')
      : current;
    const next = resolved === 'dark' ? 'light' : 'dark';
    themeManager.setTheme(next);
    updateThemeIcon();
  });

  // Also update icon when OS theme changes (relevant in auto mode)
  themeManager.subscribe(() => {
    updateThemeIcon();
  });

  updateThemeIcon();

  // --- Preferences modal ---
  function openPrefsModal(): void {
    // Populate all inputs from current settings
    const settings = getSettings();

    // Theme button label
    const prefsThemeBtn = document.getElementById('prefs-theme-btn') as HTMLButtonElement;
    const currentTheme = themeManager.getTheme();
    prefsThemeBtn.textContent = currentTheme === 'light' ? '☀️ Light' : currentTheme === 'dark' ? '🌙 Dark' : '🔄 Auto';

    // Units select
    const unitsSelect = document.getElementById('prefs-units') as HTMLSelectElement;
    unitsSelect.value = settings.units;

    // Grid spacing inputs — format using current units
    const gridVisualInput = document.getElementById('prefs-grid-visual') as HTMLInputElement;
    const gridSnapInput = document.getElementById('prefs-grid-snap') as HTMLInputElement;
    gridVisualInput.value = formatDimension(settings.gridVisualSpacing, settings.units);
    gridSnapInput.value = formatDimension(settings.gridSnapSpacing, settings.units);

    // Color pickers
    (document.getElementById('prefs-color-top') as HTMLInputElement).value = settings.layerColors.topCopper;
    (document.getElementById('prefs-color-bottom') as HTMLInputElement).value = settings.layerColors.bottomCopper;
    (document.getElementById('prefs-color-silk') as HTMLInputElement).value = settings.layerColors.silkscreen;
    (document.getElementById('prefs-color-via') as HTMLInputElement).value = settings.layerColors.via;
    (document.getElementById('prefs-color-drill') as HTMLInputElement).value = settings.layerColors.drill;

    prefsOverlay.classList.remove('hidden');
  }

  function closePrefsModal(): void {
    prefsOverlay.classList.add('hidden');
  }

  prefsBtn.addEventListener('click', () => {
    hideSearchPanel();
    openPrefsModal();
  });
  prefsClose.addEventListener('click', closePrefsModal);
  prefsOverlay.addEventListener('click', (e) => {
    if (e.target === prefsOverlay) closePrefsModal();
  });
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && !prefsOverlay.classList.contains('hidden')) {
      closePrefsModal();
    }
  });

  // --- Help modal ---
  function openHelpModal(): void {
    helpOverlay.classList.remove('hidden');
  }
  function closeHelpModal(): void {
    helpOverlay.classList.add('hidden');
  }
  helpBtn.addEventListener('click', openHelpModal);
  helpClose.addEventListener('click', closeHelpModal);
  helpOverlay.addEventListener('click', (e) => {
    if (e.target === helpOverlay) closeHelpModal();
  });
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && !helpOverlay.classList.contains('hidden')) {
      closeHelpModal();
      e.preventDefault();
    }
  });

  // Theme cycle in prefs
  document.getElementById('prefs-theme-btn')!.addEventListener('click', () => {
    const current = themeManager.getTheme();
    const next = current === 'light' ? 'dark' : current === 'dark' ? 'auto' : 'light';
    themeManager.setTheme(next);
    setPreference('theme', next);
    updateThemeIcon();
    const btn = document.getElementById('prefs-theme-btn') as HTMLButtonElement;
    btn.textContent = next === 'light' ? '☀️ Light' : next === 'dark' ? '🌙 Dark' : '🔄 Auto';
  });

  // Units select in prefs
  document.getElementById('prefs-units')!.addEventListener('change', (e) => {
    const unit = (e.target as HTMLSelectElement).value as DisplayUnit;
    setPreference('units', unit);
    // Re-format grid spacing inputs with new unit
    const settings = getSettings();
    (document.getElementById('prefs-grid-visual') as HTMLInputElement).value =
      formatDimension(settings.gridVisualSpacing, unit);
    (document.getElementById('prefs-grid-snap') as HTMLInputElement).value =
      formatDimension(settings.gridSnapSpacing, unit);
    dirty = true;
  });

  // Grid spacing inputs in prefs — parse on change
  document.getElementById('prefs-grid-visual')!.addEventListener('change', (e) => {
    const nm = parseUserDimension((e.target as HTMLInputElement).value);
    if (nm != null && nm > 0) {
      setPreference('gridVisualSpacing', nm);
      dirty = true;
    } else {
      console.warn('[prefs] Invalid grid visual spacing, reverting');
      (e.target as HTMLInputElement).value = formatDimension(getPreference('gridVisualSpacing'), getPreference('units'));
    }
  });
  document.getElementById('prefs-grid-snap')!.addEventListener('change', (e) => {
    const nm = parseUserDimension((e.target as HTMLInputElement).value);
    if (nm != null && nm > 0) {
      setPreference('gridSnapSpacing', nm);
      // Update routing state grid spacing
      routingState = { ...routingState, gridSpacing: nm };
      interactionState.routing = routingState;
      dirty = true;
    } else {
      console.warn('[prefs] Invalid grid snap spacing, reverting');
      (e.target as HTMLInputElement).value = formatDimension(getPreference('gridSnapSpacing'), getPreference('units'));
    }
  });

  // Color pickers in prefs — map data-pref to layerColors keys
  const colorInputMap: [string, keyof LayerColors][] = [
    ['prefs-color-top', 'topCopper'],
    ['prefs-color-bottom', 'bottomCopper'],
    ['prefs-color-silk', 'silkscreen'],
    ['prefs-color-via', 'via'],
    ['prefs-color-drill', 'drill'],
  ];
  for (const [elId, colorKey] of colorInputMap) {
    document.getElementById(elId)!.addEventListener('input', (e) => {
      const hex = (e.target as HTMLInputElement).value;
      if (/^#[0-9a-f]{6}$/i.test(hex)) {
        const colors = getPreference('layerColors');
        colors[colorKey] = hex;
        setPreference('layerColors', colors);
        // Propagate to renderConfig
        renderConfig.layerColors[colorKey] = hex;
        dirty = true;
      } else {
        console.warn(`[prefs] Invalid color value for ${colorKey}: ${hex}`);
      }
    });
  }

  // Subscribe to settings changes — sync renderConfig and trigger re-render
  subscribeSettings((settings: AppSettings) => {
    // Sync layer colors to renderConfig
    renderConfig.layerColors.topCopper = settings.layerColors.topCopper;
    renderConfig.layerColors.bottomCopper = settings.layerColors.bottomCopper;
    renderConfig.layerColors.silkscreen = settings.layerColors.silkscreen;
    renderConfig.layerColors.via = settings.layerColors.via;
    renderConfig.layerColors.drill = settings.layerColors.drill;

    // Sync grid visibility and net labels
    gridVisible = settings.gridVisible;
    showNetLabels = settings.netLabelsVisible;
    showRatsnest = settings.ratsnestVisible;

    // Update routing grid spacing
    if (routingState.gridSpacing !== settings.gridSnapSpacing) {
      routingState = { ...routingState, gridSpacing: settings.gridSnapSpacing };
      interactionState.routing = routingState;
    }

    dirty = true;
  });

  // Monaco editor setup
  let editorInstance: any = null;
  let editorReady = false;
  let suppressSync = false; // Prevent circular updates when setting editor content programmatically
  const { initEditor, toggleEditorPanel, isEditorVisible, getMonacoModule } = await import('./editor/editor-panel');
  const { updateDiagnostics } = await import('./editor/lsp-bridge');

  /**
   * The engine's own account of what is wrong and where.
   *
   * `load_source` hands back the messages as one blob of text; the line each
   * belongs to comes from here, because a message does not carry one.
   */
  function readDiagnostics(engine: PcbEngine): SourceDiagnostic[] {
    try {
      return JSON.parse(engine.get_diagnostics_json()) as SourceDiagnostic[];
    } catch {
      return [];
    }
  }

  /**
   * Setup editor-to-board sync with debounce
   * When editor content changes, parse and update the board viewer
   */
  function setupEditorSync(editor: any): void {
    let debounceTimer: number | null = null;

    editor.onDidChangeModelContent(() => {
      // Skip sync if content was set programmatically (file load, hot reload)
      if (suppressSync) {
        return;
      }

      // Debounce for 300ms
      if (debounceTimer !== null) {
        clearTimeout(debounceTimer);
      }

      debounceTimer = window.setTimeout(() => {
        const content = editor.getValue();
        console.log('[Editor] Content changed, reloading board...');

        // Parse and update board, with the reader this board came from. A
        // KiCad board handed to the DSL reader comes back as an empty world
        // and a page of parse errors.
        const errors = loadedKind === 'kicad_pcb'
          ? engine.load_kicad(content)
          : loadDesign(content);
        if (errors) {
          console.warn('[Editor] Parse errors:', errors);
        }

        // Update snapshot
        pullSnapshot();

        // Update 3D view if active
        if (is3DActive && renderer3d && snapshot) {
          renderer3d.updateBoard(snapshot, layers);
        }

        // Update inline diagnostics (LSP bridge)
        const monaco = getMonacoModule();
        if (monaco && editorInstance) {
          updateDiagnostics(monaco, editorInstance, readDiagnostics(engine), snapshot!.violations || []);
        }

        // Update error badge
        if (snapshot!.violations) {
          updateErrorBadge(snapshot!.violations);
        }

        // Track as loaded source
        lastLoadedSource = content;

        // Mark as dirty for re-render
        dirty = true;

        // Auto-fetch LCSC footprints if any new lcsc attributes found
        autoFetchLcscFootprints(content).then((fetched) => {
          if (fetched) reloadAfterLcscFetch(content);
        });

        debounceTimer = null;
      }, 300);
    });

    console.log('[Editor] Sync wired up with 300ms debounce');
  }

  // Fit board to viewport button
  fitBtn.addEventListener('click', () => {
    if (snapshot?.board) {
      viewport = fitBoard(viewport, snapshot.board.width_nm, snapshot.board.height_nm);
      interactionState.viewport = viewport;
      dirty = true;
    }
  });

  // 3D view toggle
  view3dBtn.addEventListener('click', async () => {
    if (!is3DActive) {
      // Switch to 3D
      if (!renderer3d) {
        try {
          const { Renderer3D } = await import('./renderer3d');
          renderer3d = new Renderer3D();
          renderer3d.init(container);
        } catch (e) {
          console.error('[3D] WebGL not available', e);
          statusText.textContent = 'WebGL not available';
          return;
        }
      }

      // Hide 2D canvas, show 3D
      canvas.style.display = 'none';
      is3DActive = true;
      view3dBtn.classList.add('active');

      // Update board in 3D
      if (snapshot) {
        renderer3d.updateBoard(snapshot, layers);
      }
    } else {
      // Switch back to 2D
      is3DActive = false;
      view3dBtn.classList.remove('active');

      // Show 2D canvas, dispose 3D
      canvas.style.display = '';
      if (renderer3d) {
        renderer3d.dispose();
        renderer3d = null;
      }

      dirty = true; // Force 2D re-render
    }
  });

  // Editor toggle button handler
  /**
   * Ensure the Monaco editor is initialized and visible.
   * Lazy-loads on first call. Returns when the editor is ready.
   */
  // Memoized so concurrent callers share one initialization. The toolbar click
  // and the idle preload can both land before Monaco finishes loading, and two
  // initEditor() calls create two editors in the same container - which is why
  // specs saw `.monaco-editor` resolve to two elements.
  let editorInitPromise: Promise<void> | null = null;

  async function ensureEditorReady(): Promise<void> {
    if (editorReady) return;

    if (!editorInitPromise) {
      editorInitPromise = (async () => {
        console.log('[Editor] Initializing Monaco editor...');
        editorInstance = await initEditor(editorContainer);

        if (lastLoadedSource) {
          suppressSync = true;
          editorInstance.setValue(lastLoadedSource);
          suppressSync = false;
        }

        setupEditorSync(editorInstance);

        editorReady = true;
        (window as any).__editor = editorInstance;
        console.log('[Editor] Monaco editor ready');
      })();
    }

    await editorInitPromise;
  }

  editorToggleBtn.addEventListener('click', async () => {
    await ensureEditorReady();
    toggleEditorPanel();
    // Trigger canvas resize + refit board to new dimensions
    resize();
    if (snapshot?.board) {
      viewport = fitBoard(viewport, snapshot.board.width_nm, snapshot.board.height_nm);
    }
    // Sync viewport to interaction state (critical for click→world coord mapping)
    interactionState.viewport = viewport;
    if (is3DActive && renderer3d) {
      renderer3d.resize();
    }
    dirty = true;
  });

  // Apply URL state if present (shared URL)
  const urlState = decodeViewState();
  let hasUrlState = urlState !== null;
  if (urlState) {
    // Apply viewport state from URL
    viewport = {
      ...viewport,
      centerX: urlState.panX,
      centerY: urlState.panY,
      scale: urlState.zoom,
    };

    // Apply layer visibility from URL
    const layersFromUrl = urlState.layers;
    topLayerCb.checked = layersFromUrl.includes('top');
    bottomLayerCb.checked = layersFromUrl.includes('bottom');
    ratsnestCb.checked = layersFromUrl.includes('ratsnest');
    if (innerLayerCb) {
      innerLayerCb.checked = innerVisibleFromUrlLayers(layersFromUrl);
    }
    layers = {
      topCopper: topLayerCb.checked,
      bottomCopper: bottomLayerCb.checked,
      innerCopper: innerLayerCb ? innerLayerCb.checked : true,
    };
    showRatsnest = ratsnestCb.checked;

    console.log('[URL State] Applied shared view state:', urlState);
  }

  // Start with empty state - user will open a file
  pullSnapshot();
  currentFilePath = null;
  statusText.textContent = usingWasm ? 'Ready (WASM) - Open a file' : 'Ready (Mock) - Open a file';

  // Preload Monaco editor in the background so Ctrl+E opens instantly
  const preloadEditor = () => {
    if (!editorReady) {
      // initEditor() only builds the editor; it never changes visibility, so the
      // preload has nothing to undo. The previous guard tested for a `hidden`
      // class the container never carries, so it toggled the panel open a few
      // seconds after every page load.
      ensureEditorReady().then(() => {
        console.log('[Editor] Preloaded in background');
      });
    }
  };
  if (typeof requestIdleCallback === 'function') {
    requestIdleCallback(preloadEditor, { timeout: 3000 });
  } else {
    setTimeout(preloadEditor, 1500);
  }

  /**
   * Build a partial RenderState for thumbnail generation from current config.
   */
  function buildRenderStateForThumbnail(): Partial<import('./renderer').RenderState> {
    return { renderConfig };
  }

  /**
   * Files the design imports, under the paths the engine asks for.
   *
   * The engine resolves `import "lib/blocks.cypcb"` and cannot read a file, so
   * the host fetches and hands it these. Empty for a design that imports
   * nothing, which is every board this viewer had until now.
   */
  const importedFiles: Record<string, string> = {};

  /**
   * Where imported files come from, or null when the design has no home.
   *
   * A template is served over HTTP, so its library is a fetch away. A file the
   * user opened through the browser's file picker has no directory to fetch
   * from, and the honest answer there is the error the engine already gives:
   * it names the path it wanted and what the host supplied.
   */
  let importReader: ((path: string) => Promise<string | null>) | null = null;

  /**
   * Load a design, with whatever of its library is already in hand.
   *
   * Every path into the engine goes through here, so a design that imports
   * gets its blocks whether it arrived from a template, the editor, a reload
   * or the undo stack.
   */
  function loadDesign(source: string): string {
    const errors = engine.load_source_with_imports(source, importedFiles);
    void followImports(source);
    return errors;
  }

  /**
   * Fetch what the design imports and, if that is new, load it again.
   *
   * Asynchronous because fetching is: the first load draws what it can - a
   * board with its own parts, and an error for each import - and this replaces
   * it once the library arrives. Same shape as `reloadAfterLcscFetch` below,
   * which does this for footprints fetched from LCSC.
   */
  async function followImports(source: string): Promise<void> {
    const reader = importReader;
    if (!reader || importedPaths(source).length === 0) return;

    const fetched = await collectImportedFiles(source, async (path) => {
      const held = importedFiles[path];
      return held !== undefined ? held : reader(path);
    });

    let changed = false;
    for (const [path, text] of Object.entries(fetched)) {
      if (importedFiles[path] !== text) {
        importedFiles[path] = text;
        changed = true;
      }
    }
    if (!changed) return;

    const errors = engine.load_source_with_imports(source, importedFiles);
    if (errors) console.warn('[Imports] After fetching the library:', errors);

    const snap = pullSnapshot();
    if (snap.board) {
      viewport = fitBoard(viewport, snap.board.width_nm, snap.board.height_nm);
      interactionState.viewport = viewport;
    }
    if (snap.violations) updateErrorBadge(snap.violations);
    if (is3DActive && renderer3d) renderer3d.updateBoard(snap, layers);
    forceRender2D();
  }

  /**
   * Re-parse + re-render after LCSC footprint fetch, updating the thumbnail.
   */
  function reloadAfterLcscFetch(source: string): void {
    console.log('[LCSC] reloadAfterLcscFetch — re-parsing source with registered footprints');
    loadDesign(source);
    const updatedSnap = pullSnapshot();
    console.log('[LCSC] After re-parse: components =', updatedSnap.components?.length, 'pads on first =', updatedSnap.components?.[0]?.pads?.length);
    forceRender2D();
    // Re-generate thumbnail now that footprints are loaded
    if (currentFilePath) {
      addRecentFile(currentFilePath, updatedSnap, buildRenderStateForThumbnail(), source);
    }
    if (is3DActive && renderer3d && snapshot) {
      renderer3d.updateBoard(snapshot, layers);
    }
  }

  // WS connection reference — assigned later, used by project manager callbacks
  let wsConnection: WsConnection | null = null;

  // --- Project Manager ---
  initProjectManager({
    onOpenFile: () => {
      handleWebFileOpen();
    },
    onLoadTemplate: (source, templateName) => {
      // Clear undo stack
      undoStack.clear();

      // A template is served from `/templates/`, so anything it imports is a
      // fetch away beside it.
      importReader = readerForBaseUrl('/templates/');

      const errors = loadDesign(source);
      if (errors) console.warn('[Template] Parse warnings:', errors);

      lastLoadedSource = source;
      const snap = pullSnapshot();

      // Update editor if initialized
      if (editorReady && editorInstance) {
        suppressSync = true;
        editorInstance.setValue(source);
        suppressSync = false;
        const monaco = getMonacoModule();
        if (monaco) updateDiagnostics(monaco, editorInstance, readDiagnostics(engine), snap.violations || []);
      }

      if (snap.board) {
        viewport = fitBoard(viewport, snap.board.width_nm, snap.board.height_nm);
        interactionState.viewport = viewport;
      }
      if (snap.violations) updateErrorBadge(snap.violations);
      if (is3DActive && renderer3d) renderer3d.updateBoard(snap, layers);

      currentFilePath = `${templateName}.cypcb`;
      statusText.textContent = `Loaded template: ${templateName}`;

      addRecentFile(currentFilePath, snap, buildRenderStateForThumbnail(), source);
      hideProjectManager();
      dirty = true;

      // Auto-fetch LCSC footprints (async — re-parses after fetch)
      autoFetchLcscFootprints(source).then((fetched) => {
        if (fetched) reloadAfterLcscFetch(source);
      });
    },
    onLoadRecent: (source, name) => {
      undoStack.clear();

      // A recent file is the design's own text out of localStorage; where it
      // came from is not stored, so there is nowhere to fetch its library
      // from. The engine says which import it could not follow.
      importReader = null;

      console.log(`[TracePersist] onLoadRecent: name=${name}, source length=${source?.length}, has trace blocks=${source?.includes('trace ') && source?.includes('path ')}`);

      const errors = loadDesign(source);
      if (errors) console.warn('[Recent] Parse warnings:', errors);

      lastLoadedSource = source;
      const snap = pullSnapshot();

      if (editorReady && editorInstance) {
        suppressSync = true;
        editorInstance.setValue(source);
        suppressSync = false;
        const monaco = getMonacoModule();
        if (monaco) updateDiagnostics(monaco, editorInstance, readDiagnostics(engine), snap.violations || []);
      }

      if (snap.board) {
        viewport = fitBoard(viewport, snap.board.width_nm, snap.board.height_nm);
        interactionState.viewport = viewport;
      }
      if (snap.violations) updateErrorBadge(snap.violations);
      if (is3DActive && renderer3d) renderer3d.updateBoard(snap, layers);

      currentFilePath = name;
      statusText.textContent = `Loaded: ${name}`;

      hideProjectManager();
      dirty = true;

      // Auto-fetch LCSC footprints (async — re-parses after fetch)
      autoFetchLcscFootprints(source).then((fetched) => {
        if (fetched) reloadAfterLcscFetch(source);
      });
    },
    onNewBlank: (source) => {
      undoStack.clear();
      loadDesign(source);
      lastLoadedSource = source;
      const snap = pullSnapshot();

      if (editorReady && editorInstance) {
        suppressSync = true;
        editorInstance.setValue(source);
        suppressSync = false;
        const monaco = getMonacoModule();
        if (monaco) updateDiagnostics(monaco, editorInstance, readDiagnostics(engine), snap.violations || []);
      }

      if (snap.board) {
        viewport = fitBoard(viewport, snap.board.width_nm, snap.board.height_nm);
        interactionState.viewport = viewport;
      }
      if (is3DActive && renderer3d) renderer3d.updateBoard(snap, layers);

      currentFilePath = null;
      statusText.textContent = usingWasm ? 'Ready (WASM)' : 'Ready (Mock)';
      hideProjectManager();
      dirty = true;
    },
    onRequestFileList: () => {
      wsConnection?.send({ type: 'list-files' });
    },
    onRefreshThumbnail: () => {
      // Regenerate thumbnail and persist trace data into recent file source
      if (currentFilePath && snapshot) {
        // Merge current traces into source so reopening preserves them
        const currentSource = (editorReady && editorInstance) ? editorInstance.getValue() : lastLoadedSource;
        // Only a `.cypcb` gets trace blocks merged into it. Splicing DSL into
        // a `(kicad_pcb ...)` file would store something neither reader can
        // open, under the name of a board the user still has.
        if (currentSource && loadedKind === 'cypcb') {
          const exportedTraces = engine.export_traces_as_dsl();
          const mergedSource = mergeTracesIntoDsl(currentSource, exportedTraces);
          lastLoadedSource = mergedSource;
        }
        addRecentFile(currentFilePath, snapshot, buildRenderStateForThumbnail(), lastLoadedSource);
      }
    },
    onOpenProjectFile: (_path, _name) => {
      // Request the file from the WS server — it will come as a reload
      wsConnection?.send({ type: 'open-file', file: _path });
    },
  });

  // Show project manager on startup
  showProjectManager();

  // --- JLCPCB Search Panel ---
  const jlcpcbSearchBtn = document.getElementById('jlcpcb-search-btn') as HTMLButtonElement;
  initSearchPanel({
    onComponentSelect: async (component) => {
      const lcscStr = `C${component.lcsc}`;
      const pkg = component.package || 'unknown';
      console.log(`[JLCPCB] Selected: ${lcscStr} (${component.mfr})`);

      // Pre-fetch footprint data (pads + 3D UUID) for this component.
      // This populates the dynamic registry so subsequent insert or re-parse
      // will use real pad geometry instead of hardcoded templates.
      try {
        const footprint = await fetchComponentFootprint(component.lcsc);
        if (footprint) {
          registerDynamicFootprint(pkg, footprint.pads, footprint.silk);
          if (footprint.modelUuid) {
            register3DModel(pkg, footprint.modelUuid);
          }
        }
      } catch (e) {
        console.warn(`[JLCPCB] Footprint pre-fetch failed for ${lcscStr}:`, e);
      }

      if (is3DActive && renderer3d) {
        console.log(`[JLCPCB] Fetching 3D model for ${lcscStr}...`);
        const objText = await fetch3DModel(component.lcsc);
        if (objText) {
          renderer3d.loadComponentFromOBJ(objText, lcscStr);
          console.log(`[3D] OBJ loaded for ${lcscStr}`);
        } else {
          console.log(`[JLCPCB] No 3D model available for ${lcscStr}`);
        }
      }
    },
    onInsertToEditor: (component) => {
      insertComponentSnippet(component);
    },
  });

  jlcpcbSearchBtn.addEventListener('click', () => {
    // Close project manager if open to avoid stacking
    if (!document.getElementById('project-manager')?.classList.contains('hidden')) {
      hideProjectManager();
    }
    toggleSearchPanel();
  });

  // --- Variant Panel ---
  initVariantPanel({
    onHover: (index) => {
      if (index != null && storedVariants[index]) {
        variantPreview = {
          routes: storedVariants[index].routes,
          vias: storedVariants[index].vias,
        };
      } else {
        variantPreview = null;
      }
      dirty = true;
    },
    onClick: (index) => {
      if (!storedVariants[index]) return;
      // Apply the clicked variant by re-routing with that variant's config
      // For now, variant click just makes it the active display — the routes
      // from auto_route_variants() already applied the best one.
      // To truly apply, we would need a per-variant apply API.
      // Mark it as active and clear preview
      variantPreview = null;
      dirty = true;
      console.log(`[Variants] Applied variant: ${storedVariants[index].name}`);
    },
  });

  /**
   * Find the best insertion line for a new component with the given refdes prefix.
   * Groups components by type: a new resistor (R) goes after the last R block,
   * a new capacitor (C) after the last C block, etc.
   * Falls back to: last component of same prefix → last component overall → before nets → EOF.
   */
  function findComponentInsertLine(model: any, refPrefix: string): number {
    const lineCount = model.getLineCount();
    const upperPrefix = refPrefix.toUpperCase();

    // Collect end-line of each component block by tracking brace depth
    const blocks: { prefix: string; endLine: number }[] = [];
    let firstNetLine = 0;
    let depth = 0;
    let currentPrefix = '';
    let inBlock = false;

    for (let i = 1; i <= lineCount; i++) {
      const line = model.getLineContent(i).trim();

      // Detect component start only at top level
      if (!inBlock && depth === 0 && /^component\s+/.test(line)) {
        inBlock = true;
        const m = line.match(/^component\s+([A-Za-z]+)\d/);
        currentPrefix = m ? m[1].toUpperCase() : '';
      }

      // Count braces
      for (const ch of line) {
        if (ch === '{') depth++;
        else if (ch === '}') depth--;
      }
      if (depth < 0) depth = 0; // safety clamp

      // Component block closed when depth returns to 0
      if (inBlock && depth === 0) {
        blocks.push({ prefix: currentPrefix, endLine: i });
        inBlock = false;
      }

      if (!firstNetLine && /^net\s+/.test(line)) {
        firstNetLine = i;
      }
    }

    // Find last block with matching prefix
    let lastSame = 0;
    let lastAny = 0;
    for (const b of blocks) {
      lastAny = b.endLine;
      if (b.prefix === upperPrefix) lastSame = b.endLine;
    }

    if (lastSame > 0) return lastSame;
    if (lastAny > 0) return lastAny;
    if (firstNetLine > 0) return firstNetLine - 1;
    return lineCount;
  }

  // --- Insert component snippet into editor ---
  async function insertComponentSnippet(component: import('./jlcpcb').JLCPCBComponent): Promise<void> {
    // Pre-fetch footprint from EasyEDA before inserting the snippet.
    // This populates the dynamic footprint registry so the parser can
    // generate real pads when the editor re-parses the source.
    const pkg = component.package || 'unknown';
    try {
      const footprint = await fetchComponentFootprint(component.lcsc);
      if (footprint) {
        registerDynamicFootprint(pkg, footprint.pads, footprint.silk);
        if (footprint.modelUuid) {
          register3DModel(pkg, footprint.modelUuid);
        }
      }
    } catch (e) {
      console.warn(`[JLCPCB] Footprint pre-fetch failed for C${component.lcsc}:`, e);
      // Continue with insert — will fall back to hardcoded pads or empty
    }

    // Collect existing refdes from current board snapshot
    const existingRefDes = snapshot?.components?.map((c) => c.refdes) ?? [];
    const snippet = buildComponentSnippet(component, existingRefDes);

    // Auto-open editor if not ready
    await ensureEditorReady();
    if (!isEditorVisible()) {
      toggleEditorPanel();
      resize();
      if (snapshot?.board) {
        viewport = fitBoard(viewport, snapshot.board.width_nm, snapshot.board.height_nm);
      }
      dirty = true;
    }

    if (!editorInstance) {
      console.warn('[JLCPCB] Editor failed to initialize');
      return;
    }

    const model = editorInstance.getModel();
    if (!model) return;

    // Extract refdes prefix from snippet (e.g. "component R3 ..." → "R")
    const prefixMatch = snippet.match(/component\s+([A-Za-z]+)\d/);
    const refPrefix = prefixMatch ? prefixMatch[1] : '';

    // Find the best insertion point: after the last component of same type,
    // grouping R with R, C with C, etc.
    const insertLine = findComponentInsertLine(model, refPrefix);
    const textToInsert = '\n' + snippet + '\n';

    editorInstance.executeEdits('jlcpcb-insert', [{
      range: {
        startLineNumber: insertLine,
        startColumn: model.getLineMaxColumn(insertLine),
        endLineNumber: insertLine,
        endColumn: model.getLineMaxColumn(insertLine),
      },
      text: textToInsert,
    }]);

    // Scroll to the inserted component (insertLine + 1 is where the new block starts)
    editorInstance.revealLineInCenter(insertLine + 2);
    console.log(`[JLCPCB] Inserted snippet for C${component.lcsc}`);
  }

  // --- Drag & drop onto editor container ---
  const editorDropTarget = document.getElementById('editor-container');
  if (editorDropTarget) {
    editorDropTarget.addEventListener('dragover', (e) => {
      if (e.dataTransfer?.types.includes('application/x-cypcb-component')) {
        e.preventDefault();
        e.dataTransfer.dropEffect = 'copy';
        editorDropTarget.classList.add('drop-hover');
      }
    });

    editorDropTarget.addEventListener('dragleave', () => {
      editorDropTarget.classList.remove('drop-hover');
    });

    editorDropTarget.addEventListener('drop', async (e) => {
      e.preventDefault();
      editorDropTarget.classList.remove('drop-hover');

      const cypcbData = e.dataTransfer?.getData('application/x-cypcb-component');
      if (!cypcbData) return;

      try {
        const comp = JSON.parse(cypcbData) as import('./jlcpcb').JLCPCBComponent;
        if (comp.lcsc) {
          await insertComponentSnippet(comp);
        }
      } catch {
        console.warn('[JLCPCB] Invalid drop data');
      }
    });
  }

  // Expose board loader for E2E tests — loads source, pulls snapshot, fits board
  // `kind` lets a test drive the KiCad path, which the file input reaches by
  // extension and no test can otherwise get at.
  (window as any).__loadBoard = (source: string, kind?: string) => {
    loadedKind = kind === 'kicad_pcb' ? 'kicad_pcb' : 'cypcb';
    if (kind === 'kicad_pcb') {
      engine.load_kicad(source);
    } else {
      loadDesign(source);
    }
    const snap = pullSnapshot();
    if (snap.board) {
      viewport = fitBoard(viewport, snap.board.width_nm, snap.board.height_nm);
    }
    // Sync viewport + snapshot to interaction state so click handlers use correct coords
    interactionState.viewport = viewport;
    interactionState.snapshot = snapshot;
    interactionState.padNetMap = padNetMap;
    hideProjectManager();
    dirty = true;
    statusText.textContent = usingWasm ? 'Ready (WASM)' : 'Ready (Mock)';
  };

  // Expose debug routing for console: __debugRoute()
  (window as any).__debugRoute = () => triggerDebugRouting();

  /**
   * Refresh the board snapshot from the engine and sync to interaction state.
   * Used as callback by undo commands after mutations.
   */
  function refreshSnapshot(): void {
    pullSnapshot();
    if (interactionState) {
      interactionState.snapshot = snapshot;
      interactionState.padNetMap = padNetMap;
    }
    if (snapshot?.violations) {
      updateErrorBadge(snapshot.violations);
    }
    // Update 3D view if active
    if (is3DActive && renderer3d && snapshot) {
      renderer3d.updateBoard(snapshot, layers);
    }
    dirty = true;
  }

  // Interaction setup (must be defined before handleFileLoad which uses it)
  const interactionState: InteractionState = {
    viewport,
    isPanning: false,
    lastX: 0,
    lastY: 0,
    dirty: false,
    snapshot,
    selectedTraceId: null,
    hoveredTraceId: null,
    onSelect: (x_nm, y_nm) => {
      // Query engine for component at point
      const hits = engine.query_point(Math.round(x_nm), Math.round(y_nm));
      if (hits && hits.length > 0) {
        selectedRefdes = hits[0];
        console.log('Selected:', selectedRefdes);
        // Show selected in status
        const comp = snapshot?.components.find(c => c.refdes === selectedRefdes);
        if (comp) {
          statusText.textContent = `Selected: ${comp.refdes} (${comp.value})`;
        }
      } else {
        selectedRefdes = null;
        statusText.textContent = usingWasm ? 'Ready (WASM)' : 'Ready (Mock)';
      }
      dirty = true;
    },
    onViewportChange: (vp) => {
      viewport = vp;
      interactionState.viewport = vp;
    },
    onTraceSelect: (traceId, screenX, screenY) => {
      selectedTraceId = traceId;
      if (traceId != null) {
        // Blur Monaco editor so Delete/Backspace goes to trace handler, not editor
        if (document.activeElement && (document.activeElement as HTMLElement).closest?.('.monaco-editor')) {
          (document.activeElement as HTMLElement).blur();
        }
        // Convert client coords to canvas-relative for label positioning
        const rect = canvas.getBoundingClientRect();
        labelPosition = { x: screenX - rect.left, y: screenY - rect.top };
        // Show trace info in status bar and highlight net
        const trace = snapshot?.traces?.find(t => t.id === traceId);
        if (trace) {
          const widthMm = (trace.width / 1_000_000).toFixed(2);
          statusText.textContent = `Trace: ${trace.net_name} (${widthMm}mm, ${trace.layer})`;
          // Highlight entire net
          if (trace.net_name && trace.net_name !== highlightedNet) {
            highlightedNet = trace.net_name;
            console.log(`[Net] Highlighted: ${highlightedNet}`);
          }
        }
      } else {
        labelPosition = null;
        selectedRefdes = null;
        // Clear net highlighting
        if (highlightedNet != null) {
          console.log('[Net] Cleared');
          highlightedNet = null;
        }
        statusText.textContent = usingWasm ? 'Ready (WASM)' : 'Ready (Mock)';
      }
      dirty = true;
    },
    onTraceHover: (traceId) => {
      hoveredTraceId = traceId;
      dirty = true;
    },
    routing: routingState,
    engine,
    onRoutingChange: (newRouting: RoutingState) => {
      routingState = newRouting;
      interactionState.routing = newRouting;
      // Update snapshot after mutations (route complete changes trace list)
      if (newRouting.mode === 'idle' && snapshot) {
        pullSnapshot();
        interactionState.snapshot = snapshot;
    interactionState.padNetMap = padNetMap;
      }
      dirty = true;
    },
    onTraceAdd: (netName: string, layer: string, width: number, segments: number[]) => {
      const cmd = new AddTraceCommand(engine, { netName, layer, width, segments }, refreshSnapshot);
      undoStack.push(cmd);
      dirty = true;
      markTracesUnsaved();
      syncEditorTraces();
    },
    onBoardResize: (oldW: number, oldH: number, newW: number, newH: number) => {
      const cmd = new ResizeBoardCommand(engine, oldW, oldH, newW, newH, refreshSnapshot);
      undoStack.push(cmd);
      const wMm = (newW / 1e6).toFixed(1);
      const hMm = (newH / 1e6).toFixed(1);
      console.log(`[Resize] Board → ${wMm}×${hMm}mm`);
      statusText.textContent = `Board resized to ${wMm} × ${hMm} mm`;
      dirty = true;
    },
    onRouteStart: (netName: string) => {
      highlightedNet = netName;
      console.log(`[Net] Highlighted: ${netName}`);
      dirty = true;
    },
    onRouteEnd: () => {
      highlightedNet = null;
      console.log('[Net] Cleared (route end)');
      dirty = true;
    },
    onTraceEdit: (oldTraceId: number, netName: string, layer: string, width: number, oldSegments: number[], newSegments: number[]) => {
      const cmd = new EditTraceCommand(
        engine,
        oldTraceId,
        { netName, layer, width, segments: oldSegments },
        { netName, layer, width, segments: newSegments },
        refreshSnapshot,
      );
      undoStack.push(cmd);
      dirty = true;
      markTracesUnsaved();
      syncEditorTraces();
    },
    onRectSelect: (traceIds: number[], componentRefdes: string[]) => {
      // For now, select the first trace or component in the rectangle
      if (traceIds.length > 0) {
        selectedTraceId = traceIds[0];
        interactionState.selectedTraceId = traceIds[0];
        const trace = snapshot?.traces?.find(t => t.id === traceIds[0]);
        if (trace) {
          highlightedNet = trace.net_name || null;
          statusText.textContent = `Selected ${traceIds.length} trace(s)${componentRefdes.length > 0 ? `, ${componentRefdes.length} component(s)` : ''}`;
        }
      } else if (componentRefdes.length > 0) {
        selectedRefdes = componentRefdes[0];
        statusText.textContent = `Selected ${componentRefdes.length} component(s)`;
      }
      dirty = true;
    },
    onTraceOptimize: async (traceId: number) => {
      const trace = snapshot?.traces?.find(t => t.id === traceId);
      if (!trace || !trace.segments.length) return;

      const { optimizeTrace } = await import('./trace-optimize');
      const optimized = optimizeTrace(trace.segments, snapshot, trace.net_name || '', 150_000, Number(trace.width), padNetMap);

      if (optimized.length < trace.segments.length) {
        // Segments were reduced
        const oldFlat: number[] = [];
        for (const s of trace.segments) oldFlat.push(Math.round(Number(s.start_x)), Math.round(Number(s.start_y)), Math.round(Number(s.end_x)), Math.round(Number(s.end_y)));
        const newFlat: number[] = [];
        for (const s of optimized) newFlat.push(Math.round(s.start_x), Math.round(s.start_y), Math.round(s.end_x), Math.round(s.end_y));

        const cmd = new EditTraceCommand(engine, traceId,
          { netName: trace.net_name || '', layer: trace.layer || 'Top', width: Number(trace.width), segments: oldFlat },
          { netName: trace.net_name || '', layer: trace.layer || 'Top', width: Number(trace.width), segments: newFlat },
          refreshSnapshot);
        undoStack.push(cmd);
        markTracesUnsaved();
        syncEditorTraces();
        statusText.textContent = `Optimized: ${trace.segments.length} → ${optimized.length} segments`;
      } else {
        statusText.textContent = `Already optimal (${trace.segments.length} segments)`;
        console.log('[Optimize] No reduction found. Try simplify-only.');
      }
      dirty = true;
    },
    dragEdit: null,
    rectSelect: null,
  };

  setupInteraction(canvas, interactionState);
  interactionReady = true;

  // Expose debug render state for programmatic inspection
  // Which reader the open board came from. Exposed because the editor's
  // debounce picks its reader from it, and a test cannot see that choice from
  // outside any other way.
  (window as any).__loadedKind = () => loadedKind;

  (window as any).__renderTiming = {
    /** Milliseconds each of the last frames spent inside `render()`. */
    samples: () => [...frameTimes],
    reset: () => {
      frameTimes.length = 0;
    },
  };

  (window as any).__renderState = {
    get selectedTraceId() { return selectedTraceId; },
    get hoveredTraceId() { return hoveredTraceId; },
    get colorByNet() { return colorByNet; },
  };

  // Expose routing state debug surface
  (window as any).__routingState = {
    get mode() { return routingState.mode; },
    get anchorPoint() { return routingState.anchorPoint; },
    get snapAngle() { return routingState.snapAngle; },
    get netName() { return routingState.netName; },
    get currentLayer() { return routingState.currentLayer; },
    get committedSegments() { return routingState.committedSegments.length; },
    get drcViolationCount() { return routingState.drcViolations.length; },
    get previewSegment() { return routingState.previewSegment; },
    get angleSnapEnabled() { return routingState.angleSnapEnabled; },
    get magneticSnapEnabled() { return routingState.magneticSnapEnabled; },
    get snappedToPad() { return routingState.snappedToPad; },
    get targetPadsCount() { return routingState.targetPads.length; },
  };

  // Expose viewport for E2E tests — live getter reads current viewport state
  (window as any).__viewport = {
    get centerX() { return viewport.centerX; },
    get centerY() { return viewport.centerY; },
    get scale() { return viewport.scale; },
    get width() { return viewport.width; },
    get height() { return viewport.height; },
  };

  /**
   * Handle loading a file (.cypcb or .ses) from file picker or drag-drop
   */
  async function handleFileLoad(file: File): Promise<void> {
    // Clear undo stack on new file load
    undoStack.clear();

    const ext = file.name.toLowerCase().split('.').pop();

    try {
      const content = await readFileAsText(file);

      if (ext === 'cypcb' || ext === 'kicad_pcb') {
        // A KiCad board goes to the importer and everything below is the same.
        // The command line learned to read, check, route and write these; the
        // viewer could open the project's own format and nothing else, so
        // somebody with a `.kicad_pcb` had no way to look at it here.
        loadedKind = ext === 'kicad_pcb' ? 'kicad_pcb' : 'cypcb';
        const errors = loadedKind === 'kicad_pcb'
          ? engine.load_kicad(content)
          : loadDesign(content);
        if (errors) {
          console.warn('Parse errors:', errors);
        }

        // Track loaded source for save operations
        lastLoadedSource = content;

        // Get new snapshot and fit board
        const snap = pullSnapshot();

        // Update editor content if initialized
        if (editorReady && editorInstance) {
          suppressSync = true;
          editorInstance.setValue(content);
          suppressSync = false;

          // Update inline diagnostics
          const monaco = getMonacoModule();
          if (monaco) {
            updateDiagnostics(monaco, editorInstance, readDiagnostics(engine), snap.violations || []);
          }
        }

        // Update current file path for routing
        currentFilePath = file.name;

        if (snap.board) {
          viewport = fitBoard(viewport, snap.board.width_nm, snap.board.height_nm);
          interactionState.viewport = viewport;
        }

        // Update error badge
        if (snap.violations) {
          updateErrorBadge(snap.violations);
        }

        // Show status
        const errorCount = errors ? errors.split('\n').filter(Boolean).length : 0;
        statusText.textContent = errorCount > 0
          ? `Loaded ${file.name} (${errorCount} warnings)`
          : `Loaded ${file.name}`;

        hideProjectManager();
        addRecentFile(file.name, snap, buildRenderStateForThumbnail(), content);
        dirty = true;

      } else if (ext === 'ses') {
        // Check if board is loaded
        if (!snapshot?.board) {
          statusText.textContent = 'Load a .cypcb file first';
          return;
        }

        // Load routes
        engine.load_routes(content);
        pullSnapshot();
        markTracesUnsaved();
        syncEditorTraces();

        statusText.textContent = `Loaded routes from ${file.name}`;
        dirty = true;

      } else {
        statusText.textContent = `Unknown file type: .${ext}`;
      }
    } catch (err) {
      console.error('File load error:', err);
      statusText.textContent = `Error loading ${file.name}`;
    }
  }

  // File picker setup (kept for drag-drop only)
  const filePicker = createFilePicker('.cypcb,.ses', handleFileLoad);

  // Open button - show project manager (which has file picker + templates + recent)
  openBtn.addEventListener('click', async () => {
    if (isDesktop()) {
      // Desktop uses its own file dialog via Tauri IPC
      filePicker.click();
      return;
    }

    // Persist current traces into source before showing project manager
    // so reopening the same project preserves routed traces
    const traceCount = engine.trace_count();
    if (lastLoadedSource && traceCount > 0) {
      const exportedTraces = engine.export_traces_as_dsl();
      lastLoadedSource = mergeTracesIntoDsl(lastLoadedSource, exportedTraces);
      if (currentFilePath) {
        addRecentFile(currentFilePath, snapshot, buildRenderStateForThumbnail(), lastLoadedSource);
      }
    }

    showProjectManager();
  });

  // File picker handler for File System Access API (called from PM's Open File button)
  async function handleWebFileOpen(): Promise<void> {
    const result = await openFile();
    if (!result) return;

    // Clear undo stack on new file load
    undoStack.clear();
    // Store handle for save-in-place
    currentFileHandle = result.handle;
    currentFilePath = result.name;

    const ext = result.name.toLowerCase().split('.').pop();

    if (ext === 'cypcb') {
      const errors = loadDesign(result.content);
      if (errors) console.warn('Parse errors:', errors);

      lastLoadedSource = result.content;
      const snap2 = pullSnapshot();

      if (editorReady && editorInstance) {
        suppressSync = true;
        editorInstance.setValue(result.content);
        suppressSync = false;
        const monaco = getMonacoModule();
        if (monaco) updateDiagnostics(monaco, editorInstance, readDiagnostics(engine), snap2.violations || []);
      }

      if (snap2.board) {
        viewport = fitBoard(viewport, snap2.board.width_nm, snap2.board.height_nm);
        interactionState.viewport = viewport;
      }
      if (snap2.violations) updateErrorBadge(snap2.violations);

      const errorCount = errors ? errors.split('\n').filter(Boolean).length : 0;
      statusText.textContent = errorCount > 0
        ? `Loaded ${result.name} (${errorCount} warnings)`
        : `Loaded ${result.name}`;

      hideProjectManager();
      addRecentFile(result.name, snap2, buildRenderStateForThumbnail(), result.content);
      dirty = true;

    } else if (ext === 'ses') {
      if (!snapshot?.board) {
        statusText.textContent = 'Load a .cypcb file first';
        return;
      }
      engine.load_routes(result.content);
      pullSnapshot();
      markTracesUnsaved();
      syncEditorTraces();
      statusText.textContent = `Loaded routes from ${result.name}`;
      dirty = true;

    } else {
      statusText.textContent = `Unknown file type: .${ext}`;
    }
  }

  // Drag-drop setup
  setupDropZone(container, handleFileLoad);

  /**
   * Populate the error list with current violations
   */
  function populateErrorList(): void {
    // Clear existing content safely
    errorList.textContent = '';

    if (!snapshot?.violations || snapshot.violations.length === 0) {
      const noErrors = document.createElement('div');
      noErrors.className = 'error-item';
      noErrors.innerHTML = '<span class="error-icon">✅</span><span class="error-body"><span class="error-title">No DRC errors</span></span>';
      errorList.appendChild(noErrors);
      return;
    }

    snapshot.violations.forEach((v) => {
      const meta = describeViolationKind(v.kind);

      // Parse detail from the raw message
      const detail = formatViolationDetail(v);
      const locationMm = `(${(v.x_nm / 1e6).toFixed(2)}, ${(v.y_nm / 1e6).toFixed(2)}) mm`;

      const item = document.createElement('div');
      item.className = 'error-item';

      const icon = document.createElement('span');
      icon.className = 'error-icon';
      icon.textContent = meta.icon;

      const body = document.createElement('div');
      body.className = 'error-body';

      const title = document.createElement('div');
      title.className = 'error-title';
      title.textContent = meta.label;

      const detailParts = detail.split('\n');

      const detailEl = document.createElement('div');
      detailEl.className = 'error-detail';
      detailEl.textContent = detailParts[0];

      const loc = document.createElement('div');
      loc.className = 'error-location';
      loc.textContent = locationMm;

      body.appendChild(title);
      body.appendChild(detailEl);

      // Entity labels line (e.g. "trace 'VCC' ↔ pad on R1")
      if (detailParts.length > 1) {
        const entitiesEl = document.createElement('div');
        entitiesEl.className = 'error-entities';
        entitiesEl.textContent = detailParts[1];
        body.appendChild(entitiesEl);
      }

      body.appendChild(loc);
      item.appendChild(icon);
      item.appendChild(body);
      errorList.appendChild(item);

      item.addEventListener('click', () => {
        zoomToLocation(v.x_nm, v.y_nm);
      });
    });
  }

  /**
   * Format violation detail into a human-readable string.
   */
  function formatViolationDetail(v: ViolationInfo): string {
    const msg = v.message;

    // Extract entity labels if present (format: "label ↔ label: original message")
    let entities = '';
    let core = msg;
    const entityMatch = msg.match(/^(.+?): (.+)$/);
    if (entityMatch && entityMatch[1].includes('↔')) {
      entities = entityMatch[1];
      core = entityMatch[2];
    } else if (entityMatch && !entityMatch[2].includes('violation')) {
      // Single entity prefix without "violation" in rest — skip
    } else if (entityMatch) {
      entities = entityMatch[1];
      core = entityMatch[2];
    }

    // Format the core message
    let detail = '';

    // "Clearance violation: 0.00mm actual, 0.15mm required"
    const clearanceMatch = core.match(/([\d.]+)mm actual.*?([\d.]+)mm required/);
    if (clearanceMatch) {
      const actual = parseFloat(clearanceMatch[1]);
      const required = parseFloat(clearanceMatch[2]);
      if (actual === 0) {
        detail = `Items touching — need ${required}mm gap`;
      } else {
        detail = `${actual}mm gap — need at least ${required}mm`;
      }
    }

    // "Unconnected pin: R1.2"
    if (!detail) {
      const pinMatch = core.match(/Unconnected pin: (.+)/);
      if (pinMatch) detail = `${pinMatch[1]} has no net connection`;
    }

    // "Component R1 placed in keepout zone 'X'"
    if (!detail) {
      const keepoutMatch = core.match(/Component (\S+) placed in (.+)/);
      if (keepoutMatch) detail = `${keepoutMatch[1]} is inside ${keepoutMatch[2]}`;
    }

    // "Courtyard overlap: ..."
    if (!detail) {
      const courtyardMatch = core.match(/Courtyard overlap.*?([\d.]+)mm actual.*?([\d.]+)mm required/);
      if (courtyardMatch) detail = `Components too close — need ${courtyardMatch[2]}mm clearance`;
    }

    // "Via diameter violation: ..."
    if (!detail) {
      const viaMatch = core.match(/Via diameter.*?([\d.]+)mm actual.*?([\d.]+)mm required/);
      if (viaMatch) detail = `Via is ${viaMatch[1]}mm — minimum ${viaMatch[2]}mm`;
    }

    // "Hole-to-hole violation: ..."
    if (!detail) {
      const holeMatch = core.match(/Hole-to-hole.*?([\d.]+)mm actual.*?([\d.]+)mm required/);
      if (holeMatch) detail = `Holes ${holeMatch[1]}mm apart — need ${holeMatch[2]}mm`;
    }

    // "Drill size violation at R1.1: ..."
    if (!detail) {
      const drillMatch = core.match(/Drill size.*?at (\S+).*?([\d.]+)mm actual.*?([\d.]+)mm minimum/);
      if (drillMatch) detail = `${drillMatch[1]}: ${drillMatch[2]}mm drill — minimum ${drillMatch[3]}mm`;
    }

    // "Annular ring violation at R1.1: ..."
    if (!detail) {
      const ringMatch = core.match(/Annular ring.*?at (\S+).*?([\d.]+)mm actual.*?([\d.]+)mm required/);
      if (ringMatch) detail = `${ringMatch[1]}: ${ringMatch[2]}mm ring — minimum ${ringMatch[3]}mm`;
    }

    // "Edge clearance violation: ..."
    if (!detail) {
      const edgeMatch = core.match(/Edge clearance.*?([\d.]+)mm actual.*?([\d.]+)mm required/);
      if (edgeMatch) detail = `${edgeMatch[1]}mm from edge — need ${edgeMatch[2]}mm`;
    }

    // Fallback
    if (!detail) detail = core;

    // Append entity labels if present
    if (entities) {
      return `${detail}\n${entities}`;
    }
    return detail;
  }

  /**
   * Zoom viewport to center on a specific location
   */
  function zoomToLocation(x_nm: number, y_nm: number): void {
    // Zoom to fit a 10mm x 10mm area around the point
    const margin = 5_000_000; // 5mm in nm
    viewport = {
      ...viewport,
      centerX: x_nm,
      centerY: y_nm,
      scale: Math.min(viewport.width, viewport.height) / (margin * 2),
    };
    interactionState.viewport = viewport;
    dirty = true;
  }

  // Error badge click - toggle error panel
  errorBadge.addEventListener('click', () => {
    errorPanel.classList.toggle('hidden');
    if (!errorPanel.classList.contains('hidden')) {
      populateErrorList();
    }
  });

  // Close button for error panel
  errorPanelClose.addEventListener('click', () => {
    errorPanel.classList.add('hidden');
  });

  // Coordinate display on mouse move + update label position for selected trace
  canvas.addEventListener('mousemove', (e) => {
    const rect = canvas.getBoundingClientRect();
    const sx = e.clientX - rect.left;
    const sy = e.clientY - rect.top;
    const [worldX, worldY] = screenToWorld(viewport, sx, sy);
    // Format with user's preferred unit
    const unit = getPreference('units');
    coordsEl.textContent = `(${formatDimension(worldX, unit)}, ${formatDimension(worldY, unit)})`;

    // Track label position when a trace is selected
    if (selectedTraceId != null) {
      labelPosition = { x: sx, y: sy };
      dirty = true;
    }
  });

  canvas.addEventListener('mouseleave', () => {
    coordsEl.textContent = '';
  });

  // Visibility state
  const showViolations = true;

  /** Force an immediate 2D canvas re-render with current state. */
  function forceRender2D(): void {
    if (is3DActive) return;
    dirty = true;
    interactionState.dirty = true;
  }

  /**
   * How long the last frames took to draw, newest last.
   *
   * A renderer nobody can time is a renderer nobody can optimise: the frame
   * budget on a big board had never been measured, only assumed. Two
   * `performance.now()` calls on a frame that was going to redraw anyway, and
   * a hundred numbers kept, is a cost small enough to leave on.
   */
  const frameTimes: number[] = [];
  const FRAME_SAMPLES = 100;

  function recordFrame(ms: number): void {
    frameTimes.push(ms);
    if (frameTimes.length > FRAME_SAMPLES) {
      frameTimes.shift();
    }
  }

  // Render loop
  function frame(): void {
    // Skip 2D rendering when 3D mode is active
    if (!is3DActive && (dirty || interactionState.dirty)) {
      // Keep interaction state snapshot in sync
      interactionState.snapshot = snapshot;
    interactionState.padNetMap = padNetMap;

      const renderState: RenderState = {
        snapshot,
        viewport,
        layers,
        selectedRefdes,
        showViolations,
        showRatsnest,
        colorByNet,
        selectedTraceId,
        hoveredTraceId,
        labelPosition,
        routing: routingState.mode === 'routing' ? routingState : null,
        highlightedNet,
        activeResizeHandle: interactionState.activeResizeHandle ?? null,
        renderConfig,
        padNetMap,
        gridVisible,
        gridVisualSpacing: getPreference('gridVisualSpacing'),
        showNetLabels,
        variantPreview,
        dragEdit: interactionState.dragEdit,
        rectSelect: interactionState.rectSelect,
      };
      const renderStartedAt = performance.now();
      render(ctx, renderState);
      recordFrame(performance.now() - renderStartedAt);

      // Debug routing overlay — draw selected net or all nets
      if (debugOverlayStage !== -1 && debugData?._activeStage) {
        renderDebugStage(ctx, viewport, debugData._activeStage);
      }

      dirty = false;
      interactionState.dirty = false;
    }
    // Update undo/redo button states (cheap boolean checks, no DOM thrash when unchanged)
    undoBtn.disabled = !undoStack.canUndo;
    redoBtn.disabled = !undoStack.canRedo;
    requestAnimationFrame(frame);
  }
  /**
   * Render debug routing stage overlay on the canvas.
   * Draws segments as colored lines, vias as small circles.
   */
  function renderDebugStage(ctx: CanvasRenderingContext2D, vp: any, stage: any): void {
    const colors = ['#FF6B6B', '#4ECDC4', '#45B7D1', '#96CEB4', '#FFEAA7', '#DDA0DD', '#98D8C8', '#F7DC6F'];

    ctx.save();
    ctx.lineWidth = 2;
    ctx.globalAlpha = 0.85;

    for (const seg of stage.segments) {
      const sx = (seg.start_x - vp.centerX) * vp.scale + vp.width / 2;
      const sy = (seg.start_y - vp.centerY) * vp.scale + vp.height / 2;
      const ex = (seg.end_x - vp.centerX) * vp.scale + vp.width / 2;
      const ey = (seg.end_y - vp.centerY) * vp.scale + vp.height / 2;

      const colorIdx = seg.net_id % colors.length;
      ctx.strokeStyle = colors[colorIdx];
      ctx.lineWidth = Math.max(1, seg.width * vp.scale * 0.5);

      ctx.beginPath();
      ctx.moveTo(sx, sy);
      ctx.lineTo(ex, ey);
      ctx.stroke();
    }

    // Draw vias
    ctx.fillStyle = 'rgba(255, 200, 0, 0.8)';
    for (const via of stage.vias) {
      const vx = (via.x - vp.centerX) * vp.scale + vp.width / 2;
      const vy = (via.y - vp.centerY) * vp.scale + vp.height / 2;
      ctx.beginPath();
      ctx.arc(vx, vy, 4, 0, Math.PI * 2);
      ctx.fill();
    }

    ctx.restore();
  }

  frame();

  // Hot reload handler - preserves viewport and selection
  function reload(content: string, _file: string): void {
    // Clear undo stack — old trace IDs are invalid after reload
    undoStack.clear();

    // Save current state
    const savedViewport = { ...viewport };
    const savedSelection = selectedRefdes;

    // Parse new content
    const errors = loadDesign(content);
    if (errors) {
      console.warn('[HotReload] Parse warnings:', errors);
    }

    // Track loaded source for save operations
    lastLoadedSource = content;

    const reloadSnap = pullSnapshot();

    // Update editor content if initialized
    if (editorReady && editorInstance) {
      suppressSync = true;
      editorInstance.setValue(content);
      suppressSync = false;

      // Update inline diagnostics
      const monaco = getMonacoModule();
      if (monaco) {
        updateDiagnostics(monaco, editorInstance, readDiagnostics(engine), reloadSnap.violations || []);
      }
    }

    console.log('[HotReload] Reloaded snapshot:', reloadSnap);

    // Restore viewport — but fit board on first load (default viewport)
    const isDefaultViewport = savedViewport.centerX === 0 && savedViewport.centerY === 0;
    if (isDefaultViewport && reloadSnap.board) {
      viewport = fitBoard(savedViewport, reloadSnap.board.width_nm, reloadSnap.board.height_nm);
    } else {
      viewport = savedViewport;
    }
    interactionState.viewport = viewport;

    // Restore selection if component still exists
    if (savedSelection && reloadSnap.components.some(c => c.refdes === savedSelection)) {
      selectedRefdes = savedSelection;
    } else {
      selectedRefdes = null;
    }

    // Show "Reloaded" status briefly
    const parseErrorCount = errors ? errors.split('\n').filter(Boolean).length : 0;
    statusText.textContent = parseErrorCount > 0 ? `Reloaded (${parseErrorCount} warnings)` : 'Reloaded';

    // Update error badge with new violations
    if (reloadSnap.violations) {
      updateErrorBadge(reloadSnap.violations);
    }

    // After 1.5s, show normal status
    setTimeout(() => {
      if (selectedRefdes && snapshot) {
        const comp = snapshot.components.find(c => c.refdes === selectedRefdes);
        if (comp) {
          statusText.textContent = `Selected: ${comp.refdes} (${comp.value})`;
        }
      } else {
        statusText.textContent = usingWasm ? 'Ready (WASM)' : 'Ready (Mock)';
      }
    }, 1500);

    // Trigger re-render
    dirty = true;

    // Update 3D view if active
    if (is3DActive && renderer3d && snapshot) {
      renderer3d.updateBoard(snapshot, layers);
    }
  }

  // ========================================================================
  // Routing Integration
  // ========================================================================

  /**
   * Autorouter progress state for UI updates
   */
  interface AutorouteUiState {
    isRouting: boolean;
    pass: number;
    routed: number;
    unrouted: number;
    elapsed: number;
  }

  /**
   * Update UI to reflect autorouter state
   */
  function updateRoutingUI(state: AutorouteUiState): void {
    const routeLabel = routeBtn.querySelector('.tb-route-label');
    if (state.isRouting) {
      routeBtn.disabled = true;
      routeBtn.classList.add('routing');
      routeMenuBtn.classList.add('routing');
      if (routeLabel) routeLabel.textContent = 'Routing…';
      cancelRouteBtn.classList.remove('hidden');
      routingStatus.classList.remove('hidden');
      routingProgress.textContent = 'This may take a moment — the browser will be unresponsive while routing.';
    } else {
      routeBtn.disabled = false;
      routeBtn.classList.remove('routing');
      routeMenuBtn.classList.remove('routing');
      if (routeLabel) routeLabel.textContent = 'Route';
      cancelRouteBtn.classList.add('hidden');
      routingStatus.classList.add('hidden');
    }
  }

  // Hot-reload WS reference — kept for list-files requests from project manager.
  let routingStartTime = 0;

  /**
   * Trigger routing via WebSocket to dev server.
   * The server runs the CLI route command and streams progress.
   */
  async function triggerRouting(): Promise<void> {
    if (isRouting) {
      console.log('[Routing] Already routing');
      return;
    }

    if (!snapshot?.board) {
      console.log('[Routing] No board loaded');
      statusText.textContent = 'Load a board first';
      setTimeout(() => {
        statusText.textContent = usingWasm ? 'Ready (WASM)' : 'Ready (Mock)';
      }, 2000);
      return;
    }

    // Clear any existing variant panel on new Route click
    hideVariants();
    variantPreview = null;
    storedVariants = [];

    isRouting = true;
    routingStartTime = Date.now();
    statusText.textContent = 'Routing…';

    updateRoutingUI({
      isRouting: true,
      pass: 0,
      routed: 0,
      unrouted: 0,
      elapsed: 0,
    });

    // Yield to browser so it can paint the "Routing..." overlay before
    // the synchronous WASM call blocks the main thread.
    await new Promise(resolve => setTimeout(resolve, 50));

    // Route with current tuning parameters from sliders
    try {
      let resultJson: string;

      try {
        const params = getPreference('autorouteParams');
        const rustParams = {
          via_cost: params.viaCost,
          layer_preference: params.layerPreference,
          roundness: params.roundness,
          density: params.density,
        };
        resultJson = engine.auto_route_with_params(JSON.stringify(rustParams));
        (window as any).__lastRouteResult = resultJson;
      } catch (routeErr) {
        console.warn('[Routing] auto_route() failed:', routeErr);
        // Reload the board to reset WASM engine state after panic
        if (lastLoadedSource) {
          loadDesign(lastLoadedSource);
        }
        statusText.textContent = `Routing failed: ${routeErr}`;
        return;
      }

      const elapsed = Math.round((Date.now() - routingStartTime) / 1000);

      // Check if it's an error response
      let parsed: any;
      try {
        parsed = JSON.parse(resultJson);
      } catch {
        statusText.textContent = `Routing error: invalid JSON response`;
        console.error('[Routing] Invalid JSON:', resultJson);
        return;
      }

      // Error response: { ok: false, error: "..." }
      if (parsed && parsed.ok === false) {
        statusText.textContent = `Routing failed: ${parsed.error}`;
        console.error('[Routing]', parsed.error);
        return;
      }

      // Success: auto_route() returned {ok:true, routed:N, unrouted:N}
      if (parsed && parsed.ok === true) {
        pullSnapshot();
        dirty = true;
        const msg = parsed.unrouted > 0
          ? `Routed ${parsed.routed} segments (${parsed.unrouted} unrouted) in ${elapsed}s`
          : `Routed ${parsed.routed} segments in ${elapsed}s`;
        statusText.textContent = msg;
        console.log(`[Routing] ${msg}`);
        return;
      }

      statusText.textContent = 'Routing produced unexpected result';
      console.warn('[Routing] Unexpected response:', parsed);

    } catch (err) {
      statusText.textContent = `Routing error: ${err}`;
      console.error('[Routing] Exception:', err);
    } finally {
      isRouting = false;
      updateRoutingUI({ isRouting: false, pass: 0, routed: 0, unrouted: 0, elapsed: 0 });
      setTimeout(() => {
        if (!isVariantPanelVisible()) {
          statusText.textContent = usingWasm ? 'Ready (WASM)' : 'Ready (Mock)';
        }
      }, 5000);
    }
  }

  /**
   * Handle routing completion from WebSocket
   */
  function handleRouteComplete(sesContent: string | null, _routesContent: string | null): void {
    isRouting = false;
    const elapsed = Math.round((Date.now() - routingStartTime) / 1000);
    updateRoutingUI({ isRouting: false, pass: 0, routed: 0, unrouted: 0, elapsed: 0 });

    if (sesContent) {
      console.log('[Routing] Loading SES routes...');
      engine.load_routes(sesContent);
      pullSnapshot();
      markTracesUnsaved();
      syncEditorTraces();
      dirty = true;
      statusText.textContent = `Routing complete (${elapsed}s)`;
    } else {
      statusText.textContent = `Routing complete, no routes (${elapsed}s)`;
    }

    // Show completion status briefly, then normal
    setTimeout(() => {
      statusText.textContent = usingWasm ? 'Ready (WASM)' : 'Ready (Mock)';
    }, 3000);
  }

  /**
   * Handle routing error from WebSocket
   */
  function handleRouteError(error: string): void {
    isRouting = false;
    updateRoutingUI({ isRouting: false, pass: 0, routed: 0, unrouted: 0, elapsed: 0 });
    statusText.textContent = `Routing error: ${error}`;
    console.error('[Routing] Error:', error);

    setTimeout(() => {
      statusText.textContent = usingWasm ? 'Ready (WASM)' : 'Ready (Mock)';
    }, 5000);
  }

  /**
   * Handle routing progress from WebSocket
   */
  function handleRouteProgress(output: string): void {
    // Parse progress output from CLI (format: "Pass X: Y routed, Z unrouted (Xs)")
    const match = output.match(/Pass (\d+): (\d+) routed, (\d+) unrouted/);
    if (match) {
      const elapsed = Math.round((Date.now() - routingStartTime) / 1000);
      updateRoutingUI({
        isRouting: true,
        pass: parseInt(match[1], 10),
        routed: parseInt(match[2], 10),
        unrouted: parseInt(match[3], 10),
        elapsed,
      });
    }
  }

  /**
   * Cancel the current routing operation
   */
  function cancelRouting(): void {
    // Cancel is only meaningful for async routing (future Web Worker implementation)
    console.log('[Routing] Cancel requested');
    isRouting = false;
    updateRoutingUI({ isRouting: false, pass: 0, routed: 0, unrouted: 0, elapsed: 0 });
  }

  /**
   * Run routing in debug mode — captures each pipeline stage and renders them
   * as toggleable overlays. Shift+click Route to activate.
   */
  async function triggerDebugRouting(): Promise<void> {
    if (!snapshot?.board) {
      statusText.textContent = 'Load a board first';
      return;
    }

    statusText.textContent = 'Debug routing…';
    await new Promise(resolve => setTimeout(resolve, 50));

    try {
      const params = getPreference('autorouteParams');
      const rustParams = {
        via_cost: params.viaCost,
        layer_preference: params.layerPreference,
        roundness: params.roundness,
        density: params.density,
      };

      const resultJson = engine.auto_route_debug(JSON.stringify(rustParams));
      const debug = JSON.parse(resultJson);

      if (debug.ok === false) {
        statusText.textContent = `Debug route failed: ${debug.error}`;
        return;
      }

      // Store globally for inspection
      (window as any).__routeDebug = debug;

      // Show debug panel
      showRouteDebugPanel(debug);

      statusText.textContent = `Debug: ${debug.stages.length} stages, grid ${debug.grid_width}×${debug.grid_height}, ${debug.unrouted_count} unrouted, ${debug.iterations} iterations${debug.converged ? ' ✓' : ' ✗'}`;
    } catch (err) {
      statusText.textContent = `Debug route error: ${err}`;
      console.error('[DebugRoute]', err);
    }
  }

  /**
   * Show debug routing panel with stage toggles.
   * Each stage can be toggled on/off — selected stage's segments render as overlay.
   */
  function showRouteDebugPanel(debug: any): void {
    debugData = debug;
    debugOverlayStage = -1;

    // Remove old panel if exists
    let panel = document.getElementById('route-debug-panel');
    if (panel) panel.remove();

    // Get final stage segments (last stage = after all processing)
    const finalStage = debug.stages[debug.stages.length - 1];
    if (!finalStage) return;

    // Group segments by net_id
    const netMap = new Map<number, { segments: any[]; vias: any[] }>();
    for (const seg of finalStage.segments) {
      if (!netMap.has(seg.net_id)) netMap.set(seg.net_id, { segments: [], vias: [] });
      netMap.get(seg.net_id)!.segments.push(seg);
    }
    for (const via of finalStage.vias) {
      if (!netMap.has(via.net_id)) netMap.set(via.net_id, { segments: [], vias: [] });
      netMap.get(via.net_id)!.vias.push(via);
    }

    const netIds = Array.from(netMap.keys()).sort((a, b) => a - b);
    const colors = ['#FF6B6B', '#4ECDC4', '#45B7D1', '#96CEB4', '#FFEAA7', '#DDA0DD', '#98D8C8', '#F7DC6F'];

    panel = document.createElement('div');
    panel.id = 'route-debug-panel';

    let infoHtml = `
      <div class="rdp-header">
        <span>Route Debug</span>
        <button id="rdp-close">&times;</button>
      </div>
      <div class="rdp-info">
        Grid: ${debug.grid_width}×${debug.grid_height} (${(debug.grid_resolution_nm / 1000).toFixed(0)}µm)
        · Nets: ${debug.net_count} · Unrouted: ${debug.unrouted_count}
        · Iters: ${debug.iterations}${debug.converged ? ' ✓' : ' ✗'}
        · Stages: ${debug.stages.map((s: any) => s.stats.segment_count).join('→')} segs
      </div>
      <div class="rdp-stages">
        <button class="rdp-stage-btn" data-net="all">All nets (${finalStage.segments.length} segs)</button>
    `;

    for (const netId of netIds) {
      const data = netMap.get(netId)!;
      const color = colors[netId % colors.length];
      infoHtml += `<button class="rdp-stage-btn" data-net="${netId}" style="border-left: 4px solid ${color}">
        Net ${netId} — ${data.segments.length} segs, ${data.vias.length} vias
      </button>`;
    }
    infoHtml += '</div>';

    panel.innerHTML = infoHtml;
    document.body.appendChild(panel);

    // Build per-net stages for overlay rendering
    const allNetStage = finalStage;
    const perNetStages = new Map<number, any>();
    for (const netId of netIds) {
      const data = netMap.get(netId)!;
      perNetStages.set(netId, {
        name: `Net ${netId}`,
        segments: data.segments,
        vias: data.vias,
        stats: { segment_count: data.segments.length, via_count: data.vias.length },
      });
    }

    // Wire buttons
    const stagesEl = panel.querySelector('.rdp-stages')!;
    stagesEl.querySelectorAll('.rdp-stage-btn').forEach((btn: any) => {
      btn.addEventListener('click', () => {
        const netVal = btn.dataset.net;
        stagesEl.querySelectorAll('.rdp-stage-btn').forEach((b: any) => b.classList.remove('active'));

        if (netVal === 'all') {
          if (debugOverlayStage === -2) {
            debugOverlayStage = -1; // toggle off
          } else {
            debugOverlayStage = -2; // special: show all
            debugData._activeStage = allNetStage;
            btn.classList.add('active');
          }
        } else {
          const netId = parseInt(netVal);
          if (debugOverlayStage === netId) {
            debugOverlayStage = -1; // toggle off
          } else {
            debugOverlayStage = netId;
            debugData._activeStage = perNetStages.get(netId);
            btn.classList.add('active');
          }
        }
        dirty = true;
      });
    });

    document.getElementById('rdp-close')!.addEventListener('click', () => {
      debugOverlayStage = -1;
      debugData = null;
      panel!.remove();
      dirty = true;
    });
  }

  // Route button: normal routing
  routeBtn.addEventListener('click', () => {
    triggerRouting();
  });

  // Cancel button click handler
  cancelRouteBtn.addEventListener('click', () => {
    cancelRouting();
  });

  // ========================================================================
  // Route Menu Dropdown (auto-route + tuning)
  // ========================================================================

  const routeMenuDropdown = document.getElementById('route-menu-dropdown')!;
  const tuneViaCost = document.getElementById('tune-via-cost') as HTMLInputElement;
  const tuneLayerPref = document.getElementById('tune-layer-pref') as HTMLInputElement;
  const tuneRoundness = document.getElementById('tune-roundness') as HTMLInputElement;
  const tuneDensity = document.getElementById('tune-density') as HTMLInputElement;
  const tuneViaCostVal = document.getElementById('tune-via-cost-val')!;
  const tuneLayerPrefVal = document.getElementById('tune-layer-pref-val')!;
  const tuneRoundnessVal = document.getElementById('tune-roundness-val')!;
  const tuneDensityVal = document.getElementById('tune-density-val')!;

  // Initialize sliders from persisted settings
  const savedParams = getPreference('autorouteParams');
  tuneViaCost.value = String(savedParams.viaCost);
  tuneViaCostVal.textContent = savedParams.viaCost.toFixed(1);
  tuneLayerPref.value = String(savedParams.layerPreference);
  tuneLayerPrefVal.textContent = savedParams.layerPreference.toFixed(1);
  tuneRoundness.value = String(savedParams.roundness);
  tuneRoundnessVal.textContent = savedParams.roundness.toFixed(2);
  tuneDensity.value = String(savedParams.density);
  tuneDensityVal.textContent = savedParams.density.toFixed(1);

  // Toggle route menu
  routeMenuBtn.addEventListener('click', (e) => {
    e.stopPropagation();
    routeMenuDropdown.classList.toggle('hidden');
    updateTuningDebugSurface();
  });

  // Close route menu on click outside
  document.addEventListener('click', (e) => {
    if (!routeMenuDropdown.classList.contains('hidden') &&
        !routeMenuDropdown.contains(e.target as Node) &&
        e.target !== routeMenuBtn) {
      routeMenuDropdown.classList.add('hidden');
      updateTuningDebugSurface();
    }
  });

  // Debounce timer for re-routing
  let tuningDebounceTimer: number | null = null;

  /**
   * Read current slider values as AutorouteParams
   */
  function readTuningSliders(): AutorouteParams {
    return {
      viaCost: parseFloat(tuneViaCost.value),
      layerPreference: parseFloat(tuneLayerPref.value),
      roundness: parseFloat(tuneRoundness.value),
      density: parseFloat(tuneDensity.value),
    };
  }

  /**
   * Handle slider input: update display, persist, and trigger debounced re-route
   */
  function onTuningSliderInput(): void {
    const params = readTuningSliders();

    // Update value displays
    tuneViaCostVal.textContent = params.viaCost.toFixed(1);
    tuneLayerPrefVal.textContent = params.layerPreference.toFixed(1);
    tuneRoundnessVal.textContent = params.roundness.toFixed(2);
    tuneDensityVal.textContent = params.density.toFixed(1);

    // Persist to settings
    setPreference('autorouteParams', params);

    // Update debug surface
    updateTuningDebugSurface();

    // Debounce parameter persistence (no auto re-route — it freezes the browser)
    if (tuningDebounceTimer !== null) {
      clearTimeout(tuningDebounceTimer);
    }

    tuningDebounceTimer = window.setTimeout(() => {
      tuningDebounceTimer = null;
      // Parameters are already persisted above in setPreference.
      // User can click Route to apply the new params.
      console.log('[Tuning] Params updated — click Route to apply');
    }, 300);
  }

  // Wire all sliders
  tuneViaCost.addEventListener('input', onTuningSliderInput);
  tuneLayerPref.addEventListener('input', onTuningSliderInput);
  tuneRoundness.addEventListener('input', onTuningSliderInput);
  tuneDensity.addEventListener('input', onTuningSliderInput);

  /**
   * Update the __tuningPanel debug surface
   */
  function updateTuningDebugSurface(): void {
    (window as any).__tuningPanel = {
      visible: !routeMenuDropdown.classList.contains('hidden'),
      params: readTuningSliders(),
    };
  }

  // Initialize debug surface
  updateTuningDebugSurface();

  /**
   * Handle saving the current file (web only).
   * Uses File System Access API with handle for save-in-place.
   */
  async function handleSaveFile(): Promise<void> {
    // Use editor content if editor is active, otherwise use lastLoadedSource
    const sourceContent = (editorReady && editorInstance) ? editorInstance.getValue() : lastLoadedSource;

    if (!sourceContent) {
      console.log('[Save] No content to save');
      statusText.textContent = 'No design loaded';
      setTimeout(() => {
        statusText.textContent = usingWasm ? 'Ready (WASM)' : 'Ready (Mock)';
      }, 2000);
      return;
    }

    // Merge routed traces into the source before saving
    const exportedTraces = engine.export_traces_as_dsl();
    const contentToSave = mergeTracesIntoDsl(sourceContent, exportedTraces);

    try {
      const defaultName = currentFilePath || 'design.cypcb';
      const newHandle = await saveFile(contentToSave, currentFileHandle, defaultName);

      // Update handle if we got a new one (from save-as)
      if (newHandle) {
        currentFileHandle = newHandle;
      }

      // Update in-memory source to match what we saved (includes traces)
      lastLoadedSource = contentToSave;

      // Update editor if open (suppress sync to avoid re-parse loop)
      if (editorReady && editorInstance) {
        suppressSync = true;
        editorInstance.setValue(contentToSave);
        suppressSync = false;
      }

      // Clear unsaved-traces flag
      tracesUnsaved = false;
      updateTitle();

      // Show saved status briefly
      statusText.textContent = 'Saved';
      setTimeout(() => {
        statusText.textContent = usingWasm ? 'Ready (WASM)' : 'Ready (Mock)';
      }, 1500);

    } catch (err) {
      console.error('[Save] Error saving file:', err);
      statusText.textContent = `Error saving file: ${err}`;
      setTimeout(() => {
        statusText.textContent = usingWasm ? 'Ready (WASM)' : 'Ready (Mock)';
      }, 3000);
    }
  }

  // Keyboard shortcuts
  document.addEventListener('keydown', async (e) => {
    // Escape: close search panel first, then handle other escape actions
    if (e.key === 'Escape') {
      // Close search panel if open
      if (isSearchPanelVisible()) {
        hideSearchPanel();
        e.preventDefault();
        return;
      }
      // Skip routing cancel — handled by interaction.ts keyboard handler
      if (routingState.mode === 'routing') return;
      // Clear net highlighting (from trace selection, not routing)
      if (highlightedNet != null) {
        console.log('[Net] Cleared');
        highlightedNet = null;
        selectedTraceId = null;
        interactionState.selectedTraceId = null;
        labelPosition = null;
        dirty = true;
        e.preventDefault();
        return;
      }
      if (isRouting) {
        cancelRouting();
        return;
      }
    }

    // Ctrl+Z: undo, Ctrl+Shift+Z / Ctrl+Y: redo
    if ((e.ctrlKey || e.metaKey) && e.key === 'z' && !e.shiftKey) {
      // Skip if typing in editor/input
      const tag = (e.target as HTMLElement)?.tagName;
      const isEditorFocused = tag === 'INPUT' || tag === 'TEXTAREA' ||
        (e.target as HTMLElement)?.closest('.monaco-editor') != null;
      if (!isEditorFocused) {
        e.preventDefault();
        // Deselects any trace too: the ids may have changed under it.
        performUndo();
      }
      return;
    }
    if ((e.ctrlKey || e.metaKey) && ((e.key === 'z' && e.shiftKey) || e.key === 'Z' || e.key === 'y')) {
      const tag = (e.target as HTMLElement)?.tagName;
      const isEditorFocused = tag === 'INPUT' || tag === 'TEXTAREA' ||
        (e.target as HTMLElement)?.closest('.monaco-editor') != null;
      if (!isEditorFocused) {
        e.preventDefault();
        performRedo();
      }
      return;
    }

    // R: rotate selected component 90° CW; Shift+R: 90° CCW
    if ((e.key === 'r' || e.key === 'R') && !e.ctrlKey && !e.metaKey && !e.altKey && routingState.mode === 'idle') {
      const tag = (e.target as HTMLElement)?.tagName;
      const isEditorFocused = tag === 'INPUT' || tag === 'TEXTAREA' ||
        (e.target as HTMLElement)?.closest('.monaco-editor') != null;
      if (!isEditorFocused && selectedRefdes != null) {
        e.preventDefault();
        const delta = e.shiftKey ? -90_000 : 90_000;
        const cmd = new RotateComponentCommand(engine, selectedRefdes, delta, refreshSnapshot);
        undoStack.push(cmd);
        const sign = delta > 0 ? '+' : '';
        console.log(`[Rotate] ${selectedRefdes} ${sign}${delta / 1000}°`);
        statusText.textContent = `Rotated ${selectedRefdes} ${sign}${delta / 1000}°`;
        dirty = true;
      }
    }

    // Delete: remove selected trace (through undo stack)
    // Skip if focus is in editor/input (let Monaco handle Delete/Backspace)
    if ((e.key === 'Delete' || e.key === 'Backspace') && routingState.mode === 'idle') {
      const tag = (e.target as HTMLElement)?.tagName;
      if (tag === 'INPUT' || tag === 'TEXTAREA' || (e.target as HTMLElement)?.closest('.monaco-editor')) {
        // Let the editor handle it
      } else if (selectedTraceId != null) {
        // Capture trace data for undo before removing
        const trace = snapshot?.traces?.find(t => t.id === selectedTraceId);
        if (trace) {
          const segments: number[] = [];
          for (const seg of trace.segments) {
            segments.push(
              Math.round(seg.start_x), Math.round(seg.start_y),
              Math.round(seg.end_x), Math.round(seg.end_y),
            );
          }
          const cmd = new RemoveTraceCommand(
            engine,
            selectedTraceId,
            { netName: trace.net_name, layer: trace.layer, width: trace.width, segments },
            refreshSnapshot,
          );
          undoStack.push(cmd);
          markTracesUnsaved();
          syncEditorTraces();
        }
        selectedTraceId = null;
        interactionState.selectedTraceId = null;
        interactionState.onTraceSelect(null, 0, 0);
        labelPosition = null;
        statusText.textContent = 'Trace deleted';
        setTimeout(() => {
          statusText.textContent = usingWasm ? 'Ready (WASM)' : 'Ready (Mock)';
        }, 1500);
        dirty = true;
        e.preventDefault();
        return;
      }
    }
    // F: fit board to view when idle (routing F handled in interaction.ts)
    if (e.key === 'f' && !e.ctrlKey && !e.metaKey && !e.altKey) {
      if (routingState.mode !== 'routing') {
        fitBtn.click();
      }
      // During routing, interaction.ts handles F for flip layer
    }
    // Ctrl+E to toggle editor
    if ((e.ctrlKey || e.metaKey) && e.key === 'e') {
      e.preventDefault();
      editorToggleBtn.click();
    }
    // Ctrl+J to toggle JLCPCB search panel
    if ((e.ctrlKey || e.metaKey) && e.key === 'j') {
      e.preventDefault();
      // Close project manager if open
      if (!document.getElementById('project-manager')?.classList.contains('hidden')) {
        hideProjectManager();
      }
      toggleSearchPanel();
    }
    // Ctrl+Shift+T to toggle theme. Match both cases: whether the browser
    // reports the shifted 'T' or the plain 't' depends on the layout and the
    // input path, and every other letter shortcut here already accepts both.
    if ((e.ctrlKey || e.metaKey) && e.shiftKey && (e.key === 'T' || e.key === 't')) {
      e.preventDefault();
      themeToggle.click();
    }
    // Ctrl+S to save (web only - desktop uses native menu)
    if ((e.ctrlKey || e.metaKey) && e.key === 's' && !isDesktop()) {
      e.preventDefault();
      await handleSaveFile();
    }
    // Ctrl+O to open file / show project manager
    if ((e.ctrlKey || e.metaKey) && e.key === 'o') {
      e.preventDefault();
      openBtn.click();
    }
    // '3' key toggles 3D view (skip if typing in editor/input)
    if (e.key === '3' && !e.ctrlKey && !e.metaKey && !e.altKey) {
      const tag = (e.target as HTMLElement)?.tagName;
      const isEditorFocused = tag === 'INPUT' || tag === 'TEXTAREA' ||
        (e.target as HTMLElement)?.closest('.monaco-editor') != null;
      if (!isEditorFocused) {
        e.preventDefault();
        view3dBtn.click();
      }
    }
    // '?' key opens help modal (skip if typing in editor/input)
    if (e.key === '?' && !e.ctrlKey && !e.metaKey && !e.altKey) {
      const tag = (e.target as HTMLElement)?.tagName;
      const isEditorFocused = tag === 'INPUT' || tag === 'TEXTAREA' ||
        (e.target as HTMLElement)?.closest('.monaco-editor') != null;
      if (!isEditorFocused) {
        e.preventDefault();
        openHelpModal();
      }
    }
  });

  // Connect WebSocket for hot reload and routing
  try {
    wsConnection = connectWebSocket({
      onConnect: () => {
        // Request workspace file list for project manager
        wsConnection?.send({ type: 'list-files' });
      },
      onReload: (content, file) => {
        // Skip WebSocket init if URL has state (shared link)
        if (hasUrlState && !currentFilePath) {
          console.log('[WS] Skipping init - URL state present');
          hasUrlState = false; // Only skip first init
          return;
        }

        // Track current file for routing
        currentFilePath = file;

        // This file came off the dev server's disk, so its library is beside
        // it there. The engine asks for paths relative to the design; the
        // server resolves them inside the directory it watches, and refuses
        // anything that climbs out of it.
        const directory = file.slice(0, file.lastIndexOf('/'));
        importReader = (path: string) =>
          wsConnection?.readFile(`${directory}/${path}`) ?? Promise.resolve(null);

        reload(content, file);

        // Hide project manager when a file is loaded
        hideProjectManager();

        // Auto-route disabled — autorouter needs fundamental rewrite
        // if (autoRouteCb.checked && !isRouting) {
        //   setTimeout(() => { triggerRouting(); }, 500);
        // }
      },
      onRouteStart: () => {
        console.log('[Routing] Server started routing...');
      },
      onRouteProgress: (output) => {
        handleRouteProgress(output);
      },
      onRouteComplete: (sesContent, routesContent) => {
        handleRouteComplete(sesContent, routesContent);
      },
      onRouteError: (error) => {
        handleRouteError(error);
      },
      onFileList: (files) => {
        updateProjectFiles(files);
      },
    });
  } catch (_err) {
    console.log('[WS] WebSocket not available');
  }

  // Initialize desktop integration if running in Tauri
  if (isDesktop()) {
    await initDesktop();

    // Desktop event listeners - handle custom events from desktop.ts
    window.addEventListener('desktop:open-file', (event: Event) => {
      const customEvent = event as CustomEvent<{ path: string; content: string }>;
      const { path, content } = customEvent.detail;

      console.log('[Desktop] Opening file:', path);

      // Load the content into the engine
      const errors = loadDesign(content);
      if (errors) {
        console.warn('[Desktop] Parse warnings:', errors);
      }

      // Track loaded source for save operations
      lastLoadedSource = content;

      // Update snapshot
      const desktopSnap = pullSnapshot();

      // Update editor content if initialized
      if (editorReady && editorInstance) {
        suppressSync = true;
        editorInstance.setValue(content);
        suppressSync = false;

        // Update inline diagnostics
        const monaco = getMonacoModule();
        if (monaco) {
          updateDiagnostics(monaco, editorInstance, readDiagnostics(engine), desktopSnap.violations || []);
        }
      }

      // Update error badge
      if (desktopSnap.violations) {
        updateErrorBadge(desktopSnap.violations);
      }

      // Fit board in viewport if it exists
      if (desktopSnap.board) {
        viewport = fitBoard(viewport, desktopSnap.board.width_nm, desktopSnap.board.height_nm);
        interactionState.viewport = viewport;
      }

      // Update current file path for routing
      currentFilePath = path;

      // Update status with filename
      const filename = path.split(/[/\\]/).pop() || path;
      const errorCount = errors ? errors.split('\n').filter(Boolean).length : 0;
      statusText.textContent = errorCount > 0
        ? `Loaded ${filename} (${errorCount} warnings)`
        : `Loaded ${filename}`;

      hideProjectManager();
      addRecentFile(filename, desktopSnap, buildRenderStateForThumbnail(), content);
      dirty = true;
    });

    window.addEventListener('desktop:content-request', () => {
      console.log('[Desktop] Content requested for save');

      // Use editor content if editor is active, otherwise use lastLoadedSource
      const sourceContent = (editorReady && editorInstance) ? editorInstance.getValue() : lastLoadedSource;

      // Merge routed traces into the source before responding
      const exportedTraces = engine.export_traces_as_dsl();
      const contentToSave = sourceContent ? mergeTracesIntoDsl(sourceContent, exportedTraces) : sourceContent;

      // Update in-memory source and clear unsaved flag
      if (contentToSave) {
        lastLoadedSource = contentToSave;
        tracesUnsaved = false;
        updateTitle();
      }

      // Respond with merged content
      const event = new CustomEvent('desktop:content-response', {
        detail: { content: contentToSave },
      });
      window.dispatchEvent(event);
    });

    window.addEventListener('desktop:viewport', (event: Event) => {
      const customEvent = event as CustomEvent<{ action: 'zoom-in' | 'zoom-out' | 'fit' }>;
      const { action } = customEvent.detail;

      console.log('[Desktop] Viewport action:', action);

      switch (action) {
        case 'zoom-in':
          viewport = {
            ...viewport,
            scale: viewport.scale * 1.5,
          };
          interactionState.viewport = viewport;
          dirty = true;
          break;

        case 'zoom-out':
          viewport = {
            ...viewport,
            scale: viewport.scale * 0.6667,
          };
          interactionState.viewport = viewport;
          dirty = true;
          break;

        case 'fit':
          if (snapshot?.board) {
            viewport = fitBoard(viewport, snapshot.board.width_nm, snapshot.board.height_nm);
            interactionState.viewport = viewport;
            dirty = true;
          }
          break;
      }
    });

    window.addEventListener('desktop:toggle-theme', () => {
      console.log('[Desktop] Toggle theme');

      const current = themeManager.getTheme();
      // Cycle: light → dark → auto → light
      const next = current === 'light' ? 'dark' : current === 'dark' ? 'auto' : 'light';
      themeManager.setTheme(next);
      updateThemeIcon();
    });

    window.addEventListener('desktop:new-file', () => {
      console.log('[Desktop] New file');

      // Clear the design
      loadDesign('');
      pullSnapshot();

      // Clear editor content if initialized
      if (editorReady && editorInstance) {
        suppressSync = true;
        editorInstance.setValue('');
        suppressSync = false;

        // Clear inline diagnostics
        const monaco = getMonacoModule();
        if (monaco) {
          updateDiagnostics(monaco, editorInstance, readDiagnostics(engine), []);
        }
      }

      // Clear file state
      currentFilePath = null;
      lastLoadedSource = null;

      // Update status
      statusText.textContent = usingWasm ? 'Ready (WASM) - Open a file' : 'Ready (Mock) - Open a file';

      hideSearchPanel();
      showProjectManager();
      dirty = true;
    });
  }
}

// Start the application
init().catch((error) => {
  console.error('Failed to initialize viewer:', error);
  const statusText = document.getElementById('status-text');
  if (statusText) {
    statusText.textContent = 'Error: ' + (error instanceof Error ? error.message : String(error));
  }
});
