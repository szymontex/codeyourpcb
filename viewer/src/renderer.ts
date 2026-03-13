/**
 * Canvas 2D rendering functions for PCB visualization
 * Draws board outline, components, pads, and grid
 */

import type { BoardSnapshot, ComponentInfo, PadInfo, ViolationInfo, TraceInfo, ViaInfo, RatsnestInfo } from './types';
import type { Viewport } from './viewport';
import type { RoutingState } from './routing';
import { worldToScreen, screenToWorld } from './viewport';
import { LAYER_COLORS, getPadColor, getTraceColor, getThemeColors, netColor, brightenColor, colorWithAlpha, type LayerVisibility } from './layers';

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
}

/**
 * Main render function - draws entire board state
 */
export function render(ctx: CanvasRenderingContext2D, state: RenderState): void {
  const { snapshot, viewport, layers, selectedRefdes, showViolations, showRatsnest } = state;

  // Get theme-dependent colors
  const themeColors = getThemeColors();

  // Clear canvas with background color
  ctx.fillStyle = themeColors.background;
  ctx.fillRect(0, 0, viewport.width, viewport.height);

  if (!snapshot || !snapshot.board) {
    drawEmptyState(ctx, viewport, themeColors);
    return;
  }

  // Draw grid (behind everything)
  drawGrid(ctx, viewport, themeColors);

  // Draw board outline
  drawBoardOutline(ctx, viewport, snapshot.board.width_nm, snapshot.board.height_nm, themeColors);

  // Draw resize handles when not routing
  if (!state.routing || state.routing.mode !== 'routing') {
    drawResizeHandles(ctx, viewport, snapshot.board.width_nm, snapshot.board.height_nm, themeColors, state.activeResizeHandle ?? null);
  }

  // Draw traces by layer (bottom first, then top)
  if (snapshot.traces) {
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
  }

  // Draw components (pads and labels)
  for (const comp of snapshot.components) {
    const isSelected = comp.refdes === selectedRefdes;
    drawComponent(ctx, viewport, comp, layers, isSelected, themeColors, state.highlightedNet);
  }

  // Draw vias on top of traces but below ratsnest
  if (snapshot.vias) {
    for (const via of snapshot.vias) {
      // Vias visible if any copper layer is visible
      if (layers.topCopper || layers.bottomCopper) {
        drawVia(ctx, viewport, via, themeColors);
      }
    }
  }

  // Draw ratsnest on top of everything (except violations)
  if (showRatsnest && snapshot.ratsnest) {
    for (const line of snapshot.ratsnest) {
      drawRatsnest(ctx, viewport, line);
    }
  }

  // Draw violations on top of everything
  if (showViolations && snapshot.violations) {
    for (const violation of snapshot.violations) {
      drawViolation(ctx, viewport, violation);
    }
  }

  // Draw routing preview (dashed trace + DRC markers)
  if (state.routing && state.routing.mode === 'routing') {
    drawRoutingPreview(ctx, viewport, state.routing);
  }

  // Draw net label for selected trace
  if (state.selectedTraceId != null && state.labelPosition && snapshot.traces) {
    const selectedTrace = snapshot.traces.find(t => t.id === state.selectedTraceId);
    if (selectedTrace) {
      drawNetLabel(ctx, state.labelPosition.x, state.labelPosition.y, selectedTrace);
    }
  }
}

/**
 * Draw placeholder when no board is loaded
 */
function drawEmptyState(ctx: CanvasRenderingContext2D, viewport: Viewport, themeColors: ReturnType<typeof getThemeColors>): void {
  ctx.fillStyle = themeColors.empty_text;
  ctx.font = '16px system-ui';
  ctx.textAlign = 'center';
  ctx.fillText('No board loaded', viewport.width / 2, viewport.height / 2);
}

/**
 * Draw grid lines
 * Adapts grid density based on zoom level for readability
 */
