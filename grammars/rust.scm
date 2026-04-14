; Function definitions
(function_item
  name: (identifier) @func.name) @func.def

; Struct definitions
(struct_item
  name: (type_identifier) @class.name) @class.def

; Enum definitions
(enum_item
  name: (type_identifier) @class.name) @class.def

; Trait definitions
(trait_item
  name: (type_identifier) @class.name) @class.def

; Impl blocks
(impl_item
  type: (type_identifier) @class.name) @class.def

; Function calls
(call_expression
  function: (identifier) @call.name) @call.def

; Method calls
(call_expression
  function: (field_expression
    value: (_) @method_call.object
    field: (field_identifier) @method_call.name)) @method_call.def

; Use declarations
(use_declaration
  argument: (scoped_identifier) @import.module) @import.def

(use_declaration
  argument: (identifier) @import.module) @import.def

(use_declaration
  argument: (use_wildcard) @import.module) @import.def
