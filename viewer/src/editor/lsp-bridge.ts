/**
 * LSP-like bridge for Monaco editor
 *
 * Two-phase context detection (inspired by GraphQL Language Service + Prisma):
 *   Phase 1: Block context — which block `{ }` or `[ ]` contains the cursor
 *   Phase 2: Inline slot — what token position is the cursor at on THIS LINE
 *
 * This gives precise, position-aware completions: only show what's valid HERE.
 */

import type { ViolationInfo } from '../types';

/** A parse or sync message with the line and column the engine says it is on. */
export interface SourceDiagnostic {
  message: string;
  line: number;
  column: number;
  end_line: number;
  end_column: number;
}

// ============================================================================
// Diagnostics (EDIT-03)
// ============================================================================

export function updateDiagnostics(
  monaco: typeof import('monaco-editor'),
  editor: any,
  diagnostics: SourceDiagnostic[],
  violations: ViolationInfo[]
): void {
  const model = editor.getModel();
  if (!model) return;

  const markers: any[] = [];

  // Where the engine says the fault is, not where a regex guessed.
  //
  // This read the line out of the message with /[Ll]ine\s+(\d+)/ and fell back
  // to 1. No parse or sync message writes the word "line" - the location lives
  // in a span the engine used to drop at the boundary - so every squiggle
  // landed on line 1 whatever line the fault was on.
  for (const d of diagnostics) {
    const line = Math.max(1, Math.min(d.line, model.getLineCount()));
    const endLine = Math.max(line, Math.min(d.end_line, model.getLineCount()));
    markers.push({
      severity: monaco.MarkerSeverity.Error,
      message: d.message,
      startLineNumber: line,
      startColumn: Math.max(1, d.column),
      endLineNumber: endLine,
      endColumn: Math.max(d.column + 1, d.end_column),
    });
  }

  for (const v of violations) {
    markers.push({
      severity: monaco.MarkerSeverity.Warning,
      message: `[DRC ${v.kind}] ${v.message}`,
      startLineNumber: 1, startColumn: 1,
      endLineNumber: 1, endColumn: model.getLineMaxColumn(1),
    });
  }

  monaco.editor.setModelMarkers(model, 'cypcb', markers);
}

// ============================================================================
// Phase 1: Block Context — which block contains the cursor?
// ============================================================================

type BlockContext =
  | 'top-level'
  | 'board'
  | 'component'
  | 'net-constraint'
  | 'net-pins'
  | 'trace'
  | 'footprint'
  | 'zone';

function detectBlockContext(model: any, position: any): BlockContext {
  let braceDepth = 0;
  let bracketDepth = 0;

  for (let lineNum = position.lineNumber; lineNum >= 1; lineNum--) {
    const lineText = lineNum === position.lineNumber
      ? model.getLineContent(lineNum).substring(0, position.column - 1)
      : model.getLineContent(lineNum);

    for (let i = lineText.length - 1; i >= 0; i--) {
      const ch = lineText[i];
      if (ch === '}') braceDepth++;
      else if (ch === '{') {
        if (braceDepth > 0) { braceDepth--; }
        else {
          const fullLine = model.getLineContent(lineNum);
          const before = fullLine.substring(0, i).trim();
          if (/^board\b/.test(before)) return 'board';
          if (/^component\b/.test(before)) return 'component';
          if (/^net\b/.test(before)) return 'net-pins';
          if (/^trace\b/.test(before)) return 'trace';
          if (/^footprint\b/.test(before)) return 'footprint';
          if (/^(zone|keepout)\b/.test(before)) return 'zone';
        }
      } else if (ch === ']') bracketDepth++;
      else if (ch === '[') {
        if (bracketDepth > 0) { bracketDepth--; }
        else {
          const fullLine = model.getLineContent(lineNum);
          const before = fullLine.substring(0, i).trim();
          if (/^net\b/.test(before)) return 'net-constraint';
        }
      }
    }
  }
  return 'top-level';
}

// ============================================================================
// Phase 2: Inline Slot — what token position is the cursor at?
// ============================================================================

type InlineSlot =
  | 'top-keyword'
  | 'component-refdes'
  | 'component-type'
  | 'block-property'
  | 'layer-value'
  | 'after-number'
  | 'pin-reference'
  | 'pin-number'
  | 'nothing';

