/**
 * DIRECTION_45 — KiCad-compatible 45-degree direction system.
 *
 * Port of KiCad's DIRECTION_45 class and BuildInitialTrace algorithm.
 * This is the core of KiCad's interactive routing behavior:
 * traces are constrained to H/V/45° segments, built as 2-segment
 * paths (one H/V + one diagonal, or vice versa).
 *
 * Reference: kicad/libs/kimath/include/geometry/direction45.h
 *            kicad/libs/kimath/src/geometry/direction_45.cpp
 */

// ---------------------------------------------------------------------------
// Direction enum — 8 octants + undefined
// ---------------------------------------------------------------------------

export enum Dir45 {
  N = 0,
  NE = 1,
  E = 2,
  SE = 3,
  S = 4,
  SW = 5,
  W = 6,
  NW = 7,
  UNDEFINED = -1,
}

export enum CornerMode {
  MITERED_45 = 0,  // H/V/45 with sharp corners (default KiCad)
  MITERED_90 = 2,  // H/V only (90-degree corners)
}

export enum AngleType {
  ANG_OBTUSE = 0x01,
  ANG_RIGHT = 0x02,
  ANG_ACUTE = 0x04,
  ANG_STRAIGHT = 0x08,
  ANG_HALF_FULL = 0x10,
  ANG_UNDEFINED = 0x20,
}

export interface Vec2 {
  x: number;
  y: number;
}

// ---------------------------------------------------------------------------
// Direction from vector
// ---------------------------------------------------------------------------

function sign(v: number): number {
  return v > 0 ? 1 : v < 0 ? -1 : 0;
}

/**
 * Compute the closest DIRECTION_45 from a vector.
 * KiCad flips Y because screen Y is down but "north" is up.
 */
function dirFromVec(vec: Vec2): Dir45 {
  if (vec.x === 0 && vec.y === 0) return Dir45.UNDEFINED;

  // KiCad: vec.y = -vec.y (screen coords → compass)
  const vy = -vec.y;
  const vx = vec.x;

  let mag = 360.0 - (180.0 / Math.PI * Math.atan2(vy, vx)) + 90.0;
  if (mag >= 360.0) mag -= 360.0;
  if (mag < 0.0) mag += 360.0;

  let dir = Math.floor((mag + 22.5) / 45.0);
  if (dir >= 8) dir -= 8;
  if (dir < 0) dir += 8;

  return dir as Dir45;
}

export function dirFromSeg(a: Vec2, b: Vec2): Dir45 {
  return dirFromVec({ x: b.x - a.x, y: b.y - a.y });
}

// ---------------------------------------------------------------------------
// Direction queries
// ---------------------------------------------------------------------------

export function isDiagonal(dir: Dir45): boolean {
  if (dir === Dir45.UNDEFINED) return false;
  return (dir % 2) === 1;
}

/** Turn right by 45° (or 90° if is90deg). */
function rightDir(dir: Dir45, is90deg = false): Dir45 {
  if (dir === Dir45.UNDEFINED) return Dir45.UNDEFINED;
  const step = is90deg ? 2 : 1;
  return ((dir + step) % 8) as Dir45;
}

/** Turn left by 45° (or 90° if is90deg). */

export function angleBetween(a: Dir45, b: Dir45): AngleType {
  if (a === Dir45.UNDEFINED || b === Dir45.UNDEFINED) return AngleType.ANG_UNDEFINED;
  const d = Math.abs(a - b);
  if (d === 1 || d === 7) return AngleType.ANG_OBTUSE;
  if (d === 2 || d === 6) return AngleType.ANG_RIGHT;
  if (d === 3 || d === 5) return AngleType.ANG_ACUTE;
  if (d === 4) return AngleType.ANG_HALF_FULL;
  return AngleType.ANG_STRAIGHT;
}

// ---------------------------------------------------------------------------
// BuildInitialTrace — the heart of KiCad's routing behavior
// ---------------------------------------------------------------------------

