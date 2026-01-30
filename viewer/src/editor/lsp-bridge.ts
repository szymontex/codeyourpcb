/**
 * LSP-like bridge for Monaco editor
 *
 * Bridges WASM engine diagnostics to Monaco marker API and provides
 * completion/hover providers without requiring a separate LSP server.
 *
 * This satisfies EDIT-02 (auto-completion), EDIT-03 (inline errors),
 * and EDIT-09 (LSP connection) by using the WASM engine as the source
 * of truth instead of connecting to tower-lsp over WebSocket.
 */

import type * as monaco from 'monaco-editor';
import type { ViolationInfo } from '../types';

// ============================================================================
// Diagnostics (EDIT-03)
// ============================================================================

/**
 * Update Monaco editor markers from WASM engine diagnostics
 *
 * Converts parse errors and DRC violations to Monaco's marker format.
 * Parse errors show as red squiggly underlines (MarkerSeverity.Error).
 * DRC violations show as warning markers (MarkerSeverity.Warning).
 *
 * @param monaco - Monaco editor module
 * @param editor - Monaco editor instance
 * @param parseErrors - Error string from engine.load_source() (newline-separated)
 * @param violations - DRC violations from snapshot.violations
 */
export function updateDiagnostics(
  monaco: typeof import('monaco-editor'),
  editor: any,
  parseErrors: string | null,
  violations: ViolationInfo[]
): void {
  const model = editor.getModel();
  if (!model) return;

  const markers: any[] = [];

  // Parse error strings and convert to markers
  if (parseErrors && parseErrors.trim()) {
    const errorLines = parseErrors.split('\n').filter(line => line.trim());

    for (const errorMsg of errorLines) {
      // Try to extract line number from error message
      // Expected format: "Line 5: unexpected token 'foo'" or similar
      const lineMatch = errorMsg.match(/[Ll]ine\s+(\d+)/);
      const lineNum = lineMatch ? parseInt(lineMatch[1], 10) : 1;

      // Ensure line number is valid
      const maxLine = model.getLineCount();
      const validLineNum = Math.max(1, Math.min(lineNum, maxLine));

      markers.push({
        severity: monaco.MarkerSeverity.Error,
        message: errorMsg,
        startLineNumber: validLineNum,
        startColumn: 1,
        endLineNumber: validLineNum,
        endColumn: model.getLineMaxColumn(validLineNum),
      });
    }
  }

  // Convert DRC violations to warning markers
  // Violations don't have line numbers (they have x_nm, y_nm positions),
  // so we add them as editor-level warnings at line 1
  for (const violation of violations) {
    markers.push({
      severity: monaco.MarkerSeverity.Warning,
      message: `[DRC ${violation.kind}] ${violation.message}`,
      startLineNumber: 1,
      startColumn: 1,
      endLineNumber: 1,
      endColumn: model.getLineMaxColumn(1),
    });
  }

  // Update markers for this model
  monaco.editor.setModelMarkers(model, 'cypcb', markers);
}

// ============================================================================
// Auto-completion (EDIT-02)
// ============================================================================

/**
 * Completion items for .cypcb language
 */
