/**
 * Mouse interaction handlers for PCB viewer
 * Provides zoom, pan, and selection behaviors
 */

import type { Viewport } from './viewport';
import type { BoardSnapshot } from './types';
import type { PcbEngine } from './wasm';
import type { RoutingState } from './routing';
import { zoomAtPoint, pan, screenToWorld } from './viewport';
import { hitTestTrace } from './hit-test';
import {
  createRoutingState,
  hitTestPad,
  startRoute,
  updatePreview,
  addWaypoint,
  completeRoute,
  setDrcViolations,
  createDrcPreviewChecker,
} from './routing';

export interface InteractionState {
  viewport: Viewport;
  isPanning: boolean;
  lastX: number;
  lastY: number;
  dirty: boolean;
  onSelect: (x_nm: number, y_nm: number) => void;
  onViewportChange: (vp: Viewport) => void;
  /** Current board snapshot for hit-testing */
  snapshot: BoardSnapshot | null;
  /** Currently selected trace entity ID */
  selectedTraceId: number | null;
  /** Currently hovered trace entity ID */
  hoveredTraceId: number | null;
  /** Callback when trace selection changes */
  onTraceSelect: (traceId: number | null, screenX: number, screenY: number) => void;
  /** Callback when hover trace changes */
  onTraceHover: (traceId: number | null) => void;
  /** Routing state machine */
  routing: RoutingState;
  /** PCB engine for trace mutations during routing */
  engine: PcbEngine | null;
  /** Callback when routing state changes (for re-render) */
  onRoutingChange: (routing: RoutingState) => void;
}

/**
 * Set up all interaction handlers for the canvas
 * - Scroll wheel: zoom centered on cursor (also pinch-to-zoom on touchpad)
 * - Two-finger touchpad/touchscreen drag: pan
 * - Middle-click + drag: pan
 * - Ctrl + left-click + drag: pan (alternative for laptops)
 * - Left-click: select component at point
 * - Right-click: reserved (context menu prevented)
 */
