/**
 * File Picker Utilities
 *
 * Provides client-side file selection and reading for .cypcb and .ses files.
 * Uses standard browser File API, FileReader, and HTML Drag and Drop API.
 */

/**
 * Read a file as text using FileReader wrapped in a Promise.
 *
 * @param file - The File object to read
 * @returns Promise resolving to the file contents as a string
 */
export function readFileAsText(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result as string);
    reader.onerror = () => reject(reader.error);
    reader.readAsText(file);
  });
}

/**
 * Create a hidden file input element that can be triggered programmatically.
 *
 * @param accept - File type filter (e.g., ".cypcb,.ses")
 * @param onFile - Callback invoked when a file is selected
 * @returns The hidden input element (call .click() to open dialog)
 */
export function createFilePicker(
  accept: string,
  onFile: (file: File) => void
): HTMLInputElement {
  const input = document.createElement('input');
  input.type = 'file';
  input.accept = accept;
  input.style.display = 'none';

  input.addEventListener('change', () => {
    const file = input.files?.[0];
    if (file) {
      onFile(file);
    }
    // Reset to allow re-selecting the same file
    input.value = '';
  });

  document.body.appendChild(input);
  return input;
}

/**
 * Set up drag-and-drop on an element with visual feedback.
 *
 * Adds 'drag-over' CSS class when files are dragged over the element.
 * Also prevents window-level drop events from causing browser navigation.
 *
 * @param element - The drop zone element
 * @param onDrop - Callback invoked when a file is dropped
 */
export function setupDropZone(
  element: HTMLElement,
  onDrop: (file: File) => void
): void {
  // Counter handles child element drag events correctly
  let dragCounter = 0;

  element.addEventListener('dragenter', (e: DragEvent) => {
    e.preventDefault();
    dragCounter++;
    element.classList.add('drag-over');
  });

  element.addEventListener('dragleave', () => {
    dragCounter--;
    if (dragCounter === 0) {
      element.classList.remove('drag-over');
    }
  });

  element.addEventListener('dragover', (e: DragEvent) => {
    e.preventDefault();
    if (e.dataTransfer) {
      e.dataTransfer.dropEffect = 'copy';
    }
  });

  element.addEventListener('drop', (e: DragEvent) => {
    e.preventDefault();
    dragCounter = 0;
    element.classList.remove('drag-over');

    const file = e.dataTransfer?.files[0];
    if (file) {
      onDrop(file);
    }
  });

  // Prevent browser from navigating when files are dropped outside the drop zone
  window.addEventListener('dragover', (e) => e.preventDefault());
  window.addEventListener('drop', (e) => e.preventDefault());
}
