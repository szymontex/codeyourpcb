/**
 * Canvas 2D rendering functions for PCB visualization
 * Professional-quality renderer with LOD, per-pad net highlighting,
 * component body outlines, pad pin numbers, net labels, and drill marks.
 */

import type { BoardSnapshot, ComponentInfo, PadInfo, ViolationInfo, TraceInfo, ViaInfo, RatsnestInfo } from './types';
import type { Viewport } from './viewport';
import type { RoutingState } from './routing';
import type { RenderConfig } from './render-config';
import { LodTier, getLodTier, createDefaultRenderConfig } from './render-config';
import { worldToScreen, screenToWorld } from './viewport';
import { LAYER_COLORS, getPadColor, getTraceColor, getThemeColors, netColor, brightenColor, colorWithAlpha, type LayerVisibility } from './layers';
import { formatDimension } from './units';
import { getPreference } from './settings';

export interface RenderState {
  snapshot: BoardSnapshot | null;
  viewport: Viewport;
  layers: LayerVisibility;
  selectedRefdes: string | null;
  showViolations: boolean;
  showRatsnest: boolean;
  colorByNet: boolean;
  selectedTraceId: number | null;
  hoveredTraceId: number | null;
  /** Screen position for the net label tooltip (set by interaction layer) */
  labelPosition: { x: number; y: number } | null;
  /** Active routing state (null when not routing) */
  routing: RoutingState | null;
  /** Net name to highlight (all traces/pads on this net glow, others dim) */
  highlightedNet: string | null;
  /** Currently dragged resize handle (for visual feedback) */
  activeResizeHandle?: ResizeHandle | null;
  /** Render configuration (colors, fonts, LOD thresholds) */
  renderConfig?: RenderConfig;
  /** Pad-to-net lookup: "refdes.pin" → net name */
  padNetMap?: Map<string, string>;
  /** Whether the visual grid overlay is drawn (separate from routing grid snap) */
  gridVisible?: boolean;
  /** Visual grid spacing in nanometers */
  gridVisualSpacing?: number;
  /** Whether net labels on traces are shown */
  showNetLabels?: boolean;
  /** Variant preview data: route segments and vias to draw as ghost overlay */
  variantPreview?: VariantPreviewData | null;
}

