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

  // Setup draggable divider
  setupDivider();

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
    // Show editor and divider
    editorContainer.style.display = 'block';
    divider.style.display = 'block';
    canvasContainer.style.flex = '1';
  } else {
    // Hide editor and divider
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

/**
 * Get the current editor instance
 *
 * @returns Monaco editor instance or null if not initialized
 */
export function getEditorInstance(): any {
  return editorInstance;
}

/**
 * Setup draggable divider between editor and canvas
 *
 * Allows user to resize editor/canvas proportions by dragging the divider.
 * Enforces minimum width (200px) and maximum width (70% of main content).
 */
export function setupDivider(): void {
  const divider = document.getElementById('divider');
  const editorContainer = document.getElementById('editor-container');
  const mainContent = document.getElementById('main-content');

  if (!divider || !editorContainer || !mainContent) {
    console.error('[Divider] Required elements not found');
    return;
  }

  let isDragging = false;

  // Mouse events
  divider.addEventListener('mousedown', (e: MouseEvent) => {
    e.preventDefault();
    isDragging = true;
    document.body.style.userSelect = 'none';
  });

  document.addEventListener('mousemove', (e: MouseEvent) => {
    if (!isDragging) return;

    const mainRect = mainContent.getBoundingClientRect();
    const newWidth = e.clientX - mainRect.left;

    // Clamp between 200px and 70% of main content width
    const minWidth = 200;
    const maxWidth = mainRect.width * 0.7;
    const clampedWidth = Math.min(Math.max(newWidth, minWidth), maxWidth);

    editorContainer.style.width = clampedWidth + 'px';

    // Trigger Monaco layout recalculation
    if (editorInstance) {
      editorInstance.layout();
    }
  });

  document.addEventListener('mouseup', () => {
    if (isDragging) {
      isDragging = false;
      document.body.style.userSelect = '';
    }
  });

  // Touch events for tablet support
  divider.addEventListener('touchstart', (e: TouchEvent) => {
    e.preventDefault();
    isDragging = true;
    document.body.style.userSelect = 'none';
  });

  document.addEventListener('touchmove', (e: TouchEvent) => {
    if (!isDragging || e.touches.length === 0) return;

    const touch = e.touches[0];
    const mainRect = mainContent.getBoundingClientRect();
    const newWidth = touch.clientX - mainRect.left;

    // Clamp between 200px and 70% of main content width
    const minWidth = 200;
    const maxWidth = mainRect.width * 0.7;
    const clampedWidth = Math.min(Math.max(newWidth, minWidth), maxWidth);

    editorContainer.style.width = clampedWidth + 'px';

    // Trigger Monaco layout recalculation
    if (editorInstance) {
      editorInstance.layout();
    }
  });

  document.addEventListener('touchend', () => {
    if (isDragging) {
      isDragging = false;
      document.body.style.userSelect = '';
    }
  });

  console.log('[Divider] Drag handlers set up');
}
