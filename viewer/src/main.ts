/**
 * Main entry point for the CodeYourPCB viewer application
 * Integrates WASM engine, rendering, and user interaction
 */

import './theme/colors.css';
import { themeManager } from './theme/theme-manager';
import { loadWasm, isWasmLoaded, type PcbEngine } from './wasm';
import type { BoardSnapshot, ViolationInfo } from './types';
import { createViewport, fitBoard, screenToWorld } from './viewport';
import { render, type RenderState } from './renderer';
import { createDefaultRenderConfig, buildPadNetMap } from './render-config';
import { setupInteraction, type InteractionState } from './interaction';
import { createRoutingState, type RoutingState } from './routing';
import { UndoStack, AddTraceCommand, RemoveTraceCommand, RotateComponentCommand, ResizeBoardCommand, installDebugSurface } from './undo';
import { createLayerVisibility } from './layers';
import { createFilePicker, setupDropZone, readFileAsText } from './file-picker';
import { openFile, saveFile } from './file-access';
import { isDesktop, initDesktop } from './desktop';
import { encodeViewState, decodeViewState } from './url-state';
import { getSettings, getPreference, setPreference, subscribe as subscribeSettings } from './settings';
import type { AppSettings, LayerColors, AutorouteParams } from './settings';
import { formatDimension, parseUserDimension } from './units';
import type { DisplayUnit } from './units';
import { initProjectManager, showProjectManager, hideProjectManager, addRecentFile } from './project-manager';
import { initSearchPanel, hideSearchPanel, toggleSearchPanel, isSearchPanelVisible, buildComponentSnippet } from './jlcpcb-panel';
import { fetch3DModel } from './jlcpcb';
import { initVariantPanel, showVariants, hideVariants, isVariantPanelVisible, type VariantData } from './variant-panel';
import type { VariantPreviewData } from './renderer';

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
}

/**
 * WebSocket connection interface for two-way communication with dev server
 */
interface WsConnection {
  send(message: object): void;
  isConnected(): boolean;
}

/**
 * Callbacks for various WebSocket events
 */
interface WsCallbacks {
  onReload: (content: string, file: string) => void;
  onRouteStart?: () => void;
  onRouteProgress?: (output: string) => void;
  onRouteComplete?: (sesContent: string | null, routesContent: string | null) => void;
  onRouteError?: (error: string) => void;
}

/**
 * Connect to the WebSocket server for hot reload and routing.
 * Automatically reconnects on disconnect.
 */
