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
      $.footprint_definition,
      $.zone_definition,
      $.trace_definition,
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
      $.current_constraint,
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

    // current 500mA or current 2A
    current_constraint: $ => seq(
      'current',
      field('value', $.current_value),
    ),

    // Current value with unit (mA or A)
    current_value: $ => seq(
      field('amount', $.number),
      field('unit', $.current_unit),
    ),

    current_unit: $ => choice('mA', 'A'),

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

    // Dimension: number with optional unit (10mm, 100mil, 1in, 1000nm, -5mm)
    dimension: $ => seq(
      optional(field('sign', '-')),
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

    // footprint NAME { ... }
    footprint_definition: $ => seq(
      'footprint',
      field('name', $.identifier),
      '{',
      repeat($.footprint_property),
      '}',
    ),

    footprint_property: $ => choice(
      $.description_property,
      $.pad_definition,
      $.courtyard_property,
    ),

    // description "text"
    description_property: $ => seq(
      'description',
      field('text', $.string),
    ),

    // pad N shape at X, Y size W x H [drill D]
    pad_definition: $ => seq(
      'pad',
      field('number', $.number),
      field('shape', $.pad_shape),
      'at',
      field('x', $.dimension),
      ',',
      field('y', $.dimension),
      'size',
      field('width', $.dimension),
      'x',
      field('height', $.dimension),
      optional(field('drill', $.drill_spec)),
    ),

    drill_spec: $ => seq('drill', $.dimension),

    pad_shape: $ => choice('rect', 'circle', 'roundrect', 'oblong'),

    // courtyard W x H
    courtyard_property: $ => seq(
      'courtyard',
      field('width', $.dimension),
      'x',
      field('height', $.dimension),
    ),

    // zone NAME { ... } or keepout NAME { ... }
    zone_definition: $ => seq(
      field('kind', choice('zone', 'keepout')),
      optional(field('name', $.identifier)),
      '{',
      repeat($.zone_property),
      '}',
    ),

    zone_property: $ => choice(
      $.zone_bounds,
      $.zone_layer,
      $.zone_net,
    ),

    // bounds X1, Y1 to X2, Y2
    zone_bounds: $ => seq(
      'bounds',
      field('min_x', $.dimension),
      ',',
      field('min_y', $.dimension),
      'to',
      field('max_x', $.dimension),
      ',',
      field('max_y', $.dimension),
    ),

    // layer top | bottom | all
    zone_layer: $ => seq('layer', field('name', $.layer_name)),

    layer_name: $ => choice('top', 'bottom', 'all'),

    // net NETNAME (for copper pour zones)
    zone_net: $ => seq('net', field('net', $.identifier)),

    // ========================================================================
    // Manual Trace Definitions
    // ========================================================================

    // trace NET_NAME { from PIN to PIN [via X, Y] [layer L] [width W] [locked] }
    trace_definition: $ => seq(
      'trace',
      field('net', $.identifier),
      '{',
      repeat($._trace_property),
      '}',
    ),

    _trace_property: $ => choice(
      $.trace_from,
      $.trace_to,
      $.trace_via,
      $.trace_layer,
      $.trace_width,
      $.trace_locked,
    ),

    // from R1.1
    trace_from: $ => seq(
      'from',
      field('pin', $.pin_ref),
    ),

    // to C1.1
    trace_to: $ => seq(
      'to',
      field('pin', $.pin_ref),
    ),

    // via 5mm, 8mm (waypoint position)
    trace_via: $ => seq(
      'via',
      field('x', $.dimension),
      ',',
      field('y', $.dimension),
    ),

    // layer Top or layer Bottom
    trace_layer: $ => seq(
      'layer',
      field('name', $.trace_layer_name),
    ),

    trace_layer_name: $ => choice('Top', 'Bottom', 'Inner1', 'Inner2', 'Inner3', 'Inner4'),

    // width 0.3mm
    trace_width: $ => seq(
      'width',
      field('value', $.dimension),
    ),

    // locked (keyword only, no value)
    trace_locked: $ => 'locked',
  },
});
