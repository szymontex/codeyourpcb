/**
 * JLCPCB / EasyEDA API Client
 *
 * Three pipelines:
 * 1. Component search via tscircuit/jlcsearch (CORS-enabled, no auth)
 * 2. Footprint fetch via EasyEDA API: LCSC ID → component data → PAD shapes → PadInfo[]
 * 3. 3D model fetch via EasyEDA API: LCSC ID → component data → 3D UUID → OBJ text
 */

import { parseEasyEDAFootprint, type EasyEDAFootprint } from './easyeda-footprint-parser';

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

/**
 * EasyEDA API base URLs.
 *
 * In dev mode, Vite proxies /easyeda-api/* and /easyeda-modules/* to the real
 * EasyEDA servers (see vite.config.ts proxy config), bypassing CORS.
 *
 * In production, requests go through the Cloudflare Worker proxy at
 * VITE_EASYEDA_PROXY_URL, or fall back to direct URLs (which will fail
 * on CORS in browsers but work in Tauri desktop).
 */
const EASYEDA_PROXY_URL = (typeof import.meta !== 'undefined' && (import.meta as any).env?.VITE_EASYEDA_PROXY_URL) || '';

const EASYEDA_API_BASE = EASYEDA_PROXY_URL
  ? EASYEDA_PROXY_URL       // production: Cloudflare Worker proxy
  : '/easyeda-api';          // dev: Vite proxy
const EASYEDA_MODULES_BASE = EASYEDA_PROXY_URL
  ? EASYEDA_PROXY_URL       // production: same Worker handles /3dmodel/ paths
  : '/easyeda-modules';      // dev: Vite proxy

/**
 * Proxy an LCSC image URL to bypass hot-link protection.
 * LCSC's CDN (assets.lcsc.com) returns 403 for cross-origin image requests
 * from browsers. We route through our Cloudflare Worker proxy which fetches
 * server-side, or in dev mode through the Vite proxy.
 */
export function proxyImageUrl(url: string, _large = false): string {
  if (!url) return '';
  if (!url.includes('assets.lcsc.com')) return url;

  // Use our own proxy (Cloudflare Worker in prod, Vite proxy in dev)
  const proxyBase = EASYEDA_PROXY_URL || '/easyeda-api';
  return `${proxyBase}/img/?url=${encodeURIComponent(url)}`;
}

/**
 * All jlcsearch category endpoints to search across.
 * The general `/components/list.json?q=` endpoint ignores the query parameter,
 * so we search category-specific endpoints and filter client-side by mfr name.
 */
const JLCSEARCH_CATEGORIES = [
  'resistors', 'capacitors', 'leds', 'diodes', 'headers',
  'microcontrollers', 'arm_processors', 'risc_v_processors', 'fpgas',
  'wifi_modules', 'mosfets', 'bjt_transistors',
  'ldos', 'voltage_regulators', 'boost_converters', 'buck_boost_converters',
  'led_drivers', 'io_expanders', 'adcs', 'dacs',
  'fuses', 'switches', 'relays',
  'usb_c_connectors', 'fpc_connectors', 'jst_connectors',
  'wire_to_board_connectors', 'battery_holders',
  'gyroscopes', 'accelerometers', 'microphones',
  'potentiometers', 'resistor_arrays', 'inductors',
];

/** Cache of category data — fetched once per session */
const categoryCache = new Map<string, any[]>();

/**
 * Search JLCPCB/LCSC components via tscircuit jlcsearch API.
 *
 * Since jlcsearch's `/components/list.json?q=` ignores query params,
 * we fetch from all category endpoints in parallel and filter client-side
 * by matching the query against mfr part number, package, and attributes.
 *
 * Results are cached per category so subsequent searches are instant.
 */
