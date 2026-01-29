/**
 * Theme type definitions for the CodeYourPCB viewer
 */

/**
 * Theme preference - can be explicit (light/dark) or auto (follow system)
 */
export type Theme = 'light' | 'dark' | 'auto';

/**
 * Resolved theme - always light or dark (auto has been resolved)
 */
export type ResolvedTheme = 'light' | 'dark';

/**
 * Callback function for theme changes
 */
export type ThemeChangeListener = (theme: ResolvedTheme) => void;
