# Phase 14: Monaco Editor Integration - Research

**Researched:** 2026-01-30
**Domain:** Monaco Editor, LSP client integration, Monarch syntax highlighting
**Confidence:** MEDIUM

## Summary

Phase 14 embeds a Monaco editor for .cypcb files alongside the existing PCB board viewer. The codebase already has significant infrastructure prepared: Phase 11 created `monaco-theme.ts` with light/dark theme definitions and an `applyMonacoTheme()` function ready to be called once Monaco loads; Phase 7 built a complete tower-lsp server with hover, completion, goto-definition, and diagnostics; and the viewer is a vanilla TypeScript Vite app with no framework.

There are two viable approaches for LSP integration, each with significant tradeoffs. The heavyweight approach uses `monaco-languageclient` v10 (which depends on `monaco-vscode-api`, a modularized VS Code Web), providing full LSP protocol handling but at the cost of massive bundle size (potentially 10MB+). The lightweight approach implements a custom LSP-to-Monaco bridge using raw WebSocket JSON-RPC, manually registering Monaco providers for completion, diagnostics, hover, and goto-definition. Given the project's strong emphasis on bundle size (WASM was optimized from 374KB to 264KB), the lightweight custom bridge is the recommended approach.

**Primary recommendation:** Use plain `monaco-editor` with `vite-plugin-monaco-editor` for bundle optimization, Monarch tokenizer for syntax highlighting, and a hand-rolled LSP client that bridges WebSocket JSON-RPC to Monaco's provider APIs. This avoids the `monaco-vscode-api` dependency entirely.

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| monaco-editor | 0.55.x | Code editor | The only serious browser-based code editor; same team as VS Code |
| vite-plugin-monaco-editor | 1.1.x | Vite integration | Handles worker bundling, allows selective language/feature inclusion |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| vscode-ws-jsonrpc | latest | WebSocket JSON-RPC framing | Only if using monaco-languageclient approach |
| monaco-languageclient | 10.6.x | Full LSP client for Monaco | Only if bundle size is acceptable (~10MB+) |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| monaco-languageclient | Custom LSP bridge | More code (~300-500 lines) but saves ~8MB bundle, no monaco-vscode-api dependency |
| vite-plugin-monaco-editor | Manual worker config | More Vite config but full control over what's bundled |
| Monarch tokenizer | TextMate grammar | TextMate requires monaco-vscode-api; Monarch is built-in and sufficient for .cypcb |

**Installation (recommended lightweight approach):**
```bash
npm install monaco-editor
npm install -D vite-plugin-monaco-editor
```

**Installation (heavyweight approach):**
```bash
npm install monaco-editor monaco-languageclient vscode-ws-jsonrpc @codingame/monaco-vscode-api
```

## Architecture Patterns

### Recommended Project Structure
```
viewer/src/
  editor/
    cypcb-language.ts     # Monarch tokenizer + language config
    lsp-client.ts         # WebSocket JSON-RPC LSP client
    lsp-bridge.ts         # Bridges LSP responses to Monaco providers
    editor-panel.ts       # Monaco editor instance management
  theme/
    monaco-theme.ts       # Already exists (Phase 11)
    theme-manager.ts      # Already exists (Phase 11)
  main.ts                 # Add editor initialization + split layout
```

### Pattern 1: Monarch Tokenizer for .cypcb
**What:** Define syntax highlighting using Monaco's built-in Monarch system
**When to use:** Custom languages that don't need TextMate grammars

