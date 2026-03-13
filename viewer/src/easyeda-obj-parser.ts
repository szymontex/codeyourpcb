/**
 * EasyEDA OBJ Parser
 *
 * Parses EasyEDA's non-standard OBJ format into geometry groups suitable
 * for Three.js BufferGeometry. Handles:
 *   - `v x y z` vertex lines
 *   - `newmtl name` / `endmtl` inline material blocks (not standard .mtl)
 *   - `Ka`, `Kd`, `Ks`, `d` material properties
 *   - `usemtl name` to switch active material
 *   - `f v// v// v//` double-slash face indices with computed normals
 *   - `d 0.0` treated as fully opaque (EasyEDA convention)
 */

export interface MaterialColor {
  r: number;
  g: number;
  b: number;
}

export interface OBJGeometryGroup {
  positions: Float32Array;
  normals: Float32Array;
  materialColor: MaterialColor;
  opacity: number;
}

interface ParsedMaterial {
  name: string;
  kd: MaterialColor;
  ks: MaterialColor;
  ka: MaterialColor;
  d: number; // dissolve — EasyEDA uses 0.0 for opaque
}

/**
 * Parse EasyEDA OBJ text into geometry groups with positions, normals, and materials.
 * Returns an empty array for empty/malformed input.
 */
export function parseEasyEdaOBJ(text: string): OBJGeometryGroup[] {
  if (!text || typeof text !== 'string') return [];

  const lines = text.split('\n');
  const vertices: [number, number, number][] = [];
  const materials = new Map<string, ParsedMaterial>();

  // First pass: collect vertices and materials
  let currentMaterial: ParsedMaterial | null = null;
  let insideMaterial = false;

  for (const rawLine of lines) {
    const line = rawLine.trim();
    if (line.length === 0 || line.startsWith('#')) continue;

    if (line.startsWith('v ')) {
      const parts = line.split(/\s+/);
      if (parts.length >= 4) {
        const x = parseFloat(parts[1]);
        const y = parseFloat(parts[2]);
        const z = parseFloat(parts[3]);
        if (!isNaN(x) && !isNaN(y) && !isNaN(z)) {
          vertices.push([x, y, z]);
        }
      }
      continue;
    }

    if (line.startsWith('newmtl ')) {
      const name = line.substring(7).trim();
      currentMaterial = {
        name,
        kd: { r: 0.8, g: 0.8, b: 0.8 },
        ks: { r: 0.0, g: 0.0, b: 0.0 },
        ka: { r: 0.0, g: 0.0, b: 0.0 },
        d: 0.0,
      };
      insideMaterial = true;
      continue;
    }

    if (line === 'endmtl') {
      if (currentMaterial) {
        materials.set(currentMaterial.name, currentMaterial);
      }
      currentMaterial = null;
      insideMaterial = false;
      continue;
    }

    if (insideMaterial && currentMaterial) {
      if (line.startsWith('Kd ')) {
        currentMaterial.kd = parseColorLine(line);
      } else if (line.startsWith('Ka ')) {
        currentMaterial.ka = parseColorLine(line);
      } else if (line.startsWith('Ks ')) {
        currentMaterial.ks = parseColorLine(line);
      } else if (line.startsWith('d ')) {
        const val = parseFloat(line.split(/\s+/)[1]);
        if (!isNaN(val)) currentMaterial.d = val;
      }
      continue;
    }
  }

  // If a material block wasn't closed with endmtl, store it anyway
  if (currentMaterial) {
    materials.set(currentMaterial.name, currentMaterial);
  }

  // Second pass: collect faces per material group
  interface FaceGroup {
    materialName: string;
    faces: [number, number, number][]; // vertex indices (0-based)
  }

  const faceGroups: FaceGroup[] = [];
  let activeMaterialName = '';
  let activeGroup: FaceGroup | null = null;

  for (const rawLine of lines) {
    const line = rawLine.trim();
    if (line.length === 0 || line.startsWith('#')) continue;

    if (line.startsWith('usemtl ')) {
      activeMaterialName = line.substring(7).trim();
      activeGroup = { materialName: activeMaterialName, faces: [] };
      faceGroups.push(activeGroup);
      continue;
    }

    if (line.startsWith('f ')) {
      const parts = line.split(/\s+/).slice(1);
      const indices: number[] = [];

      for (const part of parts) {
        // Handle formats: "v", "v/vt/vn", "v//vn", "v//"
        const idx = parseInt(part.split('/')[0], 10);
        if (!isNaN(idx)) {
          // OBJ indices are 1-based
          indices.push(idx - 1);
        }
      }

      // Triangulate: fan from first vertex for polygons with >3 vertices
      if (indices.length >= 3) {
        if (!activeGroup) {
          activeGroup = { materialName: '', faces: [] };
          faceGroups.push(activeGroup);
        }
        for (let i = 1; i < indices.length - 1; i++) {
          activeGroup.faces.push([indices[0], indices[i], indices[i + 1]]);
        }
      }
      continue;
    }
  }

  if (vertices.length === 0) return [];

  // Build geometry groups
  const result: OBJGeometryGroup[] = [];

  for (const group of faceGroups) {
    if (group.faces.length === 0) continue;

    const posArray: number[] = [];
    const normArray: number[] = [];

    for (const [i0, i1, i2] of group.faces) {
      if (i0 >= vertices.length || i1 >= vertices.length || i2 >= vertices.length) continue;
      if (i0 < 0 || i1 < 0 || i2 < 0) continue;

      const v0 = vertices[i0];
      const v1 = vertices[i1];
      const v2 = vertices[i2];

      // Compute face normal via cross product
      const e1x = v1[0] - v0[0];
      const e1y = v1[1] - v0[1];
      const e1z = v1[2] - v0[2];
      const e2x = v2[0] - v0[0];
      const e2y = v2[1] - v0[1];
      const e2z = v2[2] - v0[2];

      let nx = e1y * e2z - e1z * e2y;
      let ny = e1z * e2x - e1x * e2z;
      let nz = e1x * e2y - e1y * e2x;

      // Normalize
      const len = Math.sqrt(nx * nx + ny * ny + nz * nz);
      if (len > 0) {
        nx /= len;
        ny /= len;
        nz /= len;
      }

      // Push 3 vertices with same face normal (flat shading)
      posArray.push(v0[0], v0[1], v0[2]);
      posArray.push(v1[0], v1[1], v1[2]);
      posArray.push(v2[0], v2[1], v2[2]);
      normArray.push(nx, ny, nz, nx, ny, nz, nx, ny, nz);
    }

    if (posArray.length === 0) continue;

    const mat = materials.get(group.materialName);
    const kd = mat?.kd ?? { r: 0.8, g: 0.8, b: 0.8 };
    // EasyEDA convention: d 0.0 means opaque (inverted from standard OBJ)
    const rawD = mat?.d ?? 0.0;
    const opacity = rawD === 0.0 ? 1.0 : rawD;

    result.push({
      positions: new Float32Array(posArray),
      normals: new Float32Array(normArray),
      materialColor: kd,
      opacity,
    });
  }

  return result;
}

/** Parse a Ka/Kd/Ks line into RGB values (0-1 range) */
function parseColorLine(line: string): MaterialColor {
  const parts = line.split(/\s+/);
  return {
    r: parts.length > 1 ? parseFloat(parts[1]) || 0 : 0,
    g: parts.length > 2 ? parseFloat(parts[2]) || 0 : 0,
    b: parts.length > 3 ? parseFloat(parts[3]) || 0 : 0,
  };
}