const COMPLETION_ITEMS = {
  keywords: [
    { label: 'version', detail: 'File format version', documentation: 'Specifies the .cypcb file format version (currently 1)' },
    { label: 'board', detail: 'Board definition', documentation: 'Defines the PCB board dimensions and layer stackup' },
    { label: 'component', detail: 'Component placement', documentation: 'Places a component on the board (resistor, capacitor, ic, etc.)' },
    { label: 'net', detail: 'Electrical net', documentation: 'Defines an electrical net connecting component pins' },
    { label: 'footprint', detail: 'Custom footprint', documentation: 'Defines a custom component footprint with pads' },
    { label: 'trace', detail: 'Copper trace', documentation: 'Defines a copper trace routing a net between points' },
    { label: 'zone', detail: 'Copper zone', documentation: 'Defines a copper zone (pour) for power or ground planes' },
    { label: 'keepout', detail: 'Keepout area', documentation: 'Defines an area where components or traces cannot be placed' },
  ],
  componentTypes: [
    { label: 'resistor', detail: 'Resistor', documentation: 'Passive component - resistor' },
    { label: 'capacitor', detail: 'Capacitor', documentation: 'Passive component - capacitor' },
    { label: 'ic', detail: 'Integrated circuit', documentation: 'Active component - integrated circuit' },
    { label: 'connector', detail: 'Connector', documentation: 'Mechanical component - connector' },
    { label: 'diode', detail: 'Diode', documentation: 'Active component - diode' },
    { label: 'transistor', detail: 'Transistor', documentation: 'Active component - transistor' },
    { label: 'led', detail: 'LED', documentation: 'Active component - light-emitting diode' },
    { label: 'crystal', detail: 'Crystal', documentation: 'Passive component - crystal oscillator' },
    { label: 'inductor', detail: 'Inductor', documentation: 'Passive component - inductor' },
    { label: 'generic', detail: 'Generic component', documentation: 'Generic component type' },
  ],
  properties: [
    { label: 'size', detail: 'Board size', documentation: 'Defines board dimensions (e.g., "size 100mm x 80mm")' },
    { label: 'layers', detail: 'Layer count', documentation: 'Number of copper layers (2, 4, 6, etc.)' },
    { label: 'value', detail: 'Component value', documentation: 'Component value (resistance, capacitance, part number, etc.)' },
    { label: 'at', detail: 'Position', documentation: 'Component position on the board (e.g., "at 10mm, 20mm")' },
    { label: 'rotate', detail: 'Rotation', documentation: 'Component rotation in degrees' },
    { label: 'pin', detail: 'Pin definition', documentation: 'Defines a pin in a custom footprint' },
    { label: 'width', detail: 'Trace width', documentation: 'Width of a trace or zone' },
    { label: 'clearance', detail: 'Clearance', documentation: 'Minimum clearance around a component or trace' },
    { label: 'current', detail: 'Current rating', documentation: 'Maximum current for a net or trace' },
    { label: 'from', detail: 'Start point', documentation: 'Starting point of a trace' },
    { label: 'to', detail: 'End point', documentation: 'Ending point of a trace' },
    { label: 'via', detail: 'Via', documentation: 'Via connecting layers in a trace' },
    { label: 'layer', detail: 'Layer', documentation: 'Copper layer name (Top, Bottom, Inner1-4)' },
    { label: 'locked', detail: 'Lock flag', documentation: 'Prevents modification of the component or trace' },
    { label: 'bounds', detail: 'Boundary', documentation: 'Defines a boundary for zones or keepouts' },
    { label: 'stackup', detail: 'Layer stackup', documentation: 'Defines the board layer stackup configuration' },
    { label: 'description', detail: 'Description', documentation: 'Human-readable description' },
    { label: 'pad', detail: 'Pad definition', documentation: 'Defines a pad in a custom footprint' },
    { label: 'courtyard', detail: 'Courtyard', documentation: 'Component courtyard boundary for placement clearance' },
  ],
  layers: [
    { label: 'Top', detail: 'Top copper layer', documentation: 'Top copper layer (layer 1)' },
    { label: 'Bottom', detail: 'Bottom copper layer', documentation: 'Bottom copper layer (layer 2)' },
    { label: 'Inner1', detail: 'Inner layer 1', documentation: 'Inner copper layer 1' },
    { label: 'Inner2', detail: 'Inner layer 2', documentation: 'Inner copper layer 2' },
    { label: 'Inner3', detail: 'Inner layer 3', documentation: 'Inner copper layer 3' },
    { label: 'Inner4', detail: 'Inner layer 4', documentation: 'Inner copper layer 4' },
    { label: 'all', detail: 'All layers', documentation: 'Applies to all layers' },
  ],
  units: [
    { label: 'mm', detail: 'Millimeters', documentation: 'Millimeters (metric)' },
    { label: 'mil', detail: 'Mils', documentation: 'Mils (1/1000 inch)' },
    { label: 'mA', detail: 'Milliamps', documentation: 'Milliamps (current)' },
    { label: 'A', detail: 'Amps', documentation: 'Amps (current)' },
    { label: 'V', detail: 'Volts', documentation: 'Volts (voltage)' },
    { label: 'k', detail: 'Kilo', documentation: 'Kilo prefix (1000x)' },
    { label: 'M', detail: 'Mega', documentation: 'Mega prefix (1000000x)' },
    { label: 'u', detail: 'Micro', documentation: 'Micro prefix (0.000001x)' },
    { label: 'n', detail: 'Nano', documentation: 'Nano prefix (0.000000001x)' },
    { label: 'p', detail: 'Pico', documentation: 'Pico prefix (0.000000000001x)' },
  ],
};

