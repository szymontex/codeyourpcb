/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

/**
 * Tree-sitter grammar for CodeYourPCB DSL
 *
 * Syntax overview:
 *   version 1
 *   board name { size WxH, layers N }
 *   component REFDES type "footprint" { value "V", at X,Y }
 *   net NAME { pin.refs }
 */
module.exports = grammar({
  name: 'cypcb',

  // Whitespace and comments can appear anywhere
  extras: $ => [
    /\s/,
    $.line_comment,
    $.block_comment,
  ],

  // Reserved words for keyword optimization
  word: $ => $.identifier,

  rules: {
    // Entry point: optional version followed by definitions
    source_file: $ => seq(
      optional($.version_statement),
      repeat($._definition),
    ),

    // version 1
    version_statement: $ => seq(
      'version',
      field('number', $.number),
    ),

    // Top-level definitions
    _definition: $ => choice(
      $.board_definition,
      $.component_definition,
      $.net_definition,
    ),

    // board name { properties }
    board_definition: $ => seq(
      'board',
      field('name', $.identifier),
      '{',
      repeat($.board_property),
      '}',
    ),

    board_property: $ => choice(
      $.size_property,
      $.layers_property,
      $.stackup_property,
    ),

    // size 30mm x 20mm
    size_property: $ => seq(
      'size',
      field('width', $.dimension),
      'x',
      field('height', $.dimension),
    ),

    // layers 2
    layers_property: $ => seq(
      'layers',
      field('count', $.number),
    ),

    // stackup { ... } (placeholder for future)
    stackup_property: $ => seq(
      'stackup',
      '{',
      repeat($.stackup_layer),
      '}',
    ),

    stackup_layer: $ => seq(
      field('layer_type', choice('copper', 'prepreg', 'core', 'mask', 'silk')),
      optional(field('thickness', $.dimension)),
    ),

    // component R1 resistor "0402" { ... }
    component_definition: $ => seq(
      'component',
      field('refdes', $.identifier),
      field('type', $.component_type),
      field('footprint', $.string),
      '{',
      repeat($._component_property),
      '}',
    ),

    component_type: $ => choice(
      'resistor',
      'capacitor',
      'inductor',
      'ic',
      'led',
      'connector',
      'diode',
      'transistor',
      'crystal',
      'generic',
    ),

    _component_property: $ => choice(
      $.value_property,
      $.position_property,
      $.rotation_property,
      $.net_assignment,
    ),

    // value "330"
    value_property: $ => seq(
      'value',
      field('value', $.string),
    ),

    // at 10mm, 8mm
    position_property: $ => seq(
      'at',
      field('x', $.dimension),
      ',',
      field('y', $.dimension),
    ),

    // rotate 90 (or rotate 90deg, rotate 90degrees)
    rotation_property: $ => seq(
      'rotate',
      field('angle', $.number),
      optional(field('unit', choice('deg', 'degrees'))),
    ),

    // pin.1 = NET_NAME (inline net assignment in component)
    net_assignment: $ => seq(
      field('pin', $.pin_identifier),
      '=',
      field('net', $.identifier),
    ),

    // net VCC { J1.1, R1.1 }
    net_definition: $ => seq(
      'net',
      field('name', $.identifier),
      optional($.net_constraint_block),
      '{',
      optional($.pin_ref_list),
      '}',
    ),

    // Optional constraint block: net VCC [width 0.3mm] { ... }
    net_constraint_block: $ => seq(
      '[',
      repeat($.net_constraint),
      ']',
    ),

    net_constraint: $ => choice(
      $.width_constraint,
      $.clearance_constraint,
    ),

    // width 0.3mm
    width_constraint: $ => seq(
      'width',
      field('value', $.dimension),
    ),

    // clearance 0.2mm
    clearance_constraint: $ => seq(
      'clearance',
      field('value', $.dimension),
    ),

    // Comma-separated list of pin references
    pin_ref_list: $ => seq(
      $.pin_ref,
      repeat(seq(
        optional(','),
        $.pin_ref,
      )),
    ),

    // J1.1 or J1.VCC (component.pin)
    pin_ref: $ => seq(
      field('component', $.identifier),
      '.',
      field('pin', $.pin_identifier),
    ),

    // Pin can be a number or identifier (1, VCC, anode, cathode)
    pin_identifier: $ => choice(
      $.number,
      $.identifier,
    ),

    // Dimension: number with optional unit (10mm, 100mil, 1in, 1000nm)
    dimension: $ => seq(
      field('value', $.number),
      optional(field('unit', $.unit)),
    ),

    // Units
    unit: $ => choice('mm', 'mil', 'in', 'nm'),

    // Terminals
    identifier: $ => /[a-zA-Z_][a-zA-Z0-9_]*/,

    // Numbers: integers or decimals
    number: $ => /\d+(\.\d+)?/,

    // Strings: double-quoted
    string: $ => seq(
      '"',
      /[^"]*/,
      '"',
    ),

    // Comments
    line_comment: $ => token(seq('//', /.*/)),

    block_comment: $ => token(seq(
      '/*',
      /[^*]*\*+([^/*][^*]*\*+)*/,
      '/',
    )),
  },
});