```typescript
// viewer/src/editor/cypcb-language.ts
export const cypcbLanguage: monaco.languages.IMonarchLanguage = {
  keywords: ['version', 'board', 'component', 'net', 'footprint', 'trace',
             'zone', 'keepout', 'resistor', 'capacitor', 'ic', 'connector',
             'diode', 'transistor', 'led', 'crystal', 'inductor', 'generic'],
  properties: ['size', 'layers', 'value', 'at', 'rotate', 'pin', 'width',
               'clearance', 'current', 'from', 'to', 'via', 'layer', 'locked',
               'bounds', 'stackup', 'description', 'pad', 'courtyard'],
  layerNames: ['Top', 'Bottom', 'Inner1', 'Inner2', 'Inner3', 'Inner4', 'all'],

  tokenizer: {
    root: [
      // Comments
      [/\/\/.*$/, 'comment'],

      // Strings
      [/"[^"]*"/, 'string'],

      // Numbers with units
      [/\d+(\.\d+)?(mm|mil|mA|A|V|k|M|u|n|p|%)/, 'number'],
      [/\d+(\.\d+)?/, 'number'],

      // Keywords
      [/[a-zA-Z_]\w*/, {
        cases: {
          '@keywords': 'keyword',
          '@properties': 'type',
          '@layerNames': 'type.identifier',
          '@default': 'identifier'
        }
      }],

      // Pin references (R1.1)
      [/[A-Z][A-Z0-9]*\.\d+/, 'variable'],

      // Operators
      [/[{}()=,x]/, 'delimiter'],
    ]
  }
};

export const cypcbLanguageConfig: monaco.languages.LanguageConfiguration = {
  comments: { lineComment: '//' },
  brackets: [['{', '}']],
  autoClosingPairs: [
    { open: '{', close: '}' },
    { open: '"', close: '"' },
  ],
  surroundingPairs: [
    { open: '{', close: '}' },
    { open: '"', close: '"' },
  ],
  folding: {
    markers: {
      start: /\{/,
      end: /\}/,
    }
  },
};
```

### Pattern 2: Custom LSP Client via WebSocket
**What:** Lightweight JSON-RPC client that bridges LSP to Monaco providers
**When to use:** When bundle size matters and you only need a few LSP features

```typescript
// Simplified LSP client pattern
class CypcbLspClient {
  private ws: WebSocket;
  private requestId = 0;
  private pending = new Map<number, { resolve: Function; reject: Function }>();

  async initialize(): Promise<void> {
    await this.request('initialize', {
      capabilities: { textDocument: { completion: {}, hover: {}, publishDiagnostics: {} } }
    });
    this.notify('initialized', {});
  }

  // Register Monaco providers that call through to LSP
  registerProviders(monaco: typeof import('monaco-editor')): void {
    monaco.languages.registerCompletionItemProvider('cypcb', {
      provideCompletionItems: async (model, position) => {
        const result = await this.request('textDocument/completion', {
          textDocument: { uri: model.uri.toString() },
          position: { line: position.lineNumber - 1, character: position.column - 1 }
        });
        return convertCompletions(result); // LSP -> Monaco format
      }
    });
    // Similar for hover, diagnostics (push-based), goto-definition
  }
}
```

### Pattern 3: Split Layout (Editor + Board Viewer)
**What:** Side-by-side editor and canvas with draggable divider
**When to use:** EDIT-10 requirement

```
+--toolbar-----------------------------------------+
| [Open] [Save] ... [Layers] [Theme] [Route]       |
+--editor-panel--+--divider--+--canvas-container----+
|                |  |  |     |                      |
| Monaco Editor  |  |  |     | PCB Canvas           |
|                |  |  |     |                      |
|                |  |  |     |                      |
+----------------+--+--+----+----------------------+
| status bar                                        |
+---------------------------------------------------+
```

