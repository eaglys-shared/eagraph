; Method declarations
(method_declaration
  name: (identifier) @func.name) @func.def

; Constructor declarations
(constructor_declaration
  name: (identifier) @func.name) @func.def

; Class declarations
(class_declaration
  name: (identifier) @class.name) @class.def

; Interface declarations
(interface_declaration
  name: (identifier) @class.name) @class.def

; Struct declarations
(struct_declaration
  name: (identifier) @class.name) @class.def

; Enum declarations
(enum_declaration
  name: (identifier) @class.name) @class.def

; Base list (inheritance)
(base_list
  (identifier) @class.bases)

; Method invocations
(invocation_expression
  function: (member_access_expression
    name: (identifier) @method_call.name)) @method_call.def

; Function calls
(invocation_expression
  function: (identifier) @call.name) @call.def

; Using directives
(using_directive
  (qualified_name) @import.module) @import.def

(using_directive
  (identifier) @import.module) @import.def
