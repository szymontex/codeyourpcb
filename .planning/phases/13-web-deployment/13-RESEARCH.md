# Phase 13: Web Deployment - Research

**Researched:** 2026-01-30
**Domain:** Web deployment, WASM optimization, file system access, CDN hosting
**Confidence:** HIGH

## Summary

Web deployment for CodeYourPCB involves optimizing a Rust WASM application for fast loading over 3G connections, implementing browser-based file access without installation, enabling URL-based project sharing, and deploying to a global CDN. The project already uses Vite + wasm-pack, which provides a solid foundation.

The standard approach in 2026 is: wasm-pack for Rust->WASM compilation, wasm-opt for binary optimization, Vite for bundling with WASM support, File System Access API for local file operations (with fallback), URLSearchParams for state sharing, and Cloudflare Pages for global CDN deployment with automatic HTTPS.

Key findings: WASM files compress extremely well with Brotli (50%+ reduction), WebAssembly.compileStreaming() enables faster loading across all modern browsers, File System Access API works in Chrome/Edge/Safari but needs fallback for Firefox, and Cloudflare Pages provides better pricing than Vercel for high-traffic WASM applications.

**Primary recommendation:** Use wasm-opt -O4 for production builds, implement cache-first PWA strategy for WASM files, provide File System Access API with input fallback, encode minimal state in URL query parameters, and deploy to Cloudflare Pages with GitHub integration.

## Standard Stack

The established libraries/tools for web deployment of WASM applications:

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| wasm-pack | 0.13+ | Rust->WASM compilation | Official Rust WASM toolchain, handles wasm-bindgen automatically |
| wasm-opt | (Binaryen) | Post-compilation optimization | Can reduce binary size 15-20%, industry standard |
| Vite | 5.0+ | Build tool and dev server | Fast, native WASM support, modern ESM bundling |
| vite-plugin-wasm | latest | WASM ES module integration | Enables top-level await and proper WASM imports |
| Cloudflare Pages | N/A | CDN hosting and deployment | Global edge network, free bandwidth, native WASM support |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| vite-plugin-compression | latest | Pre-compress assets with Brotli | Optional - CDNs auto-compress, but pre-compressing reduces edge CPU |
| browser-fs-access | 0.35+ | File System Access API polyfill | Provides fallback for browsers without native support |
| Workbox | 7.0+ | Service worker generation | When implementing PWA offline support |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Cloudflare Pages | Vercel | Vercel has better DX for Next.js but charges for bandwidth; CF Pages free tier unlimited |
| Cloudflare Pages | GitHub Pages | GH Pages free but no edge functions, slower global distribution |
| File System Access API | Download/upload pattern | Works everywhere but worse UX - forces save-as every time |

**Installation:**
```bash
# WASM toolchain (one-time setup)
cargo install wasm-pack
rustup target add wasm32-unknown-unknown

# Vite plugins
npm install --save-dev vite-plugin-wasm vite-plugin-top-level-await
npm install --save-dev vite-plugin-compression  # Optional

# File system polyfill (if needed)
npm install browser-fs-access
```

## Architecture Patterns

### Recommended Project Structure
```
viewer/
├── src/
│   ├── main.ts              # Entry point, WASM init
│   ├── wasm.ts              # WASM module loader
│   ├── file-access.ts       # File System Access API wrapper
│   ├── url-state.ts         # URL parameter serialization
│   └── components/          # UI components
├── pkg/                     # wasm-pack output (gitignored)
│   ├── cypcb_render.js
│   ├── cypcb_render_bg.wasm
│   └── cypcb_render.d.ts
├── public/                  # Static assets
├── index.html               # Entry HTML
├── vite.config.ts           # Vite configuration
└── build-wasm.sh            # WASM build script
```

