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
export function hasFileSystemAccess(): boolean {
  return 'showOpenFilePicker' in window;
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
      input.accept = '.cypcb,.ses';

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
