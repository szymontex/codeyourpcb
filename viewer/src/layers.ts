/**
 * Layer colors and visibility definitions
 * KiCad-style colors for familiar PCB visualization
 */

// PCB electrical layer colors (fixed, not theme-dependent)
export const LAYER_COLORS = {
  top_copper: '#C83434',       // Red
  bottom_copper: '#3434C8',    // Blue
  top_silk: '#C8C8C8',         // Light gray
  bottom_silk: '#808080',      // Gray
  drill: '#FFFFFF',            // White (on dark) or black (on light)
  violation: '#FF0000',        // Red for DRC errors
  violation_ring: '#FF0000',   // Ring outline for violation markers
  via: '#808080',              // Gray for vias (between top/bottom)
  ratsnest: '#FFD700',         // Gold/Yellow for unrouted connections
} as const;

/**
 * Get current theme colors by reading CSS custom properties
 * These colors change based on the active theme (light/dark)
 */
export function getThemeColors() {
  const style = getComputedStyle(document.documentElement);
  return {
    background: style.getPropertyValue('--bg-canvas').trim() || '#ffffff',
    grid: style.getPropertyValue('--pcb-grid').trim() || '#e0e0e0',
    board_outline: style.getPropertyValue('--pcb-board-outline').trim() || '#cccc00',
    empty_text: style.getPropertyValue('--pcb-empty-text').trim() || '#666666',
    label: style.getPropertyValue('--pcb-label').trim() || '#333333',
  };
}

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

/**
 * Get color for a trace based on its layer name and visibility settings
 * Returns null if the layer is not visible
 */
export function getTraceColor(layer: string, visibility: LayerVisibility): string | null {
  switch (layer) {
    case 'Top':
      return visibility.topCopper ? LAYER_COLORS.top_copper : null;
    case 'Bottom':
      return visibility.bottomCopper ? LAYER_COLORS.bottom_copper : null;
    default:
      // Inner layers - show if any copper layer is visible
      if (visibility.topCopper || visibility.bottomCopper) {
        return '#34C834'; // Green for inner layers
      }
      return null;
  }
}