/**
 * Build a 2-segment trace from p0 to p1 following 45-degree routing.
 *
 * Returns an array of points forming the trace path (2 or 3 points).
 * `startDiagonal` controls posture: if true, first segment is diagonal.
 *
 * This is a direct port of KiCad's DIRECTION_45::BuildInitialTrace.
 *
 * Behavior for MITERED_45 (default KiCad mode):
 * - If w > h: horizontal segment then 45° diagonal to target
 * - If h > w: vertical segment then 45° diagonal to target
 * - startDiagonal flips: diagonal first, then H/V
 *
 * Example (w > h, startDiagonal = false):
 *   p0 ──────────── mp0
 *                      \
 *                       \
 *                        p1
 *
 * Example (w > h, startDiagonal = true):
 *   p0
 *     \
 *      \
 *       mp1 ──────────── p1
 */
export function buildInitialTrace(
  p0: Vec2,
  p1: Vec2,
  dir: Dir45,
  startDiagonalOverride?: boolean,
  mode: CornerMode = CornerMode.MITERED_45,
): Vec2[] {
  let startDiagonal: boolean;

  if (dir === Dir45.UNDEFINED) {
    startDiagonal = startDiagonalOverride ?? false;
  } else {
    startDiagonal = isDiagonal(dir);
  }

  const w = Math.abs(p1.x - p0.x);
  const h = Math.abs(p1.y - p0.y);
  const sw = sign(p1.x - p0.x);
  const sh = sign(p1.y - p0.y);

  // Shortcut: pure H, V, or 45° — single segment
  if (w === 0 || h === 0 || (mode !== CornerMode.MITERED_90 && h === w)) {
    return [{ ...p0 }, { ...p1 }];
  }

  if (mode === CornerMode.MITERED_90) {
    // 90° mode: H then V, or V then H
    let mp: Vec2;
    if (startDiagonal === (h >= w)) {
      mp = { x: p0.x + w * sw, y: p0.y }; // horizontal first
    } else {
      mp = { x: p0.x, y: p0.y + sh * h }; // vertical first
    }
    return [{ ...p0 }, { x: p0.x + mp.x, y: p0.y + mp.y }, { ...p1 }];
  }

  // MITERED_45 mode — the default KiCad behavior
  let mp0: Vec2;
  let mp1: Vec2;

  if (w > h) {
    mp0 = { x: (w - h) * sw, y: 0 };         // horizontal segment
    mp1 = { x: h * sw, y: h * sh };           // diagonal segment
  } else {
    mp0 = { x: 0, y: sh * (h - w) };          // vertical segment
    mp1 = { x: sw * w, y: sh * w };           // diagonal segment
  }

  const midpoint = startDiagonal
    ? { x: p0.x + mp1.x, y: p0.y + mp1.y }
    : { x: p0.x + mp0.x, y: p0.y + mp0.y };

  return [{ ...p0 }, midpoint, { ...p1 }];
}

// ---------------------------------------------------------------------------
// Mouse Trail Tracer — automatic posture detection
// ---------------------------------------------------------------------------

/**
 * Tracks mouse movement to automatically determine whether the user
 * intends a "straight-first" or "diagonal-first" trace posture.
 *
 * Port of KiCad's PNS::MOUSE_TRAIL_TRACER.
 */
export class MouseTrailTracer {
  private trail: Vec2[] = [];
  private tolerance = 500_000; // 0.5mm in nm
  private direction: Dir45 = Dir45.UNDEFINED;
  private lastSegDirection: Dir45 = Dir45.UNDEFINED;
  private forced = false;
  private manuallyForced = false;

  clear(): void {
    this.forced = false;
    this.manuallyForced = false;
    this.trail = [];
  }

  setTolerance(tol: number): void {
    this.tolerance = tol;
  }

  setDefaultDirections(initDir: Dir45, lastSegDir: Dir45): void {
    this.direction = initDir;
    this.lastSegDirection = lastSegDir;
  }