function detectInlineSlot(beforeCursor: string, block: BlockContext): InlineSlot {
  const trimmed = beforeCursor.trimStart();

  // Inside a string → no completions
  let inString = false;
  for (const ch of beforeCursor) if (ch === '"') inString = !inString;
  if (inString) return 'nothing';

  // After a number → suggest units (only inside blocks)
  if (/\d+(\.\d+)?\s*$/.test(trimmed) && block !== 'top-level') return 'after-number';

  // Inside a block
  if (block !== 'top-level') {
    // After "R1." → pin numbers
    if (/\b\w+\.\s*$/.test(trimmed)) return 'pin-number';

    // After "layer " → layer names
    if (/\blayer\s+$/i.test(trimmed)) return 'layer-value';

    // Net pins context
    if (block === 'net-pins') return 'pin-reference';

    // Line has property keyword + value already → nothing
    const words = trimmed.split(/\s+/).filter(Boolean);
    if (words.length >= 2) {
      const PROPS = new Set(['size','layers','value','at','rotate','lcsc','width',
        'clearance','current','from','to','path','via','layer','bounds',
        'net','pad','courtyard','description','stackup','drill']);
      if (PROPS.has(words[0])) return 'nothing';
    }

    // Start of line or partial word → suggest block properties
    if (words.length <= 1) return 'block-property';
    return 'nothing';
  }

  // Top level
  const words = trimmed.split(/\s+/).filter(Boolean);
  if (words.length === 0) return 'top-keyword';

  switch (words[0]) {
    case 'component':
      if (words.length === 1 && beforeCursor.endsWith(' ')) return 'component-refdes';
      if (words.length === 2 && beforeCursor.endsWith(' ')) return 'component-type';
      if (words.length <= 1) return 'top-keyword';
      return 'nothing';
    case 'board': case 'net': case 'trace': case 'footprint': case 'zone': case 'keepout':
      return words.length >= 2 ? 'nothing' : 'top-keyword';
    default:
      return words.length <= 1 ? 'top-keyword' : 'nothing';
  }
}

// ============================================================================
// Helpers
// ============================================================================

function extractRefdes(model: any): string[] {
  const list: string[] = [];
  for (let i = 1; i <= model.getLineCount(); i++) {
    const m = model.getLineContent(i).match(/^\s*component\s+(\w+)/);
    if (m) list.push(m[1]);
  }
  return list;
}

function nextRefdes(model: any): string {
  const existing = extractRefdes(model);
  const maxByPrefix = new Map<string, number>();
  for (const r of existing) {
    const m = r.match(/^([A-Za-z]+)(\d+)$/);
    if (m) maxByPrefix.set(m[1], Math.max(maxByPrefix.get(m[1]) || 0, parseInt(m[2], 10)));
  }
  if (maxByPrefix.size === 0) return 'R1';
  // Use the prefix with the highest number (most recently added)
  let best = 'R'; let bestN = 0;
  for (const [p, n] of maxByPrefix) { if (n >= bestN) { best = p; bestN = n; } }
  return `${best}${bestN + 1}`;
}

/** Get properties already used in the current block (full block scan). */
function getUsedProperties(model: any, position: any): Set<string> {
  const used = new Set<string>();
  let braceDepth = 0;
  let blockStartLine = -1;

  // Find block opening
  for (let ln = position.lineNumber; ln >= 1; ln--) {
    const text = ln === position.lineNumber
      ? model.getLineContent(ln).substring(0, position.column - 1)
      : model.getLineContent(ln);
    for (let i = text.length - 1; i >= 0; i--) {
      const ch = text[i];
      if (ch === '}' || ch === ']') braceDepth++;
      else if (ch === '{' || ch === '[') {
        if (braceDepth > 0) braceDepth--;
        else { blockStartLine = ln; break; }
      }
    }
    if (blockStartLine !== -1) break;
  }
  if (blockStartLine === -1) return used;

  // Find block closing
  let blockEndLine = model.getLineCount();
  let depth = 0;
  for (let ln = blockStartLine; ln <= model.getLineCount(); ln++) {
    const line = model.getLineContent(ln);
    for (const ch of line) {
      if (ch === '{' || ch === '[') depth++;
      else if (ch === '}' || ch === ']') { depth--; if (depth === 0) { blockEndLine = ln; break; } }
    }
    if (depth === 0 && ln >= blockStartLine) break;
  }

  // Collect first-word of each line in the block
  for (let ln = blockStartLine; ln <= blockEndLine; ln++) {
    let line = model.getLineContent(ln);
    if (ln === blockStartLine) {
      const idx = Math.max(line.indexOf('{'), line.indexOf('['));
      if (idx >= 0) line = line.substring(idx + 1);
    }
    if (ln === position.lineNumber) continue; // Skip cursor line
    line = line.trim();
    if (!line || line.startsWith('//') || line === '}' || line === ']') continue;
    for (const stmt of line.split(/[;]/)) {
      const w = stmt.trim().match(/^(\w+)/);
      if (w) used.add(w[1]);
    }
  }
  return used;
}

