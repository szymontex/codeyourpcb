/**
 * EasyEDA Footprint Parser
 *
 * Parses PAD and silkscreen shapes from EasyEDA component API responses.
 * EasyEDA Standard uses tilde-delimited shape strings with `#@$` separating
 * shapes within a footprint (LIB block).
 *
 * Unit system: 1 EasyEDA unit = 10 mil = 0.254 mm = 254,000 nm
 * Layer mapping: 1=TopCopper, 2=BottomCopper, 3=TopSilk, 4=BottomSilk, 11=MultiLayer (THT)
 *
 * Reference: https://docs.easyeda.com/en/DocumentFormat/EasyEDA-Format-Standard/
 */

import type { PadInfo, SilkShape } from './types';

/** EasyEDA unit → nanometers (1 unit = 10 mil = 254,000 nm) */
const EEDA_TO_NM = 254_000;

/**
 * Parsed footprint data from EasyEDA component response.
 */
export interface EasyEDAFootprint {
  /** Pad definitions converted to PadInfo format */
  pads: PadInfo[];
  /** Silkscreen shapes (outlines, markers) */
  silk: SilkShape[];
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

      // Extract origin from head (used for standalone PADs not wrapped in LIB)
      const head = item?.packageDetail?.dataStr?.head;
      const headOriginX = parseFloat(head?.x) || 0;
      const headOriginY = parseFloat(head?.y) || 0;

      let modelUuid: string | null = null;
      const allPads: PadInfo[] = [];
      const allSilk: SilkShape[] = [];
      let originX = headOriginX;
      let originY = headOriginY;
      let hasLIB = false;

      for (const shape of shapes) {
        if (typeof shape !== 'string') continue;

        // Check for 3D model UUID in SVGNODE entries
        if (shape.includes('outline3D') || shape.includes('3D')) {
          const uuidMatch = shape.match(/"uuid"\s*:\s*"([a-f0-9]{32})"/i);
          if (uuidMatch) {
            modelUuid = uuidMatch[1];
          }
        }

        // Parse LIB blocks (footprint containers — older format)
        if (shape.startsWith('LIB~')) {
          const { pads, silk, ox, oy } = parseLIBBlock(shape);
          if (pads.length > 0) {
            allPads.push(...pads);
            originX = ox;
            originY = oy;
            hasLIB = true;
          }
          allSilk.push(...silk);
          continue;
        }

        const ox = hasLIB ? 0 : headOriginX;
        const oy = hasLIB ? 0 : headOriginY;

        // Standalone PAD entries (v6 format — no LIB wrapper)
        if (shape.startsWith('PAD~')) {
          const pad = parsePADShape(shape, ox, oy);
          if (pad) allPads.push(pad);
        }

        // Silkscreen shapes: TRACK on layer 3/4, CIRCLE on layer 3/4, ARC on layer 3/4
        if (shape.startsWith('TRACK~')) {
          const silk = parseSilkTRACK(shape, ox, oy);
          allSilk.push(...silk);
        }
        if (shape.startsWith('CIRCLE~')) {
          const silk = parseSilkCIRCLE(shape, ox, oy);
          if (silk) allSilk.push(silk);
        }
        if (shape.startsWith('ARC~')) {
          const silk = parseSilkARC(shape, ox, oy);
          if (silk) allSilk.push(silk);
        }
      }