  addTrailPoint(p: Vec2): void {
    if (this.trail.length === 0) {
      this.trail.push({ ...p });
      return;
    }

    // If trail loops back near a previous point, truncate
    if (this.trail.length > 2) {
      const limit = this.tolerance * this.tolerance;
      for (let i = 0; i < this.trail.length - 2; i++) {
        const seg = this.trail[i];
        const dx = p.x - seg.x;
        const dy = p.y - seg.y;
        if (dx * dx + dy * dy <= limit) {
          this.trail = this.trail.slice(0, i + 1);
          break;
        }
      }
    }

    this.trail.push({ ...p });
  }

  /**
   * Determine the best posture (direction) given the mouse trail.
   * Returns the direction for the first segment of the trace.
   */
  getPosture(p: Vec2): Dir45 {
    const areaRatioThreshold = 1.3;
    const areaRatioEpsilon = 0.25;
    const minAreaCutoffDistanceFactor = 6;
    const lockDistanceFactor = 30;
    const unlockDistanceFactor = 10;

    if (this.trail.length < 2 || this.manuallyForced) {
      if (!this.manuallyForced && this.lastSegDirection !== Dir45.UNDEFINED) {
        this.direction = this.lastSegDirection;
      }
      return this.direction;
    }

    const p0 = this.trail[0];
    const refLength = Math.hypot(p.x - p0.x, p.y - p0.y);

    // Build "straight-first" and "diagonal-first" traces, then compare
    // which one the actual mouse trail matches better (smaller enclosed area)
    const straightTrace = buildInitialTrace(p0, p, Dir45.UNDEFINED, false);
    const diagTrace = buildInitialTrace(p0, p, Dir45.UNDEFINED, true);

    const areaS = this.computeEnclosedArea(straightTrace, this.trail);
    const areaDiag = this.computeEnclosedArea(diagTrace, this.trail);
    const ratio = areaS / (areaDiag + 1.0);

    // Detect cursor dragged back to start — reset
    if (this.forced && refLength < unlockDistanceFactor * this.tolerance) {
      this.forced = false;
      this.trail = [{ ...p0 }];
    }

    let areaOk = false;
    if (!this.forced && refLength > minAreaCutoffDistanceFactor * this.tolerance) {
      const areaCutoff = this.tolerance * refLength;
      const trailArea = this.computeTrailArea();
      if (trailArea > areaCutoff) areaOk = true;
    }

    const straightDir = straightTrace.length >= 2
      ? dirFromSeg(straightTrace[0], straightTrace[1])
      : Dir45.UNDEFINED;
    const diagDir = diagTrace.length >= 2
      ? dirFromSeg(diagTrace[0], diagTrace[1])
      : Dir45.UNDEFINED;

    if (!this.forced && areaOk && ratio > areaRatioThreshold + areaRatioEpsilon) {
      this.direction = diagDir;
    } else if (!this.forced && areaOk && ratio < (1.0 / areaRatioThreshold) - areaRatioEpsilon) {
      this.direction = straightDir;
    } else {
      this.direction = isDiagonal(this.direction) ? diagDir : straightDir;
    }

    // Lock the solution far from start
    if (!this.forced && refLength > lockDistanceFactor * this.tolerance) {
      this.forced = true;
    }

    return this.direction;
  }

  /**
   * Flip posture: straight ↔ diagonal. (KiCad: '/' key)
   */
  flipPosture(): void {
    this.direction = rightDir(this.direction);
    this.forced = true;
    this.manuallyForced = true;
  }

  isManuallyForced(): boolean {
    return this.manuallyForced;
  }

  private computeEnclosedArea(tracePts: Vec2[], trail: Vec2[]): number {
    // Build closed polygon: trace forward + trail reversed
    const polygon = [...tracePts, ...[...trail].reverse()];
    return Math.abs(this.shoelace(polygon));
  }

  private computeTrailArea(): number {
    if (this.trail.length < 3) return 0;
    return Math.abs(this.shoelace(this.trail));
  }

  private shoelace(pts: Vec2[]): number {
    let area = 0;
    const n = pts.length;
    for (let i = 0; i < n; i++) {
      const j = (i + 1) % n;
      area += pts[i].x * pts[j].y;
      area -= pts[j].x * pts[i].y;
    }
    return area / 2;
  }
}