export async function searchComponents(
  query: string,
  limit = 20,
): Promise<JLCPCBComponent[]> {
  const q = query.toLowerCase().trim();
  if (!q) return [];

  // Search term by term. "0805 10k" describes a package and a value, and no
  // single field holds both, so a whole-string match finds nothing - which is
  // what a parts search is asked for most of the time.
  const terms = q.split(/\s+/).filter(Boolean);

  // Fetch all categories in parallel (cached after first search)
  const allItems = await fetchAllCategories();

  // Score and filter results by relevance to query
  const scored: { item: any; score: number }[] = [];

  for (const item of allItems) {
    const mfr = (item.mfr || '').toLowerCase();
    const pkg = (item.package || '').toLowerCase();
    const cat = (item.category || '').toLowerCase();
    const subcat = (item.subcategory || '').toLowerCase();
    // Attributes arrive either as an `attributes` JSON string or, on the older
    // shape, nested inside `extra` - parseSearchResult reads both, so search
    // has to as well.
    let attrsText = '';
    for (const field of [item.attributes, item.extra]) {
      if (typeof field === 'string') {
        try { attrsText += JSON.stringify(JSON.parse(field)).toLowerCase(); } catch { /* */ }
      }
    }

    let score = 0;
    // Exact mfr match on the whole query
    if (mfr === q) score += 100;
    // LCSC code match (e.g. "C25744")
    if (q.startsWith('c') && String(item.lcsc) === q.slice(1)) score += 100;

    // Every term has to land somewhere; the score is the sum of what each hit.
    let allTermsMatched = true;
    for (const term of terms) {
      let termScore = 0;
      if (mfr.includes(term)) termScore += 50;
      if (pkg.includes(term)) termScore += 20;
      if (cat.includes(term) || subcat.includes(term)) termScore += 10;
      if (attrsText.includes(term)) termScore += 5;

      if (termScore === 0) {
        allTermsMatched = false;
        break;
      }
      score += termScore;
    }
    if (!allTermsMatched) score = 0;

    if (score > 0) {
      // Boost in-stock items
      if (item.stock > 0) score += 3;
      if (item.is_basic) score += 2;
      scored.push({ item, score });
    }
  }

  // Sort by score desc, then stock desc
  scored.sort((a, b) => b.score - a.score || (b.item.stock || 0) - (a.item.stock || 0));

  return scored.slice(0, limit).map(({ item }) => parseSearchResult(item));
}

/**
 * Fetch all categories in parallel. Cached after first call.
 * Each category returns up to 100 items — total ~3000-4000 items.
 */
async function fetchAllCategories(): Promise<any[]> {
  const uncached = JLCSEARCH_CATEGORIES.filter(c => !categoryCache.has(c));

  if (uncached.length > 0) {
    const fetches = uncached.map(async (cat) => {
      try {
        const resp = await fetch(`${JLCSEARCH_BASE}/${cat}/list.json?limit=100`);
        if (!resp.ok) return;
        const data = await resp.json();
        // Response key is the category name (e.g. "resistors", "wifi_modules")
        const items = data?.[cat] || data?.components || [];
        if (Array.isArray(items)) {
          // Tag each item with category for display
          const tagged = items.map((item: any) => ({
            ...item,
            category: item.category || cat.replace(/_/g, ' '),
          }));
          categoryCache.set(cat, tagged);
        }
      } catch {
        categoryCache.set(cat, []); // mark as fetched (empty) to avoid retries
      }
    });

    await Promise.all(fetches);
    console.log(`[JLCPCB] Cached ${uncached.length} categories, ${[...categoryCache.values()].reduce((n, a) => n + a.length, 0)} total items`);
  }

  // Merge all cached items
  const all: any[] = [];
  for (const items of categoryCache.values()) {
    all.push(...items);
  }
  return all;
}

/**
 * Model text already fetched, by uuid.
 *
 * The 3D scene is torn down and rebuilt on every board change, and the pass
 * that attaches models runs on each rebuild, so a session editing a board with
 * ten placed parts asked the CDN for the same ten files on every keystroke's
 * re-parse. Only successful fetches are kept: a miss is worth retrying, and
 * caching `null` would make one bad response permanent for the session.
 */
const objCache = new Map<string, string>();

/**
 * Fetch a 3D model OBJ text by its EasyEDA UUID.
 * Uses the same proxy-aware URL as fetch3DModel.
 * Auto-detects and decompresses gzipped responses (some EasyEDA models
 * are served as raw gzip without Content-Encoding header).
 * Returns null on any error — never throws.
 */
export async function fetch3DModelByUuid(uuid: string): Promise<string | null> {
  const cached = objCache.get(uuid);
  if (cached !== undefined) {
    return cached;
  }
  try {
    const objUrl = `${EASYEDA_MODULES_BASE}/3dmodel/${uuid}`;
    const objResponse = await fetch(objUrl);
    if (!objResponse.ok) {
      console.error(`[JLCPCB] 3D fetch error: HTTP ${objResponse.status} for OBJ ${uuid}`);
      return null;
    }
    const objText = await responseToText(objResponse);
    objCache.set(uuid, objText);
    return objText;
  } catch (error) {
    console.error(`[JLCPCB] 3D fetch error: ${error}`);
    return null;
  }
}

// ---------------------------------------------------------------------------
// Gzip-aware response reader
// ---------------------------------------------------------------------------