### Anti-Patterns to Avoid
- **Importing monaco-vscode-api for "full LSP support":** Pulls in a modularized VS Code Web, adding 8-15MB to bundle. The cypcb LSP only uses 4 features (hover, completion, diagnostics, goto-definition) which are straightforward to bridge manually.
- **Loading all Monaco languages:** By default Monaco includes ~80 languages. Configure the vite plugin to include zero built-in languages since we only use our custom .cypcb language.
- **Synchronous Monaco loading:** Monaco is large (~4MB minified). Always lazy-load it with dynamic `import()` so the board viewer renders immediately.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Syntax highlighting | Custom lexer | Monarch tokenizer | Monaco's built-in system, handles incremental re-tokenization efficiently |
| Code editor features | Custom textarea with overlays | Monaco editor | Find/replace, undo/redo, multi-cursor, line numbers, code folding all built-in |
| Worker bundling for Vite | Manual worker config | vite-plugin-monaco-editor | Handles esbuild bundling of Monaco workers in node_modules/.monaco |
| Theme integration | Custom CSS injection | applyMonacoTheme() (Phase 11) | Already built, wired to ThemeManager subscribe() |
| JSON-RPC framing | Custom protocol | LSP spec headers (Content-Length) | Well-defined protocol, straightforward to implement |

**Key insight:** Monaco gives you EDIT-04 through EDIT-08 for free (line numbers, code folding, find/replace, undo/redo, multi-cursor). The real work is EDIT-01 (syntax highlighting via Monarch), EDIT-02/03/09 (LSP integration), and EDIT-10 (layout).

## Common Pitfalls

### Pitfall 1: Monaco Bundle Size Explosion
**What goes wrong:** Bundle jumps from ~500KB to 5-15MB
**Why it happens:** Monaco includes all 80+ languages and their workers by default; monaco-languageclient pulls in monaco-vscode-api
**How to avoid:** Configure vite-plugin-monaco-editor with `languageWorkers: ['editorWorkerService']` only (no typescript/json/html/css workers). Set `languages: []` or only include custom language. Lazy-load Monaco with dynamic import.
**Warning signs:** Build output shows >2MB JS chunks; initial page load >3 seconds

### Pitfall 2: Worker Loading Failures in Production
**What goes wrong:** Monaco workers fail to load after deployment, editor shows no syntax highlighting
**Why it happens:** Worker URLs are incorrect after Vite build (hash-based filenames, wrong base path)
**How to avoid:** Use vite-plugin-monaco-editor which handles worker bundling, or configure `MonacoEnvironment.getWorkerUrl()` explicitly. Test production builds locally with `vite preview`.
**Warning signs:** Console errors about worker loading in production but not in dev

### Pitfall 3: LSP Position Off-by-One
**What goes wrong:** Completions/hover appear at wrong position, diagnostics highlight wrong text
**Why it happens:** Monaco positions are 1-based (line 1, column 1); LSP positions are 0-based (line 0, character 0)
**How to avoid:** Always convert: `lspLine = monacoLineNumber - 1`, `lspChar = monacoColumn - 1` and vice versa
**Warning signs:** Features work but are consistently shifted by one line or column

### Pitfall 4: WebSocket Transport for tower-lsp
**What goes wrong:** Cannot connect Monaco LSP client to tower-lsp server
**Why it happens:** tower-lsp defaults to stdio transport; browser needs WebSocket
**How to avoid:** Two options: (a) Desktop mode: spawn tower-lsp as stdio subprocess via Tauri sidecar, proxy through Tauri IPC or local WebSocket; (b) Web mode: add WebSocket transport to tower-lsp using the official `examples/websocket.rs` pattern with `ws_stream_tungstenite`
**Warning signs:** LSP server starts but no messages are exchanged

### Pitfall 5: Editor Content vs File System Desync
**What goes wrong:** Board viewer shows stale content, or editor shows content different from what was saved
**Why it happens:** Editor has its own model; board viewer uses engine.load_source() separately
**How to avoid:** Single source of truth: editor model is the authoritative content. On every model change (debounced), feed content to both LSP (textDocument/didChange) and WASM engine (load_source). On file open, set editor model content and let the change propagation handle the rest.
**Warning signs:** Board doesn't update when typing in editor, or vice versa

## Code Examples