function drawGrid(ctx: CanvasRenderingContext2D, vp: Viewport, themeColors: ReturnType<typeof getThemeColors>): void {
  // 1mm grid spacing (1,000,000 nm)
  const gridSpacing = 1_000_000;

  // Only draw if grid lines would be at least 10px apart
  const screenSpacing = gridSpacing * vp.scale;
  if (screenSpacing < 10) return;

  ctx.strokeStyle = themeColors.grid;
  ctx.lineWidth = 1;

  // Calculate visible world bounds
  const [minX, maxY] = screenToWorld(vp, 0, 0);
  const [maxX, minY] = screenToWorld(vp, vp.width, vp.height);

  // Round to grid boundaries
  const startX = Math.floor(minX / gridSpacing) * gridSpacing;
  const startY = Math.floor(minY / gridSpacing) * gridSpacing;

  ctx.beginPath();

  // Vertical lines
  for (let x = startX; x <= maxX; x += gridSpacing) {
    const [sx] = worldToScreen(vp, x, 0);
    ctx.moveTo(sx, 0);
    ctx.lineTo(sx, vp.height);
  }

  // Horizontal lines
  for (let y = startY; y <= maxY; y += gridSpacing) {
    const [, sy] = worldToScreen(vp, 0, y);
    ctx.moveTo(0, sy);
    ctx.lineTo(vp.width, sy);
  }

  ctx.stroke();
}

/**
 * Draw board outline as yellow rectangle
 */
function drawBoardOutline(ctx: CanvasRenderingContext2D, vp: Viewport, width: number, height: number, themeColors: ReturnType<typeof getThemeColors>): void {
  const [x0, y0] = worldToScreen(vp, 0, 0);
  const [x1, y1] = worldToScreen(vp, width, height);

  ctx.strokeStyle = themeColors.board_outline;
  ctx.lineWidth = 2;
  // Note: y0/y1 are flipped due to Y-down screen coords
  ctx.strokeRect(x0, y1, x1 - x0, y0 - y1);
}

// ---------------------------------------------------------------------------
// Board resize handles
// ---------------------------------------------------------------------------

/** Handle identifiers: 4 corners + 4 edges */
export type ResizeHandle =
  | 'nw' | 'n' | 'ne'
  | 'w'  |       'e'
  | 'sw' | 's' | 'se';

const HANDLE_SIZE = 8; // screen pixels, half-width

/**
 * Draw 8 resize handles on the board outline edges.
 * Call AFTER drawBoardOutline so handles are on top.
 */
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

/**
 * Compute screen-space positions for the 8 resize handles around a board rectangle.
 */
