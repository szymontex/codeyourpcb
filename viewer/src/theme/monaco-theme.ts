/**
 * Monaco Editor theme definitions for CodeYourPCB
 *
 * These themes map our application color palette to Monaco's token system.
 * Phase 14 (Monaco Editor Integration) will call applyMonacoTheme() after
 * loading the Monaco editor.
 *
 * NO monaco-editor imports - this is pure theme data to avoid dependency.
 */

import type { ResolvedTheme } from './theme-types';
import { themeManager } from './theme-manager';

/**
 * Matches monaco.editor.IStandaloneThemeData shape.
 * Defined locally to avoid monaco-editor dependency.
 */
interface MonacoThemeData {
  base: 'vs' | 'vs-dark';
  inherit: boolean;
  rules: Array<{ token: string; foreground?: string; background?: string; fontStyle?: string }>;
  colors: Record<string, string>;
}

/**
 * Light theme for Monaco editor
 * Syntax colors chosen for PCB DSL (keywords, numbers, strings, comments)
 * Editor chrome matches light theme CSS variables from colors.css
 */
export const lightTheme: MonacoThemeData = {
  base: 'vs',
  inherit: true,
  rules: [
    // PCB DSL syntax tokens
    { token: 'comment', foreground: '6a9955' },
    { token: 'keyword', foreground: '0000ff' },
    { token: 'string', foreground: 'a31515' },
    { token: 'number', foreground: '098658' },
    { token: 'type', foreground: '267f99' },
    { token: 'operator', foreground: '000000' },
  ],
  colors: {
    // Editor chrome matching colors.css --bg-primary, --text-primary, etc.
    'editor.background': '#ffffff',
    'editor.foreground': '#1a1a1a',
    'editor.lineHighlightBackground': '#f5f5f5',
    'editorLineNumber.foreground': '#999999',
    'editorGutter.background': '#f5f5f5',
    'editor.selectionBackground': '#add6ff',
    'editorCursor.foreground': '#000000',
  },
};

/**
 * Dark theme for Monaco editor
 * Syntax colors adapted for dark backgrounds
 * Editor chrome matches dark theme CSS variables from colors.css
 */
export const darkTheme: MonacoThemeData = {
  base: 'vs-dark',
  inherit: true,
  rules: [
    // PCB DSL syntax tokens (dark variant)
    { token: 'comment', foreground: '6a9955' },
    { token: 'keyword', foreground: '569cd6' },
    { token: 'string', foreground: 'ce9178' },
    { token: 'number', foreground: 'b5cea8' },
    { token: 'type', foreground: '4ec9b0' },
    { token: 'operator', foreground: 'd4d4d4' },
  ],
  colors: {
    // Editor chrome matching colors.css [data-theme="dark"]
    'editor.background': '#1e1e1e',
    'editor.foreground': '#e0e0e0',
    'editor.lineHighlightBackground': '#252525',
    'editorLineNumber.foreground': '#808080',
    'editorGutter.background': '#1e1e1e',
    'editor.selectionBackground': '#264f78',
    'editorCursor.foreground': '#ffffff',
  },
};

/**
 * Apply Monaco themes and wire to ThemeManager
 *
 * Phase 14 will call this function after loading Monaco:
 * ```typescript
 * import * as monaco from 'monaco-editor';
 * import { applyMonacoTheme } from './theme/monaco-theme';
 * applyMonacoTheme(monaco);
 * ```
 *
 * @param monaco - The monaco-editor module (typed as any to avoid dependency)
 */
export function applyMonacoTheme(monaco: any): void {
  // Define custom themes
  monaco.editor.defineTheme('cypcb-light', lightTheme);
  monaco.editor.defineTheme('cypcb-dark', darkTheme);

  // Apply initial theme
  const currentTheme = themeManager.getResolvedTheme();
  monaco.editor.setTheme(currentTheme === 'dark' ? 'cypcb-dark' : 'cypcb-light');

  // Subscribe to theme changes for auto-sync
  themeManager.subscribe((resolved: ResolvedTheme) => {
    monaco.editor.setTheme(resolved === 'dark' ? 'cypcb-dark' : 'cypcb-light');
  });
}