### Pattern 1: WASM Loading with Streaming Compilation
**What:** Use WebAssembly.compileStreaming() for faster WASM initialization
**When to use:** Always in production - supported in all modern browsers since 2021
**Example:**
```typescript
// Source: MDN WebAssembly.compileStreaming
import init from './pkg/cypcb_render.js';

export async function initWasm() {
  try {
    // vite-plugin-wasm handles this automatically with ES modules
    await init();
    console.log('WASM initialized');
  } catch (err) {
    console.error('Failed to load WASM:', err);
    throw err;
  }
}
```

### Pattern 2: File System Access API with Fallback
**What:** Progressive enhancement - use File System Access API where supported, fall back to input element
**When to use:** For all file operations in web deployment
**Example:**
```typescript
// Source: Chrome Developers File System Access API docs
async function openFile() {
  if ('showOpenFilePicker' in window) {
    // Modern API - Chrome, Edge, Safari
    const [handle] = await window.showOpenFilePicker({
      types: [{
        description: 'PCB Files',
        accept: { 'application/cypcb': ['.cypcb'] }
      }]
    });
    const file = await handle.getFile();
    return { file, handle }; // Keep handle for save
  } else {
    // Fallback - Firefox, older browsers
    return new Promise((resolve) => {
      const input = document.createElement('input');
      input.type = 'file';
      input.accept = '.cypcb';
      input.onchange = () => resolve({ file: input.files[0], handle: null });
      input.click();
    });
  }
}

async function saveFile(handle, data) {
  if (handle) {
    // Use existing handle - no save dialog
    const writable = await handle.createWritable();
    await writable.write(data);
    await writable.close();
  } else {
    // Fallback - download
    const blob = new Blob([data], { type: 'application/cypcb' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = 'design.cypcb';
    a.click();
    URL.revokeObjectURL(url);
  }
}
```

### Pattern 3: URL State Serialization
**What:** Encode minimal project state in URL query parameters for sharing
**When to use:** For share links, requires small state (layer visibility, view settings)
**Example:**
```typescript
// Source: Vanilla JavaScript URLSearchParams
interface ViewState {
  layers: string[];  // ['top', 'bottom']
  zoom: number;
  pan: [number, number];
}

function encodeState(state: ViewState): string {
  const params = new URLSearchParams();
  params.set('layers', state.layers.join(','));
  params.set('zoom', state.zoom.toFixed(2));
  params.set('pan', `${state.pan[0]},${state.pan[1]}`);
  return `${window.location.origin}?${params.toString()}`;
}

function decodeState(): ViewState | null {
  const params = new URLSearchParams(window.location.search);
  if (!params.has('layers')) return null;

  return {
    layers: params.get('layers').split(','),
    zoom: parseFloat(params.get('zoom') || '1'),
    pan: params.get('pan').split(',').map(Number) as [number, number]
  };
}
```

### Pattern 4: PWA Cache Strategy for WASM
**What:** Cache-first for WASM files, stale-while-revalidate for app shell
**When to use:** When implementing offline support (optional for this phase)
**Example:**
```javascript
// Source: MDN Progressive Web Apps Caching
// service-worker.js
const CACHE_NAME = 'cypcb-v1';
const WASM_FILES = [
  '/pkg/cypcb_render_bg.wasm',
  '/pkg/cypcb_render.js'
];

self.addEventListener('fetch', (event) => {
  const url = new URL(event.request.url);

  // Cache-first for WASM files (immutable)
  if (WASM_FILES.includes(url.pathname)) {
    event.respondWith(
      caches.match(event.request).then((cached) => {
        return cached || fetch(event.request).then((response) => {
          return caches.open(CACHE_NAME).then((cache) => {
            cache.put(event.request, response.clone());
            return response;
          });
        });
      })
    );
  }
});
```

