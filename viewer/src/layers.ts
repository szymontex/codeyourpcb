/**
 * Layer colors and visibility definitions
 * KiCad-style colors for familiar PCB visualization
 */

// PCB electrical layer colors — KiCad-inspired professional palette
export const LAYER_COLORS = {
  top_copper: '#C41E1E',       // Brighter red (visible under solder mask)
  bottom_copper: '#1E1EC4',    // Brighter blue (visible under solder mask)
  orphaned_copper: '#FF8C1A', // Copper a pour left connected to nothing
  keepout: '#E05A5A',          // Dashed outline for an area nothing may enter
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
  /**
   * Whether the copper between the outer two is drawn.
   *
   * Optional so every existing caller keeps working: absent means visible,
   * which is what a four-layer board was already getting - drawn, in one
   * undifferentiated green, with no way to turn it off.
   */
  innerCopper?: boolean;
}

/**
 * Create default layer visibility (all visible)
 */
export function createLayerVisibility(): LayerVisibility {
  return {
    topCopper: true,
    bottomCopper: true,
    innerCopper: true,
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
      return innerLayerColor(layer, visibility);
  }
}

/**
 * The colour an inner copper layer is drawn in.
 *
 * One green for every inner layer told a four-layer board's designer nothing:
 * a trace on In1 and a trace on In2 cannot cross, and they looked identical.
 * Each layer gets its own shade, and its visibility no longer rides on whether
 * an outer layer happens to be on.
 */
export function innerLayerColor(layer: string, visibility: LayerVisibility): string | null {
  if (visibility.innerCopper === false) return null;

  const match = layer.match(/^Inner(\d+)$/);
  if (!match) return null;

  // Inner1 is the first inner layer, the way the DSL writes it.
  const index = Math.max(0, parseInt(match[1], 10) - 1);
  return INNER_LAYER_COLORS[index % INNER_LAYER_COLORS.length];
}

/** One shade per inner layer, in the order the stack goes down. */
export const INNER_LAYER_COLORS = ['#2E8B2E', '#7A5CD1', '#C08A2E', '#2E8B8B'] as const;

/**
 * Whether a shared link asks for the inner copper to be hidden.
 *
 * Silence means visible. A link made before inner layers were drawn lists only
 * `top` and `bottom`, and hiding the middle of a four-layer board because an
 * old URL never mentioned it is the wrong reading of silence.
 */
export function innerVisibleFromUrlLayers(layers: string[]): boolean {
  return !layers.includes('no-inner');
}

/**
 * Which inner layer a trace names, or `null` if it names an outer one.
 *
 * `Inner1` is the first inner layer, the way the DSL writes it and the way the
 * engine now sends it. Both views ask this rather than matching the string
 * twice.
 */
export function innerLayerIndex(layer: string): number | null {
  const match = layer.match(/^Inner(\d+)$/);
  if (!match) return null;
  const number = parseInt(match[1], 10);
  return number >= 1 ? number - 1 : null;
}

/**
 * Where an inner layer sits through the thickness of the board, in mm from the
 * board's centre.
 *
 * Evenly spaced between the two faces, so which layer a trace is on can be
 * read from the side of a 3D view instead of guessed from its colour.
 */
export function innerLayerDepth(index: number, count: number, thicknessMm: number): number {
  if (count <= 0) return 0;
  return -thicknessMm / 2 + ((index + 1) * thicknessMm) / (count + 1);
}

/**
 * Where a via's barrel starts and ends through the board, in mm from centre.
 *
 * A via that stops at an inner layer - blind from one face, or buried between
 * two inner layers - is a different hole from one that goes through, and it
 * was drawn going all the way through because the span never reached the
 * viewer. `Top` and `Bottom` are the faces; `Inner1` and up sit where
 * `innerLayerDepth` puts them.
 */
export function viaSpanDepths(
  startLayer: string,
  endLayer: string,
  innerCount: number,
  thicknessMm: number,
): { bottom: number; top: number } {
  const depth = (layer: string, fallback: number): number => {
    if (layer === 'Top') return thicknessMm / 2;
    if (layer === 'Bottom') return -thicknessMm / 2;
    const index = innerLayerIndex(layer);
    if (index === null || innerCount <= 0) return fallback;
    return innerLayerDepth(Math.min(index, innerCount - 1), innerCount, thicknessMm);
  };

  const a = depth(startLayer, thicknessMm / 2);
  const b = depth(endLayer, -thicknessMm / 2);
  return { bottom: Math.min(a, b), top: Math.max(a, b) };
}
