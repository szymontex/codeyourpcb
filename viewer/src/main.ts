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
import { setupInteraction, type InteractionState } from './interaction';
import { createRoutingState, cancelRoute, flipLayer, type RoutingState } from './routing';
import { UndoStack, AddTraceCommand, RemoveTraceCommand, RotateComponentCommand, ResizeBoardCommand, installDebugSurface } from './undo';
import { createLayerVisibility } from './layers';
import { createFilePicker, setupDropZone, readFileAsText } from './file-picker';
import { openFile, saveFile } from './file-access';
import { isDesktop, initDesktop } from './desktop';
import { encodeViewState, decodeViewState } from './url-state';

// WebSocket server URL for hot reload
// Dynamic: if accessing via dev1.flightcore.pl, use dev2.flightcore.pl for WS
function getWsUrl(): string {
  const host = window.location.hostname;
  if (host === 'dev1.flightcore.pl') {
    return 'wss://dev2.flightcore.pl';
  }
  // Local development
  return 'ws://localhost:4322';
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

  function connect(): void {
    ws = new WebSocket(WS_URL);

    ws.onopen = () => {
      console.log('[WS] Connected');
      connected = true;
    };

    ws.onmessage = (event) => {
      try {
        const msg: WsMessage = JSON.parse(event.data);
        console.log(`[WS] Received: ${msg.type}`);

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
      console.log('[WS] Disconnected, reconnecting in 2s...');
      connected = false;
      setTimeout(connect, 2000);
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
  const gridSnapCb = document.getElementById('grid-snap') as HTMLInputElement;
  const undoBtn = document.getElementById('undo-btn') as HTMLButtonElement;
  const redoBtn = document.getElementById('redo-btn') as HTMLButtonElement;
  const routeBtn = document.getElementById('route-btn') as HTMLButtonElement;
  const cancelRouteBtn = document.getElementById('cancel-route-btn') as HTMLButtonElement;
  const autoRouteCb = document.getElementById('auto-route') as HTMLInputElement;
  const routingStatus = document.getElementById('routing-status')!;
  const routingProgress = document.getElementById('routing-progress')!;
  const openBtn = document.getElementById('open-btn') as HTMLButtonElement;
  // @ts-ignore - Used conditionally in web-only block (line 1001-1002)
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
  let showRatsnest = true;
  let colorByNet = true;
  let selectedTraceId: number | null = null;
  let hoveredTraceId: number | null = null;
  let labelPosition: { x: number; y: number } | null = null;
  let routingState: RoutingState = createRoutingState();
  let highlightedNet: string | null = null;

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

  // Layer checkbox handlers — update both 2D and 3D views
  topLayerCb.addEventListener('change', () => {
    layers = {
      ...layers,
      topCopper: topLayerCb.checked,
    };
    dirty = true;
    if (is3DActive && renderer3d) {
      renderer3d.updateLayerVisibility(layers);
    }
  });
  bottomLayerCb.addEventListener('change', () => {
    layers = {
      ...layers,
      bottomCopper: bottomLayerCb.checked,
    };
    dirty = true;
    if (is3DActive && renderer3d) {
      renderer3d.updateLayerVisibility(layers);
    }
  });
  ratsnestCb.addEventListener('change', () => {
    showRatsnest = ratsnestCb.checked;
    dirty = true;
  });

  // Grid snap toggle
  gridSnapCb.addEventListener('change', () => {
    routingState = { ...routingState, gridSnapEnabled: gridSnapCb.checked };
    interactionState.routing = routingState;
    console.log(`[Grid] Snap ${gridSnapCb.checked ? 'enabled' : 'disabled'} (spacing: ${routingState.gridSpacing / 1e6}mm)`);
    dirty = true;
  });

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
    // Cycle: light → dark → auto → light
    const next = current === 'light' ? 'dark' : current === 'dark' ? 'auto' : 'light';
    themeManager.setTheme(next);
    updateThemeIcon();
  });

  // Also update icon when OS theme changes (relevant in auto mode)
  themeManager.subscribe(() => {
    updateThemeIcon();
  });

  updateThemeIcon();

  // Monaco editor setup
  let editorInstance: any = null;
  let editorReady = false;
  let suppressSync = false; // Prevent circular updates when setting editor content programmatically
  const { initEditor, toggleEditorPanel, getMonacoModule } = await import('./editor/editor-panel');
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
        snapshot = engine.get_snapshot();

        // Update inline diagnostics (LSP bridge)
        const monaco = getMonacoModule();
        if (monaco && editorInstance) {
          updateDiagnostics(monaco, editorInstance, errors, snapshot.violations || []);
        }

        // Update error badge
        if (snapshot.violations) {
          updateErrorBadge(snapshot.violations);
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
  editorToggleBtn.addEventListener('click', async () => {
    // Lazy-load editor on first toggle
    if (!editorReady) {
      console.log('[Editor] Initializing Monaco editor...');
      editorInstance = await initEditor(editorContainer);

      // Set initial content if a file is loaded
      if (lastLoadedSource) {
        suppressSync = true;
        editorInstance.setValue(lastLoadedSource);
        suppressSync = false;
      }

      // Wire up editor-to-board sync
      setupEditorSync(editorInstance);

      editorReady = true;
      console.log('[Editor] Monaco editor ready');
    }
    toggleEditorPanel();
    // Trigger canvas resize
    resize();
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
  snapshot = engine.get_snapshot();
  currentFilePath = null;
  statusText.textContent = usingWasm ? 'Ready (WASM) - Open a file' : 'Ready (Mock) - Open a file';

  /**
   * Refresh the board snapshot from the engine and sync to interaction state.
   * Used as callback by undo commands after mutations.
   */
  function refreshSnapshot(): void {
    snapshot = engine.get_snapshot();
    if (interactionState) {
      interactionState.snapshot = snapshot;
    }
    if (snapshot.violations) {
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
        snapshot = engine.get_snapshot();
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
        snapshot = engine.get_snapshot();

        // Update editor content if initialized
        if (editorReady && editorInstance) {
          suppressSync = true;
          editorInstance.setValue(content);
          suppressSync = false;

          // Update inline diagnostics
          const monaco = getMonacoModule();
          if (monaco) {
            updateDiagnostics(monaco, editorInstance, errors, snapshot.violations || []);
          }
        }

        // Update current file path for routing
        currentFilePath = file.name;

        if (snapshot.board) {
          viewport = fitBoard(viewport, snapshot.board.width_nm, snapshot.board.height_nm);
          interactionState.viewport = viewport;
        }

        // Update error badge
        if (snapshot.violations) {
          updateErrorBadge(snapshot.violations);
        }

        // Show status
        const errorCount = errors ? errors.split('\n').filter(Boolean).length : 0;
        statusText.textContent = errorCount > 0
          ? `Loaded ${file.name} (${errorCount} warnings)`
          : `Loaded ${file.name}`;

        dirty = true;

      } else if (ext === 'ses') {
        // Check if board is loaded
        if (!snapshot?.board) {
          statusText.textContent = 'Load a .cypcb file first';
          return;
        }

        // Load routes
        engine.load_routes(content);
        snapshot = engine.get_snapshot();

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

  // Open button - use File System Access API on web, keep file picker for desktop
  openBtn.addEventListener('click', async () => {
    if (isDesktop()) {
      // Desktop uses its own file dialog via Tauri IPC
      filePicker.click();
    } else {
      // Web uses File System Access API with fallback
      const result = await openFile();
      if (result) {
        // Clear undo stack on new file load
        undoStack.clear();
        // Store handle for save-in-place
        currentFileHandle = result.handle;
        currentFilePath = result.name;

        // Parse the content as if it was a file load
        const ext = result.name.toLowerCase().split('.').pop();

        if (ext === 'cypcb') {
          // Load new board
          const errors = engine.load_source(result.content);
          if (errors) {
            console.warn('Parse errors:', errors);
          }

          // Track loaded source for save operations
          lastLoadedSource = result.content;

          // Get new snapshot and fit board
          snapshot = engine.get_snapshot();

          // Update editor content if initialized
          if (editorReady && editorInstance) {
            suppressSync = true;
            editorInstance.setValue(result.content);
            suppressSync = false;

            // Update inline diagnostics
            const monaco = getMonacoModule();
            if (monaco) {
              updateDiagnostics(monaco, editorInstance, errors, snapshot.violations || []);
            }
          }

          if (snapshot.board) {
            viewport = fitBoard(viewport, snapshot.board.width_nm, snapshot.board.height_nm);
            interactionState.viewport = viewport;
          }

          // Update error badge
          if (snapshot.violations) {
            updateErrorBadge(snapshot.violations);
          }

          // Show status
          const errorCount = errors ? errors.split('\n').filter(Boolean).length : 0;
          statusText.textContent = errorCount > 0
            ? `Loaded ${result.name} (${errorCount} warnings)`
            : `Loaded ${result.name}`;

          dirty = true;

        } else if (ext === 'ses') {
          // Check if board is loaded
          if (!snapshot?.board) {
            statusText.textContent = 'Load a .cypcb file first';
            return;
          }

          // Load routes
          engine.load_routes(result.content);
          snapshot = engine.get_snapshot();

          statusText.textContent = `Loaded routes from ${result.name}`;
          dirty = true;

        } else {
          statusText.textContent = `Unknown file type: .${ext}`;
        }
      }
    }
  });

  // Drag-drop setup
  setupDropZone(container, handleFileLoad);

  /**
   * Populate the error list with current violations
   */
  function populateErrorList(): void {
    if (!snapshot?.violations) {
      errorList.innerHTML = '<div class="error-item">No errors</div>';
      return;
    }

    errorList.innerHTML = snapshot.violations.map((v, i) => `
      <div class="error-item" data-index="${i}">
        <span class="error-kind">[${v.kind}]</span>
        <span class="error-message">${v.message}</span>
      </div>
    `).join('');

    // Add click handlers for zoom-to-location
    errorList.querySelectorAll('.error-item').forEach(el => {
      el.addEventListener('click', () => {
        const idx = parseInt(el.getAttribute('data-index')!, 10);
        const violation = snapshot!.violations[idx];
        zoomToLocation(violation.x_nm, violation.y_nm);
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
    // Convert to mm for display
    const xMm = (worldX / 1_000_000).toFixed(2);
    const yMm = (worldY / 1_000_000).toFixed(2);
    coordsEl.textContent = `(${xMm}, ${yMm}) mm`;

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

    snapshot = engine.get_snapshot();

    // Update editor content if initialized
    if (editorReady && editorInstance) {
      suppressSync = true;
      editorInstance.setValue(content);
      suppressSync = false;

      // Update inline diagnostics
      const monaco = getMonacoModule();
      if (monaco) {
        updateDiagnostics(monaco, editorInstance, errors, snapshot.violations || []);
      }
    }

    console.log('[HotReload] Reloaded snapshot:', snapshot);

    // Restore viewport — but fit board on first load (default viewport)
    const isDefaultViewport = savedViewport.centerX === 0 && savedViewport.centerY === 0;
    if (isDefaultViewport && snapshot.board) {
      viewport = fitBoard(savedViewport, snapshot.board.width_nm, snapshot.board.height_nm);
    } else {
      viewport = savedViewport;
    }
    interactionState.viewport = viewport;

    // Restore selection if component still exists
    if (savedSelection && snapshot.components.some(c => c.refdes === savedSelection)) {
      selectedRefdes = savedSelection;
    } else {
      selectedRefdes = null;
    }

    // Show "Reloaded" status briefly
    const parseErrorCount = errors ? errors.split('\n').filter(Boolean).length : 0;
    statusText.textContent = parseErrorCount > 0 ? `Reloaded (${parseErrorCount} warnings)` : 'Reloaded';

    // Update error badge with new violations
    if (snapshot.violations) {
      updateErrorBadge(snapshot.violations);
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
      routingProgress.textContent = `Pass ${state.pass}: ${state.routed} routed, ${state.unrouted} unrouted (${state.elapsed}s)`;
    } else {
      routeBtn.disabled = false;
      routeBtn.classList.remove('routing');
      routeBtn.textContent = 'Route';
      cancelRouteBtn.classList.add('hidden');
      routingStatus.classList.add('hidden');
    }
  }

  // WebSocket connection (initialized later)
  let wsConnection: WsConnection | null = null;
  let routingStartTime = 0;

  /**
   * Trigger routing via WebSocket to dev server.
   * The server runs the CLI route command and streams progress.
   */
  async function triggerRouting(): Promise<void> {
    if (isRouting || !currentFilePath) {
      console.log('[Routing] Cannot start routing: already routing or no file loaded');
      return;
    }

    if (!wsConnection || !wsConnection.isConnected()) {
      console.log('[Routing] WebSocket not connected, cannot route');
      statusText.textContent = 'Error: Not connected to dev server';
      return;
    }

    isRouting = true;
    routingStartTime = Date.now();

    updateRoutingUI({
      isRouting: true,
      pass: 0,
      routed: 0,
      unrouted: 0,
      elapsed: 0,
    });

    console.log('[Routing] Starting routing for:', currentFilePath);
    statusText.textContent = 'Routing...';

    // Send route request to dev server
    wsConnection.send({
      type: 'route',
      file: currentFilePath,
    });
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
      snapshot = engine.get_snapshot();
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
    // Escape: cancel manual routing first, then net highlight, then autorouting
    if (e.key === 'Escape') {
      if (routingState.mode === 'routing') {
        routingState = cancelRoute(routingState);
        interactionState.routing = routingState;
        interactionState.onRoutingChange(routingState);
        dirty = true;
        e.preventDefault();
        return;
      }
      // Clear net highlighting
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
    // F: flip layer during routing, or fit board to view when idle
    if (e.key === 'f' && !e.ctrlKey && !e.metaKey && !e.altKey) {
      if (routingState.mode === 'routing') {
        routingState = flipLayer(routingState);
        interactionState.routing = routingState;
        interactionState.onRoutingChange(routingState);
        statusText.textContent = `Layer: ${routingState.currentLayer}`;
        dirty = true;
        e.preventDefault();
      } else {
        fitBtn.click();
      }
    }
    // Ctrl+E to toggle editor
    if ((e.ctrlKey || e.metaKey) && e.key === 'e') {
      e.preventDefault();
      editorToggleBtn.click();
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
    wsConnection = connectWebSocket({
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
  } catch (err) {
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
      snapshot = engine.get_snapshot();

      // Update editor content if initialized
      if (editorReady && editorInstance) {
        suppressSync = true;
        editorInstance.setValue(content);
        suppressSync = false;

        // Update inline diagnostics
        const monaco = getMonacoModule();
        if (monaco) {
          updateDiagnostics(monaco, editorInstance, errors, snapshot.violations || []);
        }
      }

      // Update error badge
      if (snapshot.violations) {
        updateErrorBadge(snapshot.violations);
      }

      // Fit board in viewport if it exists
      if (snapshot.board) {
        viewport = fitBoard(viewport, snapshot.board.width_nm, snapshot.board.height_nm);
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
      snapshot = engine.get_snapshot();

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
