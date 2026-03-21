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
  /** 3D model offset from footprint center, in mm (X) */
  model3dOffsetX: number;
  /** 3D model offset from footprint center, in mm (Y) */
  model3dOffsetY: number;
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

      // Extract origin from head (used for standalone PADs not wrapped in LIB)
      const head = item?.packageDetail?.dataStr?.head;
      const headOriginX = parseFloat(head?.x) || 0;
      const headOriginY = parseFloat(head?.y) || 0;

      let modelUuid: string | null = null;
      let model3dOriginX = 0;
      let model3dOriginY = 0;
      const allPads: PadInfo[] = [];
      let originX = headOriginX;
      let originY = headOriginY;
      let hasLIB = false;

      for (const shape of shapes) {
        if (typeof shape !== 'string') continue;

        // Check for 3D model UUID and origin in SVGNODE entries
        if (shape.includes('outline3D') || shape.includes('3D')) {
          const uuidMatch = shape.match(/"uuid"\s*:\s*"([a-f0-9]{32})"/i);
          if (uuidMatch) {
            modelUuid = uuidMatch[1];
          }
          // Extract c_origin — the 3D model's placement point in EasyEDA coordinates
          const originMatch = shape.match(/"c_origin"\s*:\s*"([^"]+)"/);
          if (originMatch) {
            const [ox, oy] = originMatch[1].split(',').map(Number);
            if (!isNaN(ox) && !isNaN(oy)) {
              model3dOriginX = ox;
              model3dOriginY = oy;
            }
          }
        }

        // Parse LIB blocks (footprint containers — older format)
        // Format: LIB~X~Y~package`NAME`...#@$PAD~...#@$PAD~...
        if (shape.startsWith('LIB~')) {
          const { pads, ox, oy } = parseLIBBlock(shape);
          if (pads.length > 0) {
            allPads.push(...pads);
            originX = ox;
            originY = oy;
            hasLIB = true;
          }
          continue;
        }

        // Standalone PAD entries (v6 format — no LIB wrapper)
        // Origin comes from head.x, head.y
        if (shape.startsWith('PAD~')) {
          const pad = parsePADShape(shape, hasLIB ? 0 : headOriginX, hasLIB ? 0 : headOriginY);
          if (pad) allPads.push(pad);
        }
      }

      if (allPads.length > 0) {
        // Compute 3D model offset from footprint center, in mm.
        // Following KiCad's approach: offset = -(c_origin - headOrigin) * scale
        // The negation is needed because c_origin specifies where the footprint
        // center maps to in the 3D model's space, so the model needs to shift
        // in the opposite direction.
        // 1 EasyEDA unit = 10 mil = 0.254 mm
        let model3dOffsetX = 0;
        let model3dOffsetY = 0;
        if (model3dOriginX !== 0 || model3dOriginY !== 0) {
          model3dOffsetX = -(model3dOriginX - headOriginX) * 0.254;
          model3dOffsetY = -(model3dOriginY - headOriginY) * 0.254;
        }

        return { pads: allPads, modelUuid, originX, originY, model3dOffsetX, model3dOffsetY };
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
