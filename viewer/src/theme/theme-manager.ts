/**
 * Central theme coordination singleton
 *
 * Manages theme preference persistence, OS dark mode detection,
 * and notifies listeners of theme changes.
 */

import type { Theme, ResolvedTheme, ThemeChangeListener } from './theme-types';

const STORAGE_KEY = 'theme';

export class ThemeManager {
  private theme: Theme;
  private mediaQuery: MediaQueryList;
  private listeners: Set<ThemeChangeListener> = new Set();

  constructor() {
    // Read saved theme preference from localStorage
    const saved = localStorage.getItem(STORAGE_KEY);
    this.theme = (saved === 'light' || saved === 'dark' || saved === 'auto') ? saved : 'auto';

    // Set up media query for system preference
    this.mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');

    // Listen for OS theme changes
    this.mediaQuery.addEventListener('change', () => {
      if (this.theme === 'auto') {
        this.updateTheme();
      }
    });

    // Apply initial theme
    this.updateTheme();
  }

  /**
   * Set theme preference and persist to localStorage
   */
  setTheme(theme: Theme): void {
    this.theme = theme;
    localStorage.setItem(STORAGE_KEY, theme);
    this.updateTheme();
  }

  /**
   * Get current theme preference (may be 'auto')
   */
  getTheme(): Theme {
    return this.theme;
  }

  /**
   * Get resolved theme (always 'light' or 'dark')
   */
  getResolvedTheme(): ResolvedTheme {
    if (this.theme === 'auto') {
      return this.mediaQuery.matches ? 'dark' : 'light';
    }
    return this.theme;
  }

  /**
   * Subscribe to theme changes
   * @returns Unsubscribe function
   */
  subscribe(listener: ThemeChangeListener): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }

  /**
   * Update DOM and notify listeners
   */
  private updateTheme(): void {
    const resolved = this.getResolvedTheme();
    document.documentElement.setAttribute('data-theme', resolved);

    // Notify all listeners
    this.listeners.forEach(listener => {
      listener(resolved);
    });
  }
}

// Export singleton instance
export const themeManager = new ThemeManager();