### Pattern 5: Responsive Canvas Handling
**What:** Make canvas responsive to viewport size and touch-friendly
**When to use:** Always - WEB-02 requires tablet/desktop responsiveness
**Example:**
```typescript
// Source: Responsive Web Design best practices 2026
function setupResponsiveCanvas(canvas: HTMLCanvasElement) {
  function resize() {
    const container = canvas.parentElement!;
    const dpr = window.devicePixelRatio || 1;

    // CSS size
    canvas.style.width = container.clientWidth + 'px';
    canvas.style.height = container.clientHeight + 'px';

    // Actual canvas resolution (for crisp rendering)
    canvas.width = container.clientWidth * dpr;
    canvas.height = container.clientHeight * dpr;

    // Notify render engine of resize
    renderEngine.setViewport(canvas.width, canvas.height);
  }

  window.addEventListener('resize', resize);
  resize();

  // Touch support - already in place via touch-action: none
  canvas.style.touchAction = 'none';
}
```

### Anti-Patterns to Avoid
- **Blocking WASM load:** Don't wait for WASM in critical render path - show UI shell immediately
- **Large URL state:** Don't encode entire project in URL - use minimal view state only, load actual design from file
- **Sync file operations:** Don't block main thread - all File System Access API calls are async
- **Missing HTTPS:** File System Access API and service workers require secure context
- **`unsafe-eval` in CSP:** Use `wasm-unsafe-eval` directive specifically for WASM, not broad `unsafe-eval`

## Don't Hand-Roll

Problems that look simple but have existing solutions:

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| WASM binary optimization | Custom minifier | wasm-opt from Binaryen | Understands WASM semantics, can optimize beyond compiler output, 15-20% size reduction |
| File picker fallback | Manual feature detection + fallback UI | browser-fs-access library | Handles all browser quirks, permission edge cases, mobile differences |
| URL state serialization | JSON.stringify + Base64 | URLSearchParams with typed codecs | URL-safe, readable, debuggable, doesn't bloat URL length |
| Service worker generation | Hand-written SW | Workbox | Handles cache versioning, precaching strategies, updates correctly |
| Asset compression | Runtime compression | Build-time Brotli via plugin | CDN serves pre-compressed, saves edge CPU, faster TTFB |
| Responsive canvas | Manual resize handlers | ResizeObserver API | Debounced, handles all resize cases including container queries |

**Key insight:** WASM deployment has mature tooling - don't recreate what wasm-pack + wasm-opt already provide. File access patterns are complex with browser differences - use established libraries.

## Common Pitfalls

### Pitfall 1: WASM File Not Compressed
**What goes wrong:** Serving 366KB uncompressed WASM on 3G connection takes 4-5 seconds alone, fails WEB-01
**Why it happens:** Vite doesn't auto-compress, CDN compression relies on correct MIME type and headers
**How to avoid:**
- Verify `Content-Type: application/wasm` header in production
- Enable Brotli compression on CDN (Cloudflare Pages does this automatically)
- Consider pre-compressing with vite-plugin-compression for faster edge delivery
- Test with Lighthouse in slow 3G mode
**Warning signs:**
- Network tab shows WASM file without `Content-Encoding: br` header
- Lighthouse reports slow Time to Interactive (>3s on 3G)

### Pitfall 2: Missing WASM Optimization Flags
**What goes wrong:** Debug WASM build is 2-5x larger than optimized, fails performance budget
**Why it happens:** wasm-pack defaults vary by profile, wasm-opt not configured in Cargo.toml
**How to avoid:**
```toml
# crates/cypcb-render/Cargo.toml
[profile.release]
opt-level = "z"     # Optimize for size
lto = true          # Link-time optimization
codegen-units = 1   # Single codegen unit for better optimization
panic = "abort"     # Smaller panic handler

[package.metadata.wasm-pack.profile.release]
wasm-opt = ["-O4", "--enable-simd"]  # Aggressive optimization
```
**Warning signs:**
- WASM file >500KB after wasm-pack build --release
- wasm-opt not mentioned in build output

### Pitfall 3: File System Access API Without User Gesture
**What goes wrong:** `showOpenFilePicker()` throws SecurityError, confusing user experience
**Why it happens:** Browser requires user interaction to prevent abuse
**How to avoid:**
- Always call from click/keydown event handler
- Don't call from async callback that's too far from user action
- Show disabled state until user explicitly clicks "Open" button
**Warning signs:**
- Console error: "must be handling a user gesture to show a file picker"
- File picker works sometimes but not when triggered programmatically