/**
 * Read a fetch Response as text, auto-decompressing gzip if needed.
 * Some EasyEDA 3D model responses are served as raw gzip bytes without
 * the Content-Encoding header, so the browser doesn't auto-decompress.
 * We detect gzip magic bytes (0x1f 0x8b) and decompress manually.
 */
async function responseToText(response: Response): Promise<string> {
  const buffer = await response.arrayBuffer();
  const bytes = new Uint8Array(buffer);

  // Check for gzip magic bytes
  if (bytes.length >= 2 && bytes[0] === 0x1f && bytes[1] === 0x8b) {
    // Decompress using DecompressionStream (available in modern browsers)
    if (typeof DecompressionStream !== 'undefined') {
      const ds = new DecompressionStream('gzip');
      const writer = ds.writable.getWriter();
      writer.write(bytes);
      writer.close();
      const reader = ds.readable.getReader();
      const chunks: Uint8Array[] = [];
      while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        chunks.push(value);
      }
      const totalLen = chunks.reduce((a, c) => a + c.length, 0);
      const merged = new Uint8Array(totalLen);
      let offset = 0;
      for (const chunk of chunks) {
        merged.set(chunk, offset);
        offset += chunk.length;
      }
      return new TextDecoder().decode(merged);
    }
    console.warn('[JLCPCB] Gzip response but DecompressionStream unavailable');
  }

  return new TextDecoder().decode(bytes);
}

// ---------------------------------------------------------------------------
// EasyEDA component data cache (shared between footprint and 3D pipelines)
// ---------------------------------------------------------------------------

/** Cache of raw EasyEDA component API responses, keyed by LCSC number */
const easyedaDataCache = new Map<number, any>();

/**
 * Fetch EasyEDA component data with caching.
 * Both footprint and 3D model pipelines need the same API response,
 * so we cache it to avoid duplicate requests.
 */
async function fetchEasyEDAComponentData(lcscId: number): Promise<any | null> {
  if (easyedaDataCache.has(lcscId)) {
    return easyedaDataCache.get(lcscId);
  }

  try {
    const lcscStr = `C${lcscId}`;
    const componentUrl =
      `${EASYEDA_API_BASE}/api/products/${lcscStr}/components?version=6.4.19.5`;
    const compResponse = await fetch(componentUrl);

    if (!compResponse.ok) {
      console.error(`[JLCPCB] EasyEDA API error: HTTP ${compResponse.status} for ${lcscStr}`);
      return null;
    }

    const compData = await compResponse.json();
    easyedaDataCache.set(lcscId, compData);
    return compData;
  } catch (error) {
    console.error(`[JLCPCB] EasyEDA API error: ${error}`);
    return null;
  }
}

// ---------------------------------------------------------------------------
// Component image URL extraction from EasyEDA API
// ---------------------------------------------------------------------------

/** Cache of image URLs keyed by LCSC number */
const imageUrlCache = new Map<number, { thumb: string; large: string }>();

/**
 * Fetch product image URLs for an LCSC part from the LCSC product detail API.
 * Pipeline: LCSC code → wmsc.lcsc.com/ftps/wm/product/detail → productImages[]
 *
 * Returns 900x900 product photos (front, back, etc.) — the real deal, not schematic symbols.
 * Falls back to EasyEDA szlcsc.image (96x96) if LCSC API fails.
 * Returns null if no images available — never throws.
 */
export async function fetchComponentImageUrl(lcscId: number): Promise<{ thumb: string; large: string } | null> {
  if (imageUrlCache.has(lcscId)) {
    return imageUrlCache.get(lcscId)!;
  }

  try {
    // Primary: LCSC product detail API (900x900 product photos)
    const lcscCode = `C${lcscId}`;
    const proxyBase = EASYEDA_PROXY_URL || '/easyeda-api';
    const detailUrl = `${proxyBase}/lcsc/product?code=${lcscCode}`;
    const detailResp = await fetch(detailUrl);

    if (detailResp.ok) {
      const data = await detailResp.json();
      const images: string[] = data?.result?.productImages || [];
      if (images.length > 0) {
        // Use 900x900 for large, derive 96x96 for thumbnail
        const large = images[0];
        const thumb = large.replace('/900x900/', '/96x96/');
        const entry = { thumb, large };
        imageUrlCache.set(lcscId, entry);
        return entry;
      }
    }
  } catch {
    // Fall through to EasyEDA fallback
  }

  try {
    // Fallback: EasyEDA API szlcsc.image (96x96 only)
    const compData = await fetchEasyEDAComponentData(lcscId);
    if (!compData) return null;

    const result = compData?.result;
    if (!result) return null;

    const lcscImage = result?.szlcsc?.image || '';
    if (!lcscImage) return null;

    const entry = { thumb: lcscImage, large: lcscImage };
    imageUrlCache.set(lcscId, entry);
    return entry;
  } catch {
    return null;
  }
}

