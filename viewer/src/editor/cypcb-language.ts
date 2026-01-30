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
  keywords: [
    'version', 'board', 'component', 'net', 'footprint', 'trace',
    'zone', 'keepout', 'resistor', 'capacitor', 'ic', 'connector',
    'diode', 'transistor', 'led', 'crystal', 'inductor', 'generic'
  ],

  properties: [
    'size', 'layers', 'value', 'at', 'rotate', 'pin', 'width',
    'clearance', 'current', 'from', 'to', 'via', 'layer', 'locked',
    'bounds', 'stackup', 'description', 'pad', 'courtyard'
  ],

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