### Pitfall 4: Incorrect Vite WASM Configuration
**What goes wrong:** WASM import fails with "top-level await not supported" or module resolution errors
**Why it happens:** WASM requires special Vite plugin configuration for ES modules
**How to avoid:**
```typescript
// vite.config.ts
import { defineConfig } from 'vite';
import wasm from 'vite-plugin-wasm';
import topLevelAwait from 'vite-plugin-top-level-await';

export default defineConfig({
  plugins: [wasm(), topLevelAwait()],
  optimizeDeps: {
    exclude: ['cypcb-render']  // Don't pre-bundle WASM
  },
  build: {
    target: 'esnext'  // Required for top-level await
  }
});
```
**Warning signs:**
- Build error: "Top-level await is not available"
- Runtime error: "WebAssembly module is being imported"

### Pitfall 5: CSP Blocking WASM
**What goes wrong:** WASM fails to load/compile despite correct file serving
**Why it happens:** Content-Security-Policy header blocks WebAssembly compilation without `wasm-unsafe-eval`
**How to avoid:**
```
Content-Security-Policy:
  default-src 'self';
  script-src 'self' 'wasm-unsafe-eval';
  connect-src 'self';
```
- Use `wasm-unsafe-eval` NOT `unsafe-eval` (more secure, WASM-specific)
- Cloudflare Pages allows CSP configuration via _headers file
**Warning signs:**
- Console error: "wasm code generation disallowed by embedder"
- WASM loads but fails at WebAssembly.instantiate()

### Pitfall 6: URL Length Limits for Shared State
**What goes wrong:** Share URLs fail or get truncated when encoding large state
**Why it happens:** URL length limits ~2000 chars (IE), ~8000 (modern), varies by browser/server
**How to avoid:**
- Encode only minimal view state (zoom, pan, visible layers)
- Don't encode actual design data - that must be loaded from file
- Use short parameter names: `?l=t,b&z=1.5&p=100,200` not `?layers=top,bottom`
- For complex state, use server-side storage with short ID
**Warning signs:**
- URL longer than 500 characters
- Share URLs work in dev but fail in some browsers/servers

### Pitfall 7: Mobile Touch Target Sizes
**What goes wrong:** Buttons/controls too small on tablet, fails WEB-02 usability
**Why it happens:** Desktop-sized controls (24px) too small for touch
**How to avoid:**
- Minimum 44x44px touch targets (Apple HIG)
- Minimum 48x48px recommended (Material Design)
- Use `@media (pointer: coarse)` for touch-specific sizing
```css
button { min-width: 32px; min-height: 32px; }
@media (pointer: coarse) {
  button { min-width: 48px; min-height: 48px; }
}
```
**Warning signs:**
- Lighthouse accessibility audit flags small touch targets
- Users report difficulty tapping buttons on tablet

## Code Examples

Verified patterns from official sources:

### Vite Configuration for WASM Production Build
```typescript
// Source: Vite documentation + vite-plugin-wasm
import { defineConfig } from 'vite';
import wasm from 'vite-plugin-wasm';
import topLevelAwait from 'vite-plugin-top-level-await';

export default defineConfig({
  plugins: [
    wasm(),
    topLevelAwait()
  ],
  optimizeDeps: {
    exclude: ['cypcb-render'] // WASM module loaded separately
  },
  build: {
    target: 'esnext',
    minify: 'esbuild',
    sourcemap: false, // Reduce bundle size
    rollupOptions: {
      output: {
        manualChunks: {
          // Separate vendor code for better caching
          vendor: ['./src/wasm.ts']
        }
      }
    }
  },
  server: {
    port: 4321,
    host: '0.0.0.0'
  }
});
```

