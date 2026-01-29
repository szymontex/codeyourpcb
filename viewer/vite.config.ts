import { defineConfig } from 'vite';

export default defineConfig({
  // Minimal config for TypeScript + WASM support
  server: {
    port: 4321,
    host: process.env.TAURI_DEV_HOST || '0.0.0.0',
    strictPort: true,
    allowedHosts: ['dev1.flightcore.pl', 'dev2.flightcore.pl', 'dev3.flightcore.pl', 'dev4.flightcore.pl'],
    watch: {
      ignored: ['**/src-tauri/**'],
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
  },
  optimizeDeps: {
    exclude: ['cypcb-render'], // WASM module will be loaded separately
  },
});
