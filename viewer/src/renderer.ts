/**
 * Canvas 2D rendering functions for PCB visualization
 * Draws board outline, components, pads, and grid
 */

import type { BoardSnapshot, ComponentInfo, PadInfo, ViolationInfo } from './types';
import type { Viewport } from './viewport';
import { worldToScreen, screenToWorld } from './viewport';
import { LAYER_COLORS, getPadColor, type LayerVisibility } from './layers';

export interface RenderState {
  snapshot: BoardSnapshot | null;
  viewport: Viewport;
  layers: LayerVisibility;
  selectedRefdes: string | null;
  showViolations: boolean;
}

/**
 * Main render function - draws entire board state
 */
export function render(ctx: CanvasRenderingContext2D, state: RenderState): void {
  const { snapshot, viewport, layers, selectedRefdes, showViolations } = state;

  // Clear canvas with background color
  ctx.fillStyle = LAYER_COLORS.background;
  ctx.fillRect(0, 0, viewport.width, viewport.height);

  if (!snapshot || !snapshot.board) {
    drawEmptyState(ctx, viewport);
    return;
  }

  // Draw grid (behind everything)
  drawGrid(ctx, viewport);

  // Draw board outline
  drawBoardOutline(ctx, viewport, snapshot.board.width_nm, snapshot.board.height_nm);

  // Draw components (pads and labels)
  for (const comp of snapshot.components) {
    const isSelected = comp.refdes === selectedRefdes;
    drawComponent(ctx, viewport, comp, layers, isSelected);
  }

  // Draw violations on top of everything
  if (showViolations && snapshot.violations) {
    for (const violation of snapshot.violations) {
      drawViolation(ctx, viewport, violation);
    }
  }
}

/**
 * Draw placeholder when no board is loaded
 */
function drawEmptyState(ctx: CanvasRenderingContext2D, viewport: Viewport): void {
  ctx.fillStyle = '#666';
  ctx.font = '16px system-ui';
  ctx.textAlign = 'center';
  ctx.fillText('No board loaded', viewport.width / 2, viewport.height / 2);
}

/**
 * Draw grid lines
 * Adapts grid density based on zoom level for readability
 */
function drawGrid(ctx: CanvasRenderingContext2D, vp: Viewport): void {
  // 1mm grid spacing (1,000,000 nm)
  const gridSpacing = 1_000_000;

  // Only draw if grid lines would be at least 10px apart
  const screenSpacing = gridSpacing * vp.scale;
  if (screenSpacing < 10) return;

  ctx.strokeStyle = LAYER_COLORS.grid;
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
function drawBoardOutline(ctx: CanvasRenderingContext2D, vp: Viewport, width: number, height: number): void {
  const [x0, y0] = worldToScreen(vp, 0, 0);
  const [x1, y1] = worldToScreen(vp, width, height);

  ctx.strokeStyle = LAYER_COLORS.board_outline;
  ctx.lineWidth = 2;
  // Note: y0/y1 are flipped due to Y-down screen coords
  ctx.strokeRect(x0, y1, x1 - x0, y0 - y1);
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
 * Draw a component (its pads and label)
 */
function drawComponent(
  ctx: CanvasRenderingContext2D,
  vp: Viewport,
  comp: ComponentInfo,
  layers: LayerVisibility,
  isSelected: boolean
): void {
  // Draw pads
  for (const pad of comp.pads) {
    drawPad(ctx, vp, comp.x_nm, comp.y_nm, comp.rotation_mdeg, pad, layers, isSelected);
  }

  // Draw refdes label if zoomed in enough
  if (vp.scale > 0.00002) {
    const [sx, sy] = worldToScreen(vp, comp.x_nm, comp.y_nm);
    ctx.fillStyle = isSelected ? '#FF6600' : '#333';
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
  isSelected: boolean
): void {
  const color = getPadColor(pad.layer_mask, layers);
  if (!color) return; // Layer not visible

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
      ctx.fillStyle = LAYER_COLORS.background;
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
 * Create an initial render state
 */
export function createRenderState(viewport: Viewport, layers: LayerVisibility): RenderState {
  return {
    snapshot: null,
    viewport,
    layers,
    selectedRefdes: null,
    showViolations: true,
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