### wasm-pack Build Script with Optimization
```bash
# Source: wasm-pack documentation + Rust WASM book
#!/bin/bash
set -e

cd "$(dirname "$0")/.."

echo "Building optimized WASM for production..."

# Build with wasm-pack (includes wasm-bindgen)
wasm-pack build crates/cypcb-render \
  --target web \
  --release \
  --out-dir ../../viewer/pkg \
  --out-name cypcb_render \
  --no-default-features \
  --features wasm

# Additional optimization with wasm-opt (if not in Cargo.toml)
if command -v wasm-opt &> /dev/null; then
  echo "Running wasm-opt..."
  wasm-opt -O4 \
    --enable-simd \
    --converge \
    viewer/pkg/cypcb_render_bg.wasm \
    -o viewer/pkg/cypcb_render_bg.wasm
fi

echo "WASM build complete!"
ls -lh viewer/pkg/cypcb_render_bg.wasm
```

### Cloudflare Pages Deployment Configuration
```yaml
# Source: Cloudflare Pages documentation
# .github/workflows/deploy.yml
name: Deploy to Cloudflare Pages

on:
  push:
    branches: [main]

jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Setup Node
        uses: actions/setup-node@v4
        with:
          node-version: '20'

      - name: Setup Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          target: wasm32-unknown-unknown

      - name: Install wasm-pack
        run: cargo install wasm-pack

      - name: Install dependencies
        run: cd viewer && npm ci

      - name: Build
        run: cd viewer && npm run build

      - name: Deploy to Cloudflare Pages
        uses: cloudflare/wrangler-action@v3
        with:
          apiToken: ${{ secrets.CLOUDFLARE_API_TOKEN }}
          accountId: ${{ secrets.CLOUDFLARE_ACCOUNT_ID }}
          command: pages deploy viewer/dist --project-name=codeyourpcb
```

### Feature Detection and Progressive Enhancement
```typescript
// Source: Chrome Developers File System Access API docs
export const fileSystemSupport = {
  hasFileSystemAccess: 'showOpenFilePicker' in window,
  hasServiceWorker: 'serviceWorker' in navigator,
  hasWebAssembly: typeof WebAssembly !== 'undefined',
  hasWebGL2: (() => {
    const canvas = document.createElement('canvas');
    return !!(canvas.getContext('webgl2'));
  })()
};

export async function checkRequirements(): Promise<{ok: boolean; missing: string[]}> {
  const missing: string[] = [];

  if (!fileSystemSupport.hasWebAssembly) {
    missing.push('WebAssembly');
  }
  if (!fileSystemSupport.hasWebGL2) {
    missing.push('WebGL 2.0');
  }

  return {
    ok: missing.length === 0,
    missing
  };
}

// Show feature requirements on unsupported browsers
if (!fileSystemSupport.hasWebAssembly) {
  document.body.innerHTML = `
    <div style="padding: 2rem; text-align: center;">
      <h1>Browser Not Supported</h1>
      <p>CodeYourPCB requires WebAssembly support.</p>
      <p>Please use Chrome 86+, Firefox 104+, Safari 16+, or Edge 86+</p>
    </div>
  `;
}
```

