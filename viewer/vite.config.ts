import { defineConfig } from 'vite';

export default defineConfig({
  // Minimal config for TypeScript + WASM support
  build: {
    target: 'esnext',
  },
  optimizeDeps: {
    exclude: ['cypcb-render'], // WASM module will be loaded separately
  },
});
