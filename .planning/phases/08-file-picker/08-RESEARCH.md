# Phase 8: File Picker - Research

**Researched:** 2026-01-22
**Domain:** Browser File APIs (File API, Drag-and-Drop, FileReader)
**Confidence:** HIGH

## Summary

Phase 8 implements a client-side file picker for loading `.cypcb` and `.ses` files directly in the viewer. The implementation uses standard browser APIs that work without a backend server - specifically the File API with `<input type="file">` for the file picker dialog and the HTML Drag and Drop API for drag-and-drop support. Both approaches use `FileReader.readAsText()` to read file contents.

The existing viewer architecture (vanilla TypeScript, Vite, Canvas 2D) already has the infrastructure for loading and parsing `.cypcb` files via `parseSource()` in `wasm.ts` and `.ses` files via `parseSesFile()`. The file picker integration needs to bridge user file selection to these existing parsing functions.

**Primary recommendation:** Use a hidden `<input type="file">` with a styled button trigger for the file picker, combined with drag-and-drop on the canvas container. Both methods feed into a unified `loadFile()` function that reads contents with FileReader and calls the existing engine methods.

## Standard Stack

The established libraries/tools for this domain:

### Core
| API | Browser Support | Purpose | Why Standard |
|-----|-----------------|---------|--------------|
| File API | 100% | Access files selected by user | Universal browser support |
| FileReader | 100% | Read file contents as text | Standard async file reading |
| HTMLInputElement.files | 100% | FileList from input element | Native file selection |
| DataTransfer | 100% | Access dropped files | Standard drag-drop API |
| DragEvent | 100% | Typed drag events | DOM standard |

### Supporting
| Pattern | Purpose | When to Use |
|---------|---------|-------------|
| URL.createObjectURL() | Preview files | When displaying images/previews |
| FileReader.readAsArrayBuffer() | Binary files | Not needed for .cypcb/.ses (text) |
| accept attribute | Filter file types | Limit picker to specific extensions |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| File API + input | showOpenFilePicker() | Only 29% browser support (no Firefox/Safari) |
| Custom drag zone | library (dropzone.js) | Overkill for simple case, adds dependency |
| Separate upload | WebSocket server | Requires backend, against requirements |

**Installation:**
No additional packages needed - all APIs are native browser APIs included in TypeScript's DOM types.

## Architecture Patterns

### Recommended Project Structure
```
viewer/src/
├── main.ts           # Add file picker UI integration
├── wasm.ts           # Existing parseSource(), parseSesFile()
├── types.ts          # Existing BoardSnapshot types
├── file-picker.ts    # NEW: File selection and reading utilities
├── interaction.ts    # Existing mouse handlers
└── renderer.ts       # Existing rendering
```

### Pattern 1: Hidden Input with Button Trigger
**What:** Hide the native file input, trigger it from a styled button
**When to use:** Custom button styling while maintaining accessibility
**Example:**
```typescript
// Source: MDN File API documentation
const fileInput = document.createElement('input');
fileInput.type = 'file';
fileInput.accept = '.cypcb,.ses';
fileInput.style.display = 'none';
document.body.appendChild(fileInput);

const openBtn = document.getElementById('open-btn') as HTMLButtonElement;
openBtn.addEventListener('click', () => fileInput.click());

fileInput.addEventListener('change', () => {
  const file = fileInput.files?.[0];
  if (file) {
    readFileAsText(file).then(content => loadContent(file.name, content));
  }
  // Reset input to allow re-selecting same file
  fileInput.value = '';
});
```

### Pattern 2: FileReader Promise Wrapper
**What:** Wrap callback-based FileReader in a Promise for async/await
**When to use:** Always - cleaner code flow
**Example:**
```typescript
// Source: web.dev read-files guide
function readFileAsText(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result as string);
    reader.onerror = () => reject(reader.error);
    reader.readAsText(file);
  });
}
```

### Pattern 3: Drag-Drop with Visual Feedback
**What:** Handle dragenter/dragover/dragleave/drop with CSS class toggle
**When to use:** Drop zone that gives visual feedback
**Example:**
```typescript
// Source: MDN File drag and drop
const dropZone = document.getElementById('canvas-container')!;

// Prevent default to enable drop
dropZone.addEventListener('dragover', (e: DragEvent) => {
  e.preventDefault();
  e.dataTransfer!.dropEffect = 'copy';
  dropZone.classList.add('drag-over');
});

dropZone.addEventListener('dragleave', () => {
  dropZone.classList.remove('drag-over');
});

dropZone.addEventListener('drop', (e: DragEvent) => {
  e.preventDefault();
  dropZone.classList.remove('drag-over');

  const file = e.dataTransfer?.files[0];
  if (file) {
    readFileAsText(file).then(content => loadContent(file.name, content));
  }
});

// Prevent window-level drop from opening file
window.addEventListener('dragover', (e) => e.preventDefault());
window.addEventListener('drop', (e) => e.preventDefault());
```

