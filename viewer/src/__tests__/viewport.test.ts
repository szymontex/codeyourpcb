import { describe, it, expect } from 'vitest';
import {
  createViewport,
  worldToScreen,
  screenToWorld,
  zoomAtPoint,
  pan,
  fitBoard,
  resizeViewport,
  type Viewport,
} from '../viewport';

const vp800x600 = (): Viewport => createViewport(800, 600);

describe('worldToScreen / screenToWorld roundtrip', () => {
  it('center of viewport maps to center of world', () => {
    const vp = vp800x600();
    const [sx, sy] = worldToScreen(vp, 0, 0);
    expect(sx).toBeCloseTo(400);
    expect(sy).toBeCloseTo(300);
  });

  it('roundtrip preserves coordinates', () => {
    const vp = vp800x600();
    const worldPt: [number, number] = [5_000_000, -3_000_000]; // 5mm, -3mm
    const screen = worldToScreen(vp, worldPt[0], worldPt[1]);
    const back = screenToWorld(vp, screen[0], screen[1]);
    expect(back[0]).toBeCloseTo(worldPt[0], 0);
    expect(back[1]).toBeCloseTo(worldPt[1], 0);
  });

  it('roundtrip works for off-center viewport', () => {
    const vp: Viewport = { centerX: 10_000_000, centerY: 5_000_000, scale: 0.0002, width: 1024, height: 768 };
    const worldPt: [number, number] = [12_000_000, 6_000_000];
    const screen = worldToScreen(vp, worldPt[0], worldPt[1]);
    const back = screenToWorld(vp, screen[0], screen[1]);
    expect(back[0]).toBeCloseTo(worldPt[0], 0);
    expect(back[1]).toBeCloseTo(worldPt[1], 0);
  });

  it('Y-axis is flipped (positive world Y goes up on screen)', () => {
    const vp = vp800x600();
    const [, syPos] = worldToScreen(vp, 0, 1_000_000);  // 1mm up
    const [, syNeg] = worldToScreen(vp, 0, -1_000_000); // 1mm down
    // Positive world Y -> smaller screen Y (higher up)
    expect(syPos).toBeLessThan(syNeg);
  });
});

describe('zoomAtPoint', () => {
  it('preserves world-space point under cursor after zoom', () => {
    const vp = vp800x600();
    const screenPt: [number, number] = [200, 150];
    const worldBefore = screenToWorld(vp, screenPt[0], screenPt[1]);

    const zoomed = zoomAtPoint(vp, screenPt[0], screenPt[1], 2.0);
    const worldAfter = screenToWorld(zoomed, screenPt[0], screenPt[1]);

    expect(worldAfter[0]).toBeCloseTo(worldBefore[0], 0);
    expect(worldAfter[1]).toBeCloseTo(worldBefore[1], 0);
  });

  it('increases scale when factor > 1', () => {
    const vp = vp800x600();
    const zoomed = zoomAtPoint(vp, 400, 300, 2.0);
    expect(zoomed.scale).toBeCloseTo(vp.scale * 2);
  });

  it('clamps scale to min/max bounds', () => {
    const vp = vp800x600();
    const zoomedOut = zoomAtPoint(vp, 400, 300, 0.00001);
    expect(zoomedOut.scale).toBeGreaterThanOrEqual(0.000001);

    const zoomedIn = zoomAtPoint(vp, 400, 300, 1_000_000);
    expect(zoomedIn.scale).toBeLessThanOrEqual(0.01);
  });
});

describe('pan', () => {
  it('offsets center correctly for rightward pan', () => {
    const vp = vp800x600();
    const panned = pan(vp, 100, 0); // drag right 100px
    // Dragging right = moving viewport left in world = centerX decreases
    expect(panned.centerX).toBeLessThan(vp.centerX);
    expect(panned.centerY).toEqual(vp.centerY);
  });

  it('offsets center correctly for downward pan', () => {
    const vp = vp800x600();
    const panned = pan(vp, 0, 100); // drag down 100px
    // Dragging down = moving viewport up in world = centerY increases (Y-up)
    expect(panned.centerY).toBeGreaterThan(vp.centerY);
  });

  it('world displacement matches pixel displacement / scale', () => {
    const vp = vp800x600();
    const panned = pan(vp, 50, -30);
    expect(panned.centerX).toBeCloseTo(vp.centerX - 50 / vp.scale);
    expect(panned.centerY).toBeCloseTo(vp.centerY + (-30) / vp.scale);
  });
});

describe('fitBoard', () => {
  it('centers on the board', () => {
    const vp = vp800x600();
    const boardW = 50_000_000; // 50mm
    const boardH = 30_000_000; // 30mm
    const fitted = fitBoard(vp, boardW, boardH);
    expect(fitted.centerX).toEqual(boardW / 2);
    expect(fitted.centerY).toEqual(boardH / 2);
  });

  it('produces a viewport that contains the entire board with padding', () => {
    const vp = vp800x600();
    const boardW = 50_000_000;
    const boardH = 30_000_000;
    const fitted = fitBoard(vp, boardW, boardH, 50);

    // All four corners should map to within the padded viewport
    const corners: [number, number][] = [
      [0, 0], [boardW, 0], [boardW, boardH], [0, boardH],
    ];
    for (const [wx, wy] of corners) {
      const [sx, sy] = worldToScreen(fitted, wx, wy);
      expect(sx).toBeGreaterThanOrEqual(0);
      expect(sx).toBeLessThanOrEqual(fitted.width);
      expect(sy).toBeGreaterThanOrEqual(0);
      expect(sy).toBeLessThanOrEqual(fitted.height);
    }
  });

  it('returns original viewport for zero dimensions', () => {
    const vp = vp800x600();
    const fitted = fitBoard(vp, 0, 100);
    expect(fitted).toEqual(vp);
  });
});

describe('resizeViewport', () => {
  it('updates dimensions without changing center or scale', () => {
    const vp: Viewport = { centerX: 100, centerY: 200, scale: 0.001, width: 800, height: 600 };
    const resized = resizeViewport(vp, 1920, 1080);
    expect(resized.width).toEqual(1920);
    expect(resized.height).toEqual(1080);
    expect(resized.centerX).toEqual(100);
    expect(resized.centerY).toEqual(200);
    expect(resized.scale).toEqual(0.001);
  });
});