/**
 * Register auto-completion provider for .cypcb language
 *
 * Provides completions for keywords, component types, properties, layers, and units.
 * Filters based on current word prefix and context.
 *
 * @param monaco - Monaco editor module
 */
export function registerCompletionProvider(monaco: typeof import('monaco-editor')): void {
  monaco.languages.registerCompletionItemProvider('cypcb', {
    provideCompletionItems: (model, position) => {
      const word = model.getWordUntilPosition(position);
      const range = {
        startLineNumber: position.lineNumber,
        endLineNumber: position.lineNumber,
        startColumn: word.startColumn,
        endColumn: word.endColumn,
      };

      const suggestions: any[] = [];

      // Get the line content to determine context
      const lineContent = model.getLineContent(position.lineNumber);
      const beforeCursor = lineContent.substring(0, position.column - 1);

      // Check if we're after a number (for unit suggestions)
      const afterNumber = /\d+(\.\d+)?\s*$/.test(beforeCursor);

      if (afterNumber) {
        // Suggest units after a number
        for (const item of COMPLETION_ITEMS.units) {
          suggestions.push({
            label: item.label,
            kind: monaco.languages.CompletionItemKind.Unit,
            documentation: item.documentation,
            detail: item.detail,
            insertText: item.label,
            range,
          });
        }
      } else {
        // Suggest keywords
        for (const item of COMPLETION_ITEMS.keywords) {
          suggestions.push({
            label: item.label,
            kind: monaco.languages.CompletionItemKind.Keyword,
            documentation: item.documentation,
            detail: item.detail,
            insertText: item.label,
            range,
          });
        }

        // Suggest component types (context: after "component RefDes")
        if (beforeCursor.includes('component ')) {
          for (const item of COMPLETION_ITEMS.componentTypes) {
            suggestions.push({
              label: item.label,
              kind: monaco.languages.CompletionItemKind.Class,
              documentation: item.documentation,
              detail: item.detail,
              insertText: item.label,
              range,
            });
          }
        }

        // Suggest properties
        for (const item of COMPLETION_ITEMS.properties) {
          suggestions.push({
            label: item.label,
            kind: monaco.languages.CompletionItemKind.Property,
            documentation: item.documentation,
            detail: item.detail,
            insertText: item.label,
            range,
          });
        }

        // Suggest layers (context: after "layer")
        if (beforeCursor.includes('layer ')) {
          for (const item of COMPLETION_ITEMS.layers) {
            suggestions.push({
              label: item.label,
              kind: monaco.languages.CompletionItemKind.Enum,
              documentation: item.documentation,
              detail: item.detail,
              insertText: item.label,
              range,
            });
          }
        }
      }

      return { suggestions };
    },
  });
}

// ============================================================================
// Hover (partial EDIT-09)
// ============================================================================

/**
 * Keyword documentation for hover tooltips
 */
