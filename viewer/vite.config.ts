import { defineConfig } from 'vite';

export default defineConfig({
  // Minimal config for TypeScript + WASM support
  server: {
    port: 4321,
    host: '0.0.0.0',
    strictPort: true,
    allowedHosts: ['dev1.flightcore.pl', 'dev2.flightcore.pl', 'dev3.flightcore.pl', 'dev4.flightcore.pl'],
  },
  build: {
    target: 'esnext',
  },
  optimizeDeps: {
    exclude: ['cypcb-render'], // WASM module will be loaded separately
  },
});
