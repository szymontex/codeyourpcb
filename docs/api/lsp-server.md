# LSP Server and WASM Bridge

CodeYourPCB provides LSP-like features for the `.cypcb` language through a WASM bridge architecture. This document explains how the editor integration works and what features are available.

## Overview

The LSP bridge provides editor features without requiring a separate Language Server Protocol server process. Instead, the WASM engine acts as the source of diagnostics, completions, and hover information.

### Architecture

```
┌─────────────────┐
│ Monaco Editor   │  ← User types code
└────────┬────────┘
         │
         │ 300ms debounced sync
         ▼
┌─────────────────┐
│ WASM Engine     │  ← Parse + DRC
│ (cypcb-parser)  │
│ (cypcb-drc)     │
└────────┬────────┘
         │
         │ Diagnostics, Completions, Hover
         ▼
┌─────────────────┐
│ LSP Bridge      │  ← Convert to Monaco API
│ (lsp-bridge.ts) │
└────────┬────────┘
         │
         │ Monaco markers, completion items
         ▼
┌─────────────────┐
│ Monaco Editor   │  ← Display errors, suggestions
└─────────────────┘
```

### Why WASM Instead of WebSocket LSP?

1. **No backend required:** Works on static hosting (Cloudflare Pages, GitHub Pages)
2. **Faster response:** Direct WASM calls vs WebSocket round-trip
3. **Simpler lifecycle:** No server process to start/stop/reconnect
4. **Unified codebase:** Same WASM engine for desktop and web

**Future upgrade path:** Desktop app could add stdio LSP sidecar for advanced features (goto-definition, find-references) while web continues using WASM bridge.

## Features

The LSP bridge provides three main features:

### 1. Diagnostics (Inline Errors)

Parse errors and DRC violations appear as inline markers in the editor.

#### Parse Errors (Red Squiggly Underlines)

Syntax errors detected by `cypcb-parser`:

- **Severity:** `MarkerSeverity.Error` (red)
- **Source:** `cypcb-parser`
- **Error codes:**
  - `syntax` - Invalid syntax
  - `unknown-component` - Unknown component type
  - `unknown-layer` - Unknown layer name
  - `unknown-unit` - Unknown unit (mm, mil, etc.)
  - `invalid-number` - Number parsing failed
  - `missing` - Missing required token
  - `invalid-version` - Invalid file format version
  - `invalid-layers` - Invalid layer count

**Example:**
```cypcb
component R1 unknown_type "0805" {
//           ^^^^^^^^^^^^
// Error: Unknown component type: 'unknown_type'
```

#### DRC Violations (Yellow Warnings)

Design rule violations detected by `cypcb-drc`:

- **Severity:** `MarkerSeverity.Warning` (yellow)
- **Source:** `cypcb-drc`
- **Violation types:**
  - `UnconnectedPin` - Component pin not connected to any net
  - `Clearance` - Components too close together
  - `TraceWidth` - Trace width violates minimum width rule
  - `DrillSize` - Via/pad drill size violates minimum
  - `EdgeClearance` - Component too close to board edge

**Example:**
```cypcb
component C1 capacitor "0805" {
    at 10mm, 10mm
}
// Warning: Pin C1.1 is not connected to any net
// Warning: Pin C1.2 is not connected to any net
```

#### Diagnostic Limits

- Maximum 100 diagnostics per file (prevents editor slowdown)
- Overflow message: `... and N more diagnostics (truncated)`

### 2. Auto-Completion

Context-aware suggestions as you type.

#### Trigger Behavior

- **Automatic:** Triggered on alphanumeric input
- **Manual:** Ctrl+Space forces completion menu
- **Context-sensitive:** Different suggestions based on cursor position

#### Completion Categories

**Keywords** (CompletionItemKind.Keyword)
```
version, board, component, net, footprint, trace, zone, keepout
```

**Component Types** (CompletionItemKind.Class)
```
resistor, capacitor, ic, connector, diode, transistor, led, crystal, inductor, generic
```

**Properties** (CompletionItemKind.Property)
```
size, layers, value, at, rotate, pin, width, clearance, current, from, to, via, layer,
locked, bounds, stackup, description, pad, courtyard
```

**Layers** (CompletionItemKind.Enum)
```
Top, Bottom, Inner1, Inner2, Inner3, Inner4, all
```