const KEYWORD_DOCS: Record<string, string> = {
  version: 'Specifies the .cypcb file format version. Current version is 1.',
  board: 'Defines the PCB board dimensions and layer stackup. Contains size and layer count.',
  component: 'Places a component on the board. Supported types: resistor, capacitor, ic, connector, diode, transistor, led, crystal, inductor, generic.',
  net: 'Defines an electrical net connecting component pins. Nets are used for routing and DRC.',
  footprint: 'Defines a custom component footprint with pads, courtyard, and other properties.',
  trace: 'Defines a copper trace routing a net between points. Can include vias for layer changes.',
  zone: 'Defines a copper zone (pour) for power or ground planes. Fills unused area with copper.',
  keepout: 'Defines an area where components or traces cannot be placed. Used for mechanical constraints.',
  resistor: 'Passive component type - resistor. Specify value in ohms (e.g., "330" or "10k").',
  capacitor: 'Passive component type - capacitor. Specify value in farads (e.g., "100n" or "10u").',
  ic: 'Active component type - integrated circuit. Use for chips, microcontrollers, etc.',
  connector: 'Mechanical component type - connector. Use for headers, sockets, etc.',
  diode: 'Active component type - diode. Use for LEDs, signal diodes, etc.',
  transistor: 'Active component type - transistor. Use for MOSFETs, BJTs, etc.',
  led: 'Active component type - light-emitting diode.',
  crystal: 'Passive component type - crystal oscillator or resonator.',
  inductor: 'Passive component type - inductor or coil.',
  generic: 'Generic component type for components that don\'t fit other categories.',
  size: 'Defines board dimensions. Format: "size <width> x <height>" (e.g., "size 100mm x 80mm").',
  layers: 'Number of copper layers (2, 4, 6, etc.). Determines available routing layers.',
  value: 'Component value. For resistors/capacitors, use standard notation (e.g., "10k", "100n").',
  at: 'Component position on the board. Format: "at <x>, <y> [rotate <angle>]" (e.g., "at 10mm, 20mm rotate 90").',
  rotate: 'Component rotation in degrees. Can be specified with "at" or separately.',
  pin: 'Defines a pin in a custom footprint. Specifies number, position, and pad properties.',
  width: 'Width of a trace or zone. Specified with units (e.g., "width 0.2mm").',
  clearance: 'Minimum clearance around a component or trace. Used for DRC.',
  current: 'Maximum current rating for a net or trace. Used for DRC.',
  from: 'Starting point of a trace. Typically a component pin reference (e.g., "R1.1").',
  to: 'Ending point of a trace. Typically a component pin reference (e.g., "R1.2").',
  via: 'Via connecting layers in a trace. Allows routing to change copper layers.',
  layer: 'Copper layer name. Options: Top, Bottom, Inner1-4.',
  locked: 'Prevents modification of the component or trace during routing.',
  bounds: 'Defines a boundary polygon for zones or keepouts.',
  stackup: 'Defines the board layer stackup configuration (copper, dielectric, etc.).',
  description: 'Human-readable description for documentation purposes.',
  pad: 'Defines a pad in a custom footprint. Specifies shape, size, and layer.',
  courtyard: 'Component courtyard boundary. Used for placement clearance DRC.',
  Top: 'Top copper layer (layer 1). Primary component side.',
  Bottom: 'Bottom copper layer (layer 2). Secondary component side.',
  Inner1: 'Inner copper layer 1. Available on 4+ layer boards.',
  Inner2: 'Inner copper layer 2. Available on 4+ layer boards.',
  Inner3: 'Inner copper layer 3. Available on 6+ layer boards.',
  Inner4: 'Inner copper layer 4. Available on 6+ layer boards.',
  all: 'Applies to all layers. Used for through-hole pads and vias.',
};

/**
 * Register hover provider for .cypcb language
 *
 * Shows documentation tooltips when hovering over keywords.
 *
 * @param monaco - Monaco editor module
 */
export function registerHoverProvider(monaco: typeof import('monaco-editor')): void {
  monaco.languages.registerHoverProvider('cypcb', {
    provideHover: (model, position) => {
      const word = model.getWordAtPosition(position);
      if (!word) return null;

      const documentation = KEYWORD_DOCS[word.word];
      if (!documentation) return null;

      return {
        range: new monaco.Range(
          position.lineNumber,
          word.startColumn,
          position.lineNumber,
          word.endColumn
        ),
        contents: [
          { value: `**${word.word}**` },
          { value: documentation },
        ],
      };
    },
  });
}

// ============================================================================
// Provider Registration
// ============================================================================

/**
 * Register all LSP-like providers for Monaco editor
 *
 * Call this once after Monaco is loaded and the .cypcb language is registered.
 *
 * @param monaco - Monaco editor module
 */
export function registerProviders(monaco: typeof import('monaco-editor')): void {
  registerCompletionProvider(monaco);
  registerHoverProvider(monaco);
  console.log('[LSP Bridge] Completion and hover providers registered');
}