// ---------------------------------------------------------------------------
// Footprint cache (parsed PadInfo[] keyed by LCSC ID)
// ---------------------------------------------------------------------------

/** Cached parsed footprints keyed by LCSC number */
const footprintCache = new Map<number, EasyEDAFootprint>();

/**
 * Fetch and parse footprint data for an LCSC part number.
 * Pipeline: LCSC ID → EasyEDA component API → parse PAD shapes → PadInfo[].
 *
 * Returns cached result on subsequent calls for the same part.
 * Returns null if no footprint data is available — never throws.
 */
export async function fetchComponentFootprint(lcscId: number): Promise<EasyEDAFootprint | null> {
  // Check cache first
  if (footprintCache.has(lcscId)) {
    return footprintCache.get(lcscId)!;
  }

  try {
    const compData = await fetchEasyEDAComponentData(lcscId);
    if (!compData) return null;

    const footprint = parseEasyEDAFootprint(compData);
    if (!footprint) {
      console.log(`[JLCPCB] No footprint data for C${lcscId}`);
      return null;
    }

    footprintCache.set(lcscId, footprint);
    console.log(`[JLCPCB] Parsed footprint for C${lcscId}: ${footprint.pads.length} pads, 3D: ${footprint.modelUuid ? 'yes' : 'no'}`);
    return footprint;
  } catch (error) {
    console.error(`[JLCPCB] Footprint fetch error for C${lcscId}: ${error}`);
    return null;
  }
}

/**
 * Parse a single search result from jlcsearch API.
 *
 * Handles both the new format (direct fields, `attributes` as JSON string)
 * and the legacy format (nested `extra` JSON string with images).
 */
export function parseSearchResult(raw: any): JLCPCBComponent {
  let manufacturer = '';
  let attributes: Record<string, string> = {};
  let datasheetUrl = '';
  let imageUrl = '';
  let imageUrlLarge = '';
  let description = raw.description || '';

  // New format: attributes as direct JSON string field
  if (raw.attributes && typeof raw.attributes === 'string') {
    try {
      const parsed = JSON.parse(raw.attributes);
      if (parsed && typeof parsed === 'object') {
        attributes = parsed;
      }
    } catch {
      // Malformed — use empty
    }
  }

  // Legacy format: nested extra JSON string (kept for backwards compat)
  if (raw.extra && typeof raw.extra === 'string') {
    try {
      const extra = JSON.parse(raw.extra);
      manufacturer = extra?.manufacturer?.name ?? '';
      if (extra?.attributes && typeof extra.attributes === 'object') {
        attributes = { ...attributes, ...extra.attributes };
      }
      datasheetUrl = extra?.datasheet?.pdf ?? '';
      if (extra?.description) description = extra.description;

      if (Array.isArray(extra?.images) && extra.images.length > 0) {
        const first = extra.images[0];
        imageUrl = first?.['224x224'] ?? first?.['96x96'] ?? '';
        imageUrlLarge = first?.['900x900'] ?? first?.['224x224'] ?? '';
      }
    } catch {
      // Malformed extra JSON — use defaults
    }
  }

  // Build description from category + attributes if not provided
  if (!description && raw.category) {
    const parts = [raw.subcategory || raw.category];
    if (attributes['Resistance']) parts.push(attributes['Resistance']);
    if (attributes['Capacitance']) parts.push(attributes['Capacitance']);
    if (attributes['Type']) parts.push(attributes['Type']);
    description = parts.join(' — ');
  }

  return {
    lcsc: typeof raw.lcsc === 'number' ? raw.lcsc : 0,
    mfr: raw.mfr ?? '',
    package: raw.package ?? '',
    isBasic: raw.is_basic ?? raw.isBasic ?? false,
    stock: typeof raw.stock === 'number' ? raw.stock : 0,
    price: parsePrice(raw.price ?? raw.price1),
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

      // Scan shape array for outline3D SVGNODE entries — their "uuid" field
      // is the actual 3D model UUID used by modules.easyeda.com/3dmodel/{uuid}.
      // Note: head.uuid_3d is NOT the model download UUID — it returns 404.
      for (const shape of shapes) {
        if (typeof shape !== 'string') continue;

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