// ============================================================================
// Phase 3: Completion Provider
// ============================================================================

const BLOCK_PROPERTIES: Record<string, { label: string; snippet: string; detail: string }[]> = {
  board: [
    { label: 'size', snippet: 'size ${1:100}mm x ${2:80}mm', detail: 'Board dimensions' },
    { label: 'layers', snippet: 'layers ${1|2,4,6|}', detail: 'Copper layer count' },
    { label: 'stackup', snippet: 'stackup {\n\t$0\n}', detail: 'Layer stackup' },
  ],
  component: [
    { label: 'value', snippet: 'value "${1:10k}"', detail: 'Component value' },
    { label: 'at', snippet: 'at ${1:10}mm, ${2:20}mm', detail: 'Position on board' },
    { label: 'rotate', snippet: 'rotate ${1|0,90,180,270|}', detail: 'Rotation (degrees)' },
    { label: 'lcsc', snippet: 'lcsc "${1:C12345}"', detail: 'JLCPCB/LCSC part number' },
  ],
  'net-constraint': [
    { label: 'width', snippet: 'width ${1:0.25}mm', detail: 'Trace width constraint' },
    { label: 'clearance', snippet: 'clearance ${1:0.15}mm', detail: 'Min clearance' },
    { label: 'current', snippet: 'current ${1:500}mA', detail: 'Current → IPC-2221 auto-width' },
  ],
  trace: [
    { label: 'layer', snippet: 'layer ${1|Top,Bottom,Inner1,Inner2|}', detail: 'Copper layer' },
    { label: 'width', snippet: 'width ${1:0.25}mm', detail: 'Trace width' },
    { label: 'from', snippet: 'from ${1:R1}.${2:1}', detail: 'Start pin' },
    { label: 'to', snippet: 'to ${1:R1}.${2:2}', detail: 'End pin' },
    { label: 'path', snippet: 'path ${1:10}mm,${2:20}mm -> ${3:30}mm,${4:20}mm', detail: 'Polyline geometry' },
    { label: 'via', snippet: 'via ${1:15}mm,${2:20}mm drill ${3:0.3}mm', detail: 'Via + drill size' },
    { label: 'locked', snippet: 'locked', detail: 'Prevent autorouter modification' },
  ],
  footprint: [
    { label: 'description', snippet: 'description "${1:text}"', detail: 'Description' },
    { label: 'pad', snippet: 'pad ${1:1} ${2|rect,circle,roundrect,oblong|} at ${3:0}mm, ${4:0}mm size ${5:1}mm x ${6:1}mm', detail: 'Pad definition' },
    { label: 'courtyard', snippet: 'courtyard ${1:5}mm x ${2:5}mm', detail: 'Courtyard boundary' },
  ],
  zone: [
    { label: 'bounds', snippet: 'bounds ${1:0}mm, ${2:0}mm to ${3:50}mm, ${4:30}mm', detail: 'Zone boundary' },
    { label: 'layer', snippet: 'layer ${1|top,bottom,all|}', detail: 'Copper layer' },
    { label: 'net', snippet: 'net ${1:GND}', detail: 'Net for copper pour' },
  ],
};

const SINGULAR = new Set([
  'size','layers','stackup','value','at','rotate','lcsc','locked','width',
  'clearance','current','from','to','description','courtyard',
]);

