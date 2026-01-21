; CodeYourPCB syntax highlighting queries

; Keywords
[
  "version"
  "board"
  "component"
  "net"
  "size"
  "layers"
  "stackup"
  "value"
  "at"
  "rotate"
  "width"
  "clearance"
] @keyword

; Component types
[
  "resistor"
  "capacitor"
  "inductor"
  "ic"
  "led"
  "connector"
  "diode"
  "transistor"
  "crystal"
  "generic"
] @type

; Layer types in stackup
[
  "copper"
  "prepreg"
  "core"
  "mask"
  "silk"
] @type.builtin

; Units
(unit) @keyword.modifier

; Angle units
[
  "deg"
  "degrees"
] @keyword.modifier

; Numbers
(number) @number

; Strings
(string) @string

; Identifiers
(identifier) @variable

; Board name
(board_definition
  name: (identifier) @type.definition)

; Component refdes
(component_definition
  refdes: (identifier) @variable.definition)

; Net name
(net_definition
  name: (identifier) @variable.definition)

; Pin references
(pin_ref
  component: (identifier) @variable
  pin: (pin_identifier) @property)

; Comments
(line_comment) @comment
(block_comment) @comment

; Punctuation
["{" "}" "[" "]"] @punctuation.bracket
["," "." "="] @punctuation.delimiter
["x"] @operator
