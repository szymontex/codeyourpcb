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

/**
 * How hard the editor pushes the layer you are working on to the front.
 *
 * A four-layer board drawn all at once is unreadable, which is the complaint
 * this exists to answer. Every serious PCB editor has this control and calls
 * it something different: Altium cycles hide/grey/monochrome with one key,
 * KiCad dims the inactive layers by an opacity slider. Three states is the
 * useful number - off, quieter, alone - because a fourth is one more press
 * between a person and the copper they are looking at.
 */
export type LayerFocus = 'all' | 'ghost' | 'dim' | 'solo';

/** The order `X` walks, and the order the button cycles. */
export const LAYER_FOCUS_ORDER: readonly LayerFocus[] = ['all', 'ghost', 'dim', 'solo'] as const;

/** What each state is called where a person can read it. */
export const LAYER_FOCUS_LABEL: Record<LayerFocus, string> = {
  all: 'All layers',
  ghost: 'Others in grey',
  dim: 'Dim others',
  solo: 'Active only',
};

/**
 * The grey an inactive layer is drawn in under `ghost`.
 *
 * Keeping the geometry and dropping the colour is the trick Altium calls
 * grey-scale mode, and it beats transparency for the thing this is for: a
 * faint red trace still reads as top copper and competes for the eye, while a
 * grey one reads as context. The active layer is then the only coloured thing
 * on screen, which is the whole point.
 */
export const GHOST_GREY = '#6b6b6b';

/** The next state in the cycle, wrapping. */
export function nextLayerFocus(focus: LayerFocus | undefined): LayerFocus {
  const at = LAYER_FOCUS_ORDER.indexOf(focus ?? 'all');
  return LAYER_FOCUS_ORDER[(at + 1) % LAYER_FOCUS_ORDER.length];
}

/** How much of a colour survives being dimmed. */
export const DIMMED_ALPHA = 0.16;

/**
 * The copper-mask bit a layer name stands for.
 *
 * Top is bit 0, Bottom is bit 1, and `Inner(n)` counts from bit 2 with n
 * starting at 1 - the same arithmetic `Layer::to_copper_mask` does in Rust.
 * Lives here because it is a fact about layers, and the router imports it
 * rather than keeping a second copy.
 */
export function layerMaskBit(name: string): number {
  if (name === 'Top') return 0x01;
  if (name === 'Bottom') return 0x02;
  const inner = /^Inner(\d+)$/.exec(name);
  if (inner) return 1 << (2 + (Number(inner[1]) - 1));
  return 0;
}

export interface LayerVisibility {
  topCopper: boolean;
  bottomCopper: boolean;
  /**
   * How much the layers other than the active one are pushed back.
   *
   * Absent means `all`, so every caller written before this keeps the
   * behaviour it had.
   */
  focus?: LayerFocus;
  /**
   * The layer being drawn on, which is the one focus keeps.
   *
   * Absent means there is nothing to focus on and `focus` does nothing - a
   * viewer with no active layer must not blank its own canvas.
   */
  activeLayer?: string;
  /**
   * Copper layers turned off one at a time, by name.
   *
   * `innerCopper` is a single switch for every layer between the outer two,
   * so on a six-layer board there was no way to hide `Inner2` and keep
   * `Inner1` - the two are drawn in different colours precisely because they
   * are different layers, and the only control over them treated them as one
   * thing. This is where a layer gets an identity of its own.
   *
   * Named rather than indexed, because a stack can change under a design and
   * an index would then point at a different layer than the one somebody
   * switched off.
   */
  hiddenLayers?: readonly string[];
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
  // A pad counts as being on the active layer when its own mask names that
  // layer. A through-hole pad is on every copper layer, so focus never takes
  // one away - you cannot route to a hole you cannot see, and a board of
  // headers would empty its own canvas the moment somebody pressed X.
  const active = visibility.activeLayer;
  const onActive = active ? (layerMask & layerMaskBit(active)) !== 0 : false;

  const base = (() => {
    // Through-hole pads (on both layers)
    if ((layerMask & LAYER_MASK.TOP_COPPER) && (layerMask & LAYER_MASK.BOTTOM_COPPER)) {
      // Show if either layer visible
      if (isLayerVisible('Top', visibility) || isLayerVisible('Bottom', visibility)) {
        return LAYER_COLORS.pad_th; // Gold/brass for through-hole
      }
      return null;
    }

    // Top-only SMD
    if (layerMask & LAYER_MASK.TOP_COPPER) {
      return isLayerVisible('Top', visibility) ? LAYER_COLORS.pad_copper : null;
    }

    // Bottom-only SMD
    if (layerMask & LAYER_MASK.BOTTOM_COPPER) {
      return isLayerVisible('Bottom', visibility) ? LAYER_COLORS.pad_copper : null;
    }

    return null;
  })();