### Responsive Canvas with Device Pixel Ratio
```typescript
// Source: MDN Canvas API + Responsive Design best practices
export class ResponsiveCanvas {
  private resizeObserver: ResizeObserver;

  constructor(
    private canvas: HTMLCanvasElement,
    private onResize: (width: number, height: number) => void
  ) {
    this.resizeObserver = new ResizeObserver(() => this.handleResize());
    this.resizeObserver.observe(canvas.parentElement!);
    this.handleResize();
  }

  private handleResize() {
    const container = this.canvas.parentElement!;
    const dpr = window.devicePixelRatio || 1;

    // CSS size (display size)
    const cssWidth = container.clientWidth;
    const cssHeight = container.clientHeight;
    this.canvas.style.width = cssWidth + 'px';
    this.canvas.style.height = cssHeight + 'px';

    // Canvas resolution (for crisp rendering)
    this.canvas.width = Math.floor(cssWidth * dpr);
    this.canvas.height = Math.floor(cssHeight * dpr);

    // Notify renderer
    this.onResize(this.canvas.width, this.canvas.height);
  }

  destroy() {
    this.resizeObserver.disconnect();
  }
}

// Usage
const canvasManager = new ResponsiveCanvas(
  document.getElementById('pcb-canvas') as HTMLCanvasElement,
  (width, height) => {
    wasmRenderer.setViewport(width, height);
  }
);
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `unsafe-eval` for WASM | `wasm-unsafe-eval` CSP directive | 2021 (Chrome 103+) | More secure CSP without blocking WASM |
| Manual WASM loading | WebAssembly.compileStreaming() | 2021 (standard) | Faster loading via streaming compilation |
| Input element file access | File System Access API | 2020-2023 (Chrome 86+, Safari 15.2+) | Native save without download prompt |
| Base64 in localStorage | IndexedDB for file storage | Ongoing | Better performance for large files |
| gzip compression | Brotli compression | 2015+ (universal 2020+) | 15-20% better compression than gzip |
| Manual chunk splitting | Automatic with dynamic imports | Vite 2+ (2021+) | Simpler code splitting |

**Deprecated/outdated:**
- **File and FileReader API (non-File System Access)**: Still works but provides worse UX - forces save-as dialog every time instead of direct file modification
- **ApplicationCache**: Deprecated in favor of Service Workers for offline support
- **wasm-bindgen --no-modules**: Use `--target web` for ES modules support in modern bundlers
- **`unsafe-eval` CSP for WASM**: Replaced by more secure `wasm-unsafe-eval` directive

## Open Questions

Things that couldn't be fully resolved:

1. **Firefox File System Access API Support**
   - What we know: Firefox 111+ has experimental support, not enabled by default as of 2026
   - What's unclear: Whether to rely on fallback permanently or if Firefox will enable by default soon
   - Recommendation: Implement fallback pattern, monitor Firefox release notes, provide equal UX both paths

2. **Optimal wasm-opt Flags for Size vs Performance**
   - What we know: `-O4` is most aggressive, `--converge` runs until fixed point, SIMD helps math-heavy code
   - What's unclear: Whether `-Os` (optimize for size) beats `-O4` (optimize aggressively) for final bundle size in practice
   - Recommendation: Test both `-O4` and `-Os`, measure with real WASM bundle, choose based on empirical size

3. **Service Worker Caching Strategy Priority**
   - What we know: PWA offline support not required in WEB-01 through WEB-09
   - What's unclear: Whether to implement as "nice to have" or defer to future phase
   - Recommendation: Defer - focus on WEB-01 (3s load) first, add PWA in separate enhancement phase

4. **URL State Encoding Format**
   - What we know: URLSearchParams is standard, need minimal state for sharing
   - What's unclear: Exact schema for what view state to include (zoom/pan/layers vs more complex state)
   - Recommendation: Start minimal (visible layers only), iterate based on user feedback on what they want to share

## Sources

### Primary (HIGH confidence)
- [Vite Build Guide](https://vite.dev/guide/build.html) - Production build configuration
- [Cloudflare Pages Vite Deployment](https://developers.cloudflare.com/pages/framework-guides/deploy-a-vite3-project/) - Deployment steps
- [Chrome File System Access API](https://developer.chrome.com/docs/capabilities/web-apis/file-system-access) - API documentation
- [MDN File System API](https://developer.mozilla.org/en-US/docs/Web/API/File_System_API) - Browser support
- [MDN WebAssembly.compileStreaming](https://developer.mozilla.org/en-US/docs/WebAssembly/Reference/JavaScript_interface/compileStreaming_static) - Streaming compilation
- [MDN Content-Security-Policy](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Content-Security-Policy) - CSP headers
- [WebAssembly CSP Proposal](https://github.com/WebAssembly/content-security-policy/blob/main/proposals/CSP.md) - wasm-unsafe-eval directive
- [MDN Progressive Web Apps Caching](https://developer.mozilla.org/en-US/docs/Web/Progressive_web_apps/Guides/Caching) - Cache strategies
- [Rust WASM Tools Reference](https://rustwasm.github.io/book/reference/tools.html) - wasm-opt usage
- [wasm-bindgen Optimize for Size](https://rustwasm.github.io/docs/wasm-bindgen/reference/optimize-size.html) - Size optimization
- [Rust WASM Code Size Guide](https://rustwasm.github.io/docs/book/reference/code-size.html) - Cargo profile configuration

### Secondary (MEDIUM confidence)
- [Vite Features Guide](https://vite.dev/guide/features) - WASM handling
- [vite-plugin-wasm npm](https://www.npmjs.com/package/vite-plugin-wasm) - Plugin configuration
- [Cloudflare Pages GitHub Integration](https://developers.cloudflare.com/pages/configuration/git-integration/github-integration/) - Git deployment
- [Cloudflare WebAssembly Runtime](https://developers.cloudflare.com/workers/runtime-apis/webassembly/) - WASM support
- [Vercel WASM Functions](https://vercel.com/docs/functions/runtimes/wasm) - Comparison data
- [Can I Use File System Access API](https://caniuse.com/native-filesystem-api) - Browser support matrix
- [Can I Use WebAssembly](https://caniuse.com/wasm) - Browser support
- [2026 Web Performance Standards](https://www.inmotionhosting.com/blog/web-performance-benchmarks/) - Performance metrics
- [Core Web Vitals 2026](https://almcorp.com/blog/core-web-vitals-2026-technical-seo-guide/) - SEO requirements
- [Responsive Web Design 2026](https://www.keelis.com/blog/responsive-web-design-in-2026:-trends-and-best-practices) - Mobile best practices
- [BrowserStack Screen Resolutions 2026](https://www.browserstack.com/guide/common-screen-resolutions) - Device targets
- [MDN Responsive Web Design](https://developer.mozilla.org/en-US/docs/Learn_web_development/Core/CSS_layout/Responsive_Design) - RWD principles

### Tertiary (LOW confidence - WebSearch findings)
- [InfoQ WASM Optimization](https://www.infoq.com/articles/six-ways-optimize-webassembly/) - General optimization strategies
- [Leptos WASM Binary Size](https://book.leptos.dev/deployment/binary_size.html) - Real-world size reduction examples
- [LogRocket URL State Management](https://blog.logrocket.com/advanced-react-state-management-using-url-parameters/) - URL patterns (React-specific)
- [State of WebAssembly 2025-2026](https://platform.uno/blog/the-state-of-webassembly-2025-2026/) - Ecosystem trends
- [Nucamp Deployment Comparison 2026](https://www.nucamp.co/blog/deploying-full-stack-apps-in-2026-vercel-netlify-railway-and-cloud-options) - Platform comparison
- [Medium: Cloudflare vs Vercel Pricing](https://medium.com/@pedro.diniz.rocha/why-cloudflare-is-the-best-alternative-to-vercel-in-2024-an-in-depth-pricing-comparison-7e1d713f8fde) - Cost analysis
- [High Performance Browser Networking](https://hpbn.co/optimizing-for-mobile-networks/) - 3G optimization
- [Vanilla JS State Management 2026](https://medium.com/@chirag.dave/state-management-in-vanilla-js-2026-trends-f9baed7599de) - Lightweight approaches
- [vite-plugin-compression GitHub](https://github.com/vbenjs/vite-plugin-compression) - Asset compression plugin
- [wasm-pack optimization flags](https://arhan.sh/blog/wasm-pack-optimization-flags-you-never-knew-about/) - Advanced configuration

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - Vite, wasm-pack, Cloudflare Pages documented in official sources
- Architecture: HIGH - File System Access API, WASM loading patterns from MDN/Chrome docs
- Pitfalls: MEDIUM - Mix of documented issues and WebSearch-discovered common problems

**Research date:** 2026-01-30
**Valid until:** 2026-03-30 (60 days - web standards stable, tooling evolves slowly)
