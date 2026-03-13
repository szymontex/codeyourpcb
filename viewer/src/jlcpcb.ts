/**
 * JLCPCB / EasyEDA API Client
 *
 * Two pipelines:
 * 1. Component search via tscircuit/jlcsearch (CORS-enabled, no auth)
 * 2. 3D model fetch via EasyEDA API: LCSC ID → component data → 3D UUID → OBJ text
 */

/** Component result from jlcsearch API */
export interface JLCPCBComponent {
  /** LCSC part number (bare integer, e.g. 17414 for C17414) */
  lcsc: number;
  /** Manufacturer part number */
  mfr: string;
  /** Package/footprint (e.g. "0805") */
  package: string;
  /** Whether this is a JLCPCB basic part */
  isBasic: boolean;
  /** Current stock count */
  stock: number;
  /** Unit price in USD */
  price: number;
  /** Manufacturer name (from extra field) */
  manufacturer: string;
  /** Component attributes: resistance, capacitance, etc. (from extra field) */
  attributes: Record<string, string>;
  /** Datasheet PDF URL (from extra field) */
  datasheetUrl: string;
}

const JLCSEARCH_BASE = 'https://jlcsearch.tscircuit.com';
const EASYEDA_API_BASE = 'https://easyeda.com';
const EASYEDA_MODULES_BASE = 'https://modules.easyeda.com';

/**
 * Search JLCPCB/LCSC components via tscircuit jlcsearch API.
 * Returns typed results with parsed metadata from the `extra` JSON string.
 * Returns empty array on error — never throws.
 */
export async function searchComponents(
  query: string,
  limit = 20,
): Promise<JLCPCBComponent[]> {
  try {
    const params = new URLSearchParams({
      q: query,
      limit: String(limit),
      full: 'true',
    });
    const url = `${JLCSEARCH_BASE}/api/search?${params}`;
    const response = await fetch(url);

    if (!response.ok) {
      console.error(`[JLCPCB] Search error: HTTP ${response.status} for "${query}"`);
      return [];
    }

    const data = await response.json();
    const components = data?.components;

    if (!Array.isArray(components)) {
      return [];
    }

    return components.map((raw: any) => parseSearchResult(raw));
  } catch (error) {
    console.error(`[JLCPCB] Search error: ${error}`);
    return [];
  }
}

/**
 * Fetch a 3D model OBJ text for a given LCSC part number.
 * Pipeline: LCSC ID → EasyEDA component API → extract 3D UUID → fetch OBJ.
 * Returns null if no 3D model is available or on any error — never throws.
 */
export async function fetch3DModel(lcscId: number): Promise<string | null> {
  try {
    // Step 1: Fetch component data from EasyEDA
    const lcscStr = `C${lcscId}`;
    const componentUrl =
      `${EASYEDA_API_BASE}/api/products/${lcscStr}/components?version=6.4.19.5`;
    const compResponse = await fetch(componentUrl);

    if (!compResponse.ok) {
      console.error(`[JLCPCB] 3D fetch error: HTTP ${compResponse.status} for ${lcscStr}`);
      return null;
    }

    const compData = await compResponse.json();

    // Step 2: Extract 3D model UUID from shape array
    const uuid = extract3DModelUUID(compData);
    if (!uuid) {
      return null; // No 3D model available for this component
    }

    // Step 3: Fetch OBJ text
    const objUrl = `${EASYEDA_MODULES_BASE}/3dmodel/${uuid}`;
    const objResponse = await fetch(objUrl);

    if (!objResponse.ok) {
      console.error(`[JLCPCB] 3D fetch error: HTTP ${objResponse.status} for OBJ ${uuid}`);
      return null;
    }

    return await objResponse.text();
  } catch (error) {
    console.error(`[JLCPCB] 3D fetch error: ${error}`);
    return null;
  }
}

/**
 * Parse a single search result from jlcsearch API, extracting the nested
 * `extra` JSON string into structured fields.
 */
export function parseSearchResult(raw: any): JLCPCBComponent {
  let manufacturer = '';
  let attributes: Record<string, string> = {};
  let datasheetUrl = '';

  if (raw.extra && typeof raw.extra === 'string') {
    try {
      const extra = JSON.parse(raw.extra);
      manufacturer = extra?.manufacturer?.name ?? '';
      if (extra?.attributes && typeof extra.attributes === 'object') {
        attributes = extra.attributes;
      }
      datasheetUrl = extra?.datasheet?.pdf ?? '';
    } catch {
      // Malformed extra JSON — use defaults
    }
  }

  return {
    lcsc: typeof raw.lcsc === 'number' ? raw.lcsc : 0,
    mfr: raw.mfr ?? '',
    package: raw.package ?? '',
    isBasic: raw.is_basic ?? false,
    stock: typeof raw.stock === 'number' ? raw.stock : 0,
    price: typeof raw.price === 'number' ? raw.price : 0,
    manufacturer,
    attributes,
    datasheetUrl,
  };
}

/**
 * Extract the 3D model UUID from EasyEDA component API response.
 * Searches the shape array in packageDetail.dataStr for an outline3D entry.
 * Returns null if no 3D model is present.
 */
export function extract3DModelUUID(compData: any): string | null {
  try {
    // EasyEDA v6 response structure
    const result = compData?.result;
    if (!result) return null;

    // Could be a single result or array of results
    const items = Array.isArray(result) ? result : [result];

    for (const item of items) {
      const shapes = item?.packageDetail?.dataStr?.shape;
      if (!Array.isArray(shapes)) continue;

      for (const shape of shapes) {
        if (typeof shape !== 'string') continue;

        // Shape entries are like "SVGNODE~{...}" — look for outline3D with uuid
        if (shape.includes('outline3D') || shape.includes('3D')) {
          const uuidMatch = shape.match(/"uuid"\s*:\s*"([a-f0-9]{32})"/i);
          if (uuidMatch) {
            return uuidMatch[1];
          }
        }
      }
    }

    return null;
  } catch {
    return null;
  }
}
