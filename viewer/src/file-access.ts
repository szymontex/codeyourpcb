/**
 * File System Access API wrapper with fallback support
 *
 * Provides progressive enhancement for file operations:
 * - Chrome/Edge/Safari: Native File System Access API for save-in-place
 * - Firefox/older browsers: Traditional input element and download fallback
 *
 * Reference: https://developer.chrome.com/docs/capabilities/web-apis/file-system-access
 */

// TypeScript declarations for File System Access API
declare global {
  interface Window {
    showOpenFilePicker?: (options?: {
      types?: Array<{
        description?: string;
        accept: Record<string, string[]>;
      }>;
      multiple?: boolean;
    }) => Promise<FileSystemFileHandle[]>;
    showSaveFilePicker?: (options?: {
      suggestedName?: string;
      types?: Array<{
        description?: string;
        accept: Record<string, string[]>;
      }>;
    }) => Promise<FileSystemFileHandle>;
  }

  interface FileSystemFileHandle {
    getFile(): Promise<File>;
    createWritable(): Promise<FileSystemWritableFileStream>;
  }

  interface FileSystemWritableFileStream extends WritableStream {
    write(data: string | BufferSource | Blob): Promise<void>;
    close(): Promise<void>;
  }
}

/**
 * Result from opening a file
 */
export interface OpenFileResult {
  content: string;
  name: string;
  handle: FileSystemFileHandle | null;
}

/**
 * Check if File System Access API is available in this browser.
 */
function hasFileSystemAccess(): boolean {
  return 'showOpenFilePicker' in window;
}

type WrittenFormat = 'kicad_pcb' | 'cypcb';

/**
 * The format a text is in. A KiCad board is one `(kicad_pcb ...)` expression
 * and nothing after it: anything past its closing bracket is another format
 * spliced on, which KiCad's own reader would stop short of - the trace blocks
 * Ctrl+S used to append went exactly there, and the board reopened without
 * them and without a word.
 */
export function formatOfText(content: string): WrittenFormat | 'mixed' {
  const text = content.trimStart();
  if (!text.startsWith('(kicad_pcb')) return 'cypcb';
  let depth = 0;
  let quoted = false;
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (quoted) {
      if (c === '\\') i++;
      else if (c === '"') quoted = false;
    } else if (c === '"') {
      quoted = true;
    } else if (c === '(') {
      depth++;
    } else if (c === ')' && --depth === 0) {
      return text.slice(i + 1).trim() === '' ? 'kicad_pcb' : 'mixed';
    }
  }
  return 'mixed';
}

/** The format a file name promises. */
export function formatOfName(name: string): WrittenFormat {
  return name.toLowerCase().endsWith('.kicad_pcb') ? 'kicad_pcb' : 'cypcb';
}

/**
 * Refuse a write whose text is not in the format its file name promises.
 *
 * Every write of a design goes through here, in the browser and in the desktop
 * app, so no save path can put one format in a file named for another.
 */
export function refuseForeignFormat(name: string, content: string): void {
  const promised = formatOfName(name);
  const found = formatOfText(content);
  if (found === promised) return;
  const what = found === 'mixed'
    ? 'a KiCad board with other text after it'
    : found === 'kicad_pcb' ? 'a KiCad board' : 'a .cypcb design';
  throw new Error(`Not saved: ${name} would have been given ${what}`);
}

/** The name a KiCad board is saved under: its own, as a `.cypcb`. */
export function designNameFor(name: string): string {
  return formatOfName(name) === 'kicad_pcb' ? name.replace(/\.kicad_pcb$/i, '.cypcb') : name;
}

/**
 * Open a file using File System Access API with fallback.
 *
 * Returns file content, name, and handle (if API supported).
 * Handle can be used for save-in-place without showing save dialog.
 *
 * @returns Promise resolving to file content and handle, or null if cancelled
 */
