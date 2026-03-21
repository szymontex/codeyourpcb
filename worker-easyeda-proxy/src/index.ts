/**
 * EasyEDA CORS Proxy — Cloudflare Worker
 *
 * Proxies requests to easyeda.com and modules.easyeda.com APIs,
 * adding CORS headers so the CodeYourPCB web viewer can fetch
 * footprint data and 3D models from the browser.
 *
 * Deploy: wrangler deploy --name cypcb-easyeda-proxy
 *
 * Usage:
 *   GET /api/products/C17414/components?version=6.4.19.5
 *   → proxied to https://easyeda.com/api/products/C17414/components?version=6.4.19.5
 *
 *   GET /3dmodel/<uuid>
 *   → proxied to https://modules.easyeda.com/3dmodel/<uuid>
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

    if (path.startsWith('/3dmodel/')) {
      // 3D model fetch → modules.easyeda.com
      targetUrl = `https://modules.easyeda.com${path}`;
    } else if (path.startsWith('/api/')) {
      // Component API → easyeda.com
      targetUrl = `https://easyeda.com${path}${url.search}`;
    } else {
      return new Response('Not found', { status: 404 });
    }

    try {
      const response = await fetch(targetUrl, {
        method: request.method,
        headers: {
          'User-Agent': 'CodeYourPCB/1.0',
          'Accept': 'application/json, text/plain, */*',
        },
      });

      // Clone response with CORS headers
      const body = await response.arrayBuffer();
      return new Response(body, {
        status: response.status,
        statusText: response.statusText,
        headers: {
          ...Object.fromEntries(response.headers.entries()),
          ...corsHeaders(request),
        },
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
