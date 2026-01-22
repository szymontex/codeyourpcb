/**
 * Main entry point for the CodeYourPCB viewer application
 * Integrates WASM engine, rendering, and user interaction
 */

import { loadWasm, isWasmLoaded, type PcbEngine } from './wasm';
import type { BoardSnapshot, ViolationInfo } from './types';
import { createViewport, fitBoard, screenToWorld } from './viewport';
import { render, type RenderState } from './renderer';
import { setupInteraction, type InteractionState } from './interaction';
import { createLayerVisibility } from './layers';

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
  type: 'init' | 'reload';
  file: string;
  content: string;
  timestamp?: number;
}

/**
 * Connect to the WebSocket server for hot reload notifications.
 * Automatically reconnects on disconnect.
 */
function connectWebSocket(
  onReload: (content: string, file: string) => void
): void {
  let ws: WebSocket;

  function connect(): void {
    ws = new WebSocket(WS_URL);

    ws.onopen = () => {
      console.log('[HotReload] WebSocket connected');
    };

    ws.onmessage = (event) => {
      try {
        const msg: WsMessage = JSON.parse(event.data);
        if (msg.type === 'init' || msg.type === 'reload') {
          console.log(`[HotReload] ${msg.type}: ${msg.file}`);
          onReload(msg.content, msg.file);
        }
      } catch (err) {
        console.error('[HotReload] Message parse error:', err);
      }
    };

    ws.onclose = () => {
      console.log('[HotReload] WebSocket disconnected, reconnecting in 2s...');
      setTimeout(connect, 2000);
    };

    ws.onerror = () => {
      // Error is handled by onclose
    };
  }

  connect();
}

