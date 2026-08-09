import { defineConfig, mergeConfig } from 'vitest/config';
import viteConfig from './vite.config';

// `vite.config.ts` exports through Vite's own `defineConfig`, whose return type
// covers every shape a config may take - a promise, a function, an object.
// `mergeConfig` wants the object, and this is one; the two packages' types do
// not know that about each other.
export default mergeConfig(viteConfig as never, defineConfig({
  test: {
    include: ['src/__tests__/**/*.test.ts'],
    environment: 'node',
  },
}));