### Monaco Editor Initialization (Lazy Loading)
```typescript
// viewer/src/editor/editor-panel.ts
export async function initEditor(container: HTMLElement): Promise<monaco.editor.IStandaloneCodeEditor> {
  // Lazy load Monaco to avoid blocking initial render
  const monaco = await import('monaco-editor');
  const { applyMonacoTheme } = await import('../theme/monaco-theme');
  const { cypcbLanguage, cypcbLanguageConfig } = await import('./cypcb-language');

  // Register custom language
  monaco.languages.register({ id: 'cypcb', extensions: ['.cypcb'] });
  monaco.languages.setMonarchTokensProvider('cypcb', cypcbLanguage);
  monaco.languages.setLanguageConfiguration('cypcb', cypcbLanguageConfig);

  // Apply theme (wires to ThemeManager)
  applyMonacoTheme(monaco);

  // Create editor
  const editor = monaco.editor.create(container, {
    language: 'cypcb',
    theme: 'cypcb-dark', // ThemeManager will override immediately
    automaticLayout: true, // Handles resize
    minimap: { enabled: false }, // Save space for side-by-side
    fontSize: 14,
    lineNumbers: 'on',        // EDIT-04
    folding: true,             // EDIT-05
    wordWrap: 'off',
    scrollBeyondLastLine: false,
  });

  return editor;
}
```

### Vite Config for Monaco
```typescript
// vite.config.ts addition
import monacoEditorPlugin from 'vite-plugin-monaco-editor';

export default defineConfig({
  plugins: [
    wasm(),
    topLevelAwait(),
    monacoEditorPlugin({
      languageWorkers: ['editorWorkerService'], // Only the base worker
      customWorkers: [], // No language-specific workers needed
    }),
  ],
  build: {
    rollupOptions: {
      output: {
        manualChunks: {
          vendor: ['./src/wasm.ts'],
          monaco: ['monaco-editor'], // Separate chunk for lazy loading
        },
      },
    },
  },
});
```

### Split Layout HTML
```html
<!-- Replace single canvas-container with split layout -->
<div id="main-content" style="display: flex; flex: 1; overflow: hidden;">
  <div id="editor-container" style="width: 40%; min-width: 200px;">
    <!-- Monaco mounts here -->
  </div>
  <div id="divider" style="width: 4px; cursor: col-resize; background: var(--border-primary);"></div>
  <div id="canvas-container" style="flex: 1; position: relative; overflow: hidden;">
    <canvas id="pcb-canvas"></canvas>
    <!-- existing overlays -->
  </div>
</div>
```

