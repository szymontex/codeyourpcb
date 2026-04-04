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
// Context Detection
// ============================================================================

/**
 * Block context types for context-aware completions
 */
type BlockContext =
  | 'top-level'
  | 'board'
  | 'component'
  | 'net-constraint'
  | 'net-pins'
  | 'trace'
  | 'footprint'
  | 'zone';

/**
 * Detect what block context the cursor is inside by scanning backwards
 * from the cursor position to find the nearest unclosed `{` or `[`.
 *
 * Context rules:
 * - `board NAME {` -> board context
 * - `component REFDES TYPE "FP" {` -> component context
 * - `net NAME [` -> net constraint context
 * - `net NAME {` -> net pins context
 * - `trace NAME {` -> trace context
 * - `footprint NAME {` -> footprint context
 * - `zone NAME {` or `keepout NAME {` -> zone context
 * - Otherwise -> top-level
 */
function detectBlockContext(model: any, position: any): BlockContext {
  let braceDepth = 0;
  let bracketDepth = 0;

  // Scan backwards from cursor line to find enclosing block
  for (let lineNum = position.lineNumber; lineNum >= 1; lineNum--) {
    const lineText = lineNum === position.lineNumber
      ? model.getLineContent(lineNum).substring(0, position.column - 1)
      : model.getLineContent(lineNum);

    // Scan characters right-to-left on this line
    for (let i = lineText.length - 1; i >= 0; i--) {
      const ch = lineText[i];

      if (ch === '}') {
        braceDepth++;
      } else if (ch === '{') {
        if (braceDepth > 0) {
          braceDepth--;
        } else {
          // Found the unclosed `{` — check what keyword precedes it
          // Check the full line from the model for context
          const fullLine = model.getLineContent(lineNum);
          const lineBeforeBrace = fullLine.substring(0, i).trim();

          if (/^board\b/.test(lineBeforeBrace)) return 'board';
          if (/^component\b/.test(lineBeforeBrace)) return 'component';
          if (/^net\b/.test(lineBeforeBrace)) return 'net-pins';
          if (/^trace\b/.test(lineBeforeBrace)) return 'trace';
          if (/^footprint\b/.test(lineBeforeBrace)) return 'footprint';
          if (/^(zone|keepout)\b/.test(lineBeforeBrace)) return 'zone';

          // Brace not preceded by a known keyword; could be nested
          // Continue scanning outward
          // (don't return top-level, keep looking)
        }
      } else if (ch === ']') {
        bracketDepth++;
      } else if (ch === '[') {
        if (bracketDepth > 0) {
          bracketDepth--;
        } else {
          // Found unclosed `[` — check if preceded by net
          const fullLine = model.getLineContent(lineNum);
          const lineBeforeBracket = fullLine.substring(0, i).trim();

          if (/^net\b/.test(lineBeforeBracket)) return 'net-constraint';
          // Some other bracket context, continue scanning
        }
      }
    }
  }

  return 'top-level';
}

/**
 * Extract component reference designators (refdes) declared in the document.
 * Scans for `component XX` declarations and returns the refdes list.
 */
function extractComponentRefdes(model: any): string[] {
  const refdesList: string[] = [];
  const lineCount = model.getLineCount();

  for (let i = 1; i <= lineCount; i++) {
    const line = model.getLineContent(i);
    const match = line.match(/^\s*component\s+(\w+)/);
    if (match) {
      refdesList.push(match[1]);
    }
  }

  return refdesList;
}

// ============================================================================
// Auto-completion (EDIT-02) - Context-Aware Snippets
// ============================================================================

/**
 * Completion items for .cypcb language (legacy flat lists for backward compat)
 */
