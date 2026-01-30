import { defineConfig } from 'vite';
import wasm from 'vite-plugin-wasm';
import topLevelAwait from 'vite-plugin-top-level-await';
// @ts-ignore - CommonJS module
import monacoEditorPluginModule from 'vite-plugin-monaco-editor';
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
    rollupOptions: {
      output: {
        manualChunks: {
          vendor: ['./src/wasm.ts'],
          monaco: ['monaco-editor'],
        },
      },
    },
  },
  optimizeDeps: {
    exclude: ['cypcb-render'], // WASM module will be loaded separately
  },
});