// Test source for initial verification
const TEST_SOURCE = `
version 1
board test {
  size 50mm x 30mm
  layers 2
}
component R1 resistor "0402" {
  value "10k"
  at 10mm, 15mm
}
component C1 capacitor "0402" {
  value "100nF"
  at 20mm, 15mm
}
component U1 ic "DIP-8" {
  value "ATtiny85"
  at 35mm, 15mm
}
`;

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
  const routeBtn = document.getElementById('route-btn') as HTMLButtonElement;
  const cancelRouteBtn = document.getElementById('cancel-route-btn') as HTMLButtonElement;
  const autoRouteCb = document.getElementById('auto-route') as HTMLInputElement;
  const routingStatus = document.getElementById('routing-status')!;
  const routingProgress = document.getElementById('routing-progress')!;

  const ctx = canvas.getContext('2d')!;

  // Routing state
  let isRouting = false;
  let routingCancelled = false;
  let currentFilePath: string | null = null;

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

  // Layer checkbox handlers
  topLayerCb.addEventListener('change', () => {
    layers = {
      ...layers,
      topCopper: topLayerCb.checked,
    };
    dirty = true;
  });
  bottomLayerCb.addEventListener('change', () => {
    layers = {
      ...layers,
      bottomCopper: bottomLayerCb.checked,
    };
    dirty = true;
  });
  ratsnestCb.addEventListener('change', () => {
    showRatsnest = ratsnestCb.checked;
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
  statusText.textContent = usingWasm
    ? 'WASM loaded, parsing...'
    : 'Mock engine loaded, parsing...';

  // Load test source
  const errors = engine.load_source(TEST_SOURCE);
  if (errors) {
    console.warn('Parse errors:', errors);
  }

  snapshot = engine.get_snapshot();
  console.log('Loaded snapshot:', snapshot);

  // Set default file path for initial TEST_SOURCE (enables Route button)
  currentFilePath = 'test.cypcb';

  // Fit board in view
  if (snapshot.board) {
    viewport = fitBoard(viewport, snapshot.board.width_nm, snapshot.board.height_nm);
  }

  statusText.textContent = errors
    ? `Warnings: ${errors.split('\n').filter(Boolean).length}`
    : usingWasm ? 'Ready (WASM)' : 'Ready (Mock)';

  // Update error badge with initial violations
  if (snapshot.violations) {
    updateErrorBadge(snapshot.violations);
  }

  // Interaction setup
  const interactionState: InteractionState = {
    viewport,
    isPanning: false,
    lastX: 0,
    lastY: 0,
    dirty: false,
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
  };

  setupInteraction(canvas, interactionState);

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

  // Coordinate display on mouse move
  canvas.addEventListener('mousemove', (e) => {
    const rect = canvas.getBoundingClientRect();
    const [worldX, worldY] = screenToWorld(
      viewport,
      e.clientX - rect.left,
      e.clientY - rect.top
    );
    // Convert to mm for display
    const xMm = (worldX / 1_000_000).toFixed(2);
    const yMm = (worldY / 1_000_000).toFixed(2);
    coordsEl.textContent = `(${xMm}, ${yMm}) mm`;
  });

  canvas.addEventListener('mouseleave', () => {
    coordsEl.textContent = '';
  });

  // Visibility state
  const showViolations = true;
  let showRatsnest = true;

  // Render loop
  function frame(): void {
    if (dirty || interactionState.dirty) {
      const renderState: RenderState = {
        snapshot,
        viewport,
        layers,
        selectedRefdes,
        showViolations,
        showRatsnest,
      };
      render(ctx, renderState);
      dirty = false;
      interactionState.dirty = false;
    }
    requestAnimationFrame(frame);
  }
  frame();

  // Hot reload handler - preserves viewport and selection
  function reload(content: string, _file: string): void {
    // Save current state
    const savedViewport = { ...viewport };
    const savedSelection = selectedRefdes;

    // Parse new content
    const errors = engine.load_source(content);
    if (errors) {
      console.warn('[HotReload] Parse warnings:', errors);
    }

    snapshot = engine.get_snapshot();
    console.log('[HotReload] Reloaded snapshot:', snapshot);

    // Restore viewport (preserved exactly)
    viewport = savedViewport;
    interactionState.viewport = savedViewport;

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
   * Routing state for UI updates
   */
  interface RoutingState {
    isRouting: boolean;
    pass: number;
    routed: number;
    unrouted: number;
    elapsed: number;
  }

  /**
   * Update UI to reflect routing state
   */
  function updateRoutingUI(state: RoutingState): void {
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

  /**
   * Trigger routing via WebSocket message or CLI subprocess.
   * For MVP, we use a polling approach with the CLI.
   */
  async function triggerRouting(): Promise<void> {
    if (isRouting || !currentFilePath) {
      console.log('[Routing] Cannot start routing: already routing or no file loaded');
      return;
    }

    isRouting = true;
    routingCancelled = false;

    updateRoutingUI({
      isRouting: true,
      pass: 0,
      routed: 0,
      unrouted: 0,
      elapsed: 0,
    });

    console.log('[Routing] Starting routing for:', currentFilePath);
    statusText.textContent = 'Routing...';

    // For the viewer, we simulate routing progress
    // In a real implementation, this would communicate with a backend
    // that runs the CLI route command and reports progress via WebSocket

    // Simulate routing progress
    let elapsed = 0;
    const progressInterval = setInterval(() => {
      if (routingCancelled) {
        clearInterval(progressInterval);
        updateRoutingUI({ isRouting: false, pass: 0, routed: 0, unrouted: 0, elapsed: 0 });
        statusText.textContent = 'Routing cancelled';
        isRouting = false;
        return;
      }

      elapsed++;
      updateRoutingUI({
        isRouting: true,
        pass: Math.floor(elapsed / 3) + 1,
        routed: Math.min(elapsed * 2, 20),
        unrouted: Math.max(20 - elapsed * 2, 0),
        elapsed,
      });
    }, 1000);

    // Simulate routing completion after 5 seconds
    // In real implementation, this would wait for CLI completion
    setTimeout(() => {
      clearInterval(progressInterval);

      if (!routingCancelled) {
        isRouting = false;
        updateRoutingUI({ isRouting: false, pass: 0, routed: 0, unrouted: 0, elapsed: 0 });
        statusText.textContent = 'Routing complete (simulated)';

        // In real implementation:
        // 1. Poll for .routes file
        // 2. Load routes into engine
        // 3. Update snapshot
        // 4. Trigger re-render

        console.log('[Routing] Routing complete (simulated)');

        // Show completion status briefly
        setTimeout(() => {
          statusText.textContent = usingWasm ? 'Ready (WASM)' : 'Ready (Mock)';
        }, 2000);
      }
    }, 5000);
  }

  /**
   * Cancel the current routing operation
   */
  function cancelRouting(): void {
    if (isRouting) {
      console.log('[Routing] Cancelling routing...');
      routingCancelled = true;
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

  // Keyboard shortcut: Escape to cancel routing
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && isRouting) {
      cancelRouting();
    }
  });

  // Connect WebSocket for hot reload (fails gracefully if server not running)
  try {
    connectWebSocket((content, file) => {
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
    });
  } catch (err) {
    console.log('[HotReload] WebSocket not available, hot reload disabled');
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
