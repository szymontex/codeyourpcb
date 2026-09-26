/**
 * EasyEDA Footprint Parser
 *
 * Parses PAD and silkscreen shapes from EasyEDA component API responses.
 * EasyEDA Standard uses tilde-delimited shape strings with `#@$` separating
 * shapes within a footprint (LIB block).
 *
 * Unit system: 1 EasyEDA unit = 10 mil = 0.254 mm = 254,000 nm
 * Y grows down in EasyEDA and up in a footprint: see `footprintPoint`.
 * Layer mapping: 1=TopCopper, 2=BottomCopper, 3=TopSilk, 4=BottomSilk, 11=MultiLayer (THT)
 *
 * Reference: https://docs.easyeda.com/en/DocumentFormat/EasyEDA-Format-Standard/
 */

import type { PadInfo, SilkShape } from './types';

/** EasyEDA unit → nanometers (1 unit = 10 mil = 254,000 nm) */
const EEDA_TO_NM = 254_000;

/**
 * An EasyEDA point as a footprint point, in nanometres from the origin.
 *
 * EasyEDA draws on an SVG canvas, and its Y grows down the screen. A
 * footprint's Y grows up - the convention `cypcb_world::footprint` states for
 * the board and every footprint on it. So Y is negated here, and every point
 * this parser reads - pad, track, circle, arc - goes through this function.
 * easyeda2kicad (fff10a38, read 2026-09-26) copies EasyEDA's Y into KiCad's
 * unchanged, and KiCad's Y also grows down the sheet.
 *
 * Until 2026-09-26 nothing negated anything, and a part fetched from EasyEDA
 * arrived as its own mirror image: AP2112K-3.3TRG1 in SOT25 counted its pins
 * clockwise, where the datasheet (DS39724 Rev. 2-2) counts them
 * counter-clockwise seen from the top.
 */
