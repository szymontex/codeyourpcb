import { describe, it, expect } from 'vitest';
import { parseEasyEdaOBJ } from '../easyeda-obj-parser';

describe('parseEasyEdaOBJ', () => {
  it('parses a basic cube (8 vertices, 12 faces)', () => {
    const obj = `
# Cube
v 0 0 0
v 1 0 0
v 1 1 0
v 0 1 0
v 0 0 1
v 1 0 1
v 1 1 1
v 0 1 1
newmtl default
Kd 0.5 0.5 0.5
d 0.0
endmtl
usemtl default
f 1// 2// 3//
f 3// 4// 1//
f 5// 6// 7//
f 7// 8// 5//
f 1// 2// 6//
f 6// 5// 1//
f 2// 3// 7//
f 7// 6// 2//
f 3// 4// 8//
f 8// 7// 3//
f 4// 1// 5//
f 5// 8// 4//
`;
    const groups = parseEasyEdaOBJ(obj);
    expect(groups.length).toBe(1);
    // 12 faces × 3 vertices × 3 components = 108
    expect(groups[0].positions.length).toBe(12 * 3 * 3);
    expect(groups[0].normals.length).toBe(12 * 3 * 3);
    // All positions should be finite
    for (let i = 0; i < groups[0].positions.length; i++) {
      expect(isFinite(groups[0].positions[i])).toBe(true);
    }
  });

  it('parses material properties (Ka/Kd/Ks/d)', () => {
    const obj = `
v 0 0 0
v 1 0 0
v 0 1 0
newmtl red
Ka 0.1 0.0 0.0
Kd 0.9 0.1 0.1
Ks 0.5 0.5 0.5
d 0.8
endmtl
usemtl red
f 1// 2// 3//
`;
    const groups = parseEasyEdaOBJ(obj);
    expect(groups.length).toBe(1);
    expect(groups[0].materialColor.r).toBeCloseTo(0.9);
    expect(groups[0].materialColor.g).toBeCloseTo(0.1);
    expect(groups[0].materialColor.b).toBeCloseTo(0.1);
    expect(groups[0].opacity).toBeCloseTo(0.8);
  });

  it('handles double-slash face format (f v// v// v//)', () => {
    const obj = `
v 0 0 0
v 1 0 0
v 0.5 1 0
newmtl m1
Kd 0.5 0.5 0.5
endmtl
usemtl m1
f 1// 2// 3//
`;
    const groups = parseEasyEdaOBJ(obj);
    expect(groups.length).toBe(1);
    expect(groups[0].positions.length).toBe(9); // 3 vertices × 3 components
  });

  it('treats d 0.0 as fully opaque (EasyEDA convention)', () => {
    const obj = `
v 0 0 0
v 1 0 0
v 0 1 0
newmtl mat0
Kd 0.5 0.5 0.5
d 0.0
endmtl
usemtl mat0
f 1// 2// 3//
`;
    const groups = parseEasyEdaOBJ(obj);
    expect(groups.length).toBe(1);
    expect(groups[0].opacity).toBe(1.0);
  });

  it('handles multiple material groups', () => {
    const obj = `
v 0 0 0
v 1 0 0
v 0 1 0
v 1 1 0
v 0.5 0.5 1
newmtl red
Kd 1 0 0
d 0.0
endmtl
newmtl blue
Kd 0 0 1
d 0.0
endmtl
usemtl red
f 1// 2// 3//
usemtl blue
f 2// 4// 5//
`;
    const groups = parseEasyEdaOBJ(obj);
    expect(groups.length).toBe(2);
    expect(groups[0].materialColor.r).toBeCloseTo(1.0);
    expect(groups[0].materialColor.b).toBeCloseTo(0.0);
    expect(groups[1].materialColor.r).toBeCloseTo(0.0);
    expect(groups[1].materialColor.b).toBeCloseTo(1.0);
  });

  it('returns empty array for empty/malformed input', () => {
    expect(parseEasyEdaOBJ('')).toEqual([]);
    expect(parseEasyEdaOBJ('garbage data here')).toEqual([]);
    expect(parseEasyEdaOBJ(null as any)).toEqual([]);
    expect(parseEasyEdaOBJ(undefined as any)).toEqual([]);
  });

  it('ignores comment lines', () => {
    const obj = `
# This is a comment
v 0 0 0
# Another comment
v 1 0 0
v 0 1 0
newmtl m1
# Comment inside material
Kd 0.5 0.5 0.5
d 0.0
endmtl
usemtl m1
# Comment before face
f 1// 2// 3//
`;
    const groups = parseEasyEdaOBJ(obj);
    expect(groups.length).toBe(1);
    expect(groups[0].positions.length).toBe(9);
  });

  it('computes face normals from vertex cross-products', () => {
    // Triangle in XY plane — normal should point in Z direction
    const obj = `
v 0 0 0
v 1 0 0
v 0 1 0
newmtl m1
Kd 0.5 0.5 0.5
endmtl
usemtl m1
f 1// 2// 3//
`;
    const groups = parseEasyEdaOBJ(obj);
    expect(groups.length).toBe(1);

    // Check normal: should be (0, 0, 1) for counter-clockwise winding in XY
    const normals = groups[0].normals;
    // All 3 vertices get the same normal
    for (let i = 0; i < 3; i++) {
      expect(normals[i * 3 + 0]).toBeCloseTo(0); // nx
      expect(normals[i * 3 + 1]).toBeCloseTo(0); // ny
      expect(normals[i * 3 + 2]).toBeCloseTo(1); // nz
    }
  });

  it('handles EasyEDA-style sample with inline material blocks', () => {
    // Simplified version of real EasyEDA OBJ output
    const obj = `
v 0.8 0.65 0.55
v 0.8 0.65 0.6
v -0.8 0.65 0.6
v -0.8 0.65 0.55
v 0.8 -0.65 0.55
v 0.8 -0.65 0.6
v -0.8 -0.65 0.6
v -0.8 -0.65 0.55
newmtl 1
Ka 0.85 0.85 0.85
Kd 0.85 0.85 0.85
Ks 0.43 0.43 0.43
d 0.0
endmtl
newmtl 2
Ka 0.2 0.2 0.2
Kd 0.2 0.2 0.2
Ks 0.1 0.1 0.1
d 0.0
endmtl
usemtl 1
f 1// 2// 3//
f 3// 4// 1//
f 5// 6// 7//
f 7// 8// 5//
usemtl 2
f 1// 2// 6//
f 6// 5// 1//
`;
    const groups = parseEasyEdaOBJ(obj);
    expect(groups.length).toBe(2);

    // First group: 4 faces → 12 vertices × 3 = 36 position values
    expect(groups[0].positions.length).toBe(4 * 3 * 3);
    expect(groups[0].materialColor.r).toBeCloseTo(0.85);
    expect(groups[0].opacity).toBe(1.0); // d 0.0 → opaque

    // Second group: 2 faces → 6 vertices × 3 = 18 position values
    expect(groups[1].positions.length).toBe(2 * 3 * 3);
    expect(groups[1].materialColor.r).toBeCloseTo(0.2);
  });
});
