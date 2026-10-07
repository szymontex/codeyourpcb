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

The wait is `EDITOR_SYNC_DEBOUNCE_MS` in `viewer/src/main.ts`.

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

**Why not now?** These features require AST traversal and symbol tables not currently tracked by the parser.

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

### When the editor loads the text

The browser editor waits 300ms after the last keystroke, then loads the whole design into the engine: parse, import resolution, board model and the full DRC, in one call. The wait is `EDITOR_SYNC_DEBOUNCE_MS` in `viewer/src/main.ts`.

The stdio server `cypcb-lsp` does not wait. It parses the document and rebuilds the board on every change it receives (`did_change` in `crates/cypcb-lsp/src/backend.rs`).

### Parse and DRC time

Measured 2026-09-27 on the build machine: release build, quiet host (load average 2.6), 50 runs per board after one warm-up. Times are in milliseconds, median / max. Parse is the reader alone; whole load is what the editor pays after the wait.

The WASM columns are the engine the browser runs, measured the same day: the module in `viewer/pkg` (the `wasm-release` profile) in headless Chromium under Playwright, on the viewer's dev server, 20 runs per board after one warm-up. The WASM engine has no call that parses alone, so it has no parse column. The browser rounds `performance.now()` to a tenth of a millisecond, which is why those cells end in zero.

| Board | Lines | Components | Parse | DRC | Whole load | WASM DRC | WASM whole load |
|-------|------:|-----------:|------:|----:|-----------:|---------:|----------------:|
| `examples/blink.cypcb` | 112 | 9 | 0.01 / 0.02 | 0.21 / 0.32 | 0.30 / 0.34 | 1.40 / 1.70 | 1.30 / 1.70 |
| `examples/mains-sequencer.cypcb` | 404 | 33 | 0.08 / 0.11 | 1.23 / 1.36 | 1.55 / 1.97 | 3.60 / 4.90 | 3.90 / 4.90 |
| `tests/fixtures/benchmark/esp32_starter.cypcb` | 436 | 18 | 0.09 / 0.11 | 1.44 / 1.59 | 1.85 / 2.14 | 3.90 / 4.00 | 4.40 / 4.60 |
| `tests/fixtures/benchmark/led_blink.kicad_pcb` | 152 | 7 | 0.16 / 0.25 | 0.18 / 0.23 | 0.38 / 0.55 | 0.50 / 0.80 | 1.50 / 3.00 |
| `tests/fixtures/benchmark/plane_board.kicad_pcb` | 212 | 12 | 0.27 / 0.31 | 0.49 / 0.54 | 0.94 / 1.29 | 1.80 / 2.10 | 2.20 / 3.10 |
| `tests/fixtures/benchmark/qfp_fanout.kicad_pcb` | 379 | 19 | 0.56 / 0.66 | 1.33 / 1.84 | 2.08 / 2.31 | 3.30 / 3.90 | 5.20 / 5.80 |
| `tests/fixtures/benchmark/stm32_breakout.kicad_pcb` | 456 | 29 | 0.58 / 0.86 | 1.33 / 1.73 | 1.85 / 2.22 | 2.80 / 3.10 | 4.60 / 5.20 |
| `tests/fixtures/benchmark/shift_driver.kicad_pcb` | 688 | 55 | 0.70 / 0.78 | 2.14 / 2.83 | 2.86 / 3.93 | 5.00 / 6.20 | 7.20 / 7.80 |
| `tests/fixtures/benchmark/multi_ic.kicad_pcb` | 905 | 52 | 0.90 / 1.00 | 2.50 / 3.57 | 3.54 / 4.55 | 6.00 / 6.50 | 9.20 / 9.70 |

The slowest board, `multi_ic`, in each build and on a busy host. Busy is a `cargo build --release -j3` of the workspace running beside it (three `rustc` processes, load average 6.1).

| Build, host | Parse | DRC | Whole load |
|-------------|------:|----:|-----------:|
| release, quiet | 0.90 / 1.00 | 2.50 / 3.57 | 3.54 / 4.55 |
| release, busy | 0.91 / 1.31 | 2.57 / 3.04 | 3.61 / 4.40 |
| debug, quiet | 5.27 / 7.62 | 3.29 / 3.91 | 8.81 / 12.95 |
| debug, busy | 5.43 / 9.41 | 3.28 / 5.08 | 9.10 / 14.27 |

To repeat the measurement, run the test that keeps this table. It prints each row again in the same form:

```bash
cargo test --release -p cypcb-render --features native --test the_editor_timings_are_measured -- --nocapture
```

The WASM columns, and the memory below, come from a Playwright spec that prints its rows the same way:

```bash
cd viewer && npx playwright test e2e/the-wasm-timings-are-measured.spec.ts --workers=1
```

The test checks the lines and the components of every row against the board, and the wait above against `viewer/src/main.ts`. The milliseconds are this measurement and move with the machine. `loading_a_board_is_quick` sets the ceiling a load of any example must stay under.

### Memory Usage

Measured 2026-09-27 by the spec above: five page loads in headless Chromium on the viewer's dev server, median / max, in MiB. The JS heap is read after a forced garbage collection (`Runtime.getHeapUsage` over the DevTools protocol). WASM memory is the engine's linear memory, `memory.buffer.byteLength`, which the JS heap does not count.

| What | MiB |
|------|----:|
| JS heap, page ready | 3.8 / 3.8 |
| JS heap, `tests/fixtures/benchmark/multi_ic.kicad_pcb` loaded | 4.1 / 4.2 |
| JS heap added by the editor | 11.1 / 11.3 |
| WASM memory, page ready | 1.3 / 1.3 |
| WASM memory, `tests/fixtures/benchmark/multi_ic.kicad_pcb` loaded | 1.8 / 1.8 |

**When Monaco loads:** the first paint does not wait for it. Once the page is ready, `preloadEditor` in `viewer/src/main.ts` builds the editor when the browser is next idle, without the panel being opened, so the panel opens at once when it is asked for. The spec holds Monaco at the network to measure the heap before it.

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
