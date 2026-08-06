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

  // Resolve ambiguities between overlapping rules
  conflicts: $ => [
    [$.dimension, $.assert_operand],
  ],

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
      $.module_definition,
      $.module_instance,
      $.interface_definition,
      $.import_statement,
      $.assert_statement,
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

    // value "330" or value 10kohm
    value_property: $ => seq(
      'value',
      field('value', choice($.string, $.physical_value)),
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
      $.trace_path,
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

    // via 5mm, 8mm [drill 0.3mm] (waypoint position with optional drill size)
    trace_via: $ => seq(
      'via',
      field('x', $.dimension),
      ',',
      field('y', $.dimension),
      optional(seq('drill', field('drill', $.dimension))),
    ),

    // path 10mm,12mm -> 15mm,12mm -> 15mm,8mm (explicit polyline geometry)
    trace_path: $ => seq(
      'path',
      $.path_point,
      repeat(seq('->', $.path_point)),
    ),

    path_point: $ => seq(
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

    // ========================================================================
    // DSL v2: Modules, Interfaces, Imports, Asserts, Physical Units
    // ========================================================================

    // import "path" | import Name from "path" | import Name1, Name2 from "path"
    import_statement: $ => choice(
      seq('import', field('path', $.string)),
      seq('import', field('names', $.import_name_list), 'from', field('path', $.string)),
    ),

    import_name_list: $ => seq(
      $.identifier,
      repeat(seq(',', $.identifier)),
    ),

    // module Name { definitions... pin declarations... }
    module_definition: $ => seq(
      'module',
      field('name', $.identifier),
      '{',
      repeat(choice($._module_body_item)),
      '}',
    ),

    _module_body_item: $ => choice(
      $.component_definition,
      $.net_definition,
      $.pin_declaration,
      $.assert_statement,
      // A module can be built from other modules.
      $.module_instance,
    ),

    // use Module as Name [at X, Y] [rotate A] { PIN = net, ... }
    //
    // Instantiates a module: its components are placed with the instance name
    // as a prefix, and each of its exposed pins is wired to a net in the
    // enclosing design.
    module_instance: $ => seq(
      'use',
      field('module', $.identifier),
      'as',
      field('name', $.identifier),
      optional(field('position', $.position_property)),
      optional(field('rotation', $.rotation_property)),
      '{',
      repeat($.port_connection),
      '}',
    ),

    // PIN = net
    port_connection: $ => seq(
      field('pin', $.identifier),
      '=',
      field('net', $.identifier),
    ),

    // interface Name { pin declarations... }
    interface_definition: $ => seq(
      'interface',
      field('name', $.identifier),
      '{',
      repeat($.pin_declaration),
      '}',
    ),

    // pin Name
    pin_declaration: $ => seq(
      'pin',
      field('name', $.identifier),
    ),

    // assert expression
    assert_statement: $ => seq(
      'assert',
      field('expression', $.assert_expression),
    ),

    assert_expression: $ => choice(
      $.assert_comparison,
      $.assert_within,
    ),

    // expr op expr (e.g., R1.value >= 10kohm)
    assert_comparison: $ => seq(
      field('left', $.assert_operand),
      field('op', $.comparison_operator),
      field('right', $.assert_operand),
    ),

    // expr within value +/- tolerance (e.g., R1.value within 10kohm +/- 5%)
    assert_within: $ => seq(
      field('left', $.assert_operand),
      'within',
      field('target', $.physical_value),
    ),

    assert_operand: $ => choice(
      $.qualified_name,
      $.physical_value,
      $.dimension,
      $.number,
    ),

    // Dotted name: R1.value, board.layers, etc.
    qualified_name: $ => seq(
      $.identifier,
      repeat1(seq('.', $.identifier)),
    ),

    comparison_operator: $ => choice('==', '!=', '>=', '<=', '>', '<'),

    // Physical value: number + physical_unit + optional tolerance
    physical_value: $ => seq(
      field('value', $.number),
      field('unit', $.physical_unit),
      optional(field('tolerance', $.tolerance)),
    ),

    // Tolerance: +/- N% | +/- N unit | to physical_value
    tolerance: $ => choice(
      $.tolerance_plus_minus,
      $.tolerance_range,
    ),

    // +/- 5% or +/- 0.1V
    tolerance_plus_minus: $ => seq(
      '+/-',
      field('value', $.number),
      field('kind', choice('%', $.physical_unit)),
    ),

    // to physical_value (e.g., 100nF to 220nF)
    tolerance_range: $ => seq(
      'to',
      field('upper', seq($.number, $.physical_unit)),
    ),

    // All electrical unit suffixes
    physical_unit: $ => choice(
      // Resistance
      'ohm', 'kohm', 'Mohm',
      // Capacitance
      'pF', 'nF', 'uF', 'mF',
      // Inductance
      'nH', 'uH', 'mH', 'H',
      // Voltage
      'mV', 'V', 'kV',
      // Current. The note that used to sit here said mA and A were "already
      // handled by current_unit"; they were not listed, so `assert X >= 1A`
      // would not parse while `current 2A` did.
      'uA', 'mA', 'A',
      // Frequency
      'Hz', 'kHz', 'MHz', 'GHz',
      // Power
      'mW', 'W',
    ),
  },
});
