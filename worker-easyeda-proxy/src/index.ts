/**
 * EasyEDA CORS Proxy — Cloudflare Worker
 *
 * Proxies requests to easyeda.com, modules.easyeda.com, and assets.lcsc.com,
 * adding CORS headers so the CodeYourPCB web viewer can fetch
 * footprint data, 3D models, and component images from the browser.
 *
 * Deploy: wrangler deploy --name cypcb-easyeda-proxy
 *
 * Usage:
 *   GET /api/products/C17414/components?version=6.4.19.5
 *   → proxied to https://easyeda.com/api/products/C17414/components?version=6.4.19.5
 *
 *   GET /3dmodel/<uuid>
 *   → proxied to https://modules.easyeda.com/3dmodel/<uuid>
 *
 *   GET /img/?url=https://assets.lcsc.com/images/...jpg&w=48&h=48
 *   → proxied image fetch from assets.lcsc.com (bypasses hot-link protection)
 */

export default {
  async fetch(request: Request): Promise<Response> {
    const url = new URL(request.url);
    const path = url.pathname;

    // Handle CORS preflight
    if (request.method === 'OPTIONS') {
      return new Response(null, {
        status: 204,
        headers: corsHeaders(request),
      });
    }

    let targetUrl: string;
    let isImage = false;

    if (path === '/img/' || path === '/img') {
      // Image proxy → assets.lcsc.com only
      const imgUrl = url.searchParams.get('url');
      if (!imgUrl || !imgUrl.includes('assets.lcsc.com')) {
        return new Response('Only assets.lcsc.com URLs allowed', {
          status: 400,
          headers: corsHeaders(request),
        });
      }
      targetUrl = imgUrl;
      isImage = true;
    } else if (path === '/lcsc/product' || path === '/lcsc/product/') {
      // LCSC product detail proxy → wmsc.lcsc.com
      const code = url.searchParams.get('code');
      if (!code) {
        return new Response('Missing ?code= parameter', {
          status: 400,
          headers: corsHeaders(request),
        });
      }
      targetUrl = `https://wmsc.lcsc.com/ftps/wm/product/detail?productCode=${encodeURIComponent(code)}`;
    } else if (path.startsWith('/3dmodel/')) {
      // 3D model fetch → modules.easyeda.com
      targetUrl = `https://modules.easyeda.com${path}`;
    } else if (path.startsWith('/api/')) {
      // Component API → easyeda.com
      targetUrl = `https://easyeda.com${path}${url.search}`;
    } else {
      return new Response('Not found', { status: 404 });
    }

    try {
      const isLcsc = targetUrl.includes('lcsc.com');
      const response = await fetch(targetUrl, {
        method: request.method,
        headers: {
          'User-Agent': isLcsc
            ? 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 Chrome/120.0.0.0 Safari/537.36'
            : 'CodeYourPCB/1.0',
          'Accept': isImage ? 'image/*' : 'application/json, text/plain, */*',
          ...(isLcsc ? { 'Referer': 'https://www.lcsc.com/' } : {}),
        },
      });

      const body = await response.arrayBuffer();
      const headers: Record<string, string> = {
        ...corsHeaders(request),
      };

      // Preserve content-type from upstream
      const ct = response.headers.get('Content-Type');
      if (ct) headers['Content-Type'] = ct;

      // Cache images for 7 days at edge
      if (isImage && response.ok) {
        headers['Cache-Control'] = 'public, max-age=604800, immutable';
      }

      return new Response(body, {
        status: response.status,
        statusText: response.statusText,
        headers,
      });
    } catch (error) {
      return new Response(`Proxy error: ${error}`, {
        status: 502,
        headers: corsHeaders(request),
      });
    }
  },
};

function corsHeaders(request: Request): Record<string, string> {
  const origin = request.headers.get('Origin') || '*';
  return {
    'Access-Control-Allow-Origin': origin,
    'Access-Control-Allow-Methods': 'GET, OPTIONS',
    'Access-Control-Allow-Headers': 'Content-Type',
    'Access-Control-Max-Age': '86400',
  };
}