function computeHandlePositions(
  vp: Viewport,
  originX: number,
  originY: number,
  width: number,
  height: number,
): { id: ResizeHandle; cx: number; cy: number }[] {
  const [x0, y0] = worldToScreen(vp, originX, originY);
  const [x1, y1] = worldToScreen(vp, width, height);
  const left = x0;
  const right = x1;
  const top = y1;
  const bottom = y0;
  const midX = (left + right) / 2;
  const midY = (top + bottom) / 2;

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

/**
 * Hit-test resize handles. Returns handle id or null.
 */
export function hitTestResizeHandle(
  vp: Viewport,
  boardWidth: number,
  boardHeight: number,
  screenX: number,
  screenY: number,
): ResizeHandle | null {
  const handles = computeHandlePositions(vp, 0, 0, boardWidth, boardHeight);

  const tolerance = HANDLE_SIZE + 4; // generous hit region
  for (const h of handles) {
    if (Math.abs(screenX - h.cx) <= tolerance && Math.abs(screenY - h.cy) <= tolerance) {
      return h.id;
    }
  }
  return null;
}

/**
 * Get the CSS cursor for a resize handle direction.
 */
export function resizeHandleCursor(handle: ResizeHandle): string {
  switch (handle) {
    case 'n': case 's': return 'ns-resize';
    case 'e': case 'w': return 'ew-resize';
    case 'nw': case 'se': return 'nwse-resize';
    case 'ne': case 'sw': return 'nesw-resize';
  }
}

/**
 * Draw a violation marker (red circle/ring) at the violation location
 * KiCad-style marker with outer ring and inner highlight
 */
function drawViolation(
  ctx: CanvasRenderingContext2D,
  vp: Viewport,
  violation: ViolationInfo
): void {
  const [sx, sy] = worldToScreen(vp, violation.x_nm, violation.y_nm);

  // Ring style marker (KiCad-like)
  const radius = 15; // Fixed screen pixels
  const innerRadius = 10;

  // Outer ring
  ctx.beginPath();
  ctx.arc(sx, sy, radius, 0, Math.PI * 2);
  ctx.strokeStyle = LAYER_COLORS.violation_ring;
  ctx.lineWidth = 3;
  ctx.stroke();

  // Inner highlight (semi-transparent fill)
  ctx.beginPath();
  ctx.arc(sx, sy, innerRadius, 0, Math.PI * 2);
  ctx.fillStyle = 'rgba(255, 0, 0, 0.3)';
  ctx.fill();
}

/**
 * Build the polyline path for a trace (reused by main stroke, glow, and locked overlay)
 */
function tracePolyline(
  ctx: CanvasRenderingContext2D,
  vp: Viewport,
  trace: TraceInfo
): void {
  ctx.beginPath();
  const firstSeg = trace.segments[0];
  const [startX, startY] = worldToScreen(vp, firstSeg.start_x, firstSeg.start_y);
  ctx.moveTo(startX, startY);
  for (const seg of trace.segments) {
    const [endX, endY] = worldToScreen(vp, seg.end_x, seg.end_y);
    ctx.lineTo(endX, endY);
  }
}

/**
 * Draw a copper trace
 * Renders as a thick polyline with rounded ends.
 * Supports per-net coloring, selection highlight (glow + wider stroke),
 * and hover highlight (lighter overlay).
 */
function drawTrace(
  ctx: CanvasRenderingContext2D,
  vp: Viewport,
  trace: TraceInfo,
  layers: LayerVisibility,
  colorByNet: boolean,
  selectedTraceId: number | null,
  hoveredTraceId: number | null,
  highlightedNet: string | null,
): void {
  if (trace.segments.length === 0) return;

  // Net highlighting: dim traces not on the highlighted net
  const isHighlightedNet = highlightedNet != null && trace.net_name === highlightedNet;
  const isDimmed = highlightedNet != null && !isHighlightedNet;

  // Determine base color — net color or layer color
  let color: string | null;
  if (colorByNet && trace.net_name) {
    // Still check layer visibility
    const layerVisible = getTraceColor(trace.layer, layers) !== null;
    if (!layerVisible) return;
    color = netColor(trace.net_name);
  } else {
    color = getTraceColor(trace.layer, layers);
  }
  if (!color) return;

  // Apply dimming for net highlight mode
  if (isDimmed) {
    color = colorWithAlpha(color, 0.15);
  }

  const isSelected = trace.id === selectedTraceId;
  const isHovered = trace.id === hoveredTraceId && !isSelected;

  // Calculate line width in screen pixels
  const baseLineWidth = trace.width * vp.scale;
  if (baseLineWidth < 0.5) return;

  ctx.lineCap = 'round';
  ctx.lineJoin = 'round';

  // Selection glow — wider semi-transparent stroke behind
  if (isSelected) {
    const glowColor = colorWithAlpha(brightenColor(color, 25), 0.35);
    ctx.strokeStyle = glowColor;
    ctx.lineWidth = baseLineWidth * 2.5;
    tracePolyline(ctx, vp, trace);
    ctx.stroke();
  }

  // Net highlight glow — wider semi-transparent stroke behind (similar to selection but for net)
  if (isHighlightedNet && !isSelected) {
    const glowColor = colorWithAlpha(brightenColor(color, 20), 0.3);
    ctx.strokeStyle = glowColor;
    ctx.lineWidth = baseLineWidth * 2.0;
    tracePolyline(ctx, vp, trace);
    ctx.stroke();
  }

  // Main trace stroke
  const drawColor = isSelected ? brightenColor(color, 20) : isHighlightedNet ? brightenColor(color, 15) : color;
  const lineWidth = isSelected ? baseLineWidth * 1.5 : isHighlightedNet ? baseLineWidth * 1.2 : baseLineWidth;

  ctx.strokeStyle = drawColor;
  ctx.lineWidth = lineWidth;
  tracePolyline(ctx, vp, trace);
  ctx.stroke();

  // Hover overlay — lighter semi-transparent stroke on top
  if (isHovered) {
    ctx.strokeStyle = 'rgba(255, 255, 255, 0.25)';
    ctx.lineWidth = baseLineWidth;
    tracePolyline(ctx, vp, trace);
    ctx.stroke();
  }

  // Locked indicator (subtle dashed overlay)
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

/**
 * Draw a net name + width label near the cursor for the selected trace.
 * Semi-transparent badge style.
 */
function drawNetLabel(
  ctx: CanvasRenderingContext2D,
  screenX: number,
  screenY: number,
  trace: TraceInfo
): void {
  const widthMm = (trace.width / 1_000_000).toFixed(2);
  const label = `${trace.net_name} — ${widthMm}mm`;

  ctx.font = '12px system-ui, sans-serif';
  const metrics = ctx.measureText(label);
  const padX = 8;
  const boxW = metrics.width + padX * 2;
  const boxH = 20;
  const offsetX = 15;
  const offsetY = -25;

  const x = screenX + offsetX;
  const y = screenY + offsetY;

  // Background
  ctx.fillStyle = 'rgba(0, 0, 0, 0.75)';
  ctx.beginPath();
  ctx.roundRect(x, y, boxW, boxH, 4);
  ctx.fill();

  // Text
  ctx.fillStyle = '#ffffff';
  ctx.textAlign = 'left';
  ctx.textBaseline = 'middle';
  ctx.fillText(label, x + padX, y + boxH / 2);
}

/**
 * Draw a via (plated through-hole connecting layers)
 * Renders as a filled circle with a drill hole
 */
function drawVia(
  ctx: CanvasRenderingContext2D,
  vp: Viewport,
  via: ViaInfo,
  themeColors: ReturnType<typeof getThemeColors>
): void {
  const [sx, sy] = worldToScreen(vp, via.x, via.y);
  const outerRadius = (via.outer_diameter * vp.scale) / 2;
  const drillRadius = (via.drill * vp.scale) / 2;

  // Don't render if too small to see
  if (outerRadius < 1) return;

  // Draw outer copper ring (use via color - blend of top/bottom)
  ctx.beginPath();
  ctx.arc(sx, sy, outerRadius, 0, Math.PI * 2);
  ctx.fillStyle = LAYER_COLORS.via;
  ctx.fill();

  // Draw drill hole
  if (drillRadius > 0.5) {
    ctx.beginPath();
    ctx.arc(sx, sy, drillRadius, 0, Math.PI * 2);
    ctx.fillStyle = themeColors.background;
    ctx.fill();
  }
}

/**
 * Draw a ratsnest line (unrouted connection indicator)
 * Thin dashed line in high-visibility color
 */
function drawRatsnest(
  ctx: CanvasRenderingContext2D,
  vp: Viewport,
  line: RatsnestInfo
): void {
  const [startX, startY] = worldToScreen(vp, line.start_x, line.start_y);
  const [endX, endY] = worldToScreen(vp, line.end_x, line.end_y);

  ctx.save();
  ctx.strokeStyle = LAYER_COLORS.ratsnest;
  ctx.lineWidth = 1; // Always 1px regardless of zoom
  ctx.setLineDash([5, 3]); // Dashed pattern

  ctx.beginPath();
  ctx.moveTo(startX, startY);
  ctx.lineTo(endX, endY);
  ctx.stroke();

  ctx.restore();
}

/**
 * Draw a component (its pads and label)
 */
function drawComponent(
  ctx: CanvasRenderingContext2D,
  vp: Viewport,
  comp: ComponentInfo,
  layers: LayerVisibility,
  isSelected: boolean,
  themeColors: ReturnType<typeof getThemeColors>,
  highlightedNet: string | null,
): void {
  // Draw pads
  for (const pad of comp.pads) {
    drawPad(ctx, vp, comp.x_nm, comp.y_nm, comp.rotation_mdeg, pad, layers, isSelected, themeColors, highlightedNet);
  }

  // Draw refdes label if zoomed in enough
  if (vp.scale > 0.00002) {
    const [sx, sy] = worldToScreen(vp, comp.x_nm, comp.y_nm);
    ctx.fillStyle = isSelected ? '#FF6600' : themeColors.label;
    ctx.font = '10px system-ui';
    ctx.textAlign = 'center';
    ctx.fillText(comp.refdes, sx, sy - 5);
  }
}

/**
 * Draw a single pad with rotation and layer-appropriate color
 */
function drawPad(
  ctx: CanvasRenderingContext2D,
  vp: Viewport,
  compX: number,
  compY: number,
  rotationMdeg: number,
  pad: PadInfo,
  layers: LayerVisibility,
  isSelected: boolean,
  themeColors: ReturnType<typeof getThemeColors>,
  highlightedNet: string | null,
): void {
  let color = getPadColor(pad.layer_mask, layers);
  if (!color) return; // Layer not visible

  // Dim pads when a net is highlighted (pads don't carry net info, so dim all)
  if (highlightedNet != null) {
    color = colorWithAlpha(color, 0.15);
  }

  // Calculate pad position in world coords
  // Apply component rotation to pad position
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

  // Skip if pad too small to see
  if (width < 0.5 && height < 0.5) return;

  ctx.save();
  ctx.translate(screenX, screenY);
  ctx.rotate(-radians); // Negate for screen Y-down

  // Fill color (orange when selected)
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
      // Fallback to rect for unknown shapes
      ctx.fillRect(-width / 2, -height / 2, width, height);
  }

  // Draw drill hole for through-hole pads
  if (pad.drill_nm) {
    const drillRadius = pad.drill_nm * vp.scale / 2;
    if (drillRadius > 0.5) {
      ctx.fillStyle = themeColors.background;
      ctx.beginPath();
      ctx.arc(0, 0, drillRadius, 0, Math.PI * 2);
      ctx.fill();
    }
  }

  ctx.restore();
}

/**
 * Draw a rounded rectangle path
 */
function drawRoundRect(ctx: CanvasRenderingContext2D, x: number, y: number, w: number, h: number, r: number): void {
  // Clamp radius to avoid artifacts
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

/**
 * Draw an oblong (pill/stadium) shape path
 */
function drawOblong(ctx: CanvasRenderingContext2D, x: number, y: number, w: number, h: number): void {
  // Oblong is like a pill shape (fully rounded ends)
  const r = Math.min(w, h) / 2;
  drawRoundRect(ctx, x, y, w, h, r);
}

/**
 * Draw the routing preview — committed segments (solid) + preview segment (dashed).
 * Also draws DRC violation markers from the live preview check.
 */
function drawRoutingPreview(
  ctx: CanvasRenderingContext2D,
  vp: Viewport,
  routing: RoutingState,
): void {
  const color = routing.netName ? netColor(routing.netName) : '#00FF00';
  const lineWidth = routing.traceWidth * vp.scale;
  const drawWidth = Math.max(lineWidth, 2); // minimum 2px visibility

  ctx.lineCap = 'round';
  ctx.lineJoin = 'round';

  // Draw committed segments (solid but semi-transparent)
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

  // Draw preview segment (dashed)
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

    // Draw endpoint marker (small circle at cursor)
    ctx.beginPath();
    ctx.arc(ex, ey, Math.max(4, drawWidth * 0.6), 0, Math.PI * 2);
    ctx.fillStyle = colorWithAlpha(color, 0.5);
    ctx.fill();
    ctx.strokeStyle = color;
    ctx.lineWidth = 1.5;
    ctx.stroke();
  }

  // Draw anchor point marker
  const [ax, ay] = worldToScreen(vp, routing.anchorPoint.x, routing.anchorPoint.y);
  ctx.beginPath();
  ctx.arc(ax, ay, 5, 0, Math.PI * 2);
  ctx.fillStyle = '#FFFFFF';
  ctx.fill();
  ctx.strokeStyle = color;
  ctx.lineWidth = 2;
  ctx.stroke();

  // Draw DRC violations from preview
  for (const v of routing.drcViolations) {
    const [vx, vy] = worldToScreen(vp, v.x_nm, v.y_nm);
    // Pulsing red ring
    ctx.beginPath();
    ctx.arc(vx, vy, 12, 0, Math.PI * 2);
    ctx.strokeStyle = '#FF0000';
    ctx.lineWidth = 2.5;
    ctx.stroke();
    ctx.beginPath();
    ctx.arc(vx, vy, 8, 0, Math.PI * 2);
    ctx.fillStyle = 'rgba(255, 0, 0, 0.25)';
    ctx.fill();
  }

  // Draw snap angle indicator text near cursor
  if (routing.previewSegment) {
    const seg = routing.previewSegment;
    const [ex, ey] = worldToScreen(vp, seg.end_x, seg.end_y);
    ctx.font = '11px system-ui, sans-serif';
    ctx.fillStyle = 'rgba(255, 255, 255, 0.85)';
    ctx.textAlign = 'left';
    ctx.fillText(`${routing.snapAngle}°`, ex + 12, ey - 8);

    // Show net name + layer indicator
    let label = routing.currentLayer;
    if (routing.netName) label = `${routing.netName} [${routing.currentLayer}]`;
    ctx.fillText(label, ex + 12, ey + 6);
  }
}

/**
 * Create an initial render state
 */
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

/**
 * Update render state with new snapshot
 */
export function updateSnapshot(state: RenderState, snapshot: BoardSnapshot): RenderState {
  return {
    ...state,
    snapshot,
  };
}

/**
 * Update render state with new viewport
 */
export function updateViewport(state: RenderState, viewport: Viewport): RenderState {
  return {
    ...state,
    viewport,
  };
}

/**
 * Update render state with new layer visibility
 */
export function updateLayers(state: RenderState, layers: LayerVisibility): RenderState {
  return {
    ...state,
    layers,
  };
}

/**
 * Update selection
 */
export function updateSelection(state: RenderState, refdes: string | null): RenderState {
  return {
    ...state,
    selectedRefdes: refdes,
  };
}

/**
 * Update highlighted net (for net-level selection)
 */
export function updateHighlightedNet(state: RenderState, net: string | null): RenderState {
  return {
    ...state,
    highlightedNet: net,
  };
}
