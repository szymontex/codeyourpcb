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
      $.outline_definition,
      $.component_definition,
      $.net_definition,
      $.netclass_definition,
      $.diffpair_definition,
      $.footprint_definition,
      $.zone_definition,
      $.trace_definition,
      $.module_definition,
      $.module_instance,
      $.interface_definition,
      $.import_statement,
      $.assert_statement,
    ),

    // outline { point 0mm, 0mm  point 40mm, 0mm  ... }
    //
    // The board's real edge, as a ring of points. Without one a board is the
    // rectangle its size describes, which cannot say cutout, slot or chamfer.
    outline_definition: $ => seq(
      'outline',
      '{',
      repeat($.outline_point),
      '}',
    ),

    outline_point: $ => seq(
      'point',
      field('x', $.dimension),
      ',',
      field('y', $.dimension),
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
      $.fab_property,
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

    // fab jlcpcb
    //
    // Which fabricator the board is for. Every rule the checker applies and
    // every clearance the router plans to comes from one of these tables, and
    // until this existed the board could not say which - the CLI took it as a
    // flag and the viewer hard-coded jlcpcb.
    fab_property: $ => seq(
      'fab',
      field('name', $.identifier),
    ),

    // stackup { ... } (placeholder for future)
    stackup_property: $ => seq(
      'stackup',
      '{',
      repeat(choice(
        $.stackup_layer,
        $.stackup_finish,
        $.stackup_edges,
        $.stackup_pads,
        $.stackup_connector,
        $.stackup_impedance,
        $.stackup_drill,
      )),
      '}',
    ),

    // What the fabricator does to the board rather than what it presses.
    //
    // KiCad keeps these five inside its own `(stackup ...)`, which is where
    // they belong: they are bought with the build, not drawn on a layer. Each
    // starts with a different word, so neither reader has to look ahead.
    //
    //   finish "ENIG"
    //   edges plated
    //   pads castellated
    //   connector bevelled
    //   impedance controlled
    //
    // The finish is quoted for the reason `material` is: a fabricator's word
    // for it is not this language's to spell, and there is no table here to
    // check one against.
    stackup_finish: $ => seq('finish', field('finish', $.string)),

    // The flags are a keyword pair rather than `yes`/`no`, the way `locked`
    // on a trace is: a design states what it wants, and silence is the rest.
    stackup_edges: $ => seq('edges', 'plated'),
    stackup_pads: $ => seq('pads', 'castellated'),
    stackup_connector: $ => seq(
      'connector',
      field('bevel', choice('plain', 'bevelled')),
    ),
    stackup_impedance: $ => seq('impedance', 'controlled'),

    // drill Top to Inner2
    //
    // A drill span this build makes. A board is drilled and plated once per
    // lamination cycle, and each cycle reaches only the layers pressed
    // together by then - so a blind or buried via belongs to a cycle rather
    // than sitting anywhere a designer likes. Altium calls these drill pairs.
    stackup_drill: $ => seq(
      'drill',
      field('start', $.trace_layer_name),
      'to',
      field('end', $.trace_layer_name),
    ),

    // copper "F.Cu" 0.035mm
    // core "dielectric 2" 1.095mm material "FR4" dk 4.5 df 0.02
    //
    // The name is quoted because a fabricator's canonical layer names carry a
    // dot - F.Cu, In1.Cu, B.Mask - which no identifier in this language may.
    stackup_layer: $ => seq(
      field('layer_type', choice(
        'copper', 'prepreg', 'core', 'mask', 'silk', 'paste',
        // Rigid-flex: the film over a bend, and the material bonded under one
        // to stop it bending.
        'coverlay', 'stiffener',
      )),
      optional(field('name', $.string)),
      optional(field('thickness', choice($.dimension, $.copper_weight))),
      optional(seq('material', field('material', $.string))),
      // A solder mask is green unless somebody says otherwise, and a house
      // charges for saying otherwise. Quoted like `material`: KiCad's list is
      // names plus a `#RRGGBB` custom form, and neither is this language's to
      // spell.
      optional(seq('color', field('color', $.string))),
      optional(seq('dk', field('dk', $.number))),
      optional(seq('df', field('df', $.number))),
      // The rest of the sheets this slot is pressed from. A fabricator hits a
      // target thickness with the prepreg they stock - two sheets of 0.0668mm
      // rather than one of 0.1336mm - and on six layers and up that is the
      // ordinary case. KiCad writes each one as `addsublayer`.
      repeat($.stackup_sheet),
    ),

    // sheet 0.0668mm material "FR4" dk 4.5
    stackup_sheet: $ => seq(
      'sheet',
      optional(field('thickness', $.dimension)),
      optional(seq('material', field('material', $.string))),
      optional(seq('dk', field('dk', $.number))),
      optional(seq('df', field('df', $.number))),
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
      $.lcsc_property,
      $.side_property,
      $.net_assignment,
      $.spec_property,
    ),

    // spec { output 3.3V  quiescent 25mA }
    //
    // Facts about the part that only its datasheet knows. The component block
    // itself stays strict - a misspelt property there is an error - and this
    // is where a design says something the language has no keyword for, so an
    // `assert` has something to read.
    spec_property: $ => seq(
      'spec',
      '{',
      repeat($.spec_entry),
      '}',
    ),

    spec_entry: $ => seq(
      field('name', $.identifier),
      field('value', $.physical_value),
    ),

    // side bottom
    //
    // Which face of the board the part is soldered to. Saying nothing means
    // the top, unless the footprint itself is bottom-only.
    side_property: $ => seq(
      'side',
      field('face', choice('top', 'bottom')),
    ),

    // lcsc "C7593"
    //
    // The part to buy. A footprint says what the pads look like; this says
    // which part goes on them, and it is what an assembly house is given.
    lcsc_property: $ => seq(
      'lcsc',
      field('part', $.string),
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
      field('net', $.net_name),
    ),

    // net VCC { J1.1, R1.1 }
    // Anywhere a net is named.
    //
    // A schematic names nets `VBUS+`, `3V3` and `D-`, none of which the
    // identifier rule accepts, so a board carrying any of them could not be
    // written down. Quoting is the way out and it has to be accepted at every
    // site that names a net rather than only at the declaration - a net you
    // can declare and cannot reference is not usable.
    net_name: $ => choice($.identifier, $.string),

    net_definition: $ => seq(
      'net',
      field('name', $.net_name),
      optional($.net_constraint_block),
      '{',
      optional($.pin_ref_list),
      '}',
    ),

    // netclass Power [width 0.5mm] { VCC BUS_5V }
    //
    // States a rule once for a group of nets instead of repeating it on each.
    netclass_definition: $ => seq(
      'netclass',
      field('name', $.identifier),
      optional($.net_constraint_block),
      '{',
      repeat(field('member', $.net_name)),
      '}',
    ),

    // diffpair USB { USB_DP USB_DM }
    //
    // Two nets that carry one signal between them. What makes them a pair is
    // that they have to stay the same length: the receiver reads the
    // difference, and copper one net runs and the other does not is skew.
    diffpair_definition: $ => seq(
      'diffpair',
      field('name', $.identifier),
      '{',
      field('positive', $.net_name),
      field('negative', $.net_name),
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
      $.impedance_constraint,
      $.neck_constraint,
    ),

    // neck 0.8mm for 4mm, on a net rather than on one trace
    //
    // The case this was asked for is a netclass: `netclass Mains [current
    // 10A]` gives copper millimetres wide and a 2.54mm pad pitch has nowhere
    // to put it. Saying it once on the net is the difference between a rule
    // and a note repeated on every trace of it.
    //
    // Same shape as `trace_neck` and a separate rule on purpose: the two live
    // in different blocks, and one rule reachable from both would let `neck`
    // appear where the reader does not look for it.
    neck_constraint: $ => seq(
      'neck',
      field('width', $.dimension),
      'for',
      field('length', $.dimension),
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

    // impedance 90ohm
    //
    // What the net is meant to present to the signal on it. The unit is
    // written out because a bare number here would read as a width.
    impedance_constraint: $ => seq(
      'impedance',
      field('value', $.number),
      'ohm',
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

    // Dimension: number with optional unit (10mm, 100mil, 1in, 1000nm, -5mm)
    dimension: $ => seq(
      optional(field('sign', '-')),
      field('value', $.number),
      optional(field('unit', $.unit)),
    ),

    // Units
    // `um` is a length like the rest. `oz` is not here on purpose: it is a
    // weight per square foot, a thickness of copper and of nothing else, so it
    // is taken in one position - `stackup_layer`'s thickness - rather than
    // everywhere a number can appear. `size 1oz x 2oz` is not a board.
    unit: $ => choice('mm', 'um', 'mil', 'in', 'nm'),

    // 1oz, 2oz - copper foil as every fab table states it.
    copper_weight: $ => seq(field('value', $.number), 'oz'),

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
      $.silk_line,
      $.silk_circle,
    ),

    // silk line 0mm, 0mm to 2mm, 0mm width 0.15mm
    //
    // The legend the fabricator prints. A footprint without any gets the
    // courtyard outline the exporter derives, which is a box rather than
    // artwork.
    silk_line: $ => seq(
      'silk', 'line',
      field('x1', $.dimension), ',', field('y1', $.dimension),
      'to',
      field('x2', $.dimension), ',', field('y2', $.dimension),
      optional(seq('width', field('width', $.dimension))),
    ),

    // silk circle 0mm, 1mm radius 0.3mm width 0.15mm
    silk_circle: $ => seq(
      'silk', 'circle',
      field('cx', $.dimension), ',', field('cy', $.dimension),
      'radius', field('radius', $.dimension),
      optional(seq('width', field('width', $.dimension))),
    ),

    // description "text"
    description_property: $ => seq(
      'description',
      field('text', $.string),
    ),

    // pad N shape at X, Y size W x H [drill D]
    // pad 1 ... / pad A1 ... / pad "S1" ...
    //
    // A pad's name is a name, not a count. A USB-C receptacle names its pads
    // A1 and B4, a BGA names them by row and column, and an edge connector
    // names them whatever the datasheet says - none of which is a number, and
    // all of which this grammar refused. The board model has held a pad name
    // as a string since it was written; only the language insisted otherwise.
    pad_definition: $ => seq(
      'pad',
      field('number', choice($.number, $.identifier, $.string)),
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

    // `drill 0.9mm` is a round hole; `drill 2.4mm x 1.0mm` is a slot, milled
    // along its length rather than drilled. Every USB connector, barrel jack
    // and latching header holds itself to the board through one, and until the
    // second dimension existed a design written here could not describe it.
    drill_spec: $ => seq(
      'drill',
      field('width', $.dimension),
      optional(seq('x', field('height', $.dimension))),
    ),

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
      // `flex` is the third: the part of a rigid-flex board that bends. Not a
      // keepout - copper crosses it, that is what it is for - and not a pour.
      field('kind', choice('zone', 'keepout', 'flex')),
      // The same rule a net name uses: a pour is usually named after the net
      // it fills, and `VBUS+` is not an identifier. Writing it bare would
      // produce a file this project cannot read back, which is how the
      // importer came to drop such a zone with a comment.
      optional(field('name', $.net_name)),
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

    // Both spellings, in both blocks. A zone took `top` and a trace took
    // `Top`, so the same property name was an error in one place and correct
    // in the other depending on a capital letter - a trap the guide could
    // document but not fix.
    layer_name: $ => choice('top', 'bottom', 'all', 'Top', 'Bottom', 'All'),

    // net NETNAME (for copper pour zones)
    //
    // `net_name`, not `identifier`: a pour is poured to a net, and a net can
    // be called `VBUS+` or `3V3`. `net_definition` and `pad_definition` both
    // grew a quoted form; this was the last name in the language without one,
    // so a ground plane on a net the identifier rule refuses could be held in
    // the model and never written down.
    zone_net: $ => seq('net', field('net', $.net_name)),

    // ========================================================================
    // Manual Trace Definitions
    // ========================================================================

    // trace NET_NAME { from PIN to PIN [via X, Y] [layer L] [width W] [locked] }
    trace_definition: $ => seq(
      'trace',
      field('net', $.net_name),
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
      $.trace_neck,
    ),

    // neck 0.8mm for 4mm
    //
    // How narrow the copper may get on the way into a pad, and how far it may
    // run at that width. A short length of thin copper does not have time to
    // heat, which is why a trace carrying amps can still land on a 2.54mm
    // pitch - and the length is stated so the checker can measure the claim
    // rather than trust it.
    trace_neck: $ => seq(
      'neck',
      field('width', $.dimension),
      'for',
      field('length', $.dimension),
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
    // via 12mm,10mm [drill 0.3mm] [layers Top to Inner1]
    //
    // Without a layer pair a via goes through the board, which is what every
    // via written in this language was until the pair existed - the viewer,
    // the drill export and the hole rule all read a span the DSL could not
    // state.
    trace_via: $ => seq(
      'via',
      field('x', $.dimension),
      ',',
      field('y', $.dimension),
      optional(seq('drill', field('drill', $.dimension))),
      optional(seq(
        'layers',
        field('start_layer', $.trace_layer_name),
        'to',
        field('end_layer', $.trace_layer_name),
      )),
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

    trace_layer_name: $ => choice(
      'Top',
      'Bottom',
      'Inner1',
      'Inner2',
      'Inner3',
      'Inner4',
      'top',
      'bottom',
      'inner1',
      'inner2',
      'inner3',
      'inner4',
    ),

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
      // The module promises to expose an interface's pins.
      $.implements_clause,
      $.assert_statement,
      // A module can be built from other modules.
      $.module_instance,
    ),

    // implements I2C
    //
    // A claim the checker holds the module to: every pin the interface
    // declares has to be a pin the module exposes. Written one per line, the
    // way `pin` is, so a module implementing two interfaces reads as two
    // statements rather than a list.
    implements_clause: $ => seq(
      'implements',
      field('interface', $.identifier),
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
