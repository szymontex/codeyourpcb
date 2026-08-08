import tseslint from 'typescript-eslint';
import playwright from 'eslint-plugin-playwright';

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

  // End-to-end specs.
  //
  // These were outside the linted tree entirely: stage 4 of the gate ran
  // `eslint src/`, so 3,100 lines of test code had never been read by a linter.
  // The audit that preceded this found four tests that could not fail, and
  // three of those four are shapes a rule catches for free - which is why the
  // Playwright plugin is here rather than a promise to look harder next time.
  {
    files: ['e2e/**/*.ts'],
    ...playwright.configs['flat/recommended'],
    rules: {
      ...playwright.configs['flat/recommended'].rules,

      // The rule for the defect this project actually shipped: an assertion
      // inside an `if` that guards the very thing under test passes when the
      // feature is dead. `renderer-quality.spec.ts` did this for months.
      'playwright/no-conditional-expect': 'error',

      // An awaited assertion that nobody awaited never fails the test.
      'playwright/missing-playwright-await': 'error',

      // The same disease one step earlier: a branch in a test body means the
      // run took one of two paths and nobody knows which.
      'playwright/no-conditional-in-test': 'error',

      // Two rules are off with a number rather than a shrug, because both are
      // real debt and neither is this change:
      //
      //   `no-wait-for-timeout`   63 sites. A canvas has no locator to await,
      //                           so most of these are waiting for a render
      //                           loop to settle. Each one wants a
      //                           `waitForFunction` on a condition instead.
      //   `prefer-locator`        87 sites of `page.click('#id')`. Locators
      //                           retry; a raw click does not, which is a
      //                           flake waiting to happen.
      //
      // Measured 2026-08-08 with these rules on. Both are recorded in
      // docs/TRACKER.md under V3.
      'playwright/no-wait-for-timeout': 'off',
      'playwright/prefer-locator': 'off',

      // Same policy as `src/`: `any` is how a spec reaches `window.__pcbEngine`
      // and the other debug surfaces, which are deliberately untyped.
      '@typescript-eslint/no-explicit-any': 'off',
      '@typescript-eslint/no-unused-vars': ['error', {
        argsIgnorePattern: '^_',
        varsIgnorePattern: '^_',
        caughtErrorsIgnorePattern: '^_',
      }],

      // Three suites are `describe.skip` because the UI they drive is hidden
      // pending an owner decision. They are listed in `.gsd/REQUIREMENTS.md`
      // and a vitest test fails if that list drifts, so the skip is recorded
      // where it means something rather than shouted on every lint run.
      'playwright/no-skipped-test': 'off',
    },
  },
);
