import { defineConfig } from 'vite';
import wasm from 'vite-plugin-wasm';
import topLevelAwait from 'vite-plugin-top-level-await';
import monacoEditorPluginModule from 'vite-plugin-monaco-editor';
// The plugin is CommonJS: under a bundler it arrives as the function itself,
// under Node's ESM interop as `{ default: fn }`. Its types describe only the
// first, so the fallback is the part the compiler cannot see.
// @ts-expect-error - CommonJS interop the plugin's types do not describe
const monacoEditorPlugin = monacoEditorPluginModule.default || monacoEditorPluginModule;

export default defineConfig({
  plugins: [
    wasm(),
    topLevelAwait(),
    monacoEditorPlugin({
      languageWorkers: ['editorWorkerService'],
      customWorkers: [],
    }),
  ],
  server: {
    port: 4321,
    host: process.env.TAURI_DEV_HOST || '0.0.0.0',
    strictPort: true,
    allowedHosts: ['dev1.flightcore.pl', 'dev2.flightcore.pl', 'dev3.flightcore.pl', 'dev4.flightcore.pl'],
    watch: {
      // svg-pcb and circuitron are unbuilt reference checkouts that import
      // packages this project does not depend on; scanning them kills the dev
      // server, which is what kept the Playwright stage from ever running.
      ignored: ['**/src-tauri/**', '**/svg-pcb/**', '**/circuitron/**'],
    },
    proxy: {
      // LCSC product detail API proxy — fetch product images
      '/easyeda-api/lcsc/product': {
        target: 'https://wmsc.lcsc.com',
        changeOrigin: true,
        configure: (proxy: any) => {
          proxy.on('proxyReq', (proxyReq: any) => {
            proxyReq.setHeader('Referer', 'https://www.lcsc.com/');
          });
        },
        rewrite: (path: string) => {
          const url = new URL(path, 'http://localhost');
          const code = url.searchParams.get('code') || '';
          return `/ftps/wm/product/detail?productCode=${code}`;
        },
      },
      // LCSC image proxy — bypass hot-link protection for component images
      '/easyeda-api/img': {
        target: 'https://assets.lcsc.com', // placeholder, overridden by router
        changeOrigin: true,
        configure: (proxy: any) => {
          proxy.on('proxyReq', (proxyReq: any, req: any) => {
            proxyReq.setHeader('Referer', 'https://www.lcsc.com/');
            // Extract real image URL from ?url= param
            const reqUrl = new URL(req.url!, `http://${req.headers.host}`);
            const imgUrl = reqUrl.searchParams.get('url');
            if (imgUrl) {
              const parsed = new URL(imgUrl);
              proxyReq.setHeader('host', parsed.hostname);
              proxyReq.path = parsed.pathname + parsed.search;
            }
          });
        },
        // `router` is an http-proxy option. Vite passes its proxy config
        // straight through to http-proxy but types only the subset it
        // documents, so this works and does not typecheck.
        // @ts-expect-error - http-proxy option Vite's ProxyOptions omits
        router: (req: any) => {
          const reqUrl = new URL(req.url!, `http://${req.headers.host}`);
          const imgUrl = reqUrl.searchParams.get('url');
          if (imgUrl) {
            const parsed = new URL(imgUrl);
            return `${parsed.protocol}//${parsed.host}`;
          }
          return 'https://assets.lcsc.com';
        },
      },
      // EasyEDA API proxy — bypass CORS for footprint/component data
      '/easyeda-api': {
        target: 'https://easyeda.com',
        changeOrigin: true,
        rewrite: (path: string) => path.replace(/^\/easyeda-api/, ''),
      },
      // EasyEDA 3D model modules proxy
      '/easyeda-modules': {
        target: 'https://modules.easyeda.com',
        changeOrigin: true,
        rewrite: (path: string) => path.replace(/^\/easyeda-modules/, ''),
      },
    },
  },
  envPrefix: ['VITE_', 'TAURI_ENV_*'],
  build: {
    target: process.env.TAURI_ENV_PLATFORM === 'windows'
      ? 'chrome105'
      : process.env.TAURI_ENV_PLATFORM
        ? 'safari13'
        : 'esnext',
    minify: !process.env.TAURI_ENV_DEBUG ? 'esbuild' : false,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
    rollupOptions: {
      output: {
        manualChunks: {
          vendor: ['./src/wasm.ts'],
          monaco: ['monaco-editor'],
          three: ['three'],
        },
      },
    },
  },
  optimizeDeps: {
    // Scan only this app's entry points. The default scan walks every HTML file
    // under root, including the reference checkouts, and fails on their imports.
    entries: ['index.html', 'src/**/*.ts'],
    exclude: ['cypcb-render'], // WASM module will be loaded separately
    include: ['monaco-editor'], // Pre-bundle Monaco for dynamic import
  },
});
