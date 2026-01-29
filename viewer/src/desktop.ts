/**
 * Desktop integration layer for Tauri IPC and menu events
 *
 * Handles desktop-specific behavior when running in Tauri:
 * - Native menu event handling
 * - File operations via Tauri IPC
 * - Window management
 */

// Module-level state for tracking current file
let currentFilePath: string | null = null;

// Lazy-loaded Tauri APIs (dynamic import to avoid breaking web builds)
let tauriEvent: any = null;
let tauriCore: any = null;

/**
 * Detect if running in Tauri desktop environment.
 * Checks for the presence of __TAURI_INTERNALS__ global.
 */
export function isDesktop(): boolean {
  return typeof (window as any).__TAURI_INTERNALS__ !== 'undefined';
}

/**
 * Get the current file path (for save operations).
 */
export function getCurrentFilePath(): string | null {
  return currentFilePath;
}

/**
 * Set the current file path (after opening or saving).
 */
export function setCurrentFilePath(path: string | null): void {
  currentFilePath = path;
}

/**
 * Initialize desktop integrations.
 * Should only be called if isDesktop() returns true.
 */
export async function initDesktop(): Promise<void> {
  if (!isDesktop()) {
    console.warn('[Desktop] initDesktop() called in non-desktop environment');
    return;
  }

  try {
    // Dynamic imports to prevent breaking web builds
    tauriEvent = await import('@tauri-apps/api/event');
    tauriCore = await import('@tauri-apps/api/core');

    console.log('[Desktop] Tauri APIs loaded');

    // Listen for menu events from native menus
    await tauriEvent.listen('menu-action', (event: any) => {
      const action = event.payload as string;
      console.log('[Desktop] Menu action:', action);
      handleMenuAction(action);
    });

    console.log('[Desktop] Initialized successfully');
  } catch (err) {
    console.error('[Desktop] Failed to initialize:', err);
  }
}

/**
 * Handle menu action events from native menus.
 */
function handleMenuAction(action: string): void {
  switch (action) {
    case 'file.new':
      handleNewFile();
      break;
    case 'file.open':
      handleOpenFile();
      break;
    case 'file.save':
      handleSaveFile();
      break;
    case 'file.save_as':
      handleSaveFileAs();
      break;
    case 'view.zoom_in':
      dispatchViewportEvent('zoom-in');
      break;
    case 'view.zoom_out':
      dispatchViewportEvent('zoom-out');
      break;
    case 'view.fit':
      dispatchViewportEvent('fit');
      break;
    case 'view.theme':
      handleToggleTheme();
      break;
    case 'help.about':
      handleAbout();
      break;
    default:
      console.log('[Desktop] Unhandled menu action:', action);
  }
}

/**
 * Handle File > New - clear the current design.
 */
function handleNewFile(): void {
  const event = new CustomEvent('desktop:new-file');
  window.dispatchEvent(event);
  currentFilePath = null;
}

/**
 * Handle File > Open - invoke Tauri open_file command.
 */
async function handleOpenFile(): Promise<void> {
  if (!tauriCore) return;

  try {
    const result = await tauriCore.invoke('open_file') as { path: string; content: string } | null;

    if (result) {
      console.log('[Desktop] File opened:', result.path);
      currentFilePath = result.path;

      // Dispatch custom event with file content for the app to load
      const event = new CustomEvent('desktop:open-file', {
        detail: {
          path: result.path,
          content: result.content,
        },
      });
      window.dispatchEvent(event);
    }
  } catch (err) {
    console.error('[Desktop] Failed to open file:', err);
    alert(`Failed to open file: ${err}`);
  }
}

/**
 * Handle File > Save - save to current file path or trigger Save As.
 */
async function handleSaveFile(): Promise<void> {
  if (!currentFilePath) {
    // No file path yet, fall through to Save As
    await handleSaveFileAs();
    return;
  }

  if (!tauriCore) return;

  try {
    // Get current content from the app
    const content = await getCurrentFileContent();
    if (content === null) {
      console.warn('[Desktop] No content to save');
      return;
    }

    await tauriCore.invoke('save_file', { path: currentFilePath, content });
    console.log('[Desktop] File saved:', currentFilePath);

    // Dispatch save success event
    const event = new CustomEvent('desktop:file-saved', {
      detail: { path: currentFilePath },
    });
    window.dispatchEvent(event);
  } catch (err) {
    console.error('[Desktop] Failed to save file:', err);
    alert(`Failed to save file: ${err}`);
  }
}

/**
 * Handle File > Save As - save with file picker.
 */
async function handleSaveFileAs(): Promise<void> {
  if (!tauriCore) return;

  try {
    // Get current content from the app
    const content = await getCurrentFileContent();
    if (content === null) {
      console.warn('[Desktop] No content to save');
      return;
    }

    const path = await tauriCore.invoke('save_file_as', { content }) as string | null;

    if (path) {
      console.log('[Desktop] File saved as:', path);
      currentFilePath = path;

      // Dispatch save success event
      const event = new CustomEvent('desktop:file-saved', {
        detail: { path },
      });
      window.dispatchEvent(event);
    }
  } catch (err) {
    console.error('[Desktop] Failed to save file as:', err);
    alert(`Failed to save file: ${err}`);
  }
}

/**
 * Get current file content from the app.
 * Dispatches a custom event and waits for response via promise.
 */
function getCurrentFileContent(): Promise<string | null> {
  return new Promise((resolve) => {
    // Listen for response
    const handler = (event: Event) => {
      const customEvent = event as CustomEvent<{ content: string | null }>;
      window.removeEventListener('desktop:content-response', handler);
      resolve(customEvent.detail.content);
    };

    window.addEventListener('desktop:content-response', handler);

    // Request content from app
    const event = new CustomEvent('desktop:content-request');
    window.dispatchEvent(event);

    // Timeout after 1 second
    setTimeout(() => {
      window.removeEventListener('desktop:content-response', handler);
      resolve(null);
    }, 1000);
  });
}

/**
 * Dispatch a viewport action event.
 */
function dispatchViewportEvent(action: 'zoom-in' | 'zoom-out' | 'fit'): void {
  const event = new CustomEvent('desktop:viewport', {
    detail: { action },
  });
  window.dispatchEvent(event);
}

/**
 * Handle View > Toggle Theme.
 */
function handleToggleTheme(): void {
  const event = new CustomEvent('desktop:toggle-theme');
  window.dispatchEvent(event);
}

/**
 * Handle Help > About.
 */
function handleAbout(): void {
  alert('CodeYourPCB\n\nVersion 0.1.0\n\nA git-friendly PCB design tool where the source file is the design.');
}