const COMPLETION_ITEMS = {
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
 * Snippet definition for context-aware completions
 */
interface SnippetDef {
  label: string;
  detail: string;
  documentation: string;
  insertText: string;
  isSnippet: boolean;
  sortOrder?: string;
}

/**
 * Get context-specific snippet completions based on block context.
 */
function getContextSnippets(context: BlockContext, model: any): SnippetDef[] {
  switch (context) {
    case 'board':
      return [
        {
          label: 'size',
          detail: 'Board dimensions (width x height)',
          documentation: 'Sets the physical board dimensions.\n\nSyntax: `size <width>mm x <height>mm`\n\nExample: `size 100mm x 80mm`',
          insertText: 'size ${1:100}mm x ${2:80}mm',
          isSnippet: true,
          sortOrder: '0',
        },
        {
          label: 'layers',
          detail: 'Number of copper layers',
          documentation: 'Sets the PCB layer count.\n\nSyntax: `layers <count>`\n\nValid values: 2, 4, 6\n\nExample: `layers 2`',
          insertText: 'layers ${1|2,4,6|}',
          isSnippet: true,
          sortOrder: '1',
        },
        {
          label: 'stackup',
          detail: 'Layer stackup configuration',
          documentation: 'Defines the board layer stackup configuration (copper, dielectric, etc.).\n\nSyntax: `stackup { ... }`',
          insertText: 'stackup',
          isSnippet: false,
          sortOrder: '2',
        },
      ];

    case 'component':
      return [
        {
          label: 'value',
          detail: 'Component value',
          documentation: 'Sets the component value (resistance, capacitance, part number, etc.).\n\nSyntax: `value "<value>"`\n\nExamples: `value "10k"`, `value "100nF"`, `value "ATmega328P"`',
          insertText: 'value "${1:10k}"',
          isSnippet: true,
          sortOrder: '0',
        },
        {
          label: 'at',
          detail: 'Position on board',
          documentation: 'Places the component at the given board coordinates.\n\nSyntax: `at <x>mm, <y>mm`\n\nExample: `at 10mm, 20mm`',
          insertText: 'at ${1:10}mm, ${2:20}mm',
          isSnippet: true,
          sortOrder: '1',
        },
        {
          label: 'rotate',
          detail: 'Rotation in degrees (0, 90, 180, 270)',
          documentation: 'Sets the component rotation.\n\nSyntax: `rotate <degrees>`\n\nValid values: 0, 90, 180, 270\n\nExample: `rotate 90`',
          insertText: 'rotate ${1:0}',
          isSnippet: true,
          sortOrder: '2',
        },
        {
          label: 'lcsc',
          detail: 'LCSC/JLCPCB part number',
          documentation: 'Associates an LCSC part number for JLCPCB assembly.\n\nSyntax: `lcsc "<part_number>"`\n\nExample: `lcsc "C12345"`\n\nThe footprint is auto-fetched from EasyEDA API.',
          insertText: 'lcsc "${1:C12345}"',
          isSnippet: true,
          sortOrder: '3',
        },
        {
          label: 'locked',
          detail: 'Prevent autorouter from moving this component',
          documentation: 'Locks the component position so the autorouter cannot modify it.\n\nSyntax: `locked`',
          insertText: 'locked',
          isSnippet: false,
          sortOrder: '4',
        },
      ];

    case 'net-constraint':
      return [
        {
          label: 'width',
          detail: 'Trace width constraint',
          documentation: 'Sets the trace width for this net.\n\nSyntax: `width <value><unit>`\n\nExamples: `width 0.25mm`, `width 10mil`\n\nIPC-2221 default for signal: 0.15-0.25mm\nJLCPCB minimum: 0.127mm (5mil)',
          insertText: 'width ${1:0.25}mm',
          isSnippet: true,
          sortOrder: '0',
        },
        {
          label: 'clearance',
          detail: 'Min clearance to other copper',
          documentation: 'Sets the minimum clearance from traces of this net to other copper.\n\nSyntax: `clearance <value><unit>`\n\nExamples: `clearance 0.15mm`, `clearance 6mil`\n\nJLCPCB minimum: 0.127mm (5mil)',
          insertText: 'clearance ${1:0.15}mm',
          isSnippet: true,
          sortOrder: '1',
        },
        {
          label: 'current',
          detail: 'Current rating (auto-calculates min trace width)',
          documentation: 'Current rating for IPC-2221 trace width calculation.\n\nSyntax: `current <value><unit>`\n\nExamples: `current 500mA`, `current 2A`\n\nUsed inside net constraints: `net VCC [current 2A] { ... }`\nAutomatic DRC checks trace width against IPC-2221 minimum.',
          insertText: 'current ${1:500}mA',
          isSnippet: true,
          sortOrder: '2',
        },
      ];

    case 'net-pins': {
      // In net pin context, suggest component refdes from the document
      const refdesList = extractComponentRefdes(model);
      const snippets: SnippetDef[] = [];

      for (let i = 0; i < refdesList.length; i++) {
        const refdes = refdesList[i];
        snippets.push({
          label: refdes,
          detail: `Component ${refdes}`,
          documentation: `Reference to component ${refdes}.\n\nSyntax: \`${refdes}.<pin>\`\n\nExample: \`${refdes}.1\`, \`${refdes}.2\``,
          insertText: `${refdes}.\${1:1}`,
          isSnippet: true,
          sortOrder: String(i),
        });
      }

      return snippets;
    }

    case 'trace':
      return [
        {
          label: 'layer',
          detail: 'Copper layer for this trace segment',
          documentation: 'Sets the copper layer.\n\nSyntax: `layer <name>`\n\nOptions: Top, Bottom, Inner1, Inner2, Inner3, Inner4\n\nExample: `layer Top`',
          insertText: 'layer ${1|Top,Bottom,Inner1,Inner2|}',
          isSnippet: true,
          sortOrder: '0',
        },
        {
          label: 'width',
          detail: 'Override trace width (default from net constraint)',
          documentation: 'Override trace width for this specific trace.\n\nSyntax: `width <value><unit>`\n\nExamples: `width 0.25mm`, `width 10mil`\n\nOverrides the net-level constraint if set.',
          insertText: 'width ${1:0.25}mm',
          isSnippet: true,
          sortOrder: '1',
        },
        {
          label: 'from',
          detail: 'Start pin (component.pin)',
          documentation: 'Starting point of the trace.\n\nSyntax: `from <refdes>.<pin>`\n\nExample: `from R1.1`',
          insertText: 'from ${1:R1}.${2:1}',
          isSnippet: true,
          sortOrder: '2',
        },
        {
          label: 'to',
          detail: 'End pin (component.pin)',
          documentation: 'Ending point of the trace.\n\nSyntax: `to <refdes>.<pin>`\n\nExample: `to R1.2`',
          insertText: 'to ${1:R1}.${2:2}',
          isSnippet: true,
          sortOrder: '3',
        },
        {
          label: 'path',
          detail: 'Explicit polyline geometry',
          documentation: 'Defines an explicit trace path as a series of coordinates.\n\nSyntax: `path <x1>mm,<y1>mm -> <x2>mm,<y2>mm [-> ...]`\n\nExample: `path 10mm,20mm -> 30mm,20mm -> 30mm,40mm`',
          insertText: 'path ${1:10}mm,${2:20}mm -> ${3:30}mm,${4:20}mm',
          isSnippet: true,
          sortOrder: '4',
        },
        {
          label: 'via',
          detail: 'Via with drill size',
          documentation: 'Places a via for layer transition.\n\nSyntax: `via <x>mm,<y>mm drill <size>mm`\n\nExample: `via 15mm,20mm drill 0.3mm`',
          insertText: 'via ${1:15}mm,${2:20}mm drill ${3:0.3}mm',
          isSnippet: true,
          sortOrder: '5',
        },
        {
          label: 'locked',
          detail: 'Prevent autorouter from modifying this trace',
          documentation: 'Locks the trace so the autorouter cannot reroute it.\n\nSyntax: `locked`',
          insertText: 'locked',
          isSnippet: false,
          sortOrder: '6',
        },
      ];

    case 'footprint':
      return [
        {
          label: 'description',
          detail: 'Footprint description',
          documentation: 'Human-readable description of the footprint.\n\nSyntax: `description "<text>"`\n\nExample: `description "0402 resistor footprint"`',
          insertText: 'description "${1:text}"',
          isSnippet: true,
          sortOrder: '0',
        },
        {
          label: 'pad',
          detail: 'Pad definition',
          documentation: 'Defines a pad in the footprint.\n\nSyntax: `pad <number> <shape> at <x>mm, <y>mm size <w>mm x <h>mm`\n\nShapes: rect, circle, roundrect, oblong\n\nExample: `pad 1 rect at 0mm, 0mm size 1mm x 1mm`',
          insertText: 'pad ${1:1} ${2|rect,circle,roundrect,oblong|} at ${3:0}mm, ${4:0}mm size ${5:1}mm x ${6:1}mm',
          isSnippet: true,
          sortOrder: '1',
        },
        {
          label: 'courtyard',
          detail: 'Component courtyard boundary',
          documentation: 'Defines the component courtyard for placement clearance DRC.\n\nSyntax: `courtyard <width>mm x <height>mm`\n\nExample: `courtyard 5mm x 5mm`',
          insertText: 'courtyard ${1:5}mm x ${2:5}mm',
          isSnippet: true,
          sortOrder: '2',
        },
      ];

    case 'zone':
      return [
        {
          label: 'bounds',
          detail: 'Zone boundary rectangle',
          documentation: 'Defines the zone boundary as a rectangle.\n\nSyntax: `bounds <x1>mm, <y1>mm to <x2>mm, <y2>mm`\n\nExample: `bounds 0mm, 0mm to 50mm, 30mm`',
          insertText: 'bounds ${1:0}mm, ${2:0}mm to ${3:50}mm, ${4:30}mm',
          isSnippet: true,
          sortOrder: '0',
        },
        {
          label: 'layer',
          detail: 'Which copper layer (lowercase for zones)',
          documentation: 'Sets the copper layer for the zone pour.\n\nSyntax: `layer <name>`\n\nOptions: top, bottom, all\n\nExample: `layer bottom`',
          insertText: 'layer ${1|top,bottom,all|}',
          isSnippet: true,
          sortOrder: '1',
        },
        {
          label: 'net',
          detail: 'Net for copper pour',
          documentation: 'Assigns the zone to a net for copper fill.\n\nSyntax: `net <name>`\n\nExample: `net GND`',
          insertText: 'net ${1:GND}',
          isSnippet: true,
          sortOrder: '2',
        },
      ];

    case 'top-level':
    default:
      return [
        {
          label: 'version',
          detail: 'File format version',
          documentation: 'Specifies the .cypcb file format version (currently 1).\n\nSyntax: `version <number>`\n\nExample: `version 1`',
          insertText: 'version ${1:1}',
          isSnippet: true,
          sortOrder: '00',
        },
        {
          label: 'board',
          detail: 'Board definition block',
          documentation: 'Defines the PCB board dimensions and layer stackup.\n\nSyntax:\n```\nboard <name> {\n  size <w>mm x <h>mm\n  layers <count>\n}\n```',
          insertText: 'board ${1:myboard} {\n\tsize ${2:100}mm x ${3:80}mm\n\tlayers ${4:2}\n}',
          isSnippet: true,
          sortOrder: '01',
        },
        {
          label: 'component',
          detail: 'Component placement block',
          documentation: 'Places a component on the board.\n\nSyntax:\n```\ncomponent <refdes> <type> "<footprint>" {\n  value "<val>"\n  at <x>mm, <y>mm\n}\n```\n\nTypes: resistor, capacitor, ic, connector, diode, led, transistor, crystal, inductor, generic',
          insertText: 'component ${1:R1} ${2|resistor,capacitor,ic,connector,diode,led,transistor,crystal,inductor,generic|} "${3:0402}" {\n\tvalue "${4:10k}"\n\tat ${5:10}mm, ${6:20}mm\n}',
          isSnippet: true,
          sortOrder: '02',
        },
        {
          label: 'net',
          detail: 'Electrical net block',
          documentation: 'Defines an electrical net connecting component pins.\n\nSyntax:\n```\nnet <name> {\n  <refdes>.<pin>\n  <refdes>.<pin>\n}\n```\n\nWith constraints:\n```\nnet <name> [width 0.25mm] {\n  R1.1\n  C1.1\n}\n```',
          insertText: 'net ${1:VCC} {\n\t${2:R1.1}\n}',
          isSnippet: true,
          sortOrder: '03',
        },
        {
          label: 'trace',
          detail: 'Copper trace block',
          documentation: 'Defines a copper trace routing a net.\n\nSyntax:\n```\ntrace <net_name> {\n  layer <Top|Bottom>\n  width <value>mm\n  path <x1>mm,<y1>mm -> <x2>mm,<y2>mm\n}\n```',
          insertText: 'trace ${1:VCC} {\n\tlayer ${2|Top,Bottom|}\n\twidth ${3:0.25}mm\n\tpath ${4:10}mm,${5:20}mm -> ${6:30}mm,${7:20}mm\n}',
          isSnippet: true,
          sortOrder: '04',
        },
        {
          label: 'footprint',
          detail: 'Custom footprint definition',
          documentation: 'Defines a custom component footprint with pads.\n\nSyntax:\n```\nfootprint <name> {\n  description "<text>"\n  pad <n> <shape> at <x>mm, <y>mm size <w>mm x <h>mm\n}\n```',
          insertText: 'footprint ${1:my_footprint} {\n\tdescription "${2:Custom footprint}"\n\tpad ${3:1} ${4|rect,circle,roundrect|} at ${5:0}mm, ${6:0}mm size ${7:1}mm x ${8:1}mm\n}',
          isSnippet: true,
          sortOrder: '05',
        },
        {
          label: 'zone',
          detail: 'Copper zone (pour)',
          documentation: 'Defines a copper zone for power or ground planes.\n\nSyntax:\n```\nzone <name> {\n  bounds <x1>mm, <y1>mm to <x2>mm, <y2>mm\n  layer <top|bottom|all>\n  net <net_name>\n}\n```',
          insertText: 'zone ${1:GND_zone} {\n\tbounds ${2:0}mm, ${3:0}mm to ${4:50}mm, ${5:30}mm\n\tlayer ${6|top,bottom,all|}\n\tnet ${7:GND}\n}',
          isSnippet: true,
          sortOrder: '06',
        },
        {
          label: 'keepout',
          detail: 'Keepout area',
          documentation: 'Defines an area where components or traces cannot be placed.\n\nSyntax:\n```\nkeepout <name> {\n  bounds <x1>mm, <y1>mm to <x2>mm, <y2>mm\n}\n```',
          insertText: 'keepout ${1:no_go} {\n\tbounds ${2:0}mm, ${3:0}mm to ${4:10}mm, ${5:10}mm\n}',
          isSnippet: true,
          sortOrder: '07',
        },
      ];
  }
}

/**
 * Register auto-completion provider for .cypcb language
 *
 * Provides context-aware snippet completions based on the cursor position.
 * Detects the enclosing block (board, component, net, trace, etc.) and
 * offers only relevant properties with full syntax format hints.
 *
 * @param monaco - Monaco editor module
 */
export function registerCompletionProvider(monaco: typeof import('monaco-editor')): void {
  monaco.languages.registerCompletionItemProvider('cypcb', {
    triggerCharacters: ['.', ' '],

    provideCompletionItems: (model, position) => {
      const word = model.getWordUntilPosition(position);
      const range = {
        startLineNumber: position.lineNumber,
        endLineNumber: position.lineNumber,
        startColumn: word.startColumn,
        endColumn: word.endColumn,
      };

      const suggestions: any[] = [];

      // Get the line content to determine inline context
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
        return { suggestions };
      }

      // Check if we're after "component REFDES " (suggest component types)
      if (/^\s*component\s+\w+\s+$/.test(beforeCursor)) {
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
        return { suggestions };
      }

      // Check if we're after "layer " (suggest layer names)
      if (/\blayer\s+$/.test(beforeCursor)) {
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
        return { suggestions };
      }

      // Check if we're after a refdes dot (e.g. "R1.") — suggest pin numbers
      const dotMatch = beforeCursor.match(/\b(\w+)\.\s*$/);
      if (dotMatch) {
        const refdesList = extractComponentRefdes(model);
        const typedRefdes = dotMatch[1];
        if (refdesList.includes(typedRefdes)) {
          // Suggest common pin numbers
          for (let pin = 1; pin <= 8; pin++) {
            suggestions.push({
              label: String(pin),
              kind: monaco.languages.CompletionItemKind.Value,
              documentation: `Pin ${pin} of ${typedRefdes}`,
              detail: `${typedRefdes}.${pin}`,
              insertText: String(pin),
              range,
              sortText: String(pin).padStart(2, '0'),
            });
          }
          return { suggestions };
        }
      }

      // Detect block context for context-aware completions
      const context = detectBlockContext(model, position);
      const snippets = getContextSnippets(context, model);

      for (const snippet of snippets) {
        suggestions.push({
          label: snippet.label,
          kind: snippet.isSnippet
            ? monaco.languages.CompletionItemKind.Snippet
            : monaco.languages.CompletionItemKind.Keyword,
          documentation: {
            value: snippet.documentation,
            isTrusted: true,
          },
          detail: snippet.detail,
          insertText: snippet.insertText,
          insertTextRules: snippet.isSnippet
            ? monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet
            : undefined,
          range,
          sortText: snippet.sortOrder,
        });
      }

      return { suggestions };
    },
  });
}