const COMPONENT_TYPES = [
  'resistor','capacitor','ic','connector','diode','led','transistor','crystal','inductor','generic',
];

export function registerCompletionProvider(monaco: typeof import('monaco-editor')): void {
  monaco.languages.registerCompletionItemProvider('cypcb', {
    triggerCharacters: ['.', ' ', '\n', '{', '['],

    provideCompletionItems: (model, position) => {
      const word = model.getWordUntilPosition(position);
      const range = {
        startLineNumber: position.lineNumber, endLineNumber: position.lineNumber,
        startColumn: word.startColumn, endColumn: word.endColumn,
      };
      const S = monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet;
      const lineContent = model.getLineContent(position.lineNumber);
      const beforeCursor = lineContent.substring(0, position.column - 1);

      const block = detectBlockContext(model, position);
      const slot = detectInlineSlot(beforeCursor, block);
      const suggestions: any[] = [];

      switch (slot) {
        case 'nothing':
          return { suggestions: [], incomplete: true };

        case 'top-keyword': {
          const nr = nextRefdes(model);
          const items = [
            { label: 'version', insert: 'version ${1:1}', detail: 'File format version' },
            { label: 'board', insert: 'board ${1:myboard} {\n\tsize ${2:100}mm x ${3:80}mm\n\tlayers ${4:2}\n}', detail: 'Board definition' },
            { label: 'component', insert: `component \${1:${nr}} \${2|resistor,capacitor,ic,connector,diode,led,transistor,crystal,inductor,generic|} "\${3:0402}" {\n\tvalue "\${4:10k}"\n\tat \${5:10}mm, \${6:20}mm\n}`, detail: 'Component placement' },
            { label: 'net', insert: 'net ${1:VCC} {\n\t${2:R1.1}\n}', detail: 'Electrical net' },
            { label: 'trace', insert: 'trace ${1:VCC} {\n\tlayer ${2|Top,Bottom|}\n\twidth ${3:0.25}mm\n\tpath ${4:10}mm,${5:20}mm -> ${6:30}mm,${7:20}mm\n}', detail: 'Copper trace' },
            { label: 'footprint', insert: 'footprint ${1:name} {\n\tpad ${2:1} ${3|rect,circle|} at ${4:0}mm, ${5:0}mm size ${6:1}mm x ${7:1}mm\n}', detail: 'Custom footprint' },
            { label: 'zone', insert: 'zone ${1:GND_pour} {\n\tbounds ${2:0}mm, ${3:0}mm to ${4:50}mm, ${5:30}mm\n\tlayer ${6|bottom,top,all|}\n\tnet ${7:GND}\n}', detail: 'Copper zone' },
            { label: 'keepout', insert: 'keepout ${1:name} {\n\tbounds ${2:0}mm, ${3:0}mm to ${4:10}mm, ${5:10}mm\n}', detail: 'Keepout area' },
            { label: 'import', insert: 'import "${1:path.cypcb}"', detail: 'Import module (v2)' },
            { label: 'assert', insert: 'assert ${1:R1.value} ${2|>=,<=,==|} ${3:10kohm}', detail: 'Design assertion (v2)' },
          ];
          for (let i = 0; i < items.length; i++) {
            suggestions.push({
              label: items[i].label, detail: items[i].detail,
              kind: monaco.languages.CompletionItemKind.Snippet,
              insertText: items[i].insert, insertTextRules: S,
              range, sortText: String(i).padStart(2, '0'),
            });
          }
          break;
        }

        case 'component-refdes': {
          // Suggest next refdes for each prefix
          const prefixes = ['R', 'C', 'U', 'J', 'D', 'L', 'Q', 'LED'];
          for (const p of prefixes) {
            const existing = extractRefdes(model);
            let max = 0;
            for (const r of existing) {
              const m = r.match(new RegExp(`^${p}(\\d+)$`));
              if (m) max = Math.max(max, parseInt(m[1], 10));
            }
            const next = `${p}${max + 1}`;
            suggestions.push({
              label: next, detail: `Next ${p} refdes`,
              kind: monaco.languages.CompletionItemKind.Value,
              insertText: next, range, sortText: `0_${next}`,
            });
          }
          break;
        }

        case 'component-type':
          for (const t of COMPONENT_TYPES) {
            suggestions.push({
              label: t, detail: `Component type`,
              kind: monaco.languages.CompletionItemKind.Class,
              insertText: t, range,
            });
          }
          break;

        case 'block-property': {
          const props = BLOCK_PROPERTIES[block] || [];
          const used = getUsedProperties(model, position);
          for (let i = 0; i < props.length; i++) {
            if (SINGULAR.has(props[i].label) && used.has(props[i].label)) continue;
            suggestions.push({
              label: props[i].label, detail: props[i].detail,
              kind: monaco.languages.CompletionItemKind.Snippet,
              insertText: props[i].snippet, insertTextRules: S,
              range, sortText: String(i).padStart(2, '0'),
            });
          }
          break;
        }

        case 'layer-value':
          for (const l of ['Top', 'Bottom', 'Inner1', 'Inner2', 'Inner3', 'Inner4']) {
            suggestions.push({
              label: l, detail: `Copper layer`,
              kind: monaco.languages.CompletionItemKind.Enum,
              insertText: l, range,
            });
          }
          break;

        case 'after-number':
          for (const u of [
            { l: 'mm', d: 'Millimeters' }, { l: 'mil', d: 'Mils' },
            { l: 'mA', d: 'Milliamps' }, { l: 'A', d: 'Amps' },
          ]) {
            suggestions.push({
              label: u.l, detail: u.d,
              kind: monaco.languages.CompletionItemKind.Unit,
              insertText: u.l, range,
            });
          }
          break;

        case 'pin-reference': {
          const refs = extractRefdes(model);
          for (const r of refs) {
            suggestions.push({
              label: r, detail: `Component ${r}`,
              kind: monaco.languages.CompletionItemKind.Value,
              insertText: `${r}.\${1:1}`, insertTextRules: S,
              range,
            });
          }
          break;
        }

        case 'pin-number':
          for (let p = 1; p <= 8; p++) {
            suggestions.push({
              label: String(p), detail: `Pin ${p}`,
              kind: monaco.languages.CompletionItemKind.Value,
              insertText: String(p), range,
              sortText: String(p).padStart(2, '0'),
            });
          }
          break;
      }

      return { suggestions, incomplete: true };
    },
  });
}

