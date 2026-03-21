/**
 * Mouse interaction handlers for PCB viewer
 * Provides zoom, pan, and selection behaviors
 */

import type { Viewport } from './viewport';
import type { BoardSnapshot, TraceSegmentInfo } from './types';
import type { PcbEngine } from './wasm';
import type { RoutingState, PadHit } from './routing';
import { checkRouteObstacles } from './routing';
import { zoomAtPoint, pan, screenToWorld } from './viewport';
import { hitTestTrace } from './hit-test';

/** Nearest point on segment AB to point P */
function nearestPointOnSeg(px: number, py: number, ax: number, ay: number, bx: number, by: number): Vec2 {
  const dx = bx - ax, dy = by - ay;
  const lenSq = dx * dx + dy * dy;
  if (lenSq < 1) return { x: ax, y: ay };
  const t = Math.max(0, Math.min(1, ((px - ax) * dx + (py - ay) * dy) / lenSq));
  return { x: ax + t * dx, y: ay + t * dy };
}
import { hitTestTraceSegment, dragSegment, dragCorner, tracesInRect, componentsInRect, type TraceSegmentHit } from './trace-edit';
import type { Vec2 } from './direction45';
import {
  createRoutingState,
  hitTestPad,
  startRoute,
  updatePreview,
  addWaypoint,
  completeRoute,
  cancelRoute,
  flipLayer,
  toggleAngleSnap,
  flipPosture,
  toggleCornerMode,
  resetToIdle,
  setDrcViolations,
  createDrcPreviewChecker,
  computeTargetPads,
} from './routing';
import { hitTestResizeHandle, resizeHandleCursor, type ResizeHandle } from './renderer';

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
  /** Pad-to-net map for collision detection */
  padNetMap?: Map<string, string>;
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
  /** Callback when a trace is added via routing (for undo stack integration) */
  onTraceAdd?: (netName: string, layer: string, width: number, segments: number[]) => void;
  /** Callback when board is resized via drag handles (for undo stack integration) */
  onBoardResize?: (oldW: number, oldH: number, newW: number, newH: number) => void;
  /** Currently active resize handle (visual feedback for renderer) */
  activeResizeHandle?: ResizeHandle | null;
  /** Callback when routing starts — passes net name for highlightedNet */
  onRouteStart?: (netName: string) => void;
  /** Callback when routing ends (complete or cancel) — clear highlightedNet */
  onRouteEnd?: () => void;
  /** Callback when a trace is edited via drag (segment/corner move, for undo stack) */
  onTraceEdit?: (oldTraceId: number, netName: string, layer: string, width: number, oldSegments: number[], newSegments: number[]) => void;
  /** Callback when rectangle selection completes */
  onRectSelect?: (traceIds: number[], componentRefdes: string[]) => void;
  /** Callback to optimize/simplify a trace (Ctrl+L) */
  onTraceOptimize?: (traceId: number) => void;
  /** Current drag-editing state for rendering preview */
  dragEdit: DragEditState | null;
  /** Current rectangle selection state for rendering */
  rectSelect: RectSelectState | null;
}

/** State for trace segment/corner drag editing */
export interface DragEditState {
  hit: TraceSegmentHit;
  originalSegments: TraceSegmentInfo[];
  previewSegments: TraceSegmentInfo[] | null;
  isCornerDrag: boolean;
  /** True if preview collides with pads of other nets */
  hasCollision?: boolean;
}