  return applyFocus(base, onActive, visibility.focus);
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

  // Hex, which is what LAYER_COLORS actually holds. This branch used to
  // return the colour untouched and say so in a comment, so every caller
  // asking for transparency got none of it.
  const hex = /^#([0-9a-f]{3}|[0-9a-f]{6})$/i.exec(color.trim());
  if (hex) {
    const digits = hex[1].length === 3
      ? hex[1].split('').map((d) => d + d).join('')
      : hex[1];
    const r = parseInt(digits.slice(0, 2), 16);
    const g = parseInt(digits.slice(2, 4), 16);
    const b = parseInt(digits.slice(4, 6), 16);
    return `rgba(${r}, ${g}, ${b}, ${alpha})`;
  }

  // Already an rgb()/rgba() string, or something this does not parse. Leaving
  // it alone is the honest answer; inventing a conversion is not.
  return color;
}

/**
 * Get color for a trace based on its layer name and visibility settings
 * Returns null if the layer is not visible
 */
/**
 * Whether a named copper layer is drawn at all.
 *
 * Three switches can turn one off and any of them is enough: the layer's own
 * entry in `hiddenLayers`, the outer-layer checkbox it belongs to, or the
 * group switch for the inner copper. Focus is not one of them - that decides
 * how loudly a visible layer is drawn, not whether it exists.
 */
export function isLayerVisible(layer: string, visibility: LayerVisibility): boolean {
  if (visibility.hiddenLayers?.includes(layer)) return false;
  if (layer === 'Top') return visibility.topCopper;
  if (layer === 'Bottom') return visibility.bottomCopper;
  return visibility.innerCopper !== false && /^Inner\d+$/.test(layer);
}

/** The same list with one layer's visibility flipped. */
export function toggleLayerVisible(
  visibility: LayerVisibility,
  layer: string,
): LayerVisibility {
  if (layer === 'Top') return { ...visibility, topCopper: !visibility.topCopper };
  if (layer === 'Bottom') return { ...visibility, bottomCopper: !visibility.bottomCopper };

  const hidden = visibility.hiddenLayers ?? [];
  return {
    ...visibility,
    hiddenLayers: hidden.includes(layer)
      ? hidden.filter((name) => name !== layer)
      : [...hidden, layer],
  };
}

export function getTraceColor(layer: string, visibility: LayerVisibility): string | null {
  const base = (() => {
    switch (layer) {
      case 'Top':
        return isLayerVisible('Top', visibility) ? LAYER_COLORS.top_copper : null;
      case 'Bottom':
        return isLayerVisible('Bottom', visibility) ? LAYER_COLORS.bottom_copper : null;
      default:
        return innerLayerColor(layer, visibility);
    }
  })();

  return applyFocus(base, layer === visibility.activeLayer, visibility.focus);
}

/**
 * What a colour becomes once the focus mode has had its say.
 *
 * Hiding wins over dimming and both leave the active layer alone. A caller
 * that has already decided the layer is invisible stays invisible: focus
 * decides how loudly something is drawn, never whether a hidden layer comes
 * back.
 */
export function applyFocus(
  base: string | null,
  isActive: boolean,
  focus: LayerFocus | undefined,
): string | null {
  if (base === null) return null;
  if (isActive || !focus || focus === 'all') return base;
  if (focus === 'solo') return null;
  if (focus === 'ghost') return GHOST_GREY;
  return colorWithAlpha(base, DIMMED_ALPHA);
}

/**
 * The order copper is painted in, so the layer being worked on ends up on top.
 *
 * Two things were wrong with drawing it in a fixed order. The inner layers
 * were painted last, so on a four-layer board `Inner1` covered both outer
 * ones - the opposite of the stack it is meant to represent. And the active
 * layer had no priority at all, so drawing on the bottom of a board meant
 * watching the top copper paint over the trace under the cursor.
 *
 * Stack order first, which is what a board actually looks like from above,
 * then the active layer again at the end. A person can always see what they
 * are drawing.
 */
export function copperDrawOrder(
  present: readonly string[],
  activeLayer: string | undefined,
): string[] {
  const stack = [...present].sort(layerDepth);
  if (!activeLayer || !stack.includes(activeLayer)) return stack;
  return [...stack.filter((name) => name !== activeLayer), activeLayer];
}

/** Bottom first, then the inner layers deepest-last, then top. */
function layerDepth(a: string, b: string): number {
  const depth = (name: string): number => {
    if (name === 'Bottom') return -1;
    if (name === 'Top') return 1000;
    const inner = /^Inner(\d+)$/.exec(name);
    return inner ? Number(inner[1]) : 500;
  };
  return depth(a) - depth(b);
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
  if (!isLayerVisible(layer, visibility)) return null;

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