**Units** (CompletionItemKind.Unit)
```
mm, mil, mA, A, V, k, M, u, n, p
```

#### Context Detection

The completion provider analyzes the line content to provide relevant suggestions:

- **After a number:** Suggests units (mm, mil, mA, etc.)
- **After "component RefDes":** Suggests component types (resistor, capacitor, etc.)
- **After "layer":** Suggests layer names (Top, Bottom, Inner1, etc.)
- **General context:** Suggests keywords and properties

**Example:**
```cypcb
component R1 |        ← Suggests: resistor, capacitor, ic, ...
at 10|                ← Suggests: mm, mil
layer |               ← Suggests: Top, Bottom, Inner1, ...
```

### 3. Hover Documentation

Tooltips with keyword documentation when hovering over text.

#### Coverage

Hover documentation is available for:
- All keywords (version, board, component, net, footprint, trace, zone, keepout)
- All component types (resistor, capacitor, ic, etc.)
- All properties (size, layers, value, at, rotate, etc.)
- All layer names (Top, Bottom, Inner1-4)

**Example hover content:**

```
**component**
Places a component on the board. Supported types: resistor, capacitor, ic,
connector, diode, transistor, led, crystal, inductor, generic.
```

```
**resistor**
Passive component type - resistor. Specify value in ohms (e.g., "330" or "10k").
```

```
**at**
Component position on the board. Format: "at <x>, <y> [rotate <angle>]"
(e.g., "at 10mm, 20mm rotate 90").
```

## Integration Details

### Monaco Editor Sync

The LSP bridge integrates with Monaco editor through several mechanisms:

#### Debounced Content Sync (300ms)

When the user types in the editor:

1. Editor content changes
2. 300ms debounce timer starts
3. If no further changes, sync triggered
4. Content sent to WASM engine
5. Engine parses and runs DRC
6. Diagnostics returned to LSP bridge
7. LSP bridge updates Monaco markers

**Why 300ms?** Balances responsiveness with performance. Typing doesn't feel laggy, but parsing doesn't run on every keystroke.

#### Suppress-Sync Flag

Prevents circular updates when programmatically setting editor content:

```typescript
// When loading a file
suppressSync = true;
editor.setValue(content);
suppressSync = false;
```

Without this flag, `setValue()` would trigger the sync handler, causing unnecessary parsing.

### Provider Registration

Providers are registered once when Monaco loads the `.cypcb` language:

```typescript
import { registerProviders } from './lsp-bridge';

// After Monaco is loaded and .cypcb language is registered
registerProviders(monaco);
```

This registers:
- Completion provider: `monaco.languages.registerCompletionItemProvider`
- Hover provider: `monaco.languages.registerHoverProvider`

Diagnostics are updated manually via `updateDiagnostics()` after each parse.

## Desktop vs Web

Both desktop (Tauri) and web (browser) use the **same WASM bridge implementation**. There is no separate LSP server.

### Current State (WASM Bridge)

| Feature | Desktop | Web |
|---------|---------|-----|
| Diagnostics | ✓ WASM | ✓ WASM |
| Completion | ✓ WASM | ✓ WASM |
| Hover | ✓ WASM | ✓ WASM |

### Future Enhancement (Desktop Stdio LSP)

The desktop app could optionally run a stdio LSP sidecar process for advanced features:

| Feature | Desktop (Future) | Web |
|---------|------------------|-----|
| Goto Definition | ✓ stdio LSP | ✗ N/A |
| Find References | ✓ stdio LSP | ✗ N/A |
| Rename Symbol | ✓ stdio LSP | ✗ N/A |
| Call Hierarchy | ✓ stdio LSP | ✗ N/A |

**Why not now?** These features require AST traversal and symbol tables not currently tracked by the parser. WASM bridge provides 80% of value with 20% of complexity.

## Usage Examples

### Viewing Diagnostics

Open a `.cypcb` file with errors:

```cypcb
version 1

board test {
    size 50mm x 30mm
    layers 2
}

component R1 unknown_type "0805" {
    at 10mm, 10mm
}
```

**Expected markers:**
1. Parse error on line 8: `Unknown component type: 'unknown_type'`
2. DRC warnings on line 8-9: `Pin R1.1 is not connected`, `Pin R1.2 is not connected`

