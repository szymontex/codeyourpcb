/**
 * Viewport state and coordinate transformations
 * Converts between world coordinates (nanometers, Y-up) and screen coordinates (pixels, Y-down)
 */

export interface Viewport {
  // View center in world coordinates (nanometers)
  centerX: number;
  centerY: number;
  // Pixels per nanometer
  scale: number;
  // Canvas dimensions
  width: number;
  height: number;
}

/**
 * Create a new viewport with default zoom
 * Default scale: 1mm = 100px (reasonable starting zoom)
 * 1mm = 1,000,000 nm, so scale = 100 / 1,000,000 = 0.0001
 */
export function createViewport(width: number, height: number): Viewport {
  return {
    centerX: 0,
    centerY: 0,
    scale: 0.0001,
    width,
    height,
  };
}

/**
 * Convert world coordinates (nm, Y-up) to screen coordinates (px, Y-down)
 */
export function worldToScreen(vp: Viewport, worldX: number, worldY: number): [number, number] {
  const screenX = (worldX - vp.centerX) * vp.scale + vp.width / 2;
  // Y is flipped: world Y-up to screen Y-down
  const screenY = vp.height / 2 - (worldY - vp.centerY) * vp.scale;
  return [screenX, screenY];
}

/**
 * Convert screen coordinates (px, Y-down) to world coordinates (nm, Y-up)
 */
export function screenToWorld(vp: Viewport, screenX: number, screenY: number): [number, number] {
  const worldX = (screenX - vp.width / 2) / vp.scale + vp.centerX;
  // Y is flipped
  const worldY = vp.centerY - (screenY - vp.height / 2) / vp.scale;
  return [worldX, worldY];
}

/**
 * Zoom at a specific screen point (cursor position)
 * The world point under the cursor stays at the same screen position after zoom
 */
export function zoomAtPoint(vp: Viewport, screenX: number, screenY: number, factor: number): Viewport {
  // Get world point before zoom
  const [worldX, worldY] = screenToWorld(vp, screenX, screenY);

  // Apply zoom with clamping
  // Min: 0.000001 px/nm = 1mm takes 1px (very zoomed out)
  // Max: 0.01 px/nm = 1mm takes 10,000px (very zoomed in)
  const newScale = Math.max(0.000001, Math.min(0.01, vp.scale * factor));

  // Adjust center so world point stays at same screen position
  const newCenterX = worldX - (screenX - vp.width / 2) / newScale;
  const newCenterY = worldY + (screenY - vp.height / 2) / newScale;

  return {
    ...vp,
    scale: newScale,
    centerX: newCenterX,
    centerY: newCenterY,
  };
}

/**
 * Pan by screen delta (pixels)
 */
export function pan(vp: Viewport, dx: number, dy: number): Viewport {
  return {
    ...vp,
    centerX: vp.centerX - dx / vp.scale,
    centerY: vp.centerY + dy / vp.scale, // Invert Y for world coords
  };
}

/**
 * Fit board in viewport with padding
 */
export function fitBoard(vp: Viewport, boardWidth: number, boardHeight: number, padding: number = 50): Viewport {
  // Guard against zero dimensions
  if (boardWidth <= 0 || boardHeight <= 0) {
    return vp;
  }

  const scaleX = (vp.width - padding * 2) / boardWidth;
  const scaleY = (vp.height - padding * 2) / boardHeight;
  const scale = Math.min(scaleX, scaleY);

  return {
    ...vp,
    centerX: boardWidth / 2,
    centerY: boardHeight / 2,
    scale,
  };
}

/**
 * Update viewport dimensions (on canvas resize)
 */
export function resizeViewport(vp: Viewport, width: number, height: number): Viewport {
  return {
    ...vp,
    width,
    height,
  };
}