function footprintPoint(x: number, y: number, originX: number, originY: number): [number, number] {
  return [(x - originX) * EEDA_TO_NM, (originY - y) * EEDA_TO_NM];
}

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
  /**
   * Pads whose EasyEDA shape this parser has no word for, read as rectangles.
   *
   * A part placed from the JLCPCB panel brings its pads onto the board, so
   * this substitution is copper: the checker measures the rectangle, the
   * router blocks it and the Gerber flashes it. `POLYGON` is a shape somebody
   * drew, and reading it as its bounding rectangle is a guess rather than a
   * smaller description of the same pad. Named here so the caller can say so.
   */
  approximations: string[];
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
      const allApproximations: string[] = [];
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
          const { pads, silk, ox, oy } = parseLIBBlock(shape, allApproximations);
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
          const pad = parsePADShape(shape, ox, oy, allApproximations);
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
        if (shape.startsWith('HOLE~')) {
          const hole = parseHOLEShape(shape, ox, oy);
          if (hole) allPads.push(hole);
        }
      }

      if (allPads.length > 0) {
        return {
          pads: allPads,
          silk: allSilk,
          modelUuid,
          originX,
          originY,
          approximations: allApproximations,
        };
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
function parseLIBBlock(
  libStr: string,
  approximated: string[],
): { pads: PadInfo[]; silk: SilkShape[]; ox: number; oy: number } {
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
      const pad = parsePADShape(subShape, ox, oy, approximated);
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
    if (subShape.startsWith('HOLE~')) {
      const hole = parseHOLEShape(subShape, ox, oy);
      if (hole) pads.push(hole);
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
 *
 * HOLER is a radius. The format document names it `holeR` and describes it
 * as 孔直径, a diameter; measured 2026-09-26 against GCT USB4105 (drawing B4,
 * 18/12/23), the slot the drawing gives as 0.60 x 1.70mm arrives as HOLER
 * 1.378 (0.35mm) and HOLELENGTH 6.6929 (1.70mm).
 *
 * ROTATION turns the pad about its centre. The format document
 * (docs.easyeda.com EasyEDA-Format-Standard, read 2026-09-26) gives no
 * direction for it, so only the quarter turns are read: a rectangle, an oval
 * and a slot turned by 90 or 270 degrees are the same shapes with width and
 * height swapped, whichever way they turned. Any other angle is read as no
 * turn and named in `approximated`.
 */
function parsePADShape(
  padStr: string,
  originX: number,
  originY: number,
  approximated: string[],
): PadInfo | null {
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
  const rotation = parseFloat(fields[11]) || 0;
  const holeLength = parseFloat(fields[13]) || 0;

  if (isNaN(absX) || isNaN(absY) || isNaN(width) || isNaN(height)) return null;
  if (!number) return null;

  const [relX, relY] = footprintPoint(absX, absY, originX, originY);
  let widthNm = width * EEDA_TO_NM;
  let heightNm = height * EEDA_TO_NM;

  // Hole radius → diameter in nm (holeR is radius in EasyEDA units)
  const drillNm = holeR > 0 ? Math.round(holeR * 2 * EEDA_TO_NM) : null;

  // A HOLELENGTH longer than the hole is a slot. It runs along the side of
  // the pad with room for it, before the pad turns - the reading
  // easyeda2kicad's `drill_to_ki` makes (fff10a38, read 2026-09-26), and
  // the one that puts USB4105's 1.70mm slots along its 2.10mm pads.
  let slotNm: [number, number] | null = null;
  if (drillNm && holeLength * EEDA_TO_NM > drillNm) {
    const lengthNm = holeLength * EEDA_TO_NM;
    slotNm = heightNm > widthNm
      ? [drillNm, lengthNm]
      : [lengthNm, drillNm];
  }

  const quarterTurns = ((Math.round(rotation / 90) % 4) + 4) % 4;
  if (Math.abs(rotation - Math.round(rotation / 90) * 90) > 1e-6) {
    // A round pad with a round hole is the same at every angle.
    if (!(shapeType === 'ELLIPSE' && width === height && !slotNm)) {
      approximated.push(`pad ${number} states rotation ${rotation}`);
    }
  } else if (quarterTurns % 2 === 1) {
    [widthNm, heightNm] = [heightNm, widthNm];
    if (slotNm) slotNm = [slotNm[1], slotNm[0]];
  }

  // Map EasyEDA shape to our shape names
  // A shape this parser has no word for becomes a rectangle and says which
  // pad it was. `POLYGON` is the one EasyEDA writes for a pad somebody drew,
  // and the ternary here used to be `drillNm ? 'circle' : 'circle'` - both
  // arms the same, which is a question somebody meant to ask and never did.
  let shape: string;
  switch (shapeType) {
    case 'ELLIPSE':
      shape = 'circle';
      break;
    case 'RECT':
      shape = 'rect';
      break;
    case 'OVAL':
      shape = 'oblong';
      break;
    default:
      shape = 'rect';
      approximated.push(`pad ${number} states shape ${shapeType}`);
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
    ...(slotNm ? { slot_nm: [Math.round(slotNm[0]), Math.round(slotNm[1])] as [number, number] } : {}),
  };
}

/**
 * Parse a HOLE shape: a drilled hole with no copper, which is what a
 * connector's locating pegs sit in.
 *
 * Format: HOLE~X~Y~RADIUS~GID~LOCKED
 *
 * The third field is a radius, like a pad's HOLER. The format document calls
 * it `holeR` and describes it as a diameter; measured 2026-09-26 on two
 * parts against their drawings, it is half the hole: HRO TYPE-C-31-M-12
 * writes 1.1811 (0.300mm) for Ø0.60, GCT USB4105 writes 1.2795 (0.325mm)
 * for Ø0.65.
 *
 * It becomes a pad on no copper layer with a drill, which is how the engine
 * knows a non-plated hole: the drill file lists it apart, and no copper file
 * flashes it.
 */
function parseHOLEShape(holeStr: string, originX: number, originY: number): PadInfo | null {
  const fields = holeStr.split('~');
  if (fields.length < 4) return null;

  const absX = parseFloat(fields[1]);
  const absY = parseFloat(fields[2]);
  const radius = parseFloat(fields[3]);
  if (isNaN(absX) || isNaN(absY) || !(radius > 0)) return null;

  const [relX, relY] = footprintPoint(absX, absY, originX, originY);
  const diameterNm = Math.round(radius * 2 * EEDA_TO_NM);

  return {
    number: '',
    x_nm: Math.round(relX),
    y_nm: Math.round(relY),
    width_nm: diameterNm,
    height_nm: diameterNm,
    shape: 'circle',
    layer_mask: 0,
    drill_nm: diameterNm,
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
    const [x1, y1] = footprintPoint(coords[i], coords[i + 1], ox, oy);
    const [x2, y2] = footprintPoint(coords[i + 2], coords[i + 3], ox, oy);

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

  const [cx, cy] = footprintPoint(parseFloat(fields[1]), parseFloat(fields[2]), ox, oy);
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

  const [sx, sy] = footprintPoint(parseFloat(mMatch[1]), parseFloat(mMatch[2]), ox, oy);
  const rx = parseFloat(aMatch[1]) * EEDA_TO_NM;
  const ry = parseFloat(aMatch[2]) * EEDA_TO_NM;
  const largeArc = aMatch[4] === '1';
  // The sweep flag names the direction of rising angle in EasyEDA's Y-down
  // frame. Negating Y turns that direction round, so the flag turns with it.
  const sweep = aMatch[5] !== '1';
  const [ex, ey] = footprintPoint(parseFloat(aMatch[6]), parseFloat(aMatch[7]), ox, oy);

  // Convert SVG arc to center + angles for canvas rendering
  const arc = svgArcToCenter(sx, sy, rx, ry, largeArc, sweep, ex, ey);
  if (!arc) return null;

  // A silk arc runs counter-clockwise from its start to its end, which is how
  // the canvas draws it. One that runs the other way is the same ink from its
  // end to its start.
  const [startAngle, endAngle] = arc.endAngle >= arc.startAngle
    ? [arc.startAngle, arc.endAngle]
    : [arc.endAngle, arc.startAngle];

  return {
    type: 'arc',
    cx: Math.round(arc.cx),
    cy: Math.round(arc.cy),
    radius: Math.round((rx + ry) / 2), // average for elliptical arcs
    startAngle,
    endAngle,
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
