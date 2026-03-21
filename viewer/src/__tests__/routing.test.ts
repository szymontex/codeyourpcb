import { describe, it, expect, beforeEach } from 'vitest';
import type { BoardSnapshot, ComponentInfo, PadInfo, NetInfo } from '../types';
import {
  createRoutingState,
  startRoute,
  updatePreview,
  completeRoute,
  cancelRoute,
  toggleAngleSnap,
  computeTargetPads,
  findNearestTargetPad,
  resetToIdle,
  type RoutingState,
  type PadHit,
} from '../routing';

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

function makePad(number: string, x: number, y: number, w = 500_000, h = 500_000): PadInfo {
  return {
    number,
    x_nm: x,
    y_nm: y,
    width_nm: w,
    height_nm: h,
    shape: 'rect',
    layer_mask: 0x01, // Top only
    drill_nm: null,
  };
}

function makeComp(refdes: string, x: number, y: number, pads: PadInfo[]): ComponentInfo {
  return {
    refdes,
    value: '',
    x_nm: x,
    y_nm: y,
    rotation_mdeg: 0,
    footprint: '',
    pads,
    body_width_nm: 1_000_000,
    body_height_nm: 1_000_000,
    model_3d: null,
    silk: [],
  };
}

/** Build a minimal snapshot with components and nets */
function makeSnapshot(components: ComponentInfo[], nets: NetInfo[]): BoardSnapshot {
  return {
    board: { name: 'test', width_nm: 50_000_000, height_nm: 50_000_000, layer_count: 2 },
    components,
    nets,
    violations: [],
    traces: [],
    vias: [],
    ratsnest: [],
  };
}

