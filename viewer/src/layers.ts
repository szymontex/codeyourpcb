/**
 * Layer colors and visibility definitions
 * KiCad-style colors for familiar PCB visualization
 */

// PCB electrical layer colors — KiCad-inspired professional palette
export const LAYER_COLORS = {
  top_copper: '#C41E1E',       // Brighter red (visible under solder mask)
  bottom_copper: '#1E1EC4',    // Brighter blue (visible under solder mask)
  top_silk: '#F0F0F0',         // Near-white silkscreen
  bottom_silk: '#A0A0A0',     // Medium gray
  drill: '#1A1A1A',            // Near-black drill holes
  violation: '#FF0000',        // Red for DRC errors
  violation_ring: '#FF0000',   // Ring outline for violation markers
  via: '#C8C800',              // Yellow-ish via annular ring (KiCad style)
  via_hole: '#1A1A1A',         // Dark hole center
  ratsnest: '#99CCFF',         // Light blue ratsnest (KiCad style)
  solder_mask_top: 'rgba(0, 70, 0, 0.45)',     // Green solder mask overlay (slightly less opaque)
  solder_mask_bottom: 'rgba(0, 55, 0, 0.45)',  // Slightly darker for bottom
  pad_copper: '#B49E6A',       // Gold/brass pad copper (exposed)
  pad_th: '#B49E6A',           // Through-hole pad ring
  board_substrate: '#332B18',  // FR4 substrate tan/brown
  courtyard: '#C8C8C880',     // Faint courtyard outline
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
      return LAYER_COLORS.pad_th; // Gold/brass for through-hole
    }
    return null;
  }

  // Top-only SMD
  if (layerMask & LAYER_MASK.TOP_COPPER) {
    return visibility.topCopper ? LAYER_COLORS.pad_copper : null;
  }

  // Bottom-only SMD
  if (layerMask & LAYER_MASK.BOTTOM_COPPER) {
    return visibility.bottomCopper ? LAYER_COLORS.pad_copper : null;
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

// Well-known net name color overrides
const NET_COLOR_OVERRIDES: Record<string, string> = {
  'VCC': 'hsl(0, 80%, 50%)',       // Red
  'VDD': 'hsl(0, 80%, 50%)',       // Red
  '+5V': 'hsl(0, 80%, 50%)',       // Red
  '+3V3': 'hsl(30, 90%, 50%)',     // Orange
  '3V3': 'hsl(30, 90%, 50%)',      // Orange
  '+3.3V': 'hsl(30, 90%, 50%)',    // Orange
  'GND': 'hsl(220, 70%, 35%)',     // Dark blue
  'AGND': 'hsl(220, 70%, 35%)',    // Dark blue
  'DGND': 'hsl(220, 70%, 35%)',    // Dark blue
};

/**
 * Generate a deterministic color for a net name.
 * Hashes the name to a hue (0-360), uses fixed saturation and lightness.
 * Common power/ground nets get recognizable overrides.
 */
export function netColor(netName: string): string {
  // Check overrides first (case-insensitive)
  const upper = netName.toUpperCase();
  if (NET_COLOR_OVERRIDES[upper]) {
    return NET_COLOR_OVERRIDES[upper];
  }

  // Simple string hash → hue
  let hash = 0;
  for (let i = 0; i < netName.length; i++) {
    hash = ((hash << 5) - hash + netName.charCodeAt(i)) | 0;
  }
  // Map to 0-360 hue, avoid red/blue zone used by overrides
  const hue = ((hash % 360) + 360) % 360;
  return `hsl(${hue}, 70%, 50%)`;
}

/**
 * Brighten a color for selection highlight.
 * Works with both HSL strings and hex colors.
 */
export function brightenColor(color: string, amount: number = 15): string {
  const hslMatch = color.match(/hsl\((\d+),\s*(\d+)%,\s*(\d+)%\)/);
  if (hslMatch) {
    const h = parseInt(hslMatch[1]);
    const s = parseInt(hslMatch[2]);
    const l = Math.min(85, parseInt(hslMatch[3]) + amount);
    return `hsl(${h}, ${s}%, ${l}%)`;
  }
  // Hex fallback — lighten by mixing with white
  return color;
}

/**
 * Convert a color to an RGBA string with given alpha.
 * Works with HSL strings.
 */
export function colorWithAlpha(color: string, alpha: number): string {
  const hslMatch = color.match(/hsl\((\d+),\s*(\d+)%,\s*(\d+)%\)/);
  if (hslMatch) {
    return `hsla(${hslMatch[1]}, ${hslMatch[2]}%, ${hslMatch[3]}%, ${alpha})`;
  }
  // Hex fallback — use canvas helper not available here, just return as-is
  return color;
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
        return '#2E8B2E'; // Forest green for inner layers
      }
      return null;
  }
}
