/**
 * Render configuration — boundary contract for renderer, preferences (S04),
 * and routing pad highlighting (S03).
 *
 * Defines LOD tiers, layer colors, font sizing, and the pad-to-net lookup
 * that enables per-pad net highlighting.
 */

import type { NetInfo } from './types';

// ---------------------------------------------------------------------------
// LOD tiers
// ---------------------------------------------------------------------------

export enum LodTier {
  /** Shapes only, no text at all */
  Far = 0,
  /** Refdes labels + body outlines + drill marks */
  Medium = 1,
  /** Pad pin numbers + net labels on traces */
  Close = 2,
  /** Full detail — all annotations */
  Detail = 3,
}

// ---------------------------------------------------------------------------
// RenderConfig interface
// ---------------------------------------------------------------------------

export interface RenderConfig {
  layerColors: {
    topCopper: string;
    bottomCopper: string;
    silkscreen: string;
    via: string;
    drill: string;
  };

  fontConfig: {
    /** Refdes text size in nanometers (world-space). 800_000 = 0.8mm */
    refdesWorldSize: number;
    /** Minimum pad screen-pixel width before pin numbers are drawn */
    padNumberMinScreenPx: number;
    /** Minimum trace segment screen-pixel length before net labels are drawn */
    netLabelMinSegmentPx: number;
  };

  lodThresholds: {
    /** Scale above which we enter Medium tier (from Far) */
    medium: number;
    /** Scale above which we enter Close tier */
    close: number;
    /** Scale above which we enter Detail tier */
    detail: number;
  };
}

// ---------------------------------------------------------------------------
// Defaults factory
// ---------------------------------------------------------------------------

/**
 * Create a RenderConfig with sensible defaults.
 *
 * LOD thresholds are calibrated for the viewport's px/nm scale:
 * - scale 0.0001 = 1mm → 100px (default zoom)
 * - scale 0.00003 = 1mm → 30px (zoomed out)
 * - scale 0.0003 = 1mm → 300px (zoomed in)
 */
export function createDefaultRenderConfig(): RenderConfig {
  return {
    layerColors: {
      topCopper: '#C41E1E',
      bottomCopper: '#1E1EC4',
      silkscreen: '#F0F0F0',
      via: '#C8C800',
      drill: '#1A1A1A',
    },

    fontConfig: {
      refdesWorldSize: 800_000,        // 0.8mm
      padNumberMinScreenPx: 15,
      netLabelMinSegmentPx: 80,
    },

    lodThresholds: {
      medium: 0.000035,   // ~1mm = 35px — body outlines, refdes, drill marks
      close: 0.00008,     // ~1mm = 80px — pad numbers, net labels
      detail: 0.0002,     // ~1mm = 200px — everything, no culling
    },
  };
}

// ---------------------------------------------------------------------------
// LOD tier resolution
// ---------------------------------------------------------------------------

/**
 * Determine the LOD tier for a given viewport scale.
 */
export function getLodTier(scale: number, config: RenderConfig): LodTier {
  if (scale >= config.lodThresholds.detail) return LodTier.Detail;
  if (scale >= config.lodThresholds.close) return LodTier.Close;
  if (scale >= config.lodThresholds.medium) return LodTier.Medium;
  return LodTier.Far;
}

// ---------------------------------------------------------------------------
// Pad-to-net mapping
// ---------------------------------------------------------------------------

/**
 * Build a lookup from "refdes.pin" → net name using NetInfo.connections.
 *
 * Example: if net "VCC" has connection { component: "U1", pin: "1" },
 * the map will contain "U1.1" → "VCC".
 */
export function buildPadNetMap(nets: NetInfo[]): Map<string, string> {
  const map = new Map<string, string>();
  for (const net of nets) {
    if (!net.name) continue;
    for (const conn of net.connections) {
      const key = `${conn.component}.${conn.pin}`;
      map.set(key, net.name);
    }
  }
  return map;
}