function makePadHit(comp: ComponentInfo, pad: PadInfo, netName: string): PadHit {
  return {
    component: comp,
    pad,
    worldX: comp.x_nm + pad.x_nm,
    worldY: comp.y_nm + pad.y_nm,
    netName,
  };
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

// Two components, each with a pad on net "VCC" and a pad on net "GND"
const compA = makeComp('U1', 10_000_000, 10_000_000, [
  makePad('1', 0, 0),          // U1.1 — will be on VCC
  makePad('2', 1_000_000, 0),  // U1.2 — will be on GND
]);

const compB = makeComp('R1', 20_000_000, 10_000_000, [
  makePad('1', 0, 0),          // R1.1 — will be on VCC
  makePad('2', 1_000_000, 0),  // R1.2 — will be on GND
]);

const compC = makeComp('C1', 40_000_000, 40_000_000, [
  makePad('1', 0, 0),          // C1.1 — will be on VCC
]);

const nets: NetInfo[] = [
  { name: 'VCC', id: 1, connections: [
    { component: 'U1', pin: '1' },
    { component: 'R1', pin: '1' },
    { component: 'C1', pin: '1' },
  ]},
  { name: 'GND', id: 2, connections: [
    { component: 'U1', pin: '2' },
    { component: 'R1', pin: '2' },
  ]},
];

const snapshot = makeSnapshot([compA, compB, compC], nets);

// Start pad: U1.1 on VCC
const startPadHit = makePadHit(compA, compA.pads[0], 'VCC');

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('routing state machine — UX features', () => {
  let state: RoutingState;

  beforeEach(() => {
    state = createRoutingState();
  });

  // (a) startRoute pre-computes targetPads for correct net
  it('startRoute pre-computes targetPads for the correct net', () => {
    const routed = startRoute(state, startPadHit, snapshot);
    expect(routed.mode).toBe('routing');
    expect(routed.netName).toBe('VCC');
    // Should have R1.1 and C1.1 (same net, excluding start pad U1.1)
    expect(routed.targetPads.length).toBe(2);
    const refs = routed.targetPads.map(tp => `${tp.component.refdes}.${tp.pad.number}`).sort();
    expect(refs).toEqual(['C1.1', 'R1.1']);
  });

  // (b) findNearestTargetPad returns closest pad within radius
  it('findNearestTargetPad returns closest pad within radius', () => {
    const routed = startRoute(state, startPadHit, snapshot);
    // R1.1 is at (20_000_000, 10_000_000). Place cursor 500_000nm away
    const hit = findNearestTargetPad(
      20_500_000, 10_000_000,
      routed,
      1e-6, // very small scale so screen-px threshold doesn't dominate
    );
    expect(hit).not.toBeNull();
    expect(hit!.component.refdes).toBe('R1');
    expect(hit!.pad.number).toBe('1');
  });

  // (c) findNearestTargetPad returns null when no pad in range
  it('findNearestTargetPad returns null when no pad in range', () => {
    const routed = startRoute(state, startPadHit, snapshot);
    // Place cursor far from any target pad
    const hit = findNearestTargetPad(
      0, 0,
      routed,
      1e-6,
    );
    expect(hit).toBeNull();
  });

  // (d) magnetic snap overrides angle snap in updatePreview
  it('magnetic snap overrides angle snap in updatePreview', () => {
    let routed = startRoute(state, startPadHit, snapshot);
    routed = { ...routed, angleSnapEnabled: true };

    // Cursor near R1.1 (20_000_000, 10_000_000) — magnetic snap should lock to pad
    const updated = updatePreview(routed, { x: 20_200_000, y: 10_100_000 }, 1e-6);

    expect(updated.snappedToPad).not.toBeNull();
    expect(updated.snappedToPad!.component.refdes).toBe('R1');
    // Preview endpoint should be exactly the target pad center
    expect(updated.previewSegment!.end_x).toBe(20_000_000);
    expect(updated.previewSegment!.end_y).toBe(10_000_000);
  });

  // (e) toggleAngleSnap flips state
  it('toggleAngleSnap flips state', () => {
    expect(state.angleSnapEnabled).toBe(false);
    const toggled1 = toggleAngleSnap(state);
    expect(toggled1.angleSnapEnabled).toBe(true);
    const toggled2 = toggleAngleSnap(toggled1);
    expect(toggled2.angleSnapEnabled).toBe(false);
  });

  // (f) completeRoute + resetToIdle clears snappedToPad and targetPads
  it('completeRoute resets, and resetToIdle clears snap state', () => {
    let routed = startRoute(state, startPadHit, snapshot);
    // Simulate magnetic snap
    routed = updatePreview(routed, { x: 20_200_000, y: 10_100_000 }, 1e-6);
    expect(routed.snappedToPad).not.toBeNull();
    expect(routed.targetPads.length).toBeGreaterThan(0);

    // Complete the route
    const targetPad = makePadHit(compB, compB.pads[0], 'VCC');
    const result = completeRoute(routed, targetPad);
    expect(result).not.toBeNull();
    expect(result!.segments.length).toBeGreaterThan(0);

    // Reset state (as main.ts does after completeRoute)
    const idle = resetToIdle(routed);
    expect(idle.mode).toBe('idle');
    expect(idle.snappedToPad).toBeNull();
    expect(idle.targetPads.length).toBe(0);
  });

  // (g) cancelRoute clears snappedToPad and targetPads
  it('cancelRoute clears snappedToPad and targetPads', () => {
    let routed = startRoute(state, startPadHit, snapshot);
    routed = updatePreview(routed, { x: 20_200_000, y: 10_100_000 }, 1e-6);
    expect(routed.snappedToPad).not.toBeNull();

    const cancelled = cancelRoute(routed);
    expect(cancelled.mode).toBe('idle');
    expect(cancelled.snappedToPad).toBeNull();
    expect(cancelled.targetPads.length).toBe(0);
  });

  // (h) angle snap disabled by default
  it('angle snap disabled by default', () => {
    expect(state.angleSnapEnabled).toBe(false);
    // magneticSnap is enabled by default
    expect(state.magneticSnapEnabled).toBe(true);
  });
});

describe('computeTargetPads', () => {
  it('returns all pads on the net except the start pad', () => {
    const targets = computeTargetPads(snapshot, 'VCC', 'U1', '1');
    expect(targets.length).toBe(2);
    const refs = targets.map(tp => `${tp.component.refdes}.${tp.pad.number}`).sort();
    expect(refs).toEqual(['C1.1', 'R1.1']);
  });

  it('returns empty for unknown net', () => {
    const targets = computeTargetPads(snapshot, 'NONEXISTENT', 'U1', '1');
    expect(targets.length).toBe(0);
  });

  it('returns empty for null snapshot', () => {
    const targets = computeTargetPads(null, 'VCC', 'U1', '1');
    expect(targets.length).toBe(0);
  });
});

describe('findNearestTargetPad — dual threshold', () => {
  it('uses screen-pixel threshold when zoomed out (small scale)', () => {
    const routed = startRoute(createRoutingState(), startPadHit, snapshot);
    // R1.1 at (20_000_000, 10_000_000)
    // At scale 1e-7, 15px = 150_000_000 nm — everything is in range
    const hit = findNearestTargetPad(0, 0, routed, 1e-7);
    // C1.1 is at (40M, 40M) which is ~56M nm from origin — might be out of range
    // But R1.1 at (20M, 10M) is ~22.4M nm — within 150M range
    expect(hit).not.toBeNull();
  });

  it('uses world radius when zoomed in (large scale)', () => {
    const routed = startRoute(createRoutingState(), startPadHit, snapshot);
    // At large scale, 15px/scale is tiny. World radius (1mm = 1_000_000nm) dominates.
    // Cursor 500_000nm from R1.1 — within 1mm
    const hit = findNearestTargetPad(20_500_000, 10_000_000, routed, 0.001);
    expect(hit).not.toBeNull();
    expect(hit!.component.refdes).toBe('R1');
  });

  it('returns null when magneticSnapEnabled is false', () => {
    let routed = startRoute(createRoutingState(), startPadHit, snapshot);
    routed = { ...routed, magneticSnapEnabled: false };
    const hit = findNearestTargetPad(20_000_000, 10_000_000, routed, 0.001);
    expect(hit).toBeNull();
  });
});
