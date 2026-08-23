/**
 * Monaco Monarch tokenizer for .cypcb language
 *
 * Provides syntax highlighting for CodeYourPCB DSL files.
 * Token types map to Monaco's built-in theme colors.
 */

// Import Monaco types without loading the module
import type * as monaco from 'monaco-editor';

/**
 * Monarch language definition for .cypcb files
 *
 * Token categories:
 * - keyword: Board structure (board, component, net, etc.)
 * - type: Properties and layer names
 * - comment: Line comments (//)
 * - string: Quoted strings
 * - number: Numeric values with optional units
 * - variable: Pin references (R1.1, C2.2, etc.)
 * - delimiter: Braces, parens, operators
 */
export const cypcbLanguage: monaco.languages.IMonarchLanguage = {
  // Every construct the language has. `the-editor-knows-every-keyword` reads
  // the grammar and fails when this list falls behind it - which it had, by
  // three whole constructs: `outline`, `netclass` and `diffpair` were written
  // in plain text while `board` and `net` were coloured, so the editor said
  // they were not part of the language.
  keywords: [
    'version', 'board', 'outline', 'component', 'net', 'netclass', 'diffpair',
    'footprint', 'trace', 'zone', 'keepout', 'flex',
    'resistor', 'capacitor', 'ic', 'connector',
    'diode', 'transistor', 'led', 'crystal', 'inductor', 'generic',
    // v2 keywords
    'module', 'use', 'interface', 'import', 'assert', 'within'
  ],

  properties: [
    'size', 'layers', 'value', 'at', 'rotate', 'pin', 'lcsc', 'width',
    'clearance', 'current', 'from', 'to', 'path', 'via', 'layer', 'locked',
    'bounds', 'stackup', 'description', 'pad', 'courtyard', 'silk', 'point',
    'implements', 'drill', 'radius', 'as'
  ],

  layerNames: ['Top', 'Bottom', 'Inner1', 'Inner2', 'Inner3', 'Inner4', 'all'],

  tokenizer: {
    root: [
      // Comments
      [/\/\/.*$/, 'comment'],

      // Strings
      [/"[^"]*"/, 'string'],

      // Numbers with units (physical units + dimensional units)
      [/\d+(\.\d+)?(mm|mil|kohm|Mohm|ohm|pF|nF|uF|mF|nH|uH|mH|Hz|kHz|MHz|GHz|mV|kV|uA|mA|mW|W|H|V|A|%)/, 'number'],
      [/\d+(\.\d+)?/, 'number'],

      // Keywords, properties, and layer names
      [/[a-zA-Z_]\w*/, {
        cases: {
          '@keywords': 'keyword',
          '@properties': 'type',
          '@layerNames': 'type.identifier',
          '@default': 'identifier'
        }
      }],

      // Pin references (R1.1, C2.2, IC1.3, etc.)
      [/[A-Z][A-Z0-9]*\.\d+/, 'variable'],

      // Tolerance operator
      [/\+\/\-/, 'operator'],

      // Comparison operators
      [/[><=!]=|[><]/, 'operator'],

      // Delimiters and operators
      [/[{}()=,x]/, 'delimiter'],
    ]
  }
};

/**
 * Language configuration for .cypcb files
 *
 * Defines:
 * - Comment syntax
 * - Bracket pairs for matching
 * - Auto-closing pairs for typing
 * - Code folding markers
 */
export const cypcbLanguageConfig: monaco.languages.LanguageConfiguration = {
  comments: {
    lineComment: '//',
  },
  brackets: [
    ['{', '}'],
  ],
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