/** Data for rendering a ghost variant preview overlay */
export interface VariantPreviewData {
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

/** Diagnostic surface exposed on window for E2E and debugging */
export interface RenderDiag {
  lodTier: LodTier;
  padNetMapSize: number;
  lastFrameMs: number;
  textElementsDrawn: number;
  highlightedNet: string | null;
}

// Module-level counter for text elements drawn per frame
let _textElementsDrawn = 0;

/**
 * Main render function - draws entire board state
 */
export function render(ctx: CanvasRenderingContext2D, state: RenderState): void {
  const frameStart = performance.now();
  _textElementsDrawn = 0;

  const { snapshot, viewport, layers, selectedRefdes, showViolations, showRatsnest } = state;
  const config = state.renderConfig ?? createDefaultRenderConfig();
  const padNetMap = state.padNetMap ?? new Map<string, string>();
  const lodTier = getLodTier(viewport.scale, config);

  // Get theme-dependent colors
  const themeColors = getThemeColors();

  // Clear canvas with background color
  ctx.fillStyle = themeColors.background;
  ctx.fillRect(0, 0, viewport.width, viewport.height);

  if (!snapshot || !snapshot.board) {
    drawEmptyState(ctx, viewport, themeColors);
    _updateDiag(lodTier, padNetMap.size, performance.now() - frameStart, state.highlightedNet);
    return;
  }

  // Draw grid (behind everything) — uses gridVisible and gridVisualSpacing from settings
  const gridVisible = state.gridVisible ?? true;
  const gridVisualSpacing = state.gridVisualSpacing ?? 1_000_000;
  if (gridVisible) {
    drawGrid(ctx, viewport, themeColors, gridVisualSpacing);
  }

  // Draw board outline
  drawBoardOutline(ctx, viewport, snapshot.board.width_nm, snapshot.board.height_nm, themeColors);

  // Draw resize handles only when hovering near board edge (not by default)
  // They clutter the professional PCB view
  if (state.activeResizeHandle) {
    drawResizeHandles(ctx, viewport, snapshot.board.width_nm, snapshot.board.height_nm, themeColors, state.activeResizeHandle ?? null);
  }

  // Draw traces by layer (bottom first, then top)
  // When variant preview is active, dim existing traces to 0.3 alpha
  const traceAlpha = state.variantPreview ? 0.3 : 1.0;
  if (snapshot.traces) {
    ctx.save();
    ctx.globalAlpha = traceAlpha;
    const { colorByNet, selectedTraceId, hoveredTraceId, highlightedNet } = state;
    // Bottom traces first
    for (const trace of snapshot.traces) {
      if (trace.layer === 'Bottom' && layers.bottomCopper) {
        drawTrace(ctx, viewport, trace, layers, colorByNet, selectedTraceId, hoveredTraceId, highlightedNet);
      }
    }
    // Top traces on top
    for (const trace of snapshot.traces) {
      if (trace.layer === 'Top' && layers.topCopper) {
        drawTrace(ctx, viewport, trace, layers, colorByNet, selectedTraceId, hoveredTraceId, highlightedNet);
      }
    }
    // Inner layers
    for (const trace of snapshot.traces) {
      if (trace.layer !== 'Top' && trace.layer !== 'Bottom') {
        drawTrace(ctx, viewport, trace, layers, colorByNet, selectedTraceId, hoveredTraceId, highlightedNet);
      }
    }
    ctx.restore();
  }

  // ---- SOLDER MASK OVERLAY ----
  // Draw semi-transparent green solder mask over the board area.
  // Solder mask goes on top of copper traces but before pad/via redraw.
  drawSolderMask(ctx, viewport, snapshot, layers);

  // Draw components (pads, body outlines, drill marks) — ON TOP of solder mask
  // Pads are "exposed copper" visible through solder mask openings
  for (const comp of snapshot.components) {
    const isSelected = comp.refdes === selectedRefdes;
    drawComponent(ctx, viewport, comp, layers, isSelected, themeColors, state.highlightedNet, config, lodTier, padNetMap);
  }

  // Draw vias on top of solder mask (exposed annular rings)
  if (snapshot.vias) {
    for (const via of snapshot.vias) {
      if (layers.topCopper || layers.bottomCopper) {
        drawVia(ctx, viewport, via, themeColors);
      }
    }
  }

  // Draw ratsnest on top of everything (except violations)
  if (showRatsnest && snapshot.ratsnest) {
    for (const line of snapshot.ratsnest) {
      drawRatsnest(ctx, viewport, line, state.highlightedNet);
    }
  }

  // Draw violations on top of everything (disabled — DRC circles clutter the view
  // and post-routing violations are currently false positives from spatial index
  // mismatch between JS-side traces and WASM world state)
  // if (showViolations && snapshot.violations) {
  //   for (const violation of snapshot.violations) {
  //     drawViolation(ctx, viewport, violation);
  //   }
  // }

  // ---- TEXT PASS (after all shapes, for readability) ----

  // Refdes labels (LOD ≥ Medium)
  if (lodTier >= LodTier.Medium) {
    for (const comp of snapshot.components) {
      const isSelected = comp.refdes === selectedRefdes;
      drawRefdes(ctx, viewport, comp, isSelected, themeColors, config);
    }
  }

  // Pad pin numbers (LOD ≥ Close)
  if (lodTier >= LodTier.Close) {
    for (const comp of snapshot.components) {
      drawPadNumbers(ctx, viewport, comp, layers, config);
    }
  }

  // Net labels on traces (LOD ≥ Close, if enabled)
  const showNetLabels = state.showNetLabels ?? true;
  if (showNetLabels && lodTier >= LodTier.Close && snapshot.traces) {
    drawTraceNetLabels(ctx, viewport, snapshot.traces, layers, config);
  }

  // Draw routing preview (dashed trace + DRC markers)
  if (state.routing && state.routing.mode === 'routing') {
    drawRoutingPreview(ctx, viewport, state.routing);
  }

  // Draw variant ghost preview overlay when hovering a non-active variant
  if (state.variantPreview) {
    drawVariantPreview(ctx, viewport, state.variantPreview);
  }

  // Draw net label for selected trace
  if (state.selectedTraceId != null && state.labelPosition && snapshot.traces) {
    const selectedTrace = snapshot.traces.find(t => t.id === state.selectedTraceId);
    if (selectedTrace) {
      drawNetLabel(ctx, state.labelPosition.x, state.labelPosition.y, selectedTrace);
    }
  }

  const frameMs = performance.now() - frameStart;
  _updateDiag(lodTier, padNetMap.size, frameMs, state.highlightedNet);

  // Warn about slow frames
  if (frameMs > 16) {
    console.warn(`[renderer] Slow frame: ${frameMs.toFixed(1)}ms (LOD=${LodTier[lodTier]}, text=${_textElementsDrawn})`);
  }
}

/** Update the global diagnostic surface */
function _updateDiag(lodTier: LodTier, padNetMapSize: number, lastFrameMs: number, highlightedNet: string | null): void {
  const diag: RenderDiag = {
    lodTier,
    padNetMapSize,
    lastFrameMs,
    textElementsDrawn: _textElementsDrawn,
    highlightedNet,
  };
  (window as any).__renderDiag = diag;
}

// ---------------------------------------------------------------------------
// Empty state
// ---------------------------------------------------------------------------

function drawEmptyState(ctx: CanvasRenderingContext2D, viewport: Viewport, themeColors: ReturnType<typeof getThemeColors>): void {
  ctx.fillStyle = themeColors.empty_text;
  ctx.font = '16px system-ui';
  ctx.textAlign = 'center';
  ctx.fillText('No board loaded', viewport.width / 2, viewport.height / 2);
}

// ---------------------------------------------------------------------------
// Grid
// ---------------------------------------------------------------------------

function drawGrid(ctx: CanvasRenderingContext2D, vp: Viewport, themeColors: ReturnType<typeof getThemeColors>, gridSpacing = 1_000_000): void {
  const screenSpacing = gridSpacing * vp.scale;
  if (screenSpacing < 10) return;

  const [minX, maxY] = screenToWorld(vp, 0, 0);
  const [maxX, minY] = screenToWorld(vp, vp.width, vp.height);

  const startX = Math.floor(minX / gridSpacing) * gridSpacing;
  const startY = Math.floor(minY / gridSpacing) * gridSpacing;

  // Use dots instead of lines for a professional look
  const dotRadius = screenSpacing > 40 ? 1.5 : 1;
  ctx.fillStyle = themeColors.grid;

  for (let x = startX; x <= maxX; x += gridSpacing) {
    for (let y = startY; y <= maxY; y += gridSpacing) {
      const [sx, sy] = worldToScreen(vp, x, y);
      ctx.fillRect(sx - dotRadius, sy - dotRadius, dotRadius * 2, dotRadius * 2);
    }
  }
}

// ---------------------------------------------------------------------------
// Board outline + resize handles
// ---------------------------------------------------------------------------

function drawBoardOutline(ctx: CanvasRenderingContext2D, vp: Viewport, width: number, height: number, themeColors: ReturnType<typeof getThemeColors>): void {
  const [x0, y0] = worldToScreen(vp, 0, 0);
  const [x1, y1] = worldToScreen(vp, width, height);
  const w = x1 - x0;
  const h = y0 - y1;

  // Board substrate fill (FR4 tan/brown)
  ctx.fillStyle = LAYER_COLORS.board_substrate;
  ctx.fillRect(x0, y1, w, h);

  // Board edge outline — thin yellow line
  ctx.strokeStyle = themeColors.board_outline;
  ctx.lineWidth = 1.5;
  ctx.strokeRect(x0, y1, w, h);
}

export type ResizeHandle =
  | 'nw' | 'n' | 'ne'
  | 'w'  |       'e'
  | 'sw' | 's' | 'se';

const HANDLE_SIZE = 8;

export function drawResizeHandles(
  ctx: CanvasRenderingContext2D,
  vp: Viewport,
  width: number,
  height: number,
  themeColors: ReturnType<typeof getThemeColors>,
  activeHandle: ResizeHandle | null,
): void {
  const handles = computeHandlePositions(vp, 0, 0, width, height);
  const hs = HANDLE_SIZE;
  for (const h of handles) {
    const isActive = h.id === activeHandle;
    ctx.fillStyle = isActive ? '#FF6600' : themeColors.board_outline;
    ctx.strokeStyle = themeColors.background;
    ctx.lineWidth = 1;
    ctx.fillRect(h.cx - hs, h.cy - hs, hs * 2, hs * 2);
    ctx.strokeRect(h.cx - hs, h.cy - hs, hs * 2, hs * 2);
  }
}

function computeHandlePositions(
  vp: Viewport, originX: number, originY: number, width: number, height: number,
): { id: ResizeHandle; cx: number; cy: number }[] {
  const [x0, y0] = worldToScreen(vp, originX, originY);
  const [x1, y1] = worldToScreen(vp, width, height);
  const left = x0, right = x1, top = y1, bottom = y0;
  const midX = (left + right) / 2, midY = (top + bottom) / 2;
  return [
    { id: 'nw', cx: left,  cy: top },
    { id: 'n',  cx: midX,  cy: top },
    { id: 'ne', cx: right, cy: top },
    { id: 'w',  cx: left,  cy: midY },
    { id: 'e',  cx: right, cy: midY },
    { id: 'sw', cx: left,  cy: bottom },
    { id: 's',  cx: midX,  cy: bottom },
    { id: 'se', cx: right, cy: bottom },
  ];
}

export function hitTestResizeHandle(
  vp: Viewport, boardWidth: number, boardHeight: number, screenX: number, screenY: number,
): ResizeHandle | null {
  const handles = computeHandlePositions(vp, 0, 0, boardWidth, boardHeight);
  const tolerance = HANDLE_SIZE + 4;
  for (const h of handles) {
    if (Math.abs(screenX - h.cx) <= tolerance && Math.abs(screenY - h.cy) <= tolerance) {
      return h.id;
    }
  }
  return null;
}

export function resizeHandleCursor(handle: ResizeHandle): string {
  switch (handle) {
    case 'n': case 's': return 'ns-resize';
    case 'e': case 'w': return 'ew-resize';
    case 'nw': case 'se': return 'nwse-resize';
    case 'ne': case 'sw': return 'nesw-resize';
  }
}

// ---------------------------------------------------------------------------
// Violations
// ---------------------------------------------------------------------------

function drawViolation(ctx: CanvasRenderingContext2D, vp: Viewport, violation: ViolationInfo): void {
  const [sx, sy] = worldToScreen(vp, violation.x_nm, violation.y_nm);
  const radius = 15;
  const innerRadius = 10;

  ctx.beginPath();
  ctx.arc(sx, sy, radius, 0, Math.PI * 2);
  ctx.strokeStyle = LAYER_COLORS.violation_ring;
  ctx.lineWidth = 3;
  ctx.stroke();

  ctx.beginPath();
  ctx.arc(sx, sy, innerRadius, 0, Math.PI * 2);
  ctx.fillStyle = 'rgba(255, 0, 0, 0.3)';
  ctx.fill();
}

// ---------------------------------------------------------------------------
// Traces
// ---------------------------------------------------------------------------

function tracePolyline(ctx: CanvasRenderingContext2D, vp: Viewport, trace: TraceInfo): void {
  ctx.beginPath();
  const firstSeg = trace.segments[0];
  const [startX, startY] = worldToScreen(vp, firstSeg.start_x, firstSeg.start_y);
  ctx.moveTo(startX, startY);
  for (const seg of trace.segments) {
    const [endX, endY] = worldToScreen(vp, seg.end_x, seg.end_y);
    ctx.lineTo(endX, endY);
  }
}

function drawTrace(
  ctx: CanvasRenderingContext2D, vp: Viewport, trace: TraceInfo, layers: LayerVisibility,
  colorByNet: boolean, selectedTraceId: number | null, hoveredTraceId: number | null,
  highlightedNet: string | null,
): void {
  if (trace.segments.length === 0) return;

  const isHighlightedNet = highlightedNet != null && trace.net_name === highlightedNet;
  const isDimmed = highlightedNet != null && !isHighlightedNet;

  let color: string | null;
  if (colorByNet && trace.net_name) {
    const layerVisible = getTraceColor(trace.layer, layers) !== null;
    if (!layerVisible) return;
    color = netColor(trace.net_name);
  } else {
    color = getTraceColor(trace.layer, layers);
  }
  if (!color) return;

  if (isDimmed) {
    color = colorWithAlpha(color, 0.15);
  }

  const isSelected = trace.id === selectedTraceId;
  const isHovered = trace.id === hoveredTraceId && !isSelected;
  const baseLineWidth = trace.width * vp.scale;
  if (baseLineWidth < 0.5) return;

  ctx.lineCap = 'round';
  ctx.lineJoin = 'round';

  // Selection glow
  if (isSelected) {
    ctx.strokeStyle = colorWithAlpha(brightenColor(color, 25), 0.35);
    ctx.lineWidth = baseLineWidth * 2.5;
    tracePolyline(ctx, vp, trace);
    ctx.stroke();
  }

  // Net highlight glow
  if (isHighlightedNet && !isSelected) {
    ctx.strokeStyle = colorWithAlpha(brightenColor(color, 20), 0.3);
    ctx.lineWidth = baseLineWidth * 2.0;
    tracePolyline(ctx, vp, trace);
    ctx.stroke();
  }

  // Main stroke
  const drawColor = isSelected ? brightenColor(color, 20) : isHighlightedNet ? brightenColor(color, 15) : color;
  const lineWidth = isSelected ? baseLineWidth * 1.5 : isHighlightedNet ? baseLineWidth * 1.2 : baseLineWidth;
  ctx.strokeStyle = drawColor;
  ctx.lineWidth = lineWidth;
  tracePolyline(ctx, vp, trace);
  ctx.stroke();

  // Hover overlay
  if (isHovered) {
    ctx.strokeStyle = 'rgba(255, 255, 255, 0.25)';
    ctx.lineWidth = baseLineWidth;
    tracePolyline(ctx, vp, trace);
    ctx.stroke();
  }

  // Locked indicator
  if (trace.locked && baseLineWidth > 2) {
    ctx.save();
    ctx.setLineDash([5, 5]);
    ctx.strokeStyle = 'rgba(255, 255, 255, 0.4)';
    ctx.lineWidth = Math.max(1, baseLineWidth * 0.3);
    tracePolyline(ctx, vp, trace);
    ctx.stroke();
    ctx.restore();
  }
}

// ---------------------------------------------------------------------------
// Trace net labels (text pass, LOD ≥ Close)
// ---------------------------------------------------------------------------

function drawTraceNetLabels(
  ctx: CanvasRenderingContext2D, vp: Viewport, traces: TraceInfo[],
  layers: LayerVisibility, config: RenderConfig,
): void {
  // World-space label size: 0.5mm
  const labelWorldSize = 500_000;
  const fontSize = Math.min(14, Math.max(8, labelWorldSize * vp.scale));
  ctx.font = `${fontSize.toFixed(1)}px system-ui, sans-serif`;
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';

  for (const trace of traces) {
    if (!trace.net_name || trace.segments.length === 0) continue;
    // Check layer visibility
    if (getTraceColor(trace.layer, layers) === null) continue;

    // Find longest segment
    let longestLen = 0;
    let longestIdx = 0;
    for (let i = 0; i < trace.segments.length; i++) {
      const seg = trace.segments[i];
      const [sx1, sy1] = worldToScreen(vp, seg.start_x, seg.start_y);
      const [sx2, sy2] = worldToScreen(vp, seg.end_x, seg.end_y);
      const len = Math.hypot(sx2 - sx1, sy2 - sy1);
      if (len > longestLen) {
        longestLen = len;
        longestIdx = i;
      }
    }

    if (longestLen < config.fontConfig.netLabelMinSegmentPx) continue;

    const seg = trace.segments[longestIdx];
    const [sx1, sy1] = worldToScreen(vp, seg.start_x, seg.start_y);
    const [sx2, sy2] = worldToScreen(vp, seg.end_x, seg.end_y);
    const midX = (sx1 + sx2) / 2;
    const midY = (sy1 + sy2) / 2;

    // Rotate along segment
    let angle = Math.atan2(sy2 - sy1, sx2 - sx1);
    // Keep text upright (avoid upside-down text)
    if (angle > Math.PI / 2) angle -= Math.PI;
    if (angle < -Math.PI / 2) angle += Math.PI;

    ctx.save();
    ctx.translate(midX, midY);
    ctx.rotate(angle);

    // Background pill for readability
    const textMetrics = ctx.measureText(trace.net_name);
    const padX = 3;
    const padY = 2;
    ctx.fillStyle = 'rgba(0, 0, 0, 0.6)';
    ctx.fillRect(
      -textMetrics.width / 2 - padX,
      -fontSize / 2 - padY,
      textMetrics.width + padX * 2,
      fontSize + padY * 2,
    );

    ctx.fillStyle = '#FFFFFF';
    ctx.fillText(trace.net_name, 0, 0);
    ctx.restore();

    _textElementsDrawn++;
  }
}

// ---------------------------------------------------------------------------
// Net label tooltip (for selected trace)
// ---------------------------------------------------------------------------

function drawNetLabel(ctx: CanvasRenderingContext2D, screenX: number, screenY: number, trace: TraceInfo): void {
  const unit = getPreference('units');
  const widthStr = formatDimension(trace.width, unit);
  const label = `${trace.net_name} — ${widthStr}`;

  ctx.font = '12px system-ui, sans-serif';
  const metrics = ctx.measureText(label);
  const padX = 8;
  const boxW = metrics.width + padX * 2;
  const boxH = 20;
  const offsetX = 15;
  const offsetY = -25;
  const x = screenX + offsetX;
  const y = screenY + offsetY;

  ctx.fillStyle = 'rgba(0, 0, 0, 0.75)';
  ctx.beginPath();
  ctx.roundRect(x, y, boxW, boxH, 4);
  ctx.fill();

  ctx.fillStyle = '#ffffff';
  ctx.textAlign = 'left';
  ctx.textBaseline = 'middle';
  ctx.fillText(label, x + padX, y + boxH / 2);
}

// ---------------------------------------------------------------------------
// Vias
// ---------------------------------------------------------------------------

function drawVia(ctx: CanvasRenderingContext2D, vp: Viewport, via: ViaInfo, themeColors: ReturnType<typeof getThemeColors>): void {
  const [sx, sy] = worldToScreen(vp, via.x, via.y);
  const outerRadius = (via.outer_diameter * vp.scale) / 2;
  const drillRadius = (via.drill * vp.scale) / 2;
  if (outerRadius < 1) return;

  // Annular ring (copper pad around drill)
  ctx.beginPath();
  ctx.arc(sx, sy, outerRadius, 0, Math.PI * 2);
  ctx.fillStyle = LAYER_COLORS.via;
  ctx.fill();

  // Drill hole (dark center)
  if (drillRadius > 0.5) {
    ctx.beginPath();
    ctx.arc(sx, sy, drillRadius, 0, Math.PI * 2);
    ctx.fillStyle = LAYER_COLORS.via_hole;
    ctx.fill();

    // Plating ring (thin bright ring between drill and copper)
    if (outerRadius - drillRadius > 1.5) {
      ctx.beginPath();
      ctx.arc(sx, sy, drillRadius + 0.5, 0, Math.PI * 2);
      ctx.strokeStyle = '#A0A060';
      ctx.lineWidth = 1;
      ctx.stroke();
    }
  }
}

// ---------------------------------------------------------------------------
// Ratsnest
// ---------------------------------------------------------------------------

function drawRatsnest(
  ctx: CanvasRenderingContext2D, vp: Viewport, line: RatsnestInfo,
  highlightedNet: string | null,
): void {
  const [startX, startY] = worldToScreen(vp, line.start_x, line.start_y);
  const [endX, endY] = worldToScreen(vp, line.end_x, line.end_y);

  const isActiveNet = highlightedNet != null && line.net_name === highlightedNet;
  const isDimmed = highlightedNet != null && !isActiveNet;

  ctx.save();
  if (isActiveNet) {
    // Brighter, thicker line for the active routing net
    ctx.strokeStyle = colorWithAlpha(LAYER_COLORS.ratsnest, 1.0);
    ctx.lineWidth = 2;
    ctx.setLineDash([6, 2]);
  } else if (isDimmed) {
    // Dim non-matching nets during routing
    ctx.strokeStyle = colorWithAlpha(LAYER_COLORS.ratsnest, 0.15);
    ctx.lineWidth = 1;
    ctx.setLineDash([5, 3]);
  } else {
    // Normal
    ctx.strokeStyle = LAYER_COLORS.ratsnest;
    ctx.lineWidth = 1;
    ctx.setLineDash([5, 3]);
  }
  ctx.beginPath();
  ctx.moveTo(startX, startY);
  ctx.lineTo(endX, endY);
  ctx.stroke();
  ctx.restore();
}

// ---------------------------------------------------------------------------
// Components — pad drawing, body outlines, drill marks
// ---------------------------------------------------------------------------

function drawComponent(
  ctx: CanvasRenderingContext2D, vp: Viewport, comp: ComponentInfo,
  layers: LayerVisibility, isSelected: boolean,
  themeColors: ReturnType<typeof getThemeColors>,
  highlightedNet: string | null,
  config: RenderConfig, lodTier: LodTier,
  padNetMap: Map<string, string>,
): void {
  // Draw pads
  for (const pad of comp.pads) {
    drawPad(ctx, vp, comp.x_nm, comp.y_nm, comp.rotation_mdeg, pad, layers, isSelected, themeColors, highlightedNet, comp.refdes, padNetMap, lodTier);
  }

  // Component body outline (LOD ≥ Medium, non-zero body dimensions)
  if (lodTier >= LodTier.Medium && comp.body_width_nm > 0 && comp.body_height_nm > 0) {
    drawBodyOutline(ctx, vp, comp, config);
  }
}

// ---------------------------------------------------------------------------
// Component body outline (silkscreen)
// ---------------------------------------------------------------------------

function drawBodyOutline(
  ctx: CanvasRenderingContext2D, vp: Viewport,
  comp: ComponentInfo, config: RenderConfig,
): void {
  const [sx, sy] = worldToScreen(vp, comp.x_nm, comp.y_nm);
  const w = comp.body_width_nm * vp.scale;
  const h = comp.body_height_nm * vp.scale;
  const radians = (comp.rotation_mdeg / 1000) * (Math.PI / 180);

  ctx.save();
  ctx.translate(sx, sy);
  ctx.rotate(-radians); // screen Y-down
  ctx.strokeStyle = config.layerColors.silkscreen;
  ctx.lineWidth = 1.5;
  ctx.setLineDash([4, 3]); // dashed to distinguish from copper
  ctx.strokeRect(-w / 2, -h / 2, w, h);
  ctx.setLineDash([]);
  ctx.restore();
}

// ---------------------------------------------------------------------------
// Refdes text (text pass, LOD ≥ Medium)
// ---------------------------------------------------------------------------

function drawRefdes(
  ctx: CanvasRenderingContext2D, vp: Viewport,
  comp: ComponentInfo, isSelected: boolean,
  themeColors: ReturnType<typeof getThemeColors>,
  config: RenderConfig,
): void {
  const [sx, sy] = worldToScreen(vp, comp.x_nm, comp.y_nm);

  // World-space font size, clamped 8–24px
  const rawSize = config.fontConfig.refdesWorldSize * vp.scale;
  const fontSize = Math.min(24, Math.max(8, rawSize));

  ctx.fillStyle = isSelected ? '#FF6600' : themeColors.label;
  ctx.font = `bold ${fontSize.toFixed(1)}px system-ui, sans-serif`;
  ctx.textAlign = 'center';
  ctx.textBaseline = 'bottom';
  ctx.fillText(comp.refdes, sx, sy - 3);

  _textElementsDrawn++;
}

// ---------------------------------------------------------------------------
// Pad pin numbers (text pass, LOD ≥ Close)
// ---------------------------------------------------------------------------

function drawPadNumbers(
  ctx: CanvasRenderingContext2D, vp: Viewport,
  comp: ComponentInfo, layers: LayerVisibility,
  config: RenderConfig,
): void {
  const radians = (comp.rotation_mdeg / 1000) * (Math.PI / 180);
  const cos = Math.cos(radians);
  const sin = Math.sin(radians);

  // Group pads by approximate font size to minimize ctx.font changes
  for (const pad of comp.pads) {
    if (!pad.number) continue;
    // Check visibility
    if (getPadColor(pad.layer_mask, layers) === null) continue;

    const padScreenW = pad.width_nm * vp.scale;
    if (padScreenW < config.fontConfig.padNumberMinScreenPx) continue;

    // Compute pad screen position
    const rotatedX = pad.x_nm * cos - pad.y_nm * sin;
    const rotatedY = pad.x_nm * sin + pad.y_nm * cos;
    const worldX = comp.x_nm + rotatedX;
    const worldY = comp.y_nm + rotatedY;
    const [sx, sy] = worldToScreen(vp, worldX, worldY);

    // Font size proportional to pad, clamped 8–18px
    const fontSize = Math.min(18, Math.max(8, padScreenW * 0.5));

    ctx.font = `${fontSize.toFixed(0)}px system-ui, sans-serif`;
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';

    // Use contrasting color: white on dark pads (copper colors are dark)
    ctx.fillStyle = '#FFFFFF';
    ctx.fillText(pad.number, sx, sy);

    _textElementsDrawn++;
  }
}

// ---------------------------------------------------------------------------
// Single pad
// ---------------------------------------------------------------------------

function drawPad(
  ctx: CanvasRenderingContext2D, vp: Viewport,
  compX: number, compY: number, rotationMdeg: number,
  pad: PadInfo, layers: LayerVisibility, isSelected: boolean,
  themeColors: ReturnType<typeof getThemeColors>,
  highlightedNet: string | null,
  compRefdes: string,
  padNetMap: Map<string, string>,
  lodTier: LodTier,
): void {
  let color = getPadColor(pad.layer_mask, layers);
  if (!color) return;

  // Per-pad net highlighting
  if (highlightedNet != null) {
    const padKey = `${compRefdes}.${pad.number}`;
    const padNet = padNetMap.get(padKey);
    if (padNet === highlightedNet) {
      // Glow: brighten the pad color
      color = brightenColor(color, 20);
    } else {
      // Dim pads not on the highlighted net
      color = colorWithAlpha(color, 0.15);
    }
  }

  // Compute pad world position with component rotation
  const radians = (rotationMdeg / 1000) * (Math.PI / 180);
  const cos = Math.cos(radians);
  const sin = Math.sin(radians);
  const rotatedX = pad.x_nm * cos - pad.y_nm * sin;
  const rotatedY = pad.x_nm * sin + pad.y_nm * cos;
  const worldX = compX + rotatedX;
  const worldY = compY + rotatedY;

  const [screenX, screenY] = worldToScreen(vp, worldX, worldY);
  const width = pad.width_nm * vp.scale;
  const height = pad.height_nm * vp.scale;
  if (width < 0.5 && height < 0.5) return;

  ctx.save();
  ctx.translate(screenX, screenY);
  ctx.rotate(-radians);

  // Glow ring for pads on the highlighted net
  if (highlightedNet != null) {
    const padKey = `${compRefdes}.${pad.number}`;
    const padNet = padNetMap.get(padKey);
    if (padNet === highlightedNet) {
      const glowColor = colorWithAlpha(brightenColor(getPadColor(pad.layer_mask, layers) ?? '#FFFFFF', 25), 0.35);
      ctx.fillStyle = glowColor;
      // Draw a slightly larger shape behind as glow
      const gw = width * 1.4;
      const gh = height * 1.4;
      switch (pad.shape) {
        case 'circle':
          ctx.beginPath();
          ctx.arc(0, 0, gw / 2, 0, Math.PI * 2);
          ctx.fill();
          break;
        default:
          ctx.fillRect(-gw / 2, -gh / 2, gw, gh);
          break;
      }
    }
  }

  ctx.fillStyle = isSelected ? '#FF6600' : color;

  switch (pad.shape) {
    case 'circle':
      ctx.beginPath();
      ctx.arc(0, 0, width / 2, 0, Math.PI * 2);
      ctx.fill();
      break;
    case 'rect':
      ctx.fillRect(-width / 2, -height / 2, width, height);
      break;
    case 'roundrect':
      drawRoundRect(ctx, -width / 2, -height / 2, width, height, Math.min(width, height) * 0.25);
      ctx.fill();
      break;
    case 'oblong':
      drawOblong(ctx, -width / 2, -height / 2, width, height);
      ctx.fill();
      break;
    default:
      ctx.fillRect(-width / 2, -height / 2, width, height);
  }

  // Drill hole for through-hole pads
  if (pad.drill_nm) {
    const drillRadius = pad.drill_nm * vp.scale / 2;
    if (drillRadius > 0.5) {
      // Dark drill hole
      ctx.fillStyle = LAYER_COLORS.drill;
      ctx.beginPath();
      ctx.arc(0, 0, drillRadius, 0, Math.PI * 2);
      ctx.fill();

      // Plating ring
      if (drillRadius > 2) {
        ctx.strokeStyle = '#8A7A50';
        ctx.lineWidth = 1;
        ctx.beginPath();
        ctx.arc(0, 0, drillRadius + 0.5, 0, Math.PI * 2);
        ctx.stroke();
      }
    }
  }

  ctx.restore();
}

// ---------------------------------------------------------------------------
// Geometry helpers (pad shapes)
// ---------------------------------------------------------------------------

function drawRoundRect(ctx: CanvasRenderingContext2D, x: number, y: number, w: number, h: number, r: number): void {
  r = Math.min(r, w / 2, h / 2);
  ctx.beginPath();
  ctx.moveTo(x + r, y);
  ctx.lineTo(x + w - r, y);
  ctx.quadraticCurveTo(x + w, y, x + w, y + r);
  ctx.lineTo(x + w, y + h - r);
  ctx.quadraticCurveTo(x + w, y + h, x + w - r, y + h);
  ctx.lineTo(x + r, y + h);
  ctx.quadraticCurveTo(x, y + h, x, y + h - r);
  ctx.lineTo(x, y + r);
  ctx.quadraticCurveTo(x, y, x + r, y);
  ctx.closePath();
}

function drawOblong(ctx: CanvasRenderingContext2D, x: number, y: number, w: number, h: number): void {
  const r = Math.min(w, h) / 2;
  drawRoundRect(ctx, x, y, w, h, r);
}

// ---------------------------------------------------------------------------
// Solder mask overlay — the green layer that makes it look like a real PCB
// ---------------------------------------------------------------------------

/**
 * Draw solder mask as a semi-transparent green overlay on the board area.
 * Pads and vias are drawn ON TOP of the mask (they represent exposed copper).
 * Traces under the mask get the characteristic "covered" look.
 */
function drawSolderMask(
  ctx: CanvasRenderingContext2D, vp: Viewport,
  snapshot: BoardSnapshot, layers: LayerVisibility,
): void {
  if (!snapshot.board) return;

  const [x0, y0] = worldToScreen(vp, 0, 0);
  const [x1, y1] = worldToScreen(vp, snapshot.board.width_nm, snapshot.board.height_nm);
  const boardLeft = x0;
  const boardTop = y1;
  const boardW = x1 - x0;
  const boardH = y0 - y1;

  ctx.save();

  // Clip to board area
  ctx.beginPath();
  ctx.rect(boardLeft, boardTop, boardW, boardH);
  ctx.clip();

  // Draw the green solder mask fill
  ctx.fillStyle = LAYER_COLORS.solder_mask_top;
  ctx.fillRect(boardLeft, boardTop, boardW, boardH);

  ctx.restore();
}

// ---------------------------------------------------------------------------
// Variant ghost preview
// ---------------------------------------------------------------------------

/**
 * Draw a ghost overlay of a variant's routes and vias.
 * Renders in cyan at 0.4 alpha to distinguish from active routes.
 */
function drawVariantPreview(ctx: CanvasRenderingContext2D, vp: Viewport, preview: VariantPreviewData): void {
  const GHOST_COLOR = 'rgba(0, 200, 255, 0.4)';
  const VIA_COLOR = 'rgba(0, 200, 255, 0.5)';

  ctx.save();

  // Draw route segments
  ctx.strokeStyle = GHOST_COLOR;
  ctx.lineCap = 'round';
  ctx.lineJoin = 'round';

  for (const route of preview.routes) {
    const widthPx = Math.max(1, route.width * vp.scale);
    ctx.lineWidth = widthPx;

    for (const seg of route.segments) {
      const [sx, sy] = worldToScreen(vp, seg.start[0], seg.start[1]);
      const [ex, ey] = worldToScreen(vp, seg.end[0], seg.end[1]);
      ctx.beginPath();
      ctx.moveTo(sx, sy);
      ctx.lineTo(ex, ey);
      ctx.stroke();
    }
  }

  // Draw vias
  for (const via of preview.vias) {
    const [vx, vy] = worldToScreen(vp, via.x, via.y);
    const radiusPx = Math.max(2, (via.drill / 2) * vp.scale);

    ctx.beginPath();
    ctx.arc(vx, vy, radiusPx, 0, Math.PI * 2);
    ctx.fillStyle = VIA_COLOR;
    ctx.fill();
  }

  ctx.restore();
}

// ---------------------------------------------------------------------------
// Routing preview
// ---------------------------------------------------------------------------

function drawRoutingPreview(ctx: CanvasRenderingContext2D, vp: Viewport, routing: RoutingState): void {
  const color = routing.netName ? netColor(routing.netName) : '#00FF00';
  const lineWidth = routing.traceWidth * vp.scale;
  const drawWidth = Math.max(lineWidth, 2);

  ctx.lineCap = 'round';
  ctx.lineJoin = 'round';

  if (routing.committedSegments.length > 0) {
    ctx.strokeStyle = colorWithAlpha(color, 0.7);
    ctx.lineWidth = drawWidth;
    ctx.beginPath();
    const first = routing.committedSegments[0];
    const [sx, sy] = worldToScreen(vp, first.start_x, first.start_y);
    ctx.moveTo(sx, sy);
    for (const seg of routing.committedSegments) {
      const [ex, ey] = worldToScreen(vp, seg.end_x, seg.end_y);
      ctx.lineTo(ex, ey);
    }
    ctx.stroke();
  }

  if (routing.previewSegment) {
    const seg = routing.previewSegment;
    const [sx, sy] = worldToScreen(vp, seg.start_x, seg.start_y);
    const [ex, ey] = worldToScreen(vp, seg.end_x, seg.end_y);

    ctx.save();
    ctx.setLineDash([8, 4]);
    ctx.strokeStyle = colorWithAlpha(color, 0.8);
    ctx.lineWidth = drawWidth;
    ctx.beginPath();
    ctx.moveTo(sx, sy);
    ctx.lineTo(ex, ey);
    ctx.stroke();
    ctx.restore();

    ctx.beginPath();
    ctx.arc(ex, ey, Math.max(4, drawWidth * 0.6), 0, Math.PI * 2);
    ctx.fillStyle = colorWithAlpha(color, 0.5);
    ctx.fill();
    ctx.strokeStyle = color;
    ctx.lineWidth = 1.5;
    ctx.stroke();
  }

  // Magnetic snap indicator: pulsing circle + crosshair at target pad
  if (routing.snappedToPad) {
    const [snapX, snapY] = worldToScreen(vp, routing.snappedToPad.worldX, routing.snappedToPad.worldY);
    const snapRadius = 300_000 * vp.scale; // 0.3mm in screen px
    const drawRadius = Math.max(snapRadius, 6);

    // Pulsing circle (alpha oscillates via time)
    const pulse = 0.4 + 0.2 * Math.sin(Date.now() / 200);
    ctx.beginPath();
    ctx.arc(snapX, snapY, drawRadius, 0, Math.PI * 2);
    ctx.fillStyle = colorWithAlpha(color, pulse);
    ctx.fill();
    ctx.strokeStyle = colorWithAlpha(color, 0.8);
    ctx.lineWidth = 2;
    ctx.stroke();

    // Crosshair
    const crossLen = drawRadius * 1.5;
    ctx.strokeStyle = colorWithAlpha(color, 0.6);
    ctx.lineWidth = 1.5;
    ctx.beginPath();
    ctx.moveTo(snapX - crossLen, snapY);
    ctx.lineTo(snapX + crossLen, snapY);
    ctx.moveTo(snapX, snapY - crossLen);
    ctx.lineTo(snapX, snapY + crossLen);
    ctx.stroke();
  }

  const [ax, ay] = worldToScreen(vp, routing.anchorPoint.x, routing.anchorPoint.y);
  ctx.beginPath();
  ctx.arc(ax, ay, 5, 0, Math.PI * 2);
  ctx.fillStyle = '#FFFFFF';
  ctx.fill();
  ctx.strokeStyle = color;
  ctx.lineWidth = 2;
  ctx.stroke();

  // DRC violation circles during routing preview (disabled — see above)
  // for (const v of routing.drcViolations) {
  //   const [vx, vy] = worldToScreen(vp, v.x_nm, v.y_nm);
  //   ctx.beginPath();
  //   ctx.arc(vx, vy, 12, 0, Math.PI * 2);
  //   ctx.strokeStyle = '#FF0000';
  //   ctx.lineWidth = 2.5;
  //   ctx.stroke();
  //   ctx.beginPath();
  //   ctx.arc(vx, vy, 8, 0, Math.PI * 2);
  //   ctx.fillStyle = 'rgba(255, 0, 0, 0.25)';
  //   ctx.fill();
  // }

  if (routing.previewSegment) {
    const seg = routing.previewSegment;
    const [ex, ey] = worldToScreen(vp, seg.end_x, seg.end_y);
    ctx.font = '11px system-ui, sans-serif';
    ctx.fillStyle = 'rgba(255, 255, 255, 0.85)';
    ctx.textAlign = 'left';

    // Show angle + snap mode indicator
    const snapMode = routing.snappedToPad ? '⊕' : routing.angleSnapEnabled ? '∠' : '';
    ctx.fillText(`${routing.snapAngle}° ${snapMode}`, ex + 12, ey - 8);

    let label = routing.currentLayer;
    if (routing.netName) label = `${routing.netName} [${routing.currentLayer}]`;
    ctx.fillText(label, ex + 12, ey + 6);
  }
}

// ---------------------------------------------------------------------------
// State management helpers
// ---------------------------------------------------------------------------

export function createRenderState(viewport: Viewport, layers: LayerVisibility): RenderState {
  return {
    snapshot: null,
    viewport,
    layers,
    selectedRefdes: null,
    showViolations: true,
    showRatsnest: true,
    colorByNet: true,
    selectedTraceId: null,
    hoveredTraceId: null,
    labelPosition: null,
    routing: null,
    highlightedNet: null,
  };
}

export function updateSnapshot(state: RenderState, snapshot: BoardSnapshot): RenderState {
  return { ...state, snapshot };
}

export function updateViewport(state: RenderState, viewport: Viewport): RenderState {
  return { ...state, viewport };
}

export function updateLayers(state: RenderState, layers: LayerVisibility): RenderState {
  return { ...state, layers };
}

export function updateSelection(state: RenderState, refdes: string | null): RenderState {
  return { ...state, selectedRefdes: refdes };
}

export function updateHighlightedNet(state: RenderState, net: string | null): RenderState {
  return { ...state, highlightedNet: net };
}
