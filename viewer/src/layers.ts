/**
 * Layer colors and visibility definitions
 * KiCad-style colors for familiar PCB visualization
 */

// Layer color definitions (KiCad-style)
export const LAYER_COLORS = {
  top_copper: '#C83434',       // Red
  bottom_copper: '#3434C8',    // Blue
  top_silk: '#C8C8C8',         // Light gray
  bottom_silk: '#808080',      // Gray
  drill: '#FFFFFF',            // White (on dark) or black (on light)
  board_outline: '#FFFF00',    // Yellow
  background: '#FFFFFF',       // Light mode background (per user preference)
  grid: '#E0E0E0',             // Light gray grid
} as const;

// Layer bit masks (match cypcb-world Layer enum)
export const LAYER_MASK = {
  TOP_COPPER: 0x00000001,
  BOTTOM_COPPER: 0x00000002,
  // Inner layers would be 0x00000004, 0x00000008, etc.
} as const;

export interface LayerVisibility {
  topCopper: boolean;
  bottomCopper: boolean;
}

/**
 * Create default layer visibility (all visible)
 */
export function createLayerVisibility(): LayerVisibility {
  return {
    topCopper: true,
    bottomCopper: true,
  };
}

/**
 * Toggle a specific layer's visibility
 */
export function toggleLayer(layers: LayerVisibility, layer: keyof LayerVisibility): LayerVisibility {
  return {
    ...layers,
    [layer]: !layers[layer],
  };
}

/**
 * Get color for a pad based on its layer mask and visibility settings
 * Returns null if the pad should not be drawn (layer hidden)
 */
export function getPadColor(layerMask: number, visibility: LayerVisibility): string | null {
  // Through-hole pads (on both layers)
  if ((layerMask & LAYER_MASK.TOP_COPPER) && (layerMask & LAYER_MASK.BOTTOM_COPPER)) {
    // Show if either layer visible
    if (visibility.topCopper || visibility.bottomCopper) {
      return '#C8C8C8'; // Gray for through-hole
    }
    return null;
  }

  // Top-only SMD
  if (layerMask & LAYER_MASK.TOP_COPPER) {
    return visibility.topCopper ? LAYER_COLORS.top_copper : null;
  }

  // Bottom-only SMD
  if (layerMask & LAYER_MASK.BOTTOM_COPPER) {
    return visibility.bottomCopper ? LAYER_COLORS.bottom_copper : null;
  }

  return null;
}

/**
 * Check if a layer mask is on the top layer
 */
export function isTopLayer(layerMask: number): boolean {
  return (layerMask & LAYER_MASK.TOP_COPPER) !== 0;
}

/**
 * Check if a layer mask is on the bottom layer
 */
export function isBottomLayer(layerMask: number): boolean {
  return (layerMask & LAYER_MASK.BOTTOM_COPPER) !== 0;
}

/**
 * Check if a layer mask is through-hole (both layers)
 */
export function isThroughHole(layerMask: number): boolean {
  return isTopLayer(layerMask) && isBottomLayer(layerMask);
}