### Pattern 4: TypeScript Event Type Casting
**What:** Cast event.target to HTMLInputElement for files property
**When to use:** TypeScript file input change handlers
**Example:**
```typescript
// Source: TypeScript GitHub issues
fileInput.addEventListener('change', (event: Event) => {
  const target = event.target as HTMLInputElement;
  const file = target.files?.[0];
  // ...
});
```

### Anti-Patterns to Avoid
- **Using showOpenFilePicker():** Only 29% browser support, no Firefox/Safari
- **Forgetting e.preventDefault():** Drop won't work without preventing dragover default
- **Not resetting input value:** User can't re-select same file without `input.value = ''`
- **Not handling window drop:** Browser will navigate away if file dropped outside zone
- **Blocking main thread:** Always use async FileReader, never FileReaderSync outside workers

## Don't Hand-Roll

Problems that look simple but have existing solutions:

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| File reading | Custom binary parsing | FileReader.readAsText() | Handles encoding, async |
| File type filtering | Manual extension check | accept attribute | Native OS-level filtering |
| Drag-drop events | Custom mouse tracking | DataTransfer API | Works with OS file manager |
| .cypcb parsing | New parser | Existing parseSource() in wasm.ts | Already implemented |
| .ses parsing | New parser | Existing parseSesFile() in wasm.ts | Already implemented |

**Key insight:** The viewer already has complete parsing infrastructure. File picker only needs to bridge user selection to existing `engine.load_source()` and `engine.load_routes()` methods.

## Common Pitfalls

### Pitfall 1: Forgetting preventDefault on dragover
**What goes wrong:** Drop event never fires
**Why it happens:** Browser default is to not allow drop
**How to avoid:** Always call `e.preventDefault()` in dragover handler
**Warning signs:** Drop has no effect, file opens in new tab

### Pitfall 2: Browser Opens File Instead of Processing
**What goes wrong:** Dragging file opens it in browser tab
**Why it happens:** Window-level drop not prevented
**How to avoid:** Add window-level dragover/drop handlers with preventDefault
**Warning signs:** Navigation away from viewer

### Pitfall 3: Can't Re-Select Same File
**What goes wrong:** Selecting same file twice doesn't trigger change event
**Why it happens:** Input value unchanged, no change event
**How to avoid:** Reset `input.value = ''` after processing
**Warning signs:** Second open of same file does nothing

### Pitfall 4: TypeScript Files Property Error
**What goes wrong:** `event.target.files` shows type error
**Why it happens:** event.target is EventTarget, not HTMLInputElement
**How to avoid:** Cast: `(event.target as HTMLInputElement).files`
**Warning signs:** TypeScript compilation error

### Pitfall 5: Loading .ses Without .cypcb
**What goes wrong:** Routes render without board context
**Why it happens:** .ses file contains only routing, no board definition
**How to avoid:** Validate that board is loaded before applying routes, or parse .ses alongside matching .cypcb
**Warning signs:** Traces render at wrong scale/position

### Pitfall 6: Drag-Leave Fires on Children
**What goes wrong:** Highlight flickers when dragging over child elements
**Why it happens:** dragleave fires when entering child, even within drop zone
**How to avoid:** Use dragenter/dragleave counter, or check relatedTarget
**Warning signs:** Visual glitching during drag

## Code Examples

Verified patterns from official sources:

### File Input Setup (TypeScript)
```typescript
// Source: MDN Using files from web applications
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
    // Reset for re-selection
    input.value = '';
  });

  document.body.appendChild(input);
  return input;
}

// Usage
const picker = createFilePicker('.cypcb,.ses', async (file) => {
  const content = await readFileAsText(file);
  handleFile(file.name, content);
});

openButton.addEventListener('click', () => picker.click());
```

### Drop Zone Setup (TypeScript)
```typescript
// Source: MDN File drag and drop
export function setupDropZone(
  element: HTMLElement,
  onDrop: (file: File) => void
): void {
  let dragCounter = 0; // Handle child element events

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
}

// Prevent browser from handling dropped files
window.addEventListener('dragover', (e) => e.preventDefault());
window.addEventListener('drop', (e) => e.preventDefault());
```