      if (allPads.length > 0) {
        return { pads: allPads, silk: allSilk, modelUuid, originX, originY };
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
function parseLIBBlock(libStr: string): { pads: PadInfo[]; silk: SilkShape[]; ox: number; oy: number } {
  const pads: PadInfo[] = [];
  const silk: SilkShape[] = [];

  const parts = libStr.split('#@$');
  const header = parts[0];

  const headerFields = header.split('~');
  const ox = parseFloat(headerFields[1]) || 0;
  const oy = parseFloat(headerFields[2]) || 0;

  for (let i = 1; i < parts.length; i++) {
    const subShape = parts[i];
    if (subShape.startsWith('PAD~')) {
      const pad = parsePADShape(subShape, ox, oy);
      if (pad) pads.push(pad);
    }
    if (subShape.startsWith('TRACK~')) {
      silk.push(...parseSilkTRACK(subShape, ox, oy));
    }
    if (subShape.startsWith('CIRCLE~')) {
      const s = parseSilkCIRCLE(subShape, ox, oy);
      if (s) silk.push(s);
    }
    if (subShape.startsWith('ARC~')) {
      const s = parseSilkARC(subShape, ox, oy);
      if (s) silk.push(s);
    }
  }

  return { pads, silk, ox, oy };
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

// ---------------------------------------------------------------------------
// Silkscreen shape parsers
// ---------------------------------------------------------------------------

/** Map EasyEDA layer ID to silk layer. Returns null if not a silk layer. */
function silkLayer(layerId: string): 'top' | 'bottom' | null {
  if (layerId === '3') return 'top';
  if (layerId === '4') return 'bottom';
  return null;
}

/**
 * Parse TRACK on silk layer into line segments.
 * Format: TRACK~WIDTH~LAYER~NET~x1 y1 x2 y2 ...~GID~LOCKED
 */
function parseSilkTRACK(trackStr: string, ox: number, oy: number): SilkShape[] {
  const fields = trackStr.split('~');
  if (fields.length < 5) return [];

  const layer = silkLayer(fields[2]);
  if (!layer) return [];

  const width = parseFloat(fields[1]) * EEDA_TO_NM;
  const coords = fields[4].trim().split(/\s+/).map(Number);
  const segments: SilkShape[] = [];

  for (let i = 0; i < coords.length - 2; i += 2) {
    const x1 = (coords[i] - ox) * EEDA_TO_NM;
    const y1 = (coords[i + 1] - oy) * EEDA_TO_NM;
    const x2 = (coords[i + 2] - ox) * EEDA_TO_NM;
    const y2 = (coords[i + 3] - oy) * EEDA_TO_NM;

    if (!isNaN(x1) && !isNaN(y1) && !isNaN(x2) && !isNaN(y2)) {
      segments.push({
        type: 'segment',
        x1: Math.round(x1), y1: Math.round(y1),
        x2: Math.round(x2), y2: Math.round(y2),
        width: Math.round(width),
        layer,
      });
    }
  }

  return segments;
}

/**
 * Parse CIRCLE on silk layer.
 * Format: CIRCLE~CX~CY~RADIUS~WIDTH~LAYER~GID~LOCKED~~
 */
function parseSilkCIRCLE(circleStr: string, ox: number, oy: number): SilkShape | null {
  const fields = circleStr.split('~');
  if (fields.length < 6) return null;

  // CIRCLE fields: [0]=CIRCLE [1]=cx [2]=cy [3]=radius [4]=width [5]=layer
  const layer = silkLayer(fields[5]);
  if (!layer) return null;

  const cx = (parseFloat(fields[1]) - ox) * EEDA_TO_NM;
  const cy = (parseFloat(fields[2]) - oy) * EEDA_TO_NM;
  const radius = parseFloat(fields[3]) * EEDA_TO_NM;
  const width = parseFloat(fields[4]) * EEDA_TO_NM;

  if (isNaN(cx) || isNaN(cy) || isNaN(radius)) return null;

  return {
    type: 'circle',
    cx: Math.round(cx), cy: Math.round(cy),
    radius: Math.round(radius),
    width: Math.round(width),
    layer,
  };
}

/**
 * Parse ARC on silk layer.
 * Format: ARC~WIDTH~LAYER~NET~M sx sy A rx ry 0 farFlag cwFlag ex ey~...~GID
 * SVG arc path notation.
 */
function parseSilkARC(arcStr: string, ox: number, oy: number): SilkShape | null {
  const fields = arcStr.split('~');
  if (fields.length < 5) return null;

  const layer = silkLayer(fields[2]);
  if (!layer) return null;

  const width = parseFloat(fields[1]) * EEDA_TO_NM;
  const pathData = fields[4];

  // Parse SVG arc: M sx sy A rx ry rotation largeArcFlag sweepFlag ex ey
  const mMatch = pathData.match(/M\s*([-\d.]+)\s+([-\d.]+)/);
  const aMatch = pathData.match(/A\s*([-\d.]+)\s+([-\d.]+)\s+([-\d.]+)\s+(\d)\s+(\d)\s+([-\d.]+)\s+([-\d.]+)/);
  if (!mMatch || !aMatch) return null;

  const sx = (parseFloat(mMatch[1]) - ox) * EEDA_TO_NM;
  const sy = (parseFloat(mMatch[2]) - oy) * EEDA_TO_NM;
  const rx = parseFloat(aMatch[1]) * EEDA_TO_NM;
  const ry = parseFloat(aMatch[2]) * EEDA_TO_NM;
  const largeArc = aMatch[4] === '1';
  const sweep = aMatch[5] === '1';
  const ex = (parseFloat(aMatch[6]) - ox) * EEDA_TO_NM;
  const ey = (parseFloat(aMatch[7]) - oy) * EEDA_TO_NM;

  // Convert SVG arc to center + angles for canvas rendering
  const arc = svgArcToCenter(sx, sy, rx, ry, largeArc, sweep, ex, ey);
  if (!arc) return null;

  return {
    type: 'arc',
    cx: Math.round(arc.cx),
    cy: Math.round(arc.cy),
    radius: Math.round((rx + ry) / 2), // average for elliptical arcs
    startAngle: arc.startAngle,
    endAngle: arc.endAngle,
    width: Math.round(width),
    layer,
  };
}

/**
 * Convert SVG arc parameters to center-point arc (for Canvas arc()).
 * Based on the SVG spec's conversion algorithm.
 */
function svgArcToCenter(
  x1: number, y1: number, rx: number, ry: number,
  largeArc: boolean, sweep: boolean,
  x2: number, y2: number,
): { cx: number; cy: number; startAngle: number; endAngle: number } | null {
  const dx = (x1 - x2) / 2;
  const dy = (y1 - y2) / 2;

  // Use average radius for simplicity (circular approximation)
  const r = (Math.abs(rx) + Math.abs(ry)) / 2;
  if (r < 1) return null; // degenerate

  const mx = (x1 + x2) / 2;
  const my = (y1 + y2) / 2;
  const d = Math.sqrt(dx * dx + dy * dy);

  if (d > 2 * r) {
    // Points too far apart — just use midpoint
    return { cx: mx, cy: my, startAngle: 0, endAngle: Math.PI * 2 };
  }

  const h = Math.sqrt(Math.max(0, r * r - d * d));

  // Choose center side based on largeArc and sweep flags
  const sign = (largeArc !== sweep) ? 1 : -1;
  const cx = mx + sign * h * dy / d;
  const cy = my - sign * h * dx / d;

  const startAngle = Math.atan2(y1 - cy, x1 - cx);
  let endAngle = Math.atan2(y2 - cy, x2 - cx);

  // Ensure correct sweep direction
  if (sweep) {
    if (endAngle < startAngle) endAngle += Math.PI * 2;
  } else {
    if (endAngle > startAngle) endAngle -= Math.PI * 2;
  }

  return { cx, cy, startAngle, endAngle };
}
