/**
 * Monaco editor panel management
 *
 * Handles lazy loading Monaco editor, registering the .cypcb language,
 * applying themes, and managing editor visibility.
 */

// Visibility state
let editorVisible = false;
let editorInstance: any = null;

/**
 * Initialize Monaco editor in the given container
 *
 * Lazily loads Monaco to avoid blocking initial page render.
 * Registers .cypcb language with Monarch tokenizer.
 * Wires theme system via applyMonacoTheme().
 *
 * @param container - HTML element to mount editor in
 * @returns Monaco editor instance
 */
export async function initEditor(container: HTMLElement): Promise<any> {
  // Lazy load Monaco to avoid blocking initial render
  const monaco = await import('monaco-editor');
  const { applyMonacoTheme } = await import('../theme/monaco-theme');
  const { cypcbLanguage, cypcbLanguageConfig } = await import('./cypcb-language');

  // Register custom .cypcb language
  monaco.languages.register({ id: 'cypcb', extensions: ['.cypcb'] });
  monaco.languages.setMonarchTokensProvider('cypcb', cypcbLanguage);
  monaco.languages.setLanguageConfiguration('cypcb', cypcbLanguageConfig);

  // Apply theme (wires to ThemeManager)
  applyMonacoTheme(monaco);

  // Create editor instance
  const editor = monaco.editor.create(container, {
    language: 'cypcb',
    automaticLayout: true, // Handles resize automatically
    minimap: { enabled: false }, // Save space for side-by-side layout
    fontSize: 14,
    lineNumbers: 'on', // EDIT-04: Line numbers
    folding: true, // EDIT-05: Code folding
    wordWrap: 'off',
    scrollBeyondLastLine: false,
  });

  editorInstance = editor;
  return editor;
}

/**
 * Toggle editor panel visibility
 *
 * Shows/hides the editor container and adjusts canvas container.
 * Updates visibility state.
 */
export function toggleEditorPanel(): void {
  const editorContainer = document.getElementById('editor-container');
  const divider = document.getElementById('divider');
  const canvasContainer = document.getElementById('canvas-container');

  if (!editorContainer || !divider || !canvasContainer) {
    console.error('Editor toggle: required elements not found');
    return;
  }

  editorVisible = !editorVisible;

  if (editorVisible) {
    // Show editor
    editorContainer.style.display = 'block';
    divider.style.display = 'block';
    canvasContainer.style.flex = '1';
  } else {
    // Hide editor
    editorContainer.style.display = 'none';
    divider.style.display = 'none';
    canvasContainer.style.flex = '1';
  }

  // Trigger layout recalculation if editor exists
  if (editorInstance) {
    editorInstance.layout();
  }
}

/**
 * Check if editor is currently visible
 *
 * @returns true if editor panel is visible
 */
export function isEditorVisible(): boolean {
  return editorVisible;
}
