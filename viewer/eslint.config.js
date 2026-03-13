import tseslint from 'typescript-eslint';

export default tseslint.config(
  // Global ignores
  {
    ignores: [
      'dist/**',
      'node_modules/**',
      'src-tauri/**',
      'src/pkg/**',
      '*.js',
    ],
  },

  // Base recommended rules for TypeScript files
  ...tseslint.configs.recommended,

  // Project-specific overrides
  {
    files: ['src/**/*.ts'],
    rules: {
      // Allow unused vars prefixed with _ (common pattern in this codebase)
      '@typescript-eslint/no-unused-vars': ['error', {
        argsIgnorePattern: '^_',
        varsIgnorePattern: '^_',
        caughtErrorsIgnorePattern: '^_',
      }],
      // Allow explicit `any` — codebase uses it sparingly for WASM interop and window extensions
      '@typescript-eslint/no-explicit-any': 'off',
      // Allow `const self = this` in debug surface getters that need closure capture
      '@typescript-eslint/no-this-alias': 'off',
    },
  },
);