// ============================================================================
// Hover
// ============================================================================

const KEYWORD_DOCS: Record<string, string> = {
  version: 'File format version.\n\nSyntax: `version <number>`\n\nExample: `version 1`',
  board: 'Board definition.\n\nSyntax:\n```\nboard <name> {\n  size <w>mm x <h>mm\n  layers <count>\n}\n```',
  component: 'Component placement.\n\nSyntax:\n```\ncomponent <refdes> <type> "<footprint>" {\n  value "<val>"\n  at <x>mm, <y>mm\n  rotate <deg>\n}\n```\n\nTypes: resistor, capacitor, ic, connector, diode, led, transistor, crystal, inductor, generic',
  net: 'Electrical net.\n\nSyntax:\n```\nnet <name> [width 0.25mm  current 2A] {\n  R1.1\n  C1.1\n}\n```',
  trace: 'Copper trace.\n\nSyntax:\n```\ntrace <net> {\n  layer Top\n  width 0.25mm\n  path 10mm,20mm -> 30mm,20mm\n}\n```',
  footprint: 'Custom footprint.\n\nSyntax:\n```\nfootprint <name> {\n  pad 1 rect at 0mm,0mm size 1mm x 1mm\n}\n```',
  zone: 'Copper zone (pour).\n\nSyntax:\n```\nzone <name> {\n  bounds 0mm,0mm to 50mm,30mm\n  layer bottom\n  net GND\n}\n```',
  keepout: 'Keepout area.\n\nSyntax:\n```\nkeepout <name> {\n  bounds 0mm,0mm to 10mm,10mm\n}\n```',
  resistor: 'Resistor. Value in ohms: `"330"`, `"10k"`, `"4.7M"`',
  capacitor: 'Capacitor. Value in farads: `"100n"`, `"10u"`',
  ic: 'Integrated circuit. Footprint: `"SOIC-8"`, `"QFP-48"`',
  connector: 'Connector. Footprint: `"PIN-HDR-1x2"`, `"JST-XH-2"`',
  diode: 'Diode. Footprint: `"SOD-123"`, `"SMA"`',
  transistor: 'Transistor. Footprint: `"SOT-23"`, `"TO-252"`',
  led: 'LED. Footprint: `"0402"`, `"0805"`, `"5mm"`',
  crystal: 'Crystal oscillator. Footprint: `"HC49"`, `"3215"`',
  inductor: 'Inductor. Value: `"10u"`, `"100n"`',
  generic: 'Generic component type.',
  size: 'Board dimensions.\n\nSyntax: `size <w>mm x <h>mm`\n\nExample: `size 100mm x 80mm`',
  layers: 'Copper layer count.\n\nSyntax: `layers <2|4|6>`',
  value: 'Component value.\n\nSyntax: `value "<text>"`',
  at: 'Position.\n\nSyntax: `at <x>mm, <y>mm`',
  rotate: 'Rotation.\n\nSyntax: `rotate <0|90|180|270>`',
  lcsc: 'JLCPCB part.\n\nSyntax: `lcsc "<Cxxxxx>"`\n\nAuto-fetches footprint from EasyEDA.',
  width: 'Trace width.\n\nSyntax: `width <val>mm`\n\nIn net: `[width 0.5mm]`\nIn trace: `width 0.25mm`\n\nJLCPCB min: 0.127mm',
  clearance: 'Min clearance.\n\nSyntax: `clearance <val>mm`\n\nJLCPCB min: 0.127mm',
  current: 'Current rating → IPC-2221 auto trace width.\n\nSyntax: `current <val>mA` or `current <val>A`\n\nExample: `current 2A`',
  from: 'Trace start pin.\n\nSyntax: `from <refdes>.<pin>`',
  to: 'Trace end pin.\n\nSyntax: `to <refdes>.<pin>`',
  via: 'Via.\n\nSyntax: `via <x>mm,<y>mm drill <d>mm`',
  path: 'Trace polyline.\n\nSyntax: `path <x1>mm,<y1>mm -> <x2>mm,<y2>mm [-> ...]`',
  layer: 'Copper layer.\n\nTrace: `layer Top` / `layer Bottom`\nZone: `layer top` / `layer bottom` / `layer all`',
  locked: 'Prevent autorouter modification.\n\nSyntax: `locked`',
  bounds: 'Zone boundary.\n\nSyntax: `bounds <x1>mm,<y1>mm to <x2>mm,<y2>mm`',
  pad: 'Footprint pad.\n\nSyntax: `pad <n> <rect|circle|roundrect|oblong> at <x>mm,<y>mm size <w>mm x <h>mm`',
  courtyard: 'Courtyard.\n\nSyntax: `courtyard <w>mm x <h>mm`',
  description: 'Description.\n\nSyntax: `description "<text>"`',
  stackup: 'Layer stackup.\n\nSyntax: `stackup { copper ... prepreg ... }`',
  Top: 'Top copper layer (layer 1).',
  Bottom: 'Bottom copper layer (layer 2).',
  Inner1: 'Inner layer 1 (4+ layer boards).',
  Inner2: 'Inner layer 2 (4+ layer boards).',
  all: 'All layers.',
};

export function registerHoverProvider(monaco: typeof import('monaco-editor')): void {
  monaco.languages.registerHoverProvider('cypcb', {
    provideHover: (model, position) => {
      const word = model.getWordAtPosition(position);
      if (!word) return null;
      const doc = KEYWORD_DOCS[word.word];
      if (!doc) return null;
      return {
        range: new monaco.Range(position.lineNumber, word.startColumn, position.lineNumber, word.endColumn),
        contents: [{ value: `**${word.word}**` }, { value: doc }],
      };
    },
  });
}

// ============================================================================
// Provider Registration
// ============================================================================

let providersRegistered = false;
export function registerProviders(monaco: typeof import('monaco-editor')): void {
  if (providersRegistered) return;
  providersRegistered = true;
  registerCompletionProvider(monaco);
  registerHoverProvider(monaco);
  console.log('[LSP Bridge] Providers registered');
}