### Editor-to-Board Sync
```typescript
// Debounced sync from editor to board viewer
let syncTimeout: number | null = null;

editor.onDidChangeModelContent(() => {
  if (syncTimeout) clearTimeout(syncTimeout);
  syncTimeout = window.setTimeout(() => {
    const content = editor.getValue();
    // Update board viewer
    engine.load_source(content);
    snapshot = engine.get_snapshot();
    dirty = true;
    // LSP gets notified automatically via didChange
  }, 300); // 300ms debounce
});
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| monaco-languageclient + custom VS Code API shim | monaco-languageclient v10 + monaco-vscode-api | 2022-2025 | Full VS Code compatibility but massive bundle |
| Manual worker configuration | vite-plugin-monaco-editor | 2022 | Simplified worker bundling for Vite |
| TextMate grammars for custom languages | Monarch (still recommended for simple languages) | N/A (both available) | Monarch is lighter, TextMate needs oniguruma WASM |

**Deprecated/outdated:**
- `monaco-editor-wrapper`: Dropped in v10, functionality merged into `monaco-languageclient`
- `@codingame/monaco-vscode-api` before v2: Re-implemented VS Code API manually

## Open Questions

1. **LSP Transport Architecture**
   - What we know: tower-lsp supports stdio (default) and WebSocket (via example). Desktop can use Tauri sidecar for stdio. Web needs WebSocket.
   - What's unclear: Should the LSP server run as a separate process (desktop sidecar / web backend service), or should we compile LSP logic to WASM and run it in-browser? The WASM approach avoids network but the LSP crate depends on tokio and tower which may not compile to WASM easily.
   - Recommendation: Desktop uses Tauri sidecar (spawn cypcb-lsp as child process, communicate via stdio piped through IPC). Web mode starts without LSP or uses a lightweight in-browser adapter that reuses the parser/diagnostics directly from the existing WASM module (already has load_source + get_snapshot with violations). Full LSP over WebSocket deferred to when a backend server exists.

2. **Editor Toggle vs Always-On**
   - What we know: EDIT-10 says side-by-side. But the current app is viewer-only.
   - What's unclear: Should the editor always be visible, or toggled with a button/shortcut? Mobile/tablet may not have room.
   - Recommendation: Toggle-able editor panel. Default to hidden on narrow viewports (<768px), shown on desktop. Keyboard shortcut (Ctrl+E) to toggle.

3. **Bundle Size Impact**
   - What we know: Monaco core is ~2-4MB minified. Current app total is likely <1MB.
   - What's unclear: Exact gzipped size with only editorWorkerService worker and no built-in languages.
   - Recommendation: Measure after initial integration. Target: <1MB gzipped for Monaco chunk (achievable with no built-in languages and minimal features). Lazy load so initial page load is unaffected.

## Sources

### Primary (HIGH confidence)
- Codebase analysis: `viewer/src/theme/monaco-theme.ts` - Phase 11 prepared Monaco theme integration
- Codebase analysis: `crates/cypcb-lsp/src/backend.rs` - tower-lsp server with hover, completion, diagnostics, goto-definition
- Codebase analysis: `crates/cypcb-lsp/src/main.rs` - LSP uses stdio transport
- Codebase analysis: `viewer/vite.config.ts` - Current Vite setup with WASM plugins
- Codebase analysis: `viewer/src/main.ts` - Current app layout (toolbar + single canvas)

### Secondary (MEDIUM confidence)
- [TypeFox monaco-languageclient v10 blog](https://www.typefox.io/blog/monaco-languageclient-v10/) - Architecture of v10, monaco-vscode-api dependency
- [Monaco Editor Monarch docs](https://microsoft.github.io/monaco-editor/monarch.html) - Monarch tokenizer specification
- [vite-plugin-monaco-editor](https://github.com/vdesjs/vite-plugin-monaco-editor) - Vite integration plugin
- [tower-lsp WebSocket example](https://github.com/ebkalderon/tower-lsp/blob/master/examples/websocket.rs) - WebSocket transport pattern
- [DEV Community LSP+Monaco guide](https://dev.to/__4f1641/so-you-want-to-set-up-a-monaco-editor-with-a-language-server-2cpn) - Manual LSP bridge approach
- [Custom Monaco language guide](https://medium.com/@zsh-eng/integrating-lsp-with-the-monaco-code-editor-b054e9b5421f) - Manual LSP integration without monaco-languageclient

### Tertiary (LOW confidence)
- Bundle size estimates (2-4MB for Monaco core) based on community reports, not measured in this project's context

## Metadata

**Confidence breakdown:**
- Standard stack: MEDIUM - Monaco itself is clear; LSP integration approach has two valid paths, recommend lightweight but needs validation
- Architecture: MEDIUM - Split layout pattern is standard; LSP transport architecture has open questions (desktop vs web, sidecar vs WASM)
- Pitfalls: HIGH - Bundle size, worker loading, position off-by-one are well-documented community issues
- Code examples: MEDIUM - Monarch tokenizer pattern is well-established; custom LSP bridge is less common but feasible

**Research date:** 2026-01-30
**Valid until:** 2026-03-01 (Monaco releases ~monthly, check for breaking changes)
