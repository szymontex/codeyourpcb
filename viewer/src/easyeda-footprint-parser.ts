/**
 * EasyEDA Footprint Parser
 *
 * Parses PAD shapes from EasyEDA component API responses into PadInfo[].
 * EasyEDA Standard uses tilde-delimited shape strings with `#@$` separating
 * shapes within a footprint (LIB block).
 *
 * Unit system: 1 EasyEDA unit = 10 mil = 0.254 mm = 254,000 nm
 * Layer mapping: 1=TopCopper, 2=BottomCopper, 11=MultiLayer (THT)
 *
 * Reference: https://docs.easyeda.com/en/DocumentFormat/EasyEDA-Format-Standard/
 */

import type { PadInfo } from './types';

/** EasyEDA unit → nanometers (1 unit = 10 mil = 254,000 nm) */
const EEDA_TO_NM = 254_000;

/**
 * Parsed footprint data from EasyEDA component response.
 */
export interface EasyEDAFootprint {
  /** Pad definitions converted to PadInfo format */
  pads: PadInfo[];
  /** 3D model UUID (null if no 3D model) */
  modelUuid: string | null;
  /** Footprint origin X in nm (from LIB header) */
  originX: number;
  /** Footprint origin Y in nm (from LIB header) */
  originY: number;
}

/**
 * Extract footprint data (pads + 3D model UUID) from EasyEDA component API response.
 *
 * The API response structure:
 *   result[].packageDetail.dataStr.shape — array of tilde-delimited shape strings
 *
 * Returns null if no usable footprint data is found.
 */
export function parseEasyEDAFootprint(compData: any): EasyEDAFootprint | null {
  try {
    const result = compData?.result;
    if (!result) return null;

    const items = Array.isArray(result) ? result : [result];

    for (const item of items) {
      const shapes = item?.packageDetail?.dataStr?.shape;
      if (!Array.isArray(shapes)) continue;

      let modelUuid: string | null = null;
      const allPads: PadInfo[] = [];
      let originX = 0;
      let originY = 0;

      for (const shape of shapes) {
        if (typeof shape !== 'string') continue;

        // Check for 3D model UUID
        if (shape.includes('outline3D') || shape.includes('3D')) {
          const uuidMatch = shape.match(/"uuid"\s*:\s*"([a-f0-9]{32})"/i);
          if (uuidMatch) {
            modelUuid = uuidMatch[1];
          }
        }

        // Parse LIB blocks (footprint containers)
        // Format: LIB~X~Y~package`NAME`...#@$PAD~...#@$PAD~...
        if (shape.startsWith('LIB~')) {
          const { pads, ox, oy } = parseLIBBlock(shape);
          if (pads.length > 0) {
            allPads.push(...pads);
            originX = ox;
            originY = oy;
          }
          continue;
        }

        // Standalone PAD entries (outside LIB blocks)
        if (shape.startsWith('PAD~')) {
          const pad = parsePADShape(shape, 0, 0);
          if (pad) allPads.push(pad);
        }
      }

      if (allPads.length > 0) {
        return { pads: allPads, modelUuid, originX, originY };
      }
    }

    return null;
  } catch (e) {
    console.error('[EasyEDA] Footprint parse error:', e);
    return null;
  }
}

/**
 * Parse a LIB block containing sub-shapes separated by #@$.
 * LIB format: LIB~X~Y~package`NAME`...~...~gId~...
 * Sub-shapes: #@$PAD~SHAPE~X~Y~W~H~LAYER~NET~NUM~HOLER~...~GID
 */
function parseLIBBlock(libStr: string): { pads: PadInfo[]; ox: number; oy: number } {
  const pads: PadInfo[] = [];

  // Split on #@$ to get sub-shapes
  const parts = libStr.split('#@$');
  const header = parts[0]; // LIB~X~Y~...

  // Extract origin from LIB header
  const headerFields = header.split('~');
  const ox = parseFloat(headerFields[1]) || 0;
  const oy = parseFloat(headerFields[2]) || 0;

  // Parse each sub-shape
  for (let i = 1; i < parts.length; i++) {
    const subShape = parts[i];
    if (subShape.startsWith('PAD~')) {
      const pad = parsePADShape(subShape, ox, oy);
      if (pad) pads.push(pad);
    }
  }

  return { pads, ox, oy };
}

/**
 * Parse a single PAD shape string into PadInfo.
 *
 * PAD format (tilde-delimited):
 *   PAD~SHAPE~X~Y~WIDTH~HEIGHT~LAYERID~NET~NUMBER~HOLER~POINTARR~ROTATION~GID
 *
 * Additional optional fields after GID:
 *   ~HOLELENGTH~SLOTPOINTARR~PLATED~LOCKED~PASTEEXPANSION~SOLDEREXPANSION~HOLECENTER
 *
 * SHAPE values: ELLIPSE, RECT, OVAL, POLYGON
 * LAYERID: 1=TopCopper, 2=BottomCopper, 11=MultiLayer(THT)
 * Coordinates are absolute in EasyEDA units; we subtract origin to get relative.
 */
function parsePADShape(padStr: string, originX: number, originY: number): PadInfo | null {
  const fields = padStr.split('~');
  if (fields.length < 10) return null;

  const shapeType = fields[1]; // ELLIPSE, RECT, OVAL, POLYGON
  const absX = parseFloat(fields[2]);
  const absY = parseFloat(fields[3]);
  const width = parseFloat(fields[4]);
  const height = parseFloat(fields[5]);
  const layerId = fields[6];
  // fields[7] = net (empty for footprint definitions)
  const number = fields[8];
  const holeR = parseFloat(fields[9]) || 0;

  if (isNaN(absX) || isNaN(absY) || isNaN(width) || isNaN(height)) return null;
  if (!number) return null;

  // Convert coordinates relative to footprint origin, then to nanometers
  const relX = (absX - originX) * EEDA_TO_NM;
  const relY = (absY - originY) * EEDA_TO_NM;
  const widthNm = width * EEDA_TO_NM;
  const heightNm = height * EEDA_TO_NM;

  // Hole radius → diameter in nm (holeR is radius in EasyEDA units)
  const drillNm = holeR > 0 ? Math.round(holeR * 2 * EEDA_TO_NM) : null;

  // Map EasyEDA shape to our shape names
  let shape: string;
  switch (shapeType) {
    case 'ELLIPSE':
      shape = drillNm ? 'circle' : 'circle';
      break;
    case 'RECT':
      shape = 'rect';
      break;
    case 'OVAL':
      shape = 'oblong';
      break;
    case 'POLYGON':
      shape = 'rect'; // approximate
      break;
    default:
      shape = 'rect';
  }

  // Layer mask: 1=TopCopper(SMD top), 2=BottomCopper(SMD bottom), 3=both(THT)
  let layerMask: number;
  switch (layerId) {
    case '1':
      layerMask = 1; // Top only
      break;
    case '2':
      layerMask = 2; // Bottom only
      break;
    case '11':
      layerMask = 3; // Multi-layer (through-hole)
      break;
    default:
      layerMask = drillNm ? 3 : 1; // Infer from drill
  }

  return {
    number,
    x_nm: Math.round(relX),
    y_nm: Math.round(relY),
    width_nm: Math.round(widthNm),
    height_nm: Math.round(heightNm),
    shape,
    layer_mask: layerMask,
    drill_nm: drillNm ? Math.round(drillNm) : null,
  };
}
