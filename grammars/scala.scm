; Function definitions
(function_definition
  name: (identifier) @func.name) @func.def

; Class definitions
(class_definition
  name: (identifier) @class.name) @class.def

; Object definitions
(object_definition
  name: (identifier) @class.name) @class.def

; Trait definitions
(trait_definition
  name: (identifier) @class.name) @class.def

; Extends clause
(extends_clause
  (type_identifier) @class.bases)

; Function calls
(call_expression
  function: (identifier) @call.name) @call.def

; Method calls
(call_expression
  function: (field_expression
    field: (identifier) @method_call.name)) @method_call.def

; Import statements
(import_declaration
  path: (identifier) @import.module) @import.def