function connectWebSocket(callbacks: WsCallbacks): WsConnection {
  let ws: WebSocket | null = null;
  let connected = false;
  let retries = 0;
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
    }
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
  const ratsnestCb = document.getElementById('layer-ratsnest') as HTMLInputElement;
  const gridVisibleCb = document.getElementById('view-grid-visible') as HTMLInputElement;
  const netLabelsCb = document.getElementById('view-net-labels') as HTMLInputElement;
  const viewMenuBtn = document.getElementById('view-menu-btn') as HTMLButtonElement;
  const viewMenuDropdown = document.getElementById('view-menu-dropdown')!;
  const prefsBtn = document.getElementById('prefs-btn') as HTMLButtonElement;
  const prefsOverlay = document.getElementById('prefs-overlay')!;
  const prefsClose = document.getElementById('prefs-close') as HTMLButtonElement;
  const undoBtn = document.getElementById('undo-btn') as HTMLButtonElement;
  const redoBtn = document.getElementById('redo-btn') as HTMLButtonElement;
  const routeBtn = document.getElementById('route-btn') as HTMLButtonElement;
  const cancelRouteBtn = document.getElementById('cancel-route-btn') as HTMLButtonElement;
  const autoRouteCb = document.getElementById('auto-route') as HTMLInputElement;
  const routingStatus = document.getElementById('routing-status')!;
  const routingProgress = document.getElementById('routing-progress')!;
  const openBtn = document.getElementById('open-btn') as HTMLButtonElement;
  const shareBtn = document.getElementById('share-btn') as HTMLButtonElement;
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
  let lastLoadedSource: string | null = null;
  let showRatsnest = getPreference('ratsnestVisible');
  let gridVisible = getPreference('gridVisible');
  let showNetLabels = getPreference('netLabelsVisible');
  const colorByNet = true;
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

  // 3D view state
  let is3DActive = false;
  let renderer3d: import('./renderer3d').Renderer3D | null = null;

  // Resize handler
  function resize(): void {
    canvas.width = container.clientWidth;
    canvas.height = container.clientHeight;
    viewport = {
      ...viewport,
      width: canvas.width,
      height: canvas.height,
    };
    dirty = true;
  }
  resize();
  window.addEventListener('resize', resize);

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

  // Initialize View menu checkboxes from settings
  gridVisibleCb.checked = gridVisible;
  netLabelsCb.checked = showNetLabels;
  ratsnestCb.checked = getPreference('ratsnestVisible');

  // Undo/Redo toolbar buttons
  undoBtn.addEventListener('click', () => {
    undoStack.undo();
    selectedTraceId = null;
    labelPosition = null;
    dirty = true;
  });
  redoBtn.addEventListener('click', () => {
    undoStack.redo();
    selectedTraceId = null;
    labelPosition = null;
    dirty = true;
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

        // Parse and update board
        const errors = engine.load_source(content);
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
          updateDiagnostics(monaco, editorInstance, errors, snapshot!.violations || []);
        }

        // Update error badge
        if (snapshot!.violations) {
          updateErrorBadge(snapshot!.violations);
        }

        // Track as loaded source
        lastLoadedSource = content;

        // Mark as dirty for re-render
        dirty = true;

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
  async function ensureEditorReady(): Promise<void> {
    if (!editorReady) {
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
    }
  }

  editorToggleBtn.addEventListener('click', async () => {
    await ensureEditorReady();
    toggleEditorPanel();
    // Trigger canvas resize + refit board to new dimensions
    resize();
    if (snapshot?.board) {
      viewport = fitBoard(viewport, snapshot.board.width_nm, snapshot.board.height_nm);
    }
    if (is3DActive && renderer3d && snapshot) {
      renderer3d.updateBoard(snapshot, layers);
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
    layers = {
      topCopper: topLayerCb.checked,
      bottomCopper: bottomLayerCb.checked,
    };
    showRatsnest = ratsnestCb.checked;

    console.log('[URL State] Applied shared view state:', urlState);
  }

  // Start with empty state - user will open a file
  pullSnapshot();
  currentFilePath = null;
  statusText.textContent = usingWasm ? 'Ready (WASM) - Open a file' : 'Ready (Mock) - Open a file';

  /**
   * Build a partial RenderState for thumbnail generation from current config.
   */
  function buildRenderStateForThumbnail(): Partial<import('./renderer').RenderState> {
    return { renderConfig };
  }

  // --- Project Manager ---
  initProjectManager({
    onOpenFile: () => {
      handleWebFileOpen();
    },
    onLoadTemplate: (source, templateName) => {
      // Clear undo stack
      undoStack.clear();

      const errors = engine.load_source(source);
      if (errors) console.warn('[Template] Parse warnings:', errors);

      lastLoadedSource = source;
      const snap = pullSnapshot();

      // Update editor if initialized
      if (editorReady && editorInstance) {
        suppressSync = true;
        editorInstance.setValue(source);
        suppressSync = false;
        const monaco = getMonacoModule();
        if (monaco) updateDiagnostics(monaco, editorInstance, errors, snap.violations || []);
      }

      if (snap.board) {
        viewport = fitBoard(viewport, snap.board.width_nm, snap.board.height_nm);
        interactionState.viewport = viewport;
      }
      if (snap.violations) updateErrorBadge(snap.violations);

      currentFilePath = `${templateName}.cypcb`;
      statusText.textContent = `Loaded template: ${templateName}`;

      addRecentFile(currentFilePath, snap, buildRenderStateForThumbnail(), source);
      hideProjectManager();
      dirty = true;
    },
    onLoadRecent: (source, name) => {
      undoStack.clear();

      const errors = engine.load_source(source);
      if (errors) console.warn('[Recent] Parse warnings:', errors);

      lastLoadedSource = source;
      const snap = pullSnapshot();

      if (editorReady && editorInstance) {
        suppressSync = true;
        editorInstance.setValue(source);
        suppressSync = false;
        const monaco = getMonacoModule();
        if (monaco) updateDiagnostics(monaco, editorInstance, errors, snap.violations || []);
      }

      if (snap.board) {
        viewport = fitBoard(viewport, snap.board.width_nm, snap.board.height_nm);
        interactionState.viewport = viewport;
      }
      if (snap.violations) updateErrorBadge(snap.violations);

      currentFilePath = name;
      statusText.textContent = `Loaded: ${name}`;

      hideProjectManager();
      dirty = true;
    },
    onNewBlank: (source) => {
      undoStack.clear();
      engine.load_source(source);
      lastLoadedSource = source;
      const snap = pullSnapshot();

      if (editorReady && editorInstance) {
        suppressSync = true;
        editorInstance.setValue(source);
        suppressSync = false;
        const monaco = getMonacoModule();
        if (monaco) updateDiagnostics(monaco, editorInstance, null, snap.violations || []);
      }

      if (snap.board) {
        viewport = fitBoard(viewport, snap.board.width_nm, snap.board.height_nm);
        interactionState.viewport = viewport;
      }

      currentFilePath = null;
      statusText.textContent = usingWasm ? 'Ready (WASM)' : 'Ready (Mock)';
      hideProjectManager();
      dirty = true;
    },
  });

  // Show project manager on startup
  showProjectManager();

  // --- JLCPCB Search Panel ---
  const jlcpcbSearchBtn = document.getElementById('jlcpcb-search-btn') as HTMLButtonElement;
  initSearchPanel({
    onComponentSelect: async (component) => {
      const lcscStr = `C${component.lcsc}`;
      console.log(`[JLCPCB] Selected: ${lcscStr} (${component.mfr})`);

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
  (window as any).__loadBoard = (source: string) => {
    engine.load_source(source);
    const snap = pullSnapshot();
    if (snap.board) {
      viewport = fitBoard(viewport, snap.board.width_nm, snap.board.height_nm);
    }
    // Sync viewport + snapshot to interaction state so click handlers use correct coords
    interactionState.viewport = viewport;
    interactionState.snapshot = snapshot;
    hideProjectManager();
    dirty = true;
    statusText.textContent = usingWasm ? 'Ready (WASM)' : 'Ready (Mock)';
  };

  /**
   * Refresh the board snapshot from the engine and sync to interaction state.
   * Used as callback by undo commands after mutations.
   */
  function refreshSnapshot(): void {
    pullSnapshot();
    if (interactionState) {
      interactionState.snapshot = snapshot;
    }
    if (snapshot?.violations) {
      updateErrorBadge(snapshot.violations);
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
      }
      dirty = true;
    },
    onTraceAdd: (netName: string, layer: string, width: number, segments: number[]) => {
      const cmd = new AddTraceCommand(engine, { netName, layer, width, segments }, refreshSnapshot);
      undoStack.push(cmd);
      dirty = true;
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
  };

  setupInteraction(canvas, interactionState);

  // Expose debug render state for programmatic inspection
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

      if (ext === 'cypcb') {
        // Load new board
        const errors = engine.load_source(content);
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
            updateDiagnostics(monaco, editorInstance, errors, snap.violations || []);
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
      const errors = engine.load_source(result.content);
      if (errors) console.warn('Parse errors:', errors);

      lastLoadedSource = result.content;
      const snap2 = pullSnapshot();

      if (editorReady && editorInstance) {
        suppressSync = true;
        editorInstance.setValue(result.content);
        suppressSync = false;
        const monaco = getMonacoModule();
        if (monaco) updateDiagnostics(monaco, editorInstance, errors, snap2.violations || []);
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

    if (!snapshot?.violations) {
      const noErrors = document.createElement('div');
      noErrors.className = 'error-item';
      noErrors.textContent = 'No errors';
      errorList.appendChild(noErrors);
      return;
    }

    // Build DOM nodes programmatically — never insert user-controlled text as HTML
    snapshot.violations.forEach((v, i) => {
      const item = document.createElement('div');
      item.className = 'error-item';
      item.dataset.index = String(i);

      const kind = document.createElement('span');
      kind.className = 'error-kind';
      kind.textContent = `[${v.kind}]`;

      const message = document.createElement('span');
      message.className = 'error-message';
      message.textContent = v.message;

      item.appendChild(kind);
      item.appendChild(message);
      errorList.appendChild(item);

      // Click handler for zoom-to-location
      item.addEventListener('click', () => {
        zoomToLocation(v.x_nm, v.y_nm);
      });
    });
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

  // Render loop
  function frame(): void {
    // Skip 2D rendering when 3D mode is active
    if (!is3DActive && (dirty || interactionState.dirty)) {
      // Keep interaction state snapshot in sync
      interactionState.snapshot = snapshot;

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
      };
      render(ctx, renderState);
      dirty = false;
      interactionState.dirty = false;
    }
    // Update undo/redo button states (cheap boolean checks, no DOM thrash when unchanged)
    undoBtn.disabled = !undoStack.canUndo;
    redoBtn.disabled = !undoStack.canRedo;
    requestAnimationFrame(frame);
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
    const errors = engine.load_source(content);
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
        updateDiagnostics(monaco, editorInstance, errors, reloadSnap.violations || []);
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
    if (state.isRouting) {
      routeBtn.disabled = true;
      routeBtn.classList.add('routing');
      routeBtn.textContent = 'Routing...';
      cancelRouteBtn.classList.remove('hidden');
      routingStatus.classList.remove('hidden');
      routingProgress.textContent = 'This may take a moment — the browser will be unresponsive while routing.';
    } else {
      routeBtn.disabled = false;
      routeBtn.classList.remove('routing');
      routeBtn.textContent = 'Route';
      cancelRouteBtn.classList.add('hidden');
      routingStatus.classList.add('hidden');
    }
  }

  // wsConnection used to be stored for routing — now routing uses WASM directly.
  // Hot-reload WS is still connected below but the reference isn't needed.
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

    // Use single auto_route() — much faster than variant generation in WASM.
    // Variant generation is available but too slow for interactive use in browser.
    try {
      let resultJson: string;

      try {
        resultJson = engine.auto_route();
        (window as any).__lastRouteResult = resultJson;
      } catch (routeErr) {
        console.warn('[Routing] auto_route() failed:', routeErr);
        // Reload the board to reset WASM engine state after panic
        if (lastLoadedSource) {
          engine.load_source(lastLoadedSource);
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
    if (isRouting) {
      console.log('[Routing] Cancelling routing...');
      // Note: Server-side cancellation not implemented yet
      // For now, just update UI
      isRouting = false;
      updateRoutingUI({ isRouting: false, pass: 0, routed: 0, unrouted: 0, elapsed: 0 });
      statusText.textContent = 'Routing cancelled';
    }
  }

  // Route button click handler
  routeBtn.addEventListener('click', () => {
    triggerRouting();
  });

  // Cancel button click handler
  cancelRouteBtn.addEventListener('click', () => {
    cancelRouting();
  });

  // ========================================================================
  // Tuning Panel
  // ========================================================================

  const tuningToggle = document.getElementById('tuning-toggle') as HTMLButtonElement;
  const tuningPanel = document.getElementById('tuning-panel')!;
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

  // Toggle panel visibility
  tuningToggle.addEventListener('click', (e) => {
    e.stopPropagation();
    tuningPanel.classList.toggle('hidden');
    console.log(`[Tuning] Panel ${tuningPanel.classList.contains('hidden') ? 'hidden' : 'visible'}`);
    updateTuningDebugSurface();
  });

  // Close tuning panel on click outside
  document.addEventListener('click', (e) => {
    if (!tuningPanel.classList.contains('hidden') &&
        !tuningPanel.contains(e.target as Node) &&
        e.target !== tuningToggle) {
      tuningPanel.classList.add('hidden');
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

    // Debounce re-routing at 300ms
    if (tuningDebounceTimer !== null) {
      clearTimeout(tuningDebounceTimer);
    }

    tuningDebounceTimer = window.setTimeout(() => {
      tuningDebounceTimer = null;

      if (!snapshot?.board) {
        console.log('[Tuning] No board loaded, skipping re-route');
        return;
      }

      // Build Rust-side params JSON (snake_case field names)
      const rustParams = {
        via_cost: params.viaCost,
        layer_preference: params.layerPreference,
        roundness: params.roundness,
        density: params.density,
      };

      console.log('[Tuning] Re-routing with params:', rustParams);

      // Clear variant panel on tuning re-route
      hideVariants();
      variantPreview = null;
      storedVariants = [];

      // Show routing indicator
      updateRoutingUI({
        isRouting: true,
        pass: 0,
        routed: 0,
        unrouted: 0,
        elapsed: 0,
      });

      try {
        const resultJson = engine.auto_route_with_params(JSON.stringify(rustParams));
        const result = JSON.parse(resultJson);

        if (result.ok) {
          pullSnapshot();
          dirty = true;
          console.log(`[Tuning] Re-routed: ${result.routed} routed, ${result.unrouted} unrouted`);
        } else {
          console.warn('[Tuning] Route failed:', result.error);
          statusText.textContent = `Tuning route failed: ${result.error}`;
        }
      } catch (err) {
        console.warn('[Tuning] Route error:', err);
        statusText.textContent = `Tuning route error: ${err}`;
      } finally {
        updateRoutingUI({ isRouting: false, pass: 0, routed: 0, unrouted: 0, elapsed: 0 });
      }
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
      visible: !tuningPanel.classList.contains('hidden'),
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
    const contentToSave = (editorReady && editorInstance) ? editorInstance.getValue() : lastLoadedSource;

    if (!contentToSave) {
      console.log('[Save] No content to save');
      statusText.textContent = 'No design loaded';
      setTimeout(() => {
        statusText.textContent = usingWasm ? 'Ready (WASM)' : 'Ready (Mock)';
      }, 2000);
      return;
    }

    try {
      const defaultName = currentFilePath || 'design.cypcb';
      const newHandle = await saveFile(contentToSave, currentFileHandle, defaultName);

      // Update handle if we got a new one (from save-as)
      if (newHandle) {
        currentFileHandle = newHandle;
      }

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

  /**
   * Handle sharing the current view state via URL
   */
  function handleShareView(): void {
    // Build view state from current viewport and layers
    const activeLayers: string[] = [];
    if (topLayerCb.checked) activeLayers.push('top');
    if (bottomLayerCb.checked) activeLayers.push('bottom');
    if (ratsnestCb.checked) activeLayers.push('ratsnest');

    const viewState = {
      layers: activeLayers,
      zoom: viewport.scale,
      panX: viewport.centerX,
      panY: viewport.centerY,
    };

    // Generate share URL
    const queryString = encodeViewState(viewState);
    const shareUrl = window.location.origin + window.location.pathname + queryString;

    // Copy to clipboard
    navigator.clipboard.writeText(shareUrl).then(() => {
      statusText.textContent = 'Share URL copied!';
      setTimeout(() => {
        statusText.textContent = usingWasm ? 'Ready (WASM)' : 'Ready (Mock)';
      }, 2000);
    }).catch((err) => {
      console.error('[Share] Failed to copy to clipboard:', err);
      statusText.textContent = 'Failed to copy share URL';
      setTimeout(() => {
        statusText.textContent = usingWasm ? 'Ready (WASM)' : 'Ready (Mock)';
      }, 2000);
    });
  }

  // Share button (web only)
  if (!isDesktop()) {
    shareBtn.classList.remove('hidden');
    shareBtn.addEventListener('click', handleShareView);
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
        undoStack.undo();
        // Deselect any trace since IDs may have changed
        selectedTraceId = null;
        interactionState.selectedTraceId = null;
        labelPosition = null;
        dirty = true;
      }
      return;
    }
    if ((e.ctrlKey || e.metaKey) && ((e.key === 'z' && e.shiftKey) || e.key === 'Z' || e.key === 'y')) {
      const tag = (e.target as HTMLElement)?.tagName;
      const isEditorFocused = tag === 'INPUT' || tag === 'TEXTAREA' ||
        (e.target as HTMLElement)?.closest('.monaco-editor') != null;
      if (!isEditorFocused) {
        e.preventDefault();
        undoStack.redo();
        selectedTraceId = null;
        interactionState.selectedTraceId = null;
        labelPosition = null;
        dirty = true;
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
    if ((e.key === 'Delete' || e.key === 'Backspace') && routingState.mode === 'idle') {
      if (selectedTraceId != null) {
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
    // Ctrl+Shift+T to toggle theme
    if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === 'T') {
      e.preventDefault();
      themeToggle.click();
    }
    // Ctrl+S to save (web only - desktop uses native menu)
    if ((e.ctrlKey || e.metaKey) && e.key === 's' && !isDesktop()) {
      e.preventDefault();
      await handleSaveFile();
    }
    // Ctrl+Shift+S to share (web only)
    if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === 'S' && !isDesktop()) {
      e.preventDefault();
      handleShareView();
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
  });

  // Connect WebSocket for hot reload and routing
  try {
    connectWebSocket({
      onReload: (content, file) => {
        // Skip WebSocket init if URL has state (shared link)
        if (hasUrlState && !currentFilePath) {
          console.log('[WS] Skipping init - URL state present');
          hasUrlState = false; // Only skip first init
          return;
        }

        // Track current file for routing
        currentFilePath = file;
        reload(content, file);

        // Auto-route if enabled
        if (autoRouteCb.checked && !isRouting) {
          // Small delay to let reload complete
          setTimeout(() => {
            triggerRouting();
          }, 500);
        }
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
      const errors = engine.load_source(content);
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
          updateDiagnostics(monaco, editorInstance, errors, desktopSnap.violations || []);
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
      const contentToSave = (editorReady && editorInstance) ? editorInstance.getValue() : lastLoadedSource;

      // Respond with current source content
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
      engine.load_source('');
      pullSnapshot();

      // Clear editor content if initialized
      if (editorReady && editorInstance) {
        suppressSync = true;
        editorInstance.setValue('');
        suppressSync = false;

        // Clear inline diagnostics
        const monaco = getMonacoModule();
        if (monaco) {
          updateDiagnostics(monaco, editorInstance, null, []);
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