// ============================================================================
// Hover (partial EDIT-09)
// ============================================================================

/**
 * Keyword documentation for hover tooltips.
 * Each entry includes full syntax format and usage examples.
 */
const KEYWORD_DOCS: Record<string, string> = {
  version: 'File format version number.\n\nSyntax: `version <number>`\n\nExample: `version 1`\n\nCurrently the only valid version is 1.',

  board: 'Defines the PCB board dimensions and layer stackup.\n\nSyntax:\n```\nboard <name> {\n  size <width>mm x <height>mm\n  layers <count>\n}\n```\n\nExample:\n```\nboard myboard {\n  size 100mm x 80mm\n  layers 2\n}\n```',

  component: 'Places a component on the board.\n\nSyntax:\n```\ncomponent <refdes> <type> "<footprint>" {\n  value "<value>"\n  at <x>mm, <y>mm\n  rotate <degrees>\n  lcsc "<part_number>"\n}\n```\n\nTypes: resistor, capacitor, ic, connector, diode, transistor, led, crystal, inductor, generic\n\nExample:\n```\ncomponent R1 resistor "0402" {\n  value "10k"\n  at 10mm, 20mm\n}\n```',

  net: 'Defines an electrical net connecting component pins.\n\nSyntax:\n```\nnet <name> [<constraints>] {\n  <refdes>.<pin>\n  ...\n}\n```\n\nConstraints (inside `[...]`): `width`, `clearance`, `current`\n\nExample:\n```\nnet VCC [width 0.5mm, current 2A] {\n  U1.1\n  C1.1\n}\n```',

  footprint: 'Defines a custom component footprint with pads.\n\nSyntax:\n```\nfootprint <name> {\n  description "<text>"\n  pad <number> <shape> at <x>mm, <y>mm size <w>mm x <h>mm\n  courtyard <w>mm x <h>mm\n}\n```\n\nShapes: rect, circle, roundrect, oblong\n\nExample:\n```\nfootprint my_sot23 {\n  description "SOT-23 footprint"\n  pad 1 rect at -0.95mm, -1mm size 0.6mm x 0.7mm\n  pad 2 rect at 0.95mm, -1mm size 0.6mm x 0.7mm\n  pad 3 rect at 0mm, 1mm size 0.6mm x 0.7mm\n}\n```',

  trace: 'Defines a copper trace routing a net between points.\n\nSyntax:\n```\ntrace <net_name> {\n  layer <Top|Bottom|Inner1|Inner2>\n  width <value>mm\n  from <refdes>.<pin>\n  to <refdes>.<pin>\n  path <x1>mm,<y1>mm -> <x2>mm,<y2>mm\n  via <x>mm,<y>mm drill <size>mm\n  locked\n}\n```\n\nExample:\n```\ntrace VCC {\n  layer Top\n  width 0.3mm\n  from U1.1\n  to C1.1\n  path 10mm,20mm -> 30mm,20mm\n}\n```',

  zone: 'Defines a copper zone (pour) for power or ground planes.\n\nSyntax:\n```\nzone <name> {\n  bounds <x1>mm, <y1>mm to <x2>mm, <y2>mm\n  layer <top|bottom|all>\n  net <net_name>\n}\n```\n\nExample:\n```\nzone GND_pour {\n  bounds 0mm, 0mm to 100mm, 80mm\n  layer bottom\n  net GND\n}\n```',

  keepout: 'Defines an area where components or traces cannot be placed.\n\nSyntax:\n```\nkeepout <name> {\n  bounds <x1>mm, <y1>mm to <x2>mm, <y2>mm\n}\n```\n\nExample:\n```\nkeepout connector_area {\n  bounds 0mm, 0mm to 10mm, 10mm\n}\n```',

  resistor: 'Passive component type - resistor.\n\nSpecify value in ohms: `"330"`, `"10k"`, `"4.7M"`\n\nCommon footprints: "0402", "0603", "0805", "1206"',
  capacitor: 'Passive component type - capacitor.\n\nSpecify value in farads: `"100n"`, `"10u"`, `"1p"`\n\nCommon footprints: "0402", "0603", "0805", "1206"',
  ic: 'Active component type - integrated circuit.\n\nUse for chips, microcontrollers, op-amps, etc.\nSpecify the footprint package (e.g., "SOIC-8", "QFP-48").',
  connector: 'Mechanical component type - connector.\n\nUse for headers, sockets, JST, USB, etc.\nSpecify the footprint to match the connector type.',
  diode: 'Active component type - diode.\n\nUse for rectifier diodes, Zener, Schottky, etc.\nCommon footprints: "SOD-123", "SMA", "SMB"',
  transistor: 'Active component type - transistor.\n\nUse for MOSFETs, BJTs, JFETs, etc.\nCommon footprints: "SOT-23", "SOT-223", "TO-252"',
  led: 'Active component type - light-emitting diode.\n\nCommon footprints: "0402", "0603", "0805", "5mm"',
  crystal: 'Passive component type - crystal oscillator or resonator.\n\nCommon footprints: "HC49", "3215", "5032"',
  inductor: 'Passive component type - inductor or coil.\n\nSpecify value in henries: `"10u"`, `"100n"`, `"4.7u"`',
  generic: 'Generic component type for components that don\'t fit other categories.\n\nUse when no specific type applies.',

  size: 'Defines board dimensions.\n\nSyntax: `size <width>mm x <height>mm`\n\nExamples: `size 100mm x 80mm`, `size 50mm x 50mm`\n\nUsed inside `board { }` block.',

  layers: 'Number of copper layers.\n\nSyntax: `layers <count>`\n\nValid values: 2, 4, 6\n\nExample: `layers 4`\n\nUsed inside `board { }` block.',

  value: 'Component value (resistance, capacitance, part number, etc.).\n\nSyntax: `value "<text>"`\n\nExamples: `value "10k"`, `value "100nF"`, `value "ATmega328P"`\n\nUsed inside `component { }` block.',

  at: 'Component position on the board.\n\nSyntax: `at <x>mm, <y>mm`\n\nExample: `at 10mm, 20mm`\n\nUsed inside `component { }` block. Coordinates are from the board origin (top-left).',

  rotate: 'Component rotation in degrees.\n\nSyntax: `rotate <degrees>`\n\nValid values: 0, 90, 180, 270\n\nExample: `rotate 90`\n\nUsed inside `component { }` block.',

  lcsc: 'LCSC/JLCPCB part number for automated assembly.\n\nSyntax: `lcsc "<part_number>"`\n\nExample: `lcsc "C12345"`\n\nUsed inside `component { }` block.\nThe footprint is auto-fetched from the EasyEDA API when set.',

  pin: 'Defines a pin in a custom footprint.\n\nSpecifies number, position, and pad properties.',

  width: 'Trace or zone width.\n\nSyntax: `width <value><unit>`\n\nExamples: `width 0.25mm`, `width 10mil`\n\nIn net constraints: `net VCC [width 0.5mm] { ... }`\nIn trace blocks: `trace VCC { width 0.3mm ... }`\n\nIPC-2221 signal default: 0.15-0.25mm\nJLCPCB minimum: 0.127mm (5mil)',

  clearance: 'Minimum clearance to other copper.\n\nSyntax: `clearance <value><unit>`\n\nExamples: `clearance 0.15mm`, `clearance 6mil`\n\nUsed in net constraints: `net VCC [clearance 0.2mm] { ... }`\n\nJLCPCB minimum: 0.127mm (5mil)\nIPC-2221 depends on voltage class.',

  current: 'Current rating for IPC-2221 trace width calculation.\n\nSyntax: `current <value><unit>`\n\nExamples: `current 500mA`, `current 2A`\n\nUsed inside net constraints: `net VCC [current 2A] { ... }`\nAutomatic DRC checks trace width against IPC-2221 minimum.\n\nUnits: mA (milliamps) or A (amps)',

  from: 'Starting point of a trace.\n\nSyntax: `from <refdes>.<pin>`\n\nExample: `from R1.1`\n\nUsed inside `trace { }` block.',

  to: 'Ending point of a trace.\n\nSyntax: `to <refdes>.<pin>`\n\nExample: `to R1.2`\n\nUsed inside `trace { }` block.',

  via: 'Via connecting layers in a trace.\n\nSyntax: `via <x>mm,<y>mm drill <size>mm`\n\nExample: `via 15mm,20mm drill 0.3mm`\n\nUsed inside `trace { }` block. Allows routing to change copper layers.',

  layer: 'Copper layer name.\n\nSyntax: `layer <name>`\n\nIn trace blocks: `layer Top`, `layer Bottom`, `layer Inner1`, `layer Inner2`\nIn zone blocks: `layer top`, `layer bottom`, `layer all` (lowercase)',

  locked: 'Prevents modification by the autorouter.\n\nSyntax: `locked`\n\nUsed inside `component { }` or `trace { }` blocks.\nLocked items are preserved during autorouting.',

  bounds: 'Defines a boundary rectangle for zones or keepouts.\n\nSyntax: `bounds <x1>mm, <y1>mm to <x2>mm, <y2>mm`\n\nExample: `bounds 0mm, 0mm to 50mm, 30mm`',

  stackup: 'Defines the board layer stackup configuration.\n\nUsed inside `board { }` block for specifying copper, dielectric, and solder mask layers.',

  description: 'Human-readable description.\n\nSyntax: `description "<text>"`\n\nExample: `description "SOT-23 footprint"`\n\nUsed inside `footprint { }` block.',

  pad: 'Defines a pad in a custom footprint.\n\nSyntax: `pad <number> <shape> at <x>mm, <y>mm size <w>mm x <h>mm`\n\nShapes: rect, circle, roundrect, oblong\n\nExample: `pad 1 rect at 0mm, 0mm size 1.2mm x 0.6mm`\n\nUsed inside `footprint { }` block.',

  courtyard: 'Component courtyard boundary for placement clearance DRC.\n\nSyntax: `courtyard <width>mm x <height>mm`\n\nExample: `courtyard 5mm x 5mm`\n\nUsed inside `footprint { }` block.',

  path: 'Explicit polyline trace geometry.\n\nSyntax: `path <x1>mm,<y1>mm -> <x2>mm,<y2>mm [-> ...]`\n\nExample: `path 10mm,20mm -> 30mm,20mm -> 30mm,40mm`\n\nUsed inside `trace { }` block. Each `->` adds a waypoint.',

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
 * Documentation includes full syntax format, examples, and usage context.
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
