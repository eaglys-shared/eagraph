; Function declarations
(function_declaration
  name: (identifier) @func.name) @func.def

; Method declarations
(method_declaration
  name: (field_identifier) @func.name) @func.def

; Type declarations (struct, interface)
(type_declaration
  (type_spec
    name: (type_identifier) @class.name)) @class.def

; Function calls
(call_expression
  function: (identifier) @call.name) @call.def

; Method calls
(call_expression
  function: (selector_expression
    operand: (_) @method_call.object
    field: (field_identifier) @method_call.name)) @method_call.def

; Import specs
(import_spec
  path: (interpreted_string_literal) @import.module) @import.def
