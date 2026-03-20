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
  /** Component image URL — 224x224 thumbnail (from extra.images) */
  imageUrl: string;
  /** Large component image URL — 900x900 for preview (from extra.images) */
  imageUrlLarge: string;
  /** Full description (from extra.description) */
  description: string;
}

/** Thrown when the JLCPCB search API returns a non-ok HTTP status. */
export class JLCPCBSearchError extends Error {
  constructor(public readonly status: number, query: string) {
    super(`HTTP ${status} for "${query}"`);
    this.name = 'JLCPCBSearchError';
  }
}

const JLCSEARCH_BASE = 'https://jlcsearch.tscircuit.com';
const EASYEDA_API_BASE = 'https://easyeda.com';
const EASYEDA_MODULES_BASE = 'https://modules.easyeda.com';

/**
 * Proxy an LCSC image URL through wsrv.nl to bypass hot-link protection.
 * LCSC's CDN (assets.lcsc.com) returns 403 for cross-origin image requests
 * from browsers. The wsrv.nl service is an open-source image proxy that
 * fetches server-side, avoiding the referer/origin block.
 */
export function proxyImageUrl(url: string, large = false): string {
  if (!url) return '';
  // Only proxy assets.lcsc.com URLs
  if (url.includes('assets.lcsc.com')) {
    const size = large ? 300 : 48;
    return `https://wsrv.nl/?url=${encodeURIComponent(url)}&w=${size}&h=${size}&fit=cover&output=webp`;
  }
  return url;
}

/**
 * Search JLCPCB/LCSC components via tscircuit jlcsearch API.
 * Returns typed results with parsed metadata from the `extra` JSON string.
 * Throws JLCPCBSearchError on HTTP errors (4xx/5xx).
 * Returns empty array on network-level failures (DNS, timeout, CORS).
 * Retries once on 502/503 with a short delay.
 */
export async function searchComponents(
  query: string,
  limit = 20,
): Promise<JLCPCBComponent[]> {
  const params = new URLSearchParams({
    q: query,
    limit: String(limit),
    full: 'true',
  });
  const url = `${JLCSEARCH_BASE}/api/search?${params}`;

  for (let attempt = 0; attempt < 2; attempt++) {
    try {
      const response = await fetch(url);

      // Retry on 502/503 — server may be temporarily overloaded
      if ((response.status === 502 || response.status === 503) && attempt === 0) {
        console.warn(`[JLCPCB] Search got ${response.status}, retrying in 1s...`);
        await new Promise((r) => setTimeout(r, 1000));
        continue;
      }

      if (!response.ok) {
        console.error(`[JLCPCB] Search error: HTTP ${response.status} for "${query}"`);
        throw new JLCPCBSearchError(response.status, query);
      }

      const data = await response.json();
      const components = data?.components;

      if (!Array.isArray(components)) {
        return [];
      }

      return components.map((raw: any) => parseSearchResult(raw));
    } catch (error) {
      if (error instanceof JLCPCBSearchError) throw error;
      // On network error, retry once
      if (attempt === 0) {
        console.warn(`[JLCPCB] Search network error, retrying in 1s...`);
        await new Promise((r) => setTimeout(r, 1000));
        continue;
      }
      console.error(`[JLCPCB] Search error: ${error}`);
      return [];
    }
  }

  return [];
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
  let imageUrl = '';
  let imageUrlLarge = '';
  let description = '';

  if (raw.extra && typeof raw.extra === 'string') {
    try {
      const extra = JSON.parse(raw.extra);
      manufacturer = extra?.manufacturer?.name ?? '';
      if (extra?.attributes && typeof extra.attributes === 'object') {
        attributes = extra.attributes;
      }
      datasheetUrl = extra?.datasheet?.pdf ?? '';
      description = extra?.description ?? '';

      // Extract image URLs: 224x224 for thumbnail, 900x900 for hover preview
      if (Array.isArray(extra?.images) && extra.images.length > 0) {
        const first = extra.images[0];
        imageUrl = first?.['224x224'] ?? first?.['96x96'] ?? '';
        imageUrlLarge = first?.['900x900'] ?? first?.['224x224'] ?? '';
      }
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
    price: parsePrice(raw.price),
    manufacturer,
    attributes,
    datasheetUrl,
    imageUrl,
    imageUrlLarge,
    description,
  };
}

/**
 * Parse price from jlcsearch API response.
 * The `price` field can be:
 * - a number (direct price)
 * - a JSON string containing an array of quantity tiers [{qFrom, qTo, price}]
 * Returns the lowest-tier unit price, or 0 if unparseable.
 */
function parsePrice(raw: any): number {
  if (typeof raw === 'number') return raw;
  if (typeof raw === 'string') {
    try {
      const tiers = JSON.parse(raw);
      if (Array.isArray(tiers) && tiers.length > 0) {
        // Return the first tier (smallest quantity) price
        return typeof tiers[0].price === 'number' ? tiers[0].price : 0;
      }
    } catch {
      // Not valid JSON
    }
  }
  return 0;
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