### Using Auto-Completion

Type in the editor:

1. Type `comp` → Suggestions: `component`
2. Press Tab → Autocomplete `component`
3. Type `R1 res` → Suggestions: `resistor`
4. Complete to `component R1 resistor "0805" {`
5. Type `at 10` → Suggestions: `mm`, `mil`
6. Complete to `at 10mm, 10mm`

### Using Hover Documentation

Hover over any keyword to see its documentation:

- Hover over `component` → See full documentation
- Hover over `resistor` → See component type explanation
- Hover over `at` → See position format specification

## Performance Characteristics

### Parse Performance

- **Small files (<100 lines):** <10ms parse time
- **Medium files (100-500 lines):** 10-50ms parse time
- **Large files (>500 lines):** 50-200ms parse time

**300ms debounce** ensures parsing never blocks typing, even on large files.

### DRC Performance

- **Simple boards (<10 components):** <5ms DRC time
- **Medium boards (10-50 components):** 5-20ms DRC time
- **Complex boards (>50 components):** 20-100ms DRC time

DRC runs after parsing in the same sync cycle.

### Memory Usage

- WASM engine: ~2-5MB heap
- Monaco editor: ~10-15MB (includes editor, themes, language features)
- Total overhead: ~15-20MB

**Lazy loading:** Monaco editor is loaded on-demand (first toggle), so initial page load doesn't include this overhead.

## API Reference

### `updateDiagnostics(monaco, editor, parseErrors, violations)`

Updates Monaco editor markers from WASM engine diagnostics.

**Parameters:**
- `monaco` - Monaco editor module
- `editor` - Monaco editor instance
- `parseErrors` - Parse error string from `engine.load_source()` (newline-separated)
- `violations` - DRC violations from `snapshot.violations`

**Behavior:**
- Clears existing markers
- Converts parse errors to Error markers (red)
- Converts DRC violations to Warning markers (yellow)
- Sets markers on model with owner `'cypcb'`

### `registerCompletionProvider(monaco)`

Registers auto-completion provider for `.cypcb` language.

**Provides:**
- Keywords, component types, properties, layers, units
- Context-aware filtering based on cursor position
- Detail and documentation for each item

### `registerHoverProvider(monaco)`

Registers hover provider for `.cypcb` language.

**Provides:**
- Documentation tooltips for all keywords
- Markdown-formatted content

### `registerProviders(monaco)`

Convenience function to register all providers.

**Calls:**
- `registerCompletionProvider(monaco)`
- `registerHoverProvider(monaco)`

## Related Documentation

- **Editor Integration:** See Phase 14 documentation for Monaco setup
- **Parser API:** See `crates/cypcb-parser/src/lib.rs`
- **DRC Engine:** See `crates/cypcb-drc/src/lib.rs`
- **WASM Bridge Source:** `viewer/src/editor/lsp-bridge.ts`

## Troubleshooting

### Diagnostics Not Updating

**Symptom:** Errors don't appear after typing invalid syntax.

**Check:**
1. Is editor visible? Diagnostics only sync when editor is shown.
2. Is debounce timer firing? Wait 300ms after typing stops.
3. Are diagnostics being returned? Check browser console for errors.

### Completion Not Working

**Symptom:** Ctrl+Space shows no suggestions.

**Check:**
1. Is `.cypcb` language registered? Check Monaco language registry.
2. Are providers registered? Check console for "LSP Bridge" log message.
3. Is cursor in valid context? Some contexts have limited suggestions.

### Hover Not Showing

**Symptom:** Hovering over keywords shows no tooltip.

**Check:**
1. Is hover provider registered?
2. Is word in keyword documentation map?
3. Is Monaco's hover feature enabled globally?

## Future Enhancements

Potential improvements to the LSP bridge:

1. **Symbol-aware completions:** Suggest existing component RefDes and net names
2. **Signature help:** Show function-like syntax for `trace`, `zone` constructs
3. **Code actions:** Quick fixes for common errors (e.g., "Add missing net assignment")
4. **Semantic highlighting:** Color code different token types beyond syntax highlighting
5. **Folding regions:** Collapse component/net blocks
6. **Document symbols:** Outline view showing all components/nets
7. **Desktop stdio LSP:** Goto-definition, find-references for advanced navigation