/** State for rectangle selection */
export interface RectSelectState {
  /** Start point in world coordinates */
  startWorld: Vec2;
  /** Current point in world coordinates */
  currentWorld: Vec2;
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
/**
 * Find a trace segment at a given world point (for trace snap completion).
 */
function findTraceAtPoint(
  snapshot: BoardSnapshot | null,
  pt: Vec2,
  netName: string,
): { trace: import('./types').TraceInfo; segmentIndex: number } | null {
  if (!snapshot?.traces) return null;
  const tolerance = 200_000; // 0.2mm
  for (const trace of snapshot.traces) {
    if (trace.net_name !== netName) continue;
    for (let i = 0; i < trace.segments.length; i++) {
      const seg = trace.segments[i];
      const sx = Number(seg.start_x), sy = Number(seg.start_y);
      const ex = Number(seg.end_x), ey = Number(seg.end_y);
      const dx = ex - sx, dy = ey - sy;
      const lenSq = dx * dx + dy * dy;
      if (lenSq < 1) continue;
      const t = Math.max(0, Math.min(1, ((pt.x - sx) * dx + (pt.y - sy) * dy) / lenSq));
      const nx = sx + t * dx, ny = sy + t * dy;
      if (Math.hypot(pt.x - nx, pt.y - ny) <= tolerance) {
        return { trace, segmentIndex: i };
      }
    }
  }
  return null;
}

/**
 * Split an existing trace at a junction point — KiCad SplitAdjacentSegments.
 * Removes the original trace and adds two new traces:
 * one from trace start to junction, one from junction to trace end.
 */
function splitTraceAtPoint(
  engine: PcbEngine,
  trace: import('./types').TraceInfo,
  segIdx: number,
  junctionPt: Vec2,
  onTraceAdd?: (net: string, layer: string, width: number, segs: number[]) => void,
): void {
  const seg = trace.segments[segIdx];
  const sx = Number(seg.start_x), sy = Number(seg.start_y);
  const ex = Number(seg.end_x), ey = Number(seg.end_y);
  const jx = Math.round(junctionPt.x), jy = Math.round(junctionPt.y);

  // Don't split if junction is at segment endpoint
  if ((Math.abs(jx - sx) < 100 && Math.abs(jy - sy) < 100) ||
      (Math.abs(jx - ex) < 100 && Math.abs(jy - ey) < 100)) {
    return;
  }

  // Remove original trace
  engine.remove_trace(trace.id);

  const net = trace.net_name || '';
  const layer = trace.layer || 'Top';
  const width = Number(trace.width);

  // Build segments BEFORE junction
  const segsBefore: number[] = [];
  for (let i = 0; i <= segIdx; i++) {
    const s = trace.segments[i];
    if (i < segIdx) {
      segsBefore.push(Math.round(Number(s.start_x)), Math.round(Number(s.start_y)),
                       Math.round(Number(s.end_x)), Math.round(Number(s.end_y)));
    } else {
      // Last segment before junction: start → junction
      segsBefore.push(Math.round(Number(s.start_x)), Math.round(Number(s.start_y)), jx, jy);
    }
  }

  // Build segments AFTER junction
  const segsAfter: number[] = [];
  for (let i = segIdx; i < trace.segments.length; i++) {
    const s = trace.segments[i];
    if (i === segIdx) {
      // First segment after junction: junction → end
      segsAfter.push(jx, jy, Math.round(Number(s.end_x)), Math.round(Number(s.end_y)));
    } else {
      segsAfter.push(Math.round(Number(s.start_x)), Math.round(Number(s.start_y)),
                      Math.round(Number(s.end_x)), Math.round(Number(s.end_y)));
    }
  }

  // Add both halves
  if (segsBefore.length >= 4) {
    if (onTraceAdd) onTraceAdd(net, layer, width, segsBefore);
    else engine.add_trace(net, layer, width, segsBefore);
  }
  if (segsAfter.length >= 4) {
    if (onTraceAdd) onTraceAdd(net, layer, width, segsAfter);
    else engine.add_trace(net, layer, width, segsAfter);
  }
}

export function setupInteraction(
  canvas: HTMLCanvasElement,
  state: InteractionState
): void {
  // Pointer cache for multi-touch pan detection
  const pointerCache: Array<{ pointerId: number; clientX: number; clientY: number }> = [];

  // Resize handle drag state
  let resizeDrag: {
    handle: ResizeHandle;
    startScreenX: number;
    startScreenY: number;
    origWidth: number;
    origHeight: number;
  } | null = null;

  // Trace drag edit state (segment or corner drag)
  let traceDrag: {
    hit: TraceSegmentHit;
    originalSegments: TraceSegmentInfo[];
    isCornerDrag: boolean;
    startWorldX: number;
    startWorldY: number;
    moved: boolean; // true once mouse moves beyond dead zone
  } | null = null;

  // Rectangle selection state
  let rectDrag: {
    startWorldX: number;
    startWorldY: number;
    startScreenX: number;
    startScreenY: number;
    active: boolean; // true once mouse moves beyond dead zone
  } | null = null;

  const DRAG_DEAD_ZONE_PX = 4; // pixels before drag activates

  const MIN_BOARD_SIZE = 5_000_000; // 5mm minimum in nm

  // Wheel zoom (zoom to cursor position)
  canvas.addEventListener('wheel', (e) => {
    e.preventDefault();
    const rect = canvas.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;

    // Scale zoom intensity by deltaY magnitude.
    // Touchpads send small deltas (±2–10), mice send large (±100).
    // Clamp to avoid extreme jumps. Matches Three.js OrbitControls feel.
    const delta = Math.max(-150, Math.min(150, e.deltaY));
    const zoomSpeed = 0.0018;
    const factor = Math.pow(0.95, delta * zoomSpeed * 10);

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
  // Also: left-click on resize handle starts board resize drag
  //        left-click on trace segment/corner starts trace drag
  //        left-click on empty space starts rectangle selection
  canvas.addEventListener('mousedown', (e) => {
    // Resize handle drag (left-click, not during routing)
    if (e.button === 0 && !e.ctrlKey && state.routing.mode !== 'routing' && state.snapshot?.board) {
      const rect = canvas.getBoundingClientRect();
      const sx = e.clientX - rect.left;
      const sy = e.clientY - rect.top;
      const handle = hitTestResizeHandle(state.viewport, state.snapshot.board.width_nm, state.snapshot.board.height_nm, sx, sy);
      if (handle) {
        resizeDrag = {
          handle,
          startScreenX: e.clientX,
          startScreenY: e.clientY,
          origWidth: state.snapshot.board.width_nm,
          origHeight: state.snapshot.board.height_nm,
        };
        state.activeResizeHandle = handle;
        state.dirty = true;
        canvas.style.cursor = resizeHandleCursor(handle);
        e.preventDefault();
        return; // Don't fall through to pan
      }

      // Trace segment/corner drag detection
      {
        const [worldX, worldY] = screenToWorld(state.viewport, sx, sy);
        const toleranceNm = 5 / state.viewport.scale;
        const segHit = hitTestTraceSegment(state.snapshot, worldX, worldY, toleranceNm);

        if (segHit && !segHit.trace.locked) {
          // Start potential trace drag (confirmed after dead zone)
          traceDrag = {
            hit: segHit,
            originalSegments: [...segHit.trace.segments],
            isCornerDrag: segHit.nearCorner,
            startWorldX: worldX,
            startWorldY: worldY,
            moved: false,
          };
          e.preventDefault();
          return;
        }

        // No trace/resize hit — check if we're on a pad (route start) or component
        const padHit = hitTestPad(state.snapshot, worldX, worldY, 500_000);
        if (!padHit) {
          // Empty space: start potential rectangle selection
          rectDrag = {
            startWorldX: worldX,
            startWorldY: worldY,
            startScreenX: e.clientX,
            startScreenY: e.clientY,
            active: false,
          };
          // Don't preventDefault — let click handler work if no drag happens
        }
      }
    }

    if (e.button === 1 || (e.button === 0 && e.ctrlKey)) {
      state.isPanning = true;
      state.lastX = e.clientX;
      state.lastY = e.clientY;
      e.preventDefault();
      canvas.style.cursor = 'grabbing';
    }
  });

  canvas.addEventListener('mousemove', (e) => {
    // Resize handle drag
    if (resizeDrag && state.snapshot?.board) {
      const dxScreen = e.clientX - resizeDrag.startScreenX;
      const dyScreen = e.clientY - resizeDrag.startScreenY;
      // Convert screen delta to world delta (Y is inverted)
      const dxWorld = dxScreen / state.viewport.scale;
      const dyWorld = -dyScreen / state.viewport.scale; // invert for world coords

      let newW = resizeDrag.origWidth;
      let newH = resizeDrag.origHeight;
      const h = resizeDrag.handle;

      // East handles: width increases with rightward drag
      if (h === 'e' || h === 'ne' || h === 'se') {
        newW = resizeDrag.origWidth + dxWorld;
      }
      // West handles: width increases with leftward drag
      if (h === 'w' || h === 'nw' || h === 'sw') {
        newW = resizeDrag.origWidth - dxWorld;
      }
      // North handles: height increases with upward drag (negative screen dy → positive world dy)
      if (h === 'n' || h === 'nw' || h === 'ne') {
        newH = resizeDrag.origHeight + dyWorld;
      }
      // South handles: height increases with downward drag
      if (h === 's' || h === 'sw' || h === 'se') {
        newH = resizeDrag.origHeight - dyWorld;
      }

      // Clamp to minimum
      newW = Math.max(MIN_BOARD_SIZE, Math.round(newW));
      newH = Math.max(MIN_BOARD_SIZE, Math.round(newH));

      // Live-preview: update board size via engine directly (will push undo on mouseup)
      if (state.engine) {
        state.engine.set_board_size(newW, newH);
        // Update snapshot for re-render
        state.snapshot = state.engine.get_snapshot();
      }
      state.dirty = true;
      return;
    }

    // Trace segment/corner drag
    if (traceDrag) {
      const rect = canvas.getBoundingClientRect();
      const sx = e.clientX - rect.left;
      const sy = e.clientY - rect.top;

      // Check dead zone
      if (!traceDrag.moved) {
        const [worldX, worldY] = screenToWorld(state.viewport, sx, sy);
        const worldDist = Math.hypot(worldX - traceDrag.startWorldX, worldY - traceDrag.startWorldY);
        const screenDist = worldDist * state.viewport.scale;
        if (screenDist < DRAG_DEAD_ZONE_PX) return;
        traceDrag.moved = true;
        canvas.style.cursor = 'move';
      }

      const [worldX, worldY] = screenToWorld(state.viewport, sx, sy);
      const newPos: Vec2 = { x: worldX, y: worldY };

      let previewSegs: TraceSegmentInfo[] | null;
      if (traceDrag.isCornerDrag && traceDrag.hit.cornerIndex !== undefined) {
        previewSegs = dragCorner(traceDrag.originalSegments, traceDrag.hit.cornerIndex, newPos);
      } else {
        previewSegs = dragSegment(traceDrag.originalSegments, traceDrag.hit.segmentIndex, newPos);
      }

      // Check if new segments collide with pads of other nets
      let hasCollision = false;
      if (previewSegs && state.snapshot && state.padNetMap) {
        const traceNet = traceDrag.hit.trace.net_name || '';
        const path: Vec2[] = previewSegs.length > 0
          ? [{ x: previewSegs[0].start_x, y: previewSegs[0].start_y },
             ...previewSegs.map(s => ({ x: s.end_x, y: s.end_y }))]
          : [];
        if (path.length >= 2) {
          const obstacles = checkRouteObstacles(path, state.snapshot, traceNet, 150_000, Number(traceDrag.hit.trace.width), state.padNetMap);
          hasCollision = obstacles.length > 0;
        }
      }

      state.dragEdit = {
        hit: traceDrag.hit,
        originalSegments: traceDrag.originalSegments,
        previewSegments: hasCollision ? null : previewSegs, // null = red/invalid
        isCornerDrag: traceDrag.isCornerDrag,
        hasCollision,
      };
      state.dirty = true;
      return;
    }

    // Rectangle selection drag
    if (rectDrag) {
      const rect = canvas.getBoundingClientRect();
      const sx = e.clientX - rect.left;
      const sy = e.clientY - rect.top;

      // Check dead zone
      if (!rectDrag.active) {
        const screenDist = Math.hypot(e.clientX - rectDrag.startScreenX, e.clientY - rectDrag.startScreenY);
        if (screenDist < DRAG_DEAD_ZONE_PX) return;
        rectDrag.active = true;
        canvas.style.cursor = 'crosshair';
      }

      const [worldX, worldY] = screenToWorld(state.viewport, sx, sy);
      state.rectSelect = {
        startWorld: { x: rectDrag.startWorldX, y: rectDrag.startWorldY },
        currentWorld: { x: worldX, y: worldY },
      };
      state.dirty = true;
      return;
    }

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
    // Set suppress flag before clearing drag state
    if (traceDrag?.moved || rectDrag?.active) {
      suppressNextClick = true;
    }

    // Finish resize drag — push undo command with original → final dimensions
    if (resizeDrag && state.snapshot?.board) {
      const finalW = state.snapshot.board.width_nm;
      const finalH = state.snapshot.board.height_nm;
      // Only push undo if dimensions actually changed
      if (finalW !== resizeDrag.origWidth || finalH !== resizeDrag.origHeight) {
        // Revert to original so the undo command's execute() applies the change
        if (state.engine) {
          state.engine.set_board_size(resizeDrag.origWidth, resizeDrag.origHeight);
        }
        if (state.onBoardResize) {
          state.onBoardResize(resizeDrag.origWidth, resizeDrag.origHeight, finalW, finalH);
        }
      }
      resizeDrag = null;
      state.activeResizeHandle = null;
      canvas.style.cursor = 'default';
      state.dirty = true;
    }

    // Finish trace drag — commit via undo stack
    if (traceDrag) {
      if (traceDrag.moved && state.dragEdit?.previewSegments && !state.dragEdit?.hasCollision) {
        const hit = traceDrag.hit;
        const oldSegs = traceDrag.originalSegments;
        const newSegs = state.dragEdit.previewSegments;

        // Build flat segment arrays for undo command
        const oldFlat: number[] = [];
        for (const s of oldSegs) {
          oldFlat.push(Math.round(s.start_x), Math.round(s.start_y), Math.round(s.end_x), Math.round(s.end_y));
        }
        const newFlat: number[] = [];
        for (const s of newSegs) {
          newFlat.push(Math.round(s.start_x), Math.round(s.start_y), Math.round(s.end_x), Math.round(s.end_y));
        }

        if (state.onTraceEdit) {
          state.onTraceEdit(
            hit.traceId,
            hit.trace.net_name,
            hit.trace.layer,
            hit.trace.width,
            oldFlat,
            newFlat,
          );
        }

        console.log(`[TraceDrag] Committed ${traceDrag.isCornerDrag ? 'corner' : 'segment'} drag on trace ${hit.traceId}`);
      }
      traceDrag = null;
      state.dragEdit = null;
      canvas.style.cursor = 'default';
      state.dirty = true;
    }

    // Finish rectangle selection
    if (rectDrag) {
      if (rectDrag.active && state.rectSelect) {
        const { startWorld, currentWorld } = state.rectSelect;
        const traceIds = tracesInRect(state.snapshot, startWorld.x, startWorld.y, currentWorld.x, currentWorld.y);
        const compIds = componentsInRect(state.snapshot, startWorld.x, startWorld.y, currentWorld.x, currentWorld.y);

        if (state.onRectSelect && (traceIds.length > 0 || compIds.length > 0)) {
          state.onRectSelect(traceIds, compIds);
        }
        console.log(`[RectSelect] ${traceIds.length} traces, ${compIds.length} components`);
      }
      rectDrag = null;
      state.rectSelect = null;
      canvas.style.cursor = 'default';
      state.dirty = true;
    }

    if (state.isPanning) {
      state.isPanning = false;
      canvas.style.cursor = 'default';
    }
  });

  canvas.addEventListener('mouseleave', () => {
    // Cancel resize on leave — revert to original
    if (resizeDrag) {
      if (state.engine) {
        state.engine.set_board_size(resizeDrag.origWidth, resizeDrag.origHeight);
        state.snapshot = state.engine.get_snapshot();
      }
      resizeDrag = null;
      state.activeResizeHandle = null;
      state.dirty = true;
    }
    // Cancel trace drag on leave
    if (traceDrag) {
      traceDrag = null;
      state.dragEdit = null;
      state.dirty = true;
    }
    // Cancel rect selection on leave
    if (rectDrag) {
      rectDrag = null;
      state.rectSelect = null;
      state.dirty = true;
    }
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

  // Track if a drag just completed (to suppress the click event that follows mouseup)
  let suppressNextClick = false;

  // Left-click selection (but not if Ctrl held - that's pan)
  canvas.addEventListener('click', (e) => {
    if (e.button !== 0 || e.ctrlKey) return; // Left click only, no Ctrl

    // Suppress click if it followed a drag operation
    if (suppressNextClick) {
      suppressNextClick = false;
      return;
    }

    const rect = canvas.getBoundingClientRect();
    const screenX = e.clientX - rect.left;
    const screenY = e.clientY - rect.top;
    const [worldX, worldY] = screenToWorld(state.viewport, screenX, screenY);

    // ---- Routing state machine: click handling ----
    if (state.routing.mode === 'routing') {
      // While routing: check if we clicked a pad to complete, or empty space for waypoint
      const padHit = hitTestPad(state.snapshot, worldX, worldY, PAD_HIT_TOLERANCE_NM);

      if (padHit) {
        // KiCad behavior: only allow completing route on a pad of the SAME net
        if (padHit.netName && padHit.netName === state.routing.netName) {
          // KiCad behavior: block route completion if path has DRC violations
          if (state.routing.hasCollision) {
            console.log('[Route] Cannot complete route — path collides with obstacle');
            return;
          }
          // Complete the route — pad is on the same net, no collisions
          const result = completeRoute(state.routing, padHit);
          if (result && state.engine) {
            // Build flat segment array
            const flat: number[] = [];
            for (const s of result.segments) {
              flat.push(Math.round(s.start_x), Math.round(s.start_y), Math.round(s.end_x), Math.round(s.end_y));
            }
            if (state.onTraceAdd) {
              // Route through undo stack
              state.onTraceAdd(result.netName, result.layer, result.width, flat);
            } else {
              // Fallback: direct engine call
              const traceId = state.engine.add_trace(result.netName, result.layer, result.width, flat);
              if (traceId !== 0xFFFFFFFF) {
                console.log(`[Route] Trace added: id=${traceId} net=${result.netName}`);
                state.engine.run_drc_incremental();
              } else {
                console.warn('[Route] Failed to add trace');
              }
            }
          }
          // Reset to idle (preserve user preferences)
          state.routing = resetToIdle(state.routing);
          if (drcChecker) drcChecker.cancel();
          state.onRoutingChange(state.routing);
          state.onRouteEnd?.();
          state.dirty = true;
          return;
        } else {
          // Clicked a pad on a DIFFERENT net — ignore (KiCad beeps here)
          console.log(`[Route] Cannot connect to pad ${padHit.component.refdes}.${padHit.pad.number} — different net (${padHit.netName} vs ${state.routing.netName})`);
          return;
        }
      }

      // No pad hit — check if we're snapped to a trace (magnetic snap) or clicked near one
      if (!state.routing.hasCollision) {
        // If magnetically snapped to a trace, complete immediately
        const isTraceSnap = state.routing.snappedToPad?.component?.refdes === '__trace__';
        
        // Use snapped position if available, else raw click position
        const checkX = isTraceSnap ? state.routing.snappedToPad!.worldX : worldX;
        const checkY = isTraceSnap ? state.routing.snappedToPad!.worldY : worldY;
        
        const traceHit = hitTestTrace(state.snapshot, state.viewport, screenX, screenY);
        
        if (isTraceSnap || (traceHit && traceHit.trace.net_name === state.routing.netName)) {
          // Get snap point: from magnetic snap or from trace hit
          let snapPt: Vec2;
          if (isTraceSnap) {
            snapPt = { x: state.routing.snappedToPad!.worldX, y: state.routing.snappedToPad!.worldY };
          } else {
            const seg = traceHit!.trace.segments[traceHit!.segmentIndex];
            snapPt = nearestPointOnSeg(worldX, worldY, Number(seg.start_x), Number(seg.start_y), Number(seg.end_x), Number(seg.end_y));
          }
          
          const targetTrace = traceHit ?? (isTraceSnap ? findTraceAtPoint(state.snapshot, snapPt, state.routing.netName) : null);

          // Complete route to this point (create a synthetic target)
          const syntheticTarget: PadHit = {
            component: { refdes: '__trace__', value: '', x_nm: snapPt.x, y_nm: snapPt.y, rotation_mdeg: 0, footprint: '', pads: [], body_width_nm: 0, body_height_nm: 0, model_3d: null, silk: [] },
            pad: { number: '0', x_nm: 0, y_nm: 0, width_nm: 100000, height_nm: 100000, shape: 'rect', layer_mask: 1, drill_nm: 0 },
            worldX: snapPt.x,
            worldY: snapPt.y,
            netName: state.routing.netName,
          };

          const result = completeRoute(state.routing, syntheticTarget);
          if (result && state.engine) {
            const flat: number[] = [];
            for (const s of result.segments) {
              flat.push(Math.round(s.start_x), Math.round(s.start_y), Math.round(s.end_x), Math.round(s.end_y));
            }
            if (state.onTraceAdd) {
              state.onTraceAdd(result.netName, result.layer, result.width, flat);
            }

            // Split the existing trace at the junction point (KiCad SplitAdjacentSegments)
            if (targetTrace) {
              splitTraceAtPoint(state.engine, targetTrace.trace, targetTrace.segmentIndex, snapPt, state.onTraceAdd);
            }
          }

          state.routing = resetToIdle(state.routing);
          if (drcChecker) drcChecker.cancel();
          state.onRoutingChange(state.routing);
          state.onRouteEnd?.();
          console.log(`[Route] Completed to trace (T-junction)`);
          state.dirty = true;
          return;
        }
      }

      // No pad or trace hit — add waypoint (only if no collision)
      if (state.routing.hasCollision) {
        // KiCad behavior: cannot place waypoint when path has DRC violations
        console.log('[Route] Cannot place waypoint — path collides with obstacle');
        return;
      }
      state.routing = addWaypoint(state.routing);
      state.onRoutingChange(state.routing);
      state.dirty = true;
      return;
    }

    // ---- Idle mode: check pads first (to start routing), then traces, then components ----

    // Hit-test pads to start routing
    const padHit = hitTestPad(state.snapshot, worldX, worldY, PAD_HIT_TOLERANCE_NM);
    console.log(`[Click] idle mode: world=(${(worldX/1e6).toFixed(2)}, ${(worldY/1e6).toFixed(2)}) padHit=${padHit ? padHit.component.refdes + '.' + padHit.pad.number + ' net=' + padHit.netName : 'null'}`);
    if (padHit && padHit.netName) {
      state.routing = startRoute(state.routing, padHit, state.snapshot);
      // Read clearance from engine design rules
      if (state.engine) {
        state.routing = { ...state.routing, clearanceNm: state.engine.get_min_clearance_nm() };
      }
      ensureDrcChecker();
      state.onRoutingChange(state.routing);
      state.onRouteStart?.(padHit.netName);
      state.dirty = true;
      return;
    }

    // Try trace hit-test — single click = SELECT, double click = start routing
    const hit = hitTestTrace(state.snapshot, state.viewport, screenX, screenY);
    if (hit) {
      // Single click: just select the trace
      state.selectedTraceId = hit.trace.id;
      state.onTraceSelect(hit.trace.id, e.clientX, e.clientY);
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

  // Hover tracking for traces + routing preview + resize handles (rAF-guarded)
  let hoverRafPending = false;
  canvas.addEventListener('mousemove', (e) => {
    if (state.isPanning || resizeDrag || hoverRafPending) return;

    hoverRafPending = true;
    requestAnimationFrame(() => {
      hoverRafPending = false;
      const rect = canvas.getBoundingClientRect();
      const screenX = e.clientX - rect.left;
      const screenY = e.clientY - rect.top;
      const [worldX, worldY] = screenToWorld(state.viewport, screenX, screenY);

      // --- Routing preview update ---
      if (state.routing.mode === 'routing') {
        state.routing = updatePreview(state.routing, { x: worldX, y: worldY }, state.viewport.scale, state.snapshot, state.padNetMap);
        state.onRoutingChange(state.routing);

        // Schedule debounced DRC check
        const checker = ensureDrcChecker();
        if (checker) checker.check(state.routing);

        state.dirty = true;
        canvas.style.cursor = 'crosshair';
        return;
      }

      // --- Resize handle hover cursor ---
      if (state.snapshot?.board) {
        const handle = hitTestResizeHandle(state.viewport, state.snapshot.board.width_nm, state.snapshot.board.height_nm, screenX, screenY);
        if (handle) {
          canvas.style.cursor = resizeHandleCursor(handle);
          // Still update trace hover to clear if needed
          if (state.hoveredTraceId != null) {
            state.hoveredTraceId = null;
            state.onTraceHover(null);
            state.dirty = true;
          }
          return;
        }
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

  // ---------------------------------------------------------------------------
  // Keyboard handler for routing mode (Escape, F, A)
  // ---------------------------------------------------------------------------
  function isEditorFocused(): boolean {
    const el = document.activeElement;
    if (!el) return false;
    const tag = el.tagName;
    if (tag === 'INPUT' || tag === 'TEXTAREA') return true;
    // Monaco editor uses a textarea with class containing 'monaco'
    if (el.closest?.('.monaco-editor') != null) return true;
    return false;
  }

  function handleKeydown(e: KeyboardEvent): void {
    // Never intercept when user is typing in an editor or input
    if (isEditorFocused()) return;

    // Only handle routing-specific keys when actively routing
    if (state.routing.mode === 'routing') {
      if (e.key === 'Escape') {
        state.routing = cancelRoute(state.routing);
        if (drcChecker) drcChecker.cancel();
        state.onRoutingChange(state.routing);
        state.onRouteEnd?.();
        state.dirty = true;
        e.preventDefault();
        return;
      }

      if (e.key === 'f' || e.key === 'F') {
        state.routing = flipLayer(state.routing);
        state.onRoutingChange(state.routing);
        state.dirty = true;
        e.preventDefault();
        return;
      }

      if (e.key === 'a' || e.key === 'A') {
        state.routing = toggleAngleSnap(state.routing);
        state.onRoutingChange(state.routing);
        state.dirty = true;
        e.preventDefault();
        return;
      }

      // '/' to flip posture (KiCad-style: straight-first ↔ diagonal-first)
      if (e.key === '/') {
        state.routing = flipPosture(state.routing);
        state.onRoutingChange(state.routing);
        state.dirty = true;
        e.preventDefault();
        return;
      }

      // 'q' or 'Q' to toggle corner mode (45° mitered ↔ 90° only)
      if (e.key === 'q' || e.key === 'Q') {
        state.routing = toggleCornerMode(state.routing);
        state.onRoutingChange(state.routing);
        state.dirty = true;
        e.preventDefault();
        return;
      }
    }

    // Ctrl+L: simplify/optimize selected trace (like Inkscape)
    if ((e.ctrlKey || e.metaKey) && e.key === 'l' && state.selectedTraceId != null) {
      e.preventDefault();
      if (state.onTraceOptimize) {
        state.onTraceOptimize(state.selectedTraceId);
        state.dirty = true;
      }
      return;
    }
  }

  document.addEventListener('keydown', handleKeydown);

  // Prevent context menu on right-click (reserve for future)
  canvas.addEventListener('contextmenu', (e) => e.preventDefault());

  // Double-click on trace: start routing from that point (T-junction source)
  canvas.addEventListener('dblclick', (e) => {
    if (e.button !== 0) return;
    if (state.routing.mode === 'routing') return; // already routing

    const rect = canvas.getBoundingClientRect();
    const screenX = e.clientX - rect.left;
    const screenY = e.clientY - rect.top;
    const [worldX, worldY] = screenToWorld(state.viewport, screenX, screenY);

    const hit = hitTestTrace(state.snapshot, state.viewport, screenX, screenY);
    if (!hit || !hit.trace.net_name) return;

    const seg = hit.trace.segments[hit.segmentIndex];
    const snapPt = nearestPointOnSeg(worldX, worldY,
      Number(seg.start_x), Number(seg.start_y),
      Number(seg.end_x), Number(seg.end_y));

    state.routing = {
      ...createRoutingState(),
      mode: 'routing' as const,
      currentLayer: hit.trace.layer || 'Top',
      anchorPoint: { x: snapPt.x, y: snapPt.y },
      netName: hit.trace.net_name,
      traceWidth: Number(hit.trace.width) || 250_000,
      angleSnapEnabled: state.routing.angleSnapEnabled,
      gridSnapEnabled: state.routing.gridSnapEnabled,
      gridSpacing: state.routing.gridSpacing,
      magneticSnapEnabled: state.routing.magneticSnapEnabled,
      magneticSnapRadius: state.routing.magneticSnapRadius,
      cornerMode: state.routing.cornerMode,
      clearanceNm: state.routing.clearanceNm,
    };
    if (state.snapshot) {
      state.routing.targetPads = computeTargetPads(state.snapshot, hit.trace.net_name, '', '');
    }
    ensureDrcChecker();
    state.onRoutingChange(state.routing);
    state.onRouteStart?.(hit.trace.net_name);
    state.dirty = true;
  });
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
    dragEdit: null,
    rectSelect: null,
  };
}