export async function openFile(): Promise<OpenFileResult | null> {
  if (hasFileSystemAccess()) {
    // Modern API - Chrome, Edge, Safari
    try {
      const [handle] = await window.showOpenFilePicker!({
        types: [
          {
            description: 'PCB Design Files',
            accept: {
              'application/x-cypcb': ['.cypcb'],
              'application/x-specctra-ses': ['.ses'],
              'application/x-kicad-pcb': ['.kicad_pcb'],
            },
          },
        ],
        multiple: false,
      });

      const file = await handle.getFile();
      const content = await file.text();

      return {
        content,
        name: file.name,
        handle,
      };
    } catch (err) {
      // User cancelled or permission denied
      if ((err as Error).name === 'AbortError') {
        console.log('[FileAccess] User cancelled file open');
        return null;
      }
      console.error('[FileAccess] Error opening file:', err);
      throw err;
    }
  } else {
    // Fallback - Firefox, older browsers
    return new Promise((resolve) => {
      const input = document.createElement('input');
      input.type = 'file';
      input.accept = '.cypcb,.ses,.kicad_pcb';

      input.onchange = async () => {
        const file = input.files?.[0];
        if (!file) {
          resolve(null);
          return;
        }

        try {
          const content = await file.text();
          resolve({
            content,
            name: file.name,
            handle: null, // No handle in fallback mode
          });
        } catch (err) {
          console.error('[FileAccess] Error reading file:', err);
          resolve(null);
        }

        // Clean up
        input.remove();
      };

      input.oncancel = () => {
        console.log('[FileAccess] User cancelled file open');
        input.remove();
        resolve(null);
      };

      input.click();
    });
  }
}

/**
 * Save a file using File System Access API with fallback.
 *
 * If handle is provided and valid, saves directly without showing dialog.
 * If no handle, shows save picker (if API available) or triggers download.
 *
 * @param content - File content to save
 * @param handle - File handle from previous open (if available)
 * @param defaultName - Default filename for save-as or download
 * @returns Promise resolving to file handle (for next save), or null if cancelled/fallback
 */
export async function saveFile(
  content: string,
  handle: FileSystemFileHandle | null,
  defaultName: string
): Promise<FileSystemFileHandle | null> {
  if (handle) {
    // Outside the try: a refusal is an answer, not a failure to fall through
    // to save-as on.
    refuseForeignFormat(handle.name, content);
    // Save to existing handle - no dialog
    try {
      const writable = await handle.createWritable();
      await writable.write(content);
      await writable.close();
      console.log('[FileAccess] File saved successfully');
      return handle; // Return same handle for next save
    } catch (err) {
      console.error('[FileAccess] Error saving file:', err);
      // If handle save fails, fall through to save-as
      handle = null;
    }
  }

  // No handle - need to get one
  if (hasFileSystemAccess() && !handle) {
    // Show save picker
    try {
      const newHandle = await window.showSaveFilePicker!({
        suggestedName: defaultName,
        types: [
          {
            description: 'PCB Design Files',
            accept: {
              'application/x-cypcb': ['.cypcb'],
            },
          },
        ],
      });

      refuseForeignFormat(newHandle.name, content);
      const writable = await newHandle.createWritable();
      await writable.write(content);
      await writable.close();
      console.log('[FileAccess] File saved as:', defaultName);

      return newHandle; // Return new handle for future saves
    } catch (err) {
      // User cancelled or permission denied
      if ((err as Error).name === 'AbortError') {
        console.log('[FileAccess] User cancelled save');
        return null;
      }
      console.error('[FileAccess] Error saving file:', err);
      throw err;
    }
  } else {
    // Fallback - trigger download
    refuseForeignFormat(defaultName, content);
    const blob = new Blob([content], { type: 'application/x-cypcb' });
    const url = URL.createObjectURL(blob);

    const a = document.createElement('a');
    a.href = url;
    a.download = defaultName;
    a.click();

    // Clean up
    setTimeout(() => {
      URL.revokeObjectURL(url);
      a.remove();
    }, 100);

    console.log('[FileAccess] File downloaded:', defaultName);
    return null; // No handle in fallback mode
  }
}