### Unified Load Handler
```typescript
// Integration with existing viewer architecture
async function handleFileLoad(
  filename: string,
  content: string,
  engine: PcbEngine
): { success: boolean; error?: string } {
  const ext = filename.toLowerCase().split('.').pop();

  if (ext === 'cypcb') {
    const errors = engine.load_source(content);
    return { success: true, error: errors || undefined };
  }

  if (ext === 'ses') {
    // .ses requires a board to be loaded first
    const snapshot = engine.get_snapshot();
    if (!snapshot.board) {
      return {
        success: false,
        error: 'Load a .cypcb file before loading routes'
      };
    }
    engine.load_routes(content);
    return { success: true };
  }

  return { success: false, error: `Unknown file type: ${ext}` };
}
```

### CSS for Drag Feedback
```css
/* Visual feedback when dragging over drop zone */
#canvas-container.drag-over {
  outline: 3px dashed #28a745;
  outline-offset: -3px;
  background: rgba(40, 167, 69, 0.05);
}

#canvas-container.drag-over::after {
  content: 'Drop .cypcb or .ses file';
  position: absolute;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  font-size: 18px;
  color: #28a745;
  pointer-events: none;
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Flash-based uploaders | Native File API | 2010+ | No plugins needed |
| XMLHttpRequest upload | File API + fetch | 2015+ | Simpler async |
| input type=file only | File System Access API | 2020+ | Direct FS access (Chrome only) |

**Deprecated/outdated:**
- **Flash uploaders:** Dead, no browser support
- **showOpenFilePicker() for cross-browser:** Only 29% support, not viable for general web apps
- **FileReaderSync:** Only in Web Workers, blocks thread

## Integration Points with Existing Code

### wasm.ts (Existing)
- `parseSource(source: string)`: Parses .cypcb content, returns BoardSnapshot
- `parseSesFile(sesContent: string)`: Parses .ses content, returns traces/vias
- `PcbEngine.load_source(source: string)`: Loads and parses .cypcb
- `PcbEngine.load_routes(sesContent: string)`: Loads routing data

### main.ts (Modifications Needed)
- Add "Open" button to toolbar
- Wire up file input and drop zone
- Call existing `engine.load_source()` and `engine.load_routes()`
- Update status text on file load
- Handle errors gracefully

### index.html (Modifications Needed)
- Add "Open" button in toolbar
- Add CSS for drag-over state

### Sequence: Load .cypcb
1. User clicks Open button OR drags file
2. File API provides File object
3. FileReader reads as text
4. Content passed to `engine.load_source()`
5. `engine.get_snapshot()` for new board
6. Viewport fitted to new board
7. Re-render

### Sequence: Load .ses (with .cypcb)
1. User loads .cypcb first (establishes board)
2. User loads .ses file
3. Content passed to `engine.load_routes()`
4. Traces/vias added to snapshot
5. Re-render shows routes

### Sequence: Auto-detect Adjacent .ses
1. User loads `foo.cypcb`
2. Check if `foo.ses` exists (not possible client-side without user action)
3. **Note:** Browser security prevents scanning filesystem
4. User must explicitly load .ses file OR use a file picker that selects both

## Open Questions

Things that couldn't be fully resolved:

1. **Multiple file selection for .cypcb + .ses pair**
   - What we know: `multiple` attribute allows selecting multiple files
   - What's unclear: Best UX for selecting both files at once vs sequentially
   - Recommendation: Allow multiple selection, detect types and process in order

2. **File path display**
   - What we know: Browser only exposes filename, not full path (security)
   - What's unclear: How much path context users expect
   - Recommendation: Show filename only, consistent with browser security model

## Sources

### Primary (HIGH confidence)
- [MDN FileReader API](https://developer.mozilla.org/en-US/docs/Web/API/FileReader) - Core file reading
- [MDN File drag and drop](https://developer.mozilla.org/en-US/docs/Web/API/HTML_Drag_and_Drop_API/File_drag_and_drop) - Drop implementation
- [MDN Using files from web applications](https://developer.mozilla.org/en-US/docs/Web/API/File_API/Using_files_from_web_applications) - Input element patterns
- [MDN DataTransfer](https://developer.mozilla.org/en-US/docs/Web/API/DataTransfer) - Drag data access
- [MDN DragEvent](https://developer.mozilla.org/en-US/docs/Web/API/DragEvent) - TypeScript types

### Secondary (MEDIUM confidence)
- [web.dev Read files](https://web.dev/read-files/) - Best practices
- [Can I Use showOpenFilePicker](https://caniuse.com/mdn-api_window_showopenfilepicker) - Browser support data

### Tertiary (LOW confidence)
- [TypeScript GitHub #31816](https://github.com/microsoft/TypeScript/issues/31816) - Files type issue

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - Browser APIs are stable and well-documented
- Architecture: HIGH - Patterns from MDN official documentation
- Pitfalls: HIGH - Well-known issues documented across multiple sources

**Research date:** 2026-01-22
**Valid until:** 6+ months (Browser File API is stable, rarely changes)