export function setupInteraction(
  canvas: HTMLCanvasElement,
  state: InteractionState
): void {
  // Pointer cache for multi-touch pan detection
  const pointerCache: Array<{ pointerId: number; clientX: number; clientY: number }> = [];

  // Wheel zoom (zoom to cursor position)
  canvas.addEventListener('wheel', (e) => {
    e.preventDefault();
    const rect = canvas.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;

    // Zoom in on scroll up, out on scroll down
    const factor = e.deltaY < 0 ? 1.15 : 0.87;
    state.viewport = zoomAtPoint(state.viewport, x, y, factor);
    state.dirty = true;
    state.onViewportChange(state.viewport);
  }, { passive: false });

  // Pointer Events for two-finger touchpad/touchscreen pan
  canvas.addEventListener('pointerdown', (e) => {
    pointerCache.push({
      pointerId: e.pointerId,
      clientX: e.clientX,
      clientY: e.clientY,
    });
  });

  canvas.addEventListener('pointermove', (e) => {
    // Find this pointer in cache
    const index = pointerCache.findIndex(p => p.pointerId === e.pointerId);
    if (index === -1) return;

    // If exactly 2 pointers, perform two-finger pan
    if (pointerCache.length === 2) {
      const cached = pointerCache[index];
      const dx = e.clientX - cached.clientX;
      const dy = e.clientY - cached.clientY;

      // Half delta since both fingers contribute to pan
      state.viewport = pan(state.viewport, dx / 2, dy / 2);
      state.dirty = true;
      state.onViewportChange(state.viewport);
    }

    // Update cached position
    pointerCache[index].clientX = e.clientX;
    pointerCache[index].clientY = e.clientY;
  });

  // Shared cleanup function for pointer removal
  const removePointer = (e: PointerEvent) => {
    const index = pointerCache.findIndex(p => p.pointerId === e.pointerId);
    if (index !== -1) {
      pointerCache.splice(index, 1);
    }
  };

  canvas.addEventListener('pointerup', removePointer);
  canvas.addEventListener('pointercancel', removePointer);
  canvas.addEventListener('pointerout', removePointer);
  canvas.addEventListener('pointerleave', removePointer);

  // Middle-click pan OR Ctrl+left-click pan (for laptops without middle button)
  canvas.addEventListener('mousedown', (e) => {
    if (e.button === 1 || (e.button === 0 && e.ctrlKey)) {
      state.isPanning = true;
      state.lastX = e.clientX;
      state.lastY = e.clientY;
      e.preventDefault();
      canvas.style.cursor = 'grabbing';
    }
  });

  canvas.addEventListener('mousemove', (e) => {
    if (state.isPanning) {
      const dx = e.clientX - state.lastX;
      const dy = e.clientY - state.lastY;
      state.viewport = pan(state.viewport, dx, dy);
      state.lastX = e.clientX;
      state.lastY = e.clientY;
      state.dirty = true;
      state.onViewportChange(state.viewport);
    }
  });

  canvas.addEventListener('mouseup', () => {
    if (state.isPanning) {
      state.isPanning = false;
      canvas.style.cursor = 'default';
    }
  });

  canvas.addEventListener('mouseleave', () => {
    if (state.isPanning) {
      state.isPanning = false;
      canvas.style.cursor = 'default';
    }
  });

  // --- DRC preview checker (debounced, used during routing) ---
  let drcChecker: ReturnType<typeof createDrcPreviewChecker> | null = null;

  function ensureDrcChecker(): ReturnType<typeof createDrcPreviewChecker> {
    if (!drcChecker && state.engine) {
      drcChecker = createDrcPreviewChecker(state.engine, 100);
      drcChecker.onViolations((violations) => {
        state.routing = setDrcViolations(state.routing, violations);
        state.onRoutingChange(state.routing);
        state.dirty = true;
      });
    }
    return drcChecker!;
  }

  // Pad hit tolerance — generous for easy targeting (1mm in nm, plus pixel tolerance)
  const PAD_HIT_TOLERANCE_NM = 500_000; // 0.5mm extra

  // Left-click selection (but not if Ctrl held - that's pan)
  canvas.addEventListener('click', (e) => {
    if (e.button !== 0 || e.ctrlKey) return; // Left click only, no Ctrl

    const rect = canvas.getBoundingClientRect();
    const screenX = e.clientX - rect.left;
    const screenY = e.clientY - rect.top;
    const [worldX, worldY] = screenToWorld(state.viewport, screenX, screenY);

    // ---- Routing state machine: click handling ----
    if (state.routing.mode === 'routing') {
      // While routing: check if we clicked a pad to complete, or empty space for waypoint
      const padHit = hitTestPad(state.snapshot, worldX, worldY, PAD_HIT_TOLERANCE_NM);

      if (padHit) {
        // Complete the route
        const result = completeRoute(state.routing, padHit);
        if (result && state.engine) {
          // Build flat segment array
          const flat: number[] = [];
          for (const s of result.segments) {
            flat.push(Math.round(s.start_x), Math.round(s.start_y), Math.round(s.end_x), Math.round(s.end_y));
          }
          const traceId = state.engine.add_trace(result.netName, result.layer, result.width, flat);
          if (traceId !== 0xFFFFFFFF) {
            console.log(`[Route] Trace added: id=${traceId} net=${result.netName}`);
            // Run DRC after adding
            state.engine.run_drc_incremental();
          } else {
            console.warn('[Route] Failed to add trace');
          }
        }
        // Reset to idle
        state.routing = createRoutingState();
        if (drcChecker) drcChecker.cancel();
        state.onRoutingChange(state.routing);
        state.dirty = true;
        return;
      }

      // No pad hit — add waypoint
      state.routing = addWaypoint(state.routing);
      state.onRoutingChange(state.routing);
      state.dirty = true;
      return;
    }

    // ---- Idle mode: check pads first (to start routing), then traces, then components ----

    // Hit-test pads to start routing
    const padHit = hitTestPad(state.snapshot, worldX, worldY, PAD_HIT_TOLERANCE_NM);
    if (padHit && padHit.netName) {
      state.routing = startRoute(state.routing, padHit);
      ensureDrcChecker();
      state.onRoutingChange(state.routing);
      state.dirty = true;
      return;
    }

    // Try trace hit-test
    const hit = hitTestTrace(state.snapshot, state.viewport, screenX, screenY);
    if (hit) {
      state.selectedTraceId = hit.trace.id;
      state.onTraceSelect(hit.trace.id, e.clientX, e.clientY);
      console.log('[Trace] Selected:', hit.trace.net_name, 'id:', hit.trace.id, 'seg:', hit.segmentIndex);
      state.dirty = true;
      return;
    }

    // No trace hit — deselect trace and fall through to component selection
    if (state.selectedTraceId !== null) {
      state.selectedTraceId = null;
      state.onTraceSelect(null, 0, 0);
      state.dirty = true;
    }

    state.onSelect(worldX, worldY);
  });

  // Hover tracking for traces + routing preview (rAF-guarded)
  let hoverRafPending = false;
  canvas.addEventListener('mousemove', (e) => {
    if (state.isPanning || hoverRafPending) return;

    hoverRafPending = true;
    requestAnimationFrame(() => {
      hoverRafPending = false;
      const rect = canvas.getBoundingClientRect();
      const screenX = e.clientX - rect.left;
      const screenY = e.clientY - rect.top;
      const [worldX, worldY] = screenToWorld(state.viewport, screenX, screenY);

      // --- Routing preview update ---
      if (state.routing.mode === 'routing') {
        state.routing = updatePreview(state.routing, { x: worldX, y: worldY });
        state.onRoutingChange(state.routing);

        // Schedule debounced DRC check
        const checker = ensureDrcChecker();
        if (checker) checker.check(state.routing);

        state.dirty = true;
        canvas.style.cursor = 'crosshair';
        return;
      }

      // --- Normal hover for traces ---
      const hit = hitTestTrace(state.snapshot, state.viewport, screenX, screenY);
      const newHovered = hit ? hit.trace.id : null;

      if (newHovered !== state.hoveredTraceId) {
        state.hoveredTraceId = newHovered;
        state.onTraceHover(newHovered);
        state.dirty = true;

        // Update cursor
        canvas.style.cursor = newHovered != null ? 'pointer' : 'default';
      }
    });
  });

  canvas.addEventListener('mouseleave', () => {
    if (state.hoveredTraceId !== null) {
      state.hoveredTraceId = null;
      state.onTraceHover(null);
      state.dirty = true;
    }
  });

  // Prevent context menu on right-click (reserve for future)
  canvas.addEventListener('contextmenu', (e) => e.preventDefault());
}

/**
 * Create initial interaction state
 */
export function createInteractionState(
  viewport: Viewport,
  onSelect: (x_nm: number, y_nm: number) => void,
  onViewportChange: (vp: Viewport) => void
): InteractionState {
  return {
    viewport,
    isPanning: false,
    lastX: 0,
    lastY: 0,
    dirty: false,
    onSelect,
    onViewportChange,
    snapshot: null,
    selectedTraceId: null,
    hoveredTraceId: null,
    onTraceSelect: () => {},
    onTraceHover: () => {},
    routing: createRoutingState(),
    engine: null,
    onRoutingChange: () => {},
  };
}
